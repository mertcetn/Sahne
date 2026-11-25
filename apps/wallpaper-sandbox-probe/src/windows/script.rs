//! Fixed trusted fixtures only. The caller must already be a verified AppContainer worker.
use super::Result;
use rquickjs::{Context, Error, Runtime, context::intrinsic};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub const RESULT_BIT: u32 = 512;
const HEAP_BYTES: usize = 16 * 1024 * 1024;
const STACK_BYTES: usize = 256 * 1024;
const JOB_LIMIT: usize = 64;

pub fn run(case: &str) -> Result<()> {
    if !wallpaper_sandbox::is_appcontainer()? {
        return Err("JS fixture refuses non-AppContainer execution".into());
    }
    let (runtime, interrupted) = bounded_runtime()?;
    // Base objects plus source evaluation and Promises; no loaders, native modules,
    // typed arrays, RegExp, Date, host callbacks or browser/system APIs are installed.
    let context = Context::custom::<(intrinsic::Eval, intrinsic::Promise)>(&runtime)?;
    run_case(case, &runtime, &context, &interrupted)
}

fn bounded_runtime() -> Result<(Runtime, Arc<AtomicBool>)> {
    let runtime = Runtime::new()?;
    // Keep the default C allocator: rust-alloc/allocator features would disable this limit.
    runtime.set_memory_limit(HEAP_BYTES);
    runtime.set_max_stack_size(STACK_BYTES);
    let interrupted = Arc::new(AtomicBool::new(false));
    let observed = interrupted.clone();
    let deadline = Instant::now() + Duration::from_millis(100);
    runtime.set_interrupt_handler(Some(Box::new(move || {
        let expired = Instant::now() >= deadline;
        if expired {
            observed.store(true, Ordering::Relaxed);
        }
        expired
    })));
    Ok((runtime, interrupted))
}

