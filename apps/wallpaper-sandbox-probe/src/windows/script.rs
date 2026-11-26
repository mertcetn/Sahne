//! Fixed trusted fixtures only. The caller must already be a verified AppContainer worker.
use super::Result;
use rquickjs::{Context, Error, Function, Object, Runtime, context::intrinsic};
use std::{
    cell::Cell,
    collections::BTreeMap,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use wallpaper_scene::{Color, transaction::Edit};
mod timers;

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
    let interrupted = arm_deadline(&runtime);
    Ok((runtime, interrupted))
}

fn arm_deadline(runtime: &Runtime) -> Arc<AtomicBool> {
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
    interrupted
}

pub struct RegisteredClock {
    pub callbacks: RetainedCallbacks,
    cancelled: Rc<Cell<bool>>,
}

impl RegisteredClock {
    pub fn new() -> Result<(Self, u32)> {
        if !wallpaper_sandbox::is_appcontainer()? {
            return Err("Interval fixture refuses non-AppContainer execution".into());
        }
        let (runtime, interrupted) = bounded_runtime()?;
        let context = Context::custom::<(intrinsic::Eval,)>(&runtime)?;
        let cancelled = Rc::new(Cell::new(false));
        let delay = Rc::new(Cell::new(None));
        context.with(|ctx| -> rquickjs::Result<()> {
            let requested = Rc::clone(&delay);
            let register = Function::new(ctx.clone(), move |milliseconds: u32| {
                if requested.get().is_some() {
                    return 0;
                }
                requested.set(Some(milliseconds));
                1
            })?;
            let stopped = Rc::clone(&cancelled);
            let clear = Function::new(ctx.clone(), move |handle: u32| {
                if handle == 1 {
                    stopped.set(true);
                }
            })?;
            let clock: Function = ctx.eval(include_str!("clock_fixture.js"))?;
            let setup: Function = ctx.eval(include_str!("script/registered_clock.js"))?;
            let callbacks: Object = setup.call((register, clear, clock))?;
            ctx.globals().set("registeredClock", callbacks)
        })?;
        check_callback_budget(&runtime, &interrupted)?;
        let delay_ms = delay
            .get()
            .ok_or("Missing JavaScript interval registration")?;
        Ok((
            Self {
                callbacks: RetainedCallbacks { context, runtime },
                cancelled,
            },
            delay_ms,
        ))
    }

    pub fn cancelled(&self) -> bool {
        self.cancelled.get()
    }
}

pub struct RetainedCallbacks {
    context: Context,
    runtime: Runtime,
}

impl RetainedCallbacks {
    pub fn from_source(source: &str) -> Result<Self> {
        if !wallpaper_sandbox::is_appcontainer()? {
            return Err("Lifecycle probe refuses non-AppContainer execution".into());
        }
        let (runtime, interrupted) = bounded_runtime()?;
        let context = Context::custom::<(intrinsic::Eval,)>(&runtime)?;
        context
            .with(|ctx| -> rquickjs::Result<()> {
                let callbacks: Object = ctx.eval(source)?;
                for name in ["pause", "resume", "tick"] {
                    let _: Function = callbacks.get(name)?;
                }
                ctx.globals().set("registeredClock", callbacks)
            })
            .map_err(|_| "Expected an object with pause, resume and tick callbacks")?;
        check_callback_budget(&runtime, &interrupted)?;
        Ok(Self { context, runtime })
    }

    pub fn edits(&self, bindings: &[Binding], display_time: &str) -> Result<Vec<Edit>> {
        let interrupted = arm_deadline(&self.runtime);
        let edits = self.context.with(|ctx| {
            let callbacks: Object = ctx.globals().get("registeredClock")?;
            let callback: Function = callbacks.get("tick")?;
            run_scene_callback(&ctx, bindings, callback, display_time)
        })?;
        check_callback_budget(&self.runtime, &interrupted)?;
        Ok(edits)
    }

    pub fn lifecycle(&self, paused: bool) -> Result<()> {
        let interrupted = arm_deadline(&self.runtime);
        self.context.with(|ctx| -> rquickjs::Result<()> {
            let callbacks: Object = ctx.globals().get("registeredClock")?;
            let callback: Function = callbacks.get(if paused { "pause" } else { "resume" })?;
            callback.call(())
        })?;
        check_callback_budget(&self.runtime, &interrupted)
    }
}

fn check_callback_budget(runtime: &Runtime, interrupted: &AtomicBool) -> Result<()> {
    if interrupted.load(Ordering::Relaxed) || runtime.is_job_pending() {
        return Err("Scene callback exceeded its execution budget".into());
    }
    Ok(())
}

