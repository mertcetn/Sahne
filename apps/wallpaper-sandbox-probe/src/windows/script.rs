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
    // Base objects plus source evaluation and Promises; no loaders, native modules,
    // typed arrays, RegExp, Date, host callbacks or browser/system APIs are installed.
    let context = Context::custom::<(intrinsic::Eval, intrinsic::Promise)>(&runtime)?;
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