fn run_case(
    case: &str,
    runtime: &Runtime,
    context: &Context,
    interrupted: &AtomicBool,
) -> Result<()> {
    match case {
        "js-basic" => {
            let valid = context.with(|ctx| {
                ctx.eval::<bool, _>(
                    r#"
                6 * 7 === 42 &&
                typeof std === 'undefined' && typeof os === 'undefined' &&
                typeof require === 'undefined' && typeof fetch === 'undefined' &&
                typeof document === 'undefined' && typeof setTimeout === 'undefined' &&
                typeof setInterval === 'undefined' && typeof console === 'undefined' &&
                typeof WebAssembly === 'undefined' && typeof SharedArrayBuffer === 'undefined' &&
                typeof RegExp === 'undefined' && typeof Date === 'undefined' &&
                typeof Uint8Array === 'undefined'
            "#,
                )
            })?;
            if !valid {
                return Err("JS arithmetic or absent-API checks failed".into());
            }
        }
        "js-interrupt" => {
            let result = context.with(|ctx| ctx.eval::<(), _>("for (;;) {}"));
            if !matches!(result, Err(Error::Exception)) || !interrupted.load(Ordering::Relaxed) {
                return Err("Infinite JS did not raise a deadline interrupt".into());
            }
            // Discard the exception without executing user-controlled formatting code.
            context.with(|ctx| {
                let _ = ctx.catch();
            });
        }
        "js-memory" => {
            let bounded = context.with(|ctx| {
                ctx.eval::<bool, _>(
                    r#"
                (() => {
                    const allocations = [];
                    try {
                        for (;;) allocations.push(new Array(65536).fill(1));
                    } catch (error) {
                        allocations.length = 0;
                        return error instanceof InternalError && error.message === 'out of memory';
                    }
                })()
            "#,
                )
            })?;
            if !bounded || interrupted.load(Ordering::Relaxed) {
                return Err("Expected a JS allocation failure, not another budget failure".into());
            }
        }
        "js-stack" => {
            let bounded = context.with(|ctx| ctx.eval::<bool, _>(r#"
                (() => {
                    function recurse() { return 1 + recurse(); }
                    try { recurse(); return false; }
                    catch (error) { return error instanceof RangeError && error.message.includes('stack'); }
                })()
            "#))?;
            if !bounded || interrupted.load(Ordering::Relaxed) {
                return Err("Expected a JS stack RangeError, not another budget failure".into());
            }
        }
        "js-jobs" => {
            context.with(|ctx| {
                ctx.eval::<(), _>(
                    r#"
                Promise.resolve().then(function repeat() {
                    Promise.resolve().then(repeat);
                    Promise.resolve().then(repeat);
                });
            "#,
                )
            })?;
            for _ in 0..JOB_LIMIT {
                if !runtime
                    .execute_pending_job()
                    .map_err(|_| "Promise job raised an exception")?
                {
                    return Err("Promise fixture unexpectedly drained before its job budget".into());
                }
            }
            if !runtime.is_job_pending() || interrupted.load(Ordering::Relaxed) {
                return Err("Expected pending Promise work at the explicit 64-job budget".into());
            }
        }
        _ => return Err("Unknown fixed JS fixture".into()),
    }
    if case != "js-interrupt" && interrupted.load(Ordering::Relaxed) {
        return Err("Unexpected JS deadline interrupt".into());
    }
    // Runtime/context are destroyed with any pending jobs; no job runner continues afterward.
    Ok(())
}

/// Rust resolves the adapter's local indices against the host fixture bindings.
pub fn clock_edits(display_time: &str) -> Result<Vec<wallpaper_scene::transaction::Edit>> {
    if !wallpaper_sandbox::is_appcontainer()? {
        return Err("Clock fixture refuses non-AppContainer execution".into());
    }
    let (runtime, interrupted) = bounded_runtime()?;
    let context = Context::custom::<(intrinsic::Eval,)>(&runtime)?;
    let bindings = [("clock", 1, Some("initial")), ("accent", 2, None)];
    let edits = context.with(|ctx| {
        run_scene_callback(
            &ctx,
            &bindings,
            include_str!("clock_fixture.js"),
            display_time,
        )
    })?;
    if interrupted.load(Ordering::Relaxed) || runtime.is_job_pending() {
        return Err("Clock fixture exceeded its execution budget".into());
    }
    Ok(edits)
}

fn run_scene_callback(
    ctx: &rquickjs::Ctx<'_>,
    bindings: &[(&str, u32, Option<&str>)],
    embedded_callback: &str,
    value: &str,
) -> Result<Vec<wallpaper_scene::transaction::Edit>> {
    use rquickjs::{Array, Function};
    use wallpaper_scene::{
        Color, MAX_TEXT_BYTES,
        transaction::{Edit, MAX_BATCH_TEXT_BYTES, MAX_EDITS},
    };

    if !(1..=MAX_EDITS).contains(&bindings.len()) {
        return Err("Scene binding count outside bounds".into());
    }
    let input = Array::new(ctx.clone())?;
    for (index, (name, _, text)) in bindings.iter().enumerate() {
        let entry = Array::new(ctx.clone())?;
        entry.set(0, *name)?;
        entry.set(1, *text)?;
        let color = Array::new(ctx.clone())?;
        for channel in 0..4 {
            color.set(channel, 255u8)?;
        }
        entry.set(2, color)?;
        input.set(index, entry)?;
    }
    let adapter: Function = ctx.eval(include_str!("dom_adapter.js"))?;
    let callback: Function = ctx.eval(embedded_callback)?;
    let staged: Array = adapter
        .call((input, callback, value))
        .map_err(|_| "Fixed clock callback failed")?;
    let count = staged.len();
    if !(1..=MAX_EDITS).contains(&count) {
        return Err("Clock fixture edit count outside bounds".into());
    }
    let mut edits = Vec::with_capacity(count);
    let mut total = 0;
    for index in 0..count {
        let edit: Array = staged.get(index)?;
        let binding: usize = edit.get(0)?;
        let (_, node, text) = *bindings
            .get(binding)
            .ok_or("Invalid fixture binding index")?;
        match edit.get::<u8>(1)? {
            1 if text.is_some() => {
                let value: String = edit.get(2)?;
                total += value.len();
                if value.len() > MAX_TEXT_BYTES || total > MAX_BATCH_TEXT_BYTES {
                    return Err("Clock fixture exceeded its UTF-8 text budget".into());
                }
                edits.push(Edit::Text { node, value });
            }
            2 => {
                let color: Array = edit.get(2)?;
                if color.len() != 4 {
                    return Err("Expected four RGBA bytes".into());
                }
                edits.push(Edit::Color {
                    node,
                    value: Color([color.get(0)?, color.get(1)?, color.get(2)?, color.get(3)?]),
                });
            }
            _ => return Err("Invalid fixture edit".into()),
        }
    }
    Ok(edits)
}