fn run_case(
    case: &str,
    runtime: &Runtime,
    context: &Context,
    interrupted: &AtomicBool,
) -> Result<()> {
    match case {
        "js-timers" => timers::check(context)?,
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

pub struct Binding {
    pub name: String,
    pub node: u32,
    pub text: Option<String>,
    pub color: Color,
}

pub fn bindings_from_initial(
    targets: BTreeMap<String, u32>,
    initial: Vec<Edit>,
) -> Result<Vec<Binding>> {
    let mut text = BTreeMap::new();
    let mut colors = BTreeMap::new();
    for edit in initial {
        match edit {
            Edit::Text { node, value } => {
                text.insert(node, value);
            }
            Edit::Color { node, value } => {
                colors.insert(node, value);
            }
        }
    }
    targets
        .into_iter()
        .map(|(name, node)| {
            Ok(Binding {
                name,
                node,
                text: text.remove(&node),
                color: colors.remove(&node).ok_or("Missing initial scene color")?,
            })
        })
        .collect()
}

pub fn apply_accepted_edits(bindings: &mut [Binding], edits: Vec<Edit>) {
    for edit in edits {
        match edit {
            Edit::Text { node, value } => {
                if let Some(binding) = bindings.iter_mut().find(|binding| binding.node == node) {
                    binding.text = Some(value);
                }
            }
            Edit::Color { node, value } => {
                if let Some(binding) = bindings.iter_mut().find(|binding| binding.node == node) {
                    binding.color = value;
                }
            }
        }
    }
}

/// Rust resolves the adapter's local indices against the host-authorized bindings.
pub fn scene_edits(
    bindings: &[Binding],
    display_time: &str,
    package_source: Option<&str>,
) -> Result<Vec<wallpaper_scene::transaction::Edit>> {
    if !wallpaper_sandbox::is_appcontainer()? {
        return Err("Clock fixture refuses non-AppContainer execution".into());
    }
    let (runtime, interrupted) = bounded_runtime()?;
    let context = Context::custom::<(intrinsic::Eval,)>(&runtime)?;
    let edits = context.with(|ctx| {
        let callback: Function = ctx
            .eval(package_source.unwrap_or(include_str!("clock_fixture.js")))
            .map_err(|_| "Scene source did not evaluate to a callback")?;
        run_scene_callback(&ctx, bindings, callback, display_time)
    })?;
    check_callback_budget(&runtime, &interrupted)?;
    Ok(edits)
}

fn run_scene_callback<'js>(
    ctx: &rquickjs::Ctx<'js>,
    bindings: &[Binding],
    callback: Function<'js>,
    value: &str,
) -> Result<Vec<wallpaper_scene::transaction::Edit>> {
    use rquickjs::Array;
    use wallpaper_scene::{
        Color, MAX_TEXT_BYTES,
        transaction::{Edit, MAX_BATCH_TEXT_BYTES, MAX_EDITS},
    };

    if !(1..=MAX_EDITS).contains(&bindings.len()) {
        return Err("Scene binding count outside bounds".into());
    }
    let input = Array::new(ctx.clone())?;
    for (index, binding) in bindings.iter().enumerate() {
        let entry = Array::new(ctx.clone())?;
        entry.set(0, binding.name.as_str())?;
        entry.set(1, binding.text.as_deref())?;
        let color = Array::new(ctx.clone())?;
        for (channel, value) in binding.color.0.into_iter().enumerate() {
            color.set(channel, value)?;
        }
        entry.set(2, color)?;
        input.set(index, entry)?;
    }
    let adapter: Function = ctx.eval(include_str!("dom_adapter.js"))?;
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
        let binding = bindings
            .get(binding)
            .ok_or("Invalid fixture binding index")?;
        match edit.get::<u8>(1)? {
            1 if binding.text.is_some() => {
                let value: String = edit.get(2)?;
                total += value.len();
                if value.len() > MAX_TEXT_BYTES || total > MAX_BATCH_TEXT_BYTES {
                    return Err("Clock fixture exceeded its UTF-8 text budget".into());
                }
                edits.push(Edit::Text {
                    node: binding.node,
                    value,
                });
            }
            2 => {
                let color: Array = edit.get(2)?;
                if color.len() != 4 {
                    return Err("Expected four RGBA bytes".into());
                }
                edits.push(Edit::Color {
                    node: binding.node,
                    value: Color([color.get(0)?, color.get(1)?, color.get(2)?, color.get(3)?]),
                });
            }
            _ => return Err("Invalid fixture edit".into()),
        }
    }
    Ok(edits)
}
