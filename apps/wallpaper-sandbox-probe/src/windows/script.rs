//! Fixed trusted fixtures only. The caller must already be a verified AppContainer worker.
use super::Result;
use rquickjs::{
    Array, Context, Error, Function, Object, Persistent, Runtime, Value, context::intrinsic,
    promise::PromiseState,
};
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
    run_case(case, &runtime, &context, &interrupted.flag)
}

struct Deadline {
    at: Instant,
    flag: Arc<AtomicBool>,
}

fn bounded_runtime() -> Result<(Runtime, Deadline)> {
    let runtime = Runtime::new()?;
    // Keep the default C allocator: rust-alloc/allocator features would disable this limit.
    runtime.set_memory_limit(HEAP_BYTES);
    runtime.set_max_stack_size(STACK_BYTES);
    let interrupted = arm_deadline(&runtime);
    Ok((runtime, interrupted))
}

fn arm_deadline(runtime: &Runtime) -> Deadline {
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
    Deadline {
        at: deadline,
        flag: interrupted,
    }
}

pub struct RegisteredClock {
    pub callbacks: RetainedCallbacks,
    controls: Rc<ClockControls>,
}

pub const MAX_FIXTURE_INTERVAL_MS: u32 = 64;

#[derive(Default)]
struct ClockControls {
    phase: Cell<IntervalPhase>,
    delay: Cell<Option<u32>>,
    cancelled: Cell<bool>,
    invalid: Cell<bool>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum IntervalPhase {
    #[default]
    Setup,
    Idle,
    Tick,
}

impl ClockControls {
    fn register(&self, milliseconds: f64) -> u32 {
        if self.phase.get() != IntervalPhase::Setup
            || self.delay.get().is_some()
            || milliseconds.fract() != 0.0
            || !(wallpaper_scene::timer::MIN_DELAY.as_secs_f64() * 1000.0
                ..=f64::from(MAX_FIXTURE_INTERVAL_MS))
                .contains(&milliseconds)
        {
            self.invalid.set(true);
            return 0;
        }
        self.delay.set(Some(milliseconds as u32));
        1
    }

    fn clear(&self, handle: f64) -> bool {
        if self.phase.get() != IntervalPhase::Tick
            || handle != 1.0
            || self.delay.get().is_none()
            || self.cancelled.get()
        {
            self.invalid.set(true);
            return false;
        }
        self.cancelled.set(true);
        true
    }

    fn check(&self) -> Result<()> {
        if self.invalid.get() {
            return Err("Invalid or repeated interval host call".into());
        }
        Ok(())
    }
}

impl RegisteredClock {
    pub fn new() -> Result<(Self, u32)> {
        if !wallpaper_sandbox::is_appcontainer()? {
            return Err("Interval fixture refuses non-AppContainer execution".into());
        }
        let (runtime, interrupted) = bounded_runtime()?;
        let context = Context::custom::<(intrinsic::Eval, intrinsic::Promise)>(&runtime)?;
        let controls = Rc::new(ClockControls::default());
        context.with(|ctx| -> rquickjs::Result<()> {
            let register_state = Rc::clone(&controls);
            let register = Function::new(ctx.clone(), move |milliseconds: f64| {
                register_state.register(milliseconds)
            })?;
            let clear_state = Rc::clone(&controls);
            let clear = Function::new(ctx.clone(), move |handle: f64| clear_state.clear(handle))?;
            let clock: Function = ctx.eval(include_str!("clock_fixture.js"))?;
            let setup: Function = ctx.eval(include_str!("script/registered_clock.js"))?;
            let callbacks: Object = setup.call((register, clear, clock))?;
            ctx.globals().set("registeredClock", callbacks)
        })?;
        check_callback_budget(&runtime, &interrupted)?;
        controls.check()?;
        controls.phase.set(IntervalPhase::Idle);
        let delay_ms = controls
            .delay
            .get()
            .ok_or("Missing JavaScript interval registration")?;
        Ok((
            Self {
                callbacks: RetainedCallbacks { context, runtime },
                controls,
            },
            delay_ms,
        ))
    }

    pub fn cancelled(&self) -> bool {
        self.controls.cancelled.get()
    }

    pub fn check_controls(&self) -> Result<()> {
        self.controls.check()
    }

    pub fn edits(&self, bindings: &[Binding], display_time: &str) -> Result<Vec<Edit>> {
        self.controls.phase.set(IntervalPhase::Tick);
        let result = self.callbacks.edits(bindings, display_time);
        self.controls.phase.set(IntervalPhase::Idle);
        self.controls.check()?;
        result
    }
}

pub struct RetainedCallbacks {
    context: Context,
    runtime: Runtime,
}

impl RetainedCallbacks {
    pub fn utc_clock() -> Result<Self> {
        Self::from_source(&format!(
            "({})({})",
            include_str!("script/utc_clock.js"),
            include_str!("clock_fixture.js")
        ))
    }

    pub fn from_source(source: &str) -> Result<Self> {
        if !wallpaper_sandbox::is_appcontainer()? {
            return Err("Lifecycle probe refuses non-AppContainer execution".into());
        }
        let (runtime, interrupted) = bounded_runtime()?;
        let context = Context::custom::<(intrinsic::Eval, intrinsic::Promise)>(&runtime)?;
        let rejected = track_rejections(&runtime);
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
        check_rejections(&rejected)?;
        Ok(Self { context, runtime })
    }

    pub fn edits(&self, bindings: &[Binding], display_time: &str) -> Result<Vec<Edit>> {
        let interrupted = arm_deadline(&self.runtime);
        let rejected = track_rejections(&self.runtime);
        let staged = self.context.with(|ctx| {
            let callbacks: Object = ctx.globals().get("registeredClock")?;
            let callback: Function = callbacks.get("tick")?;
            stage_scene_callback(&ctx, bindings, callback, display_time)
        })?;
        finish_scene_callback(
            &self.context,
            &self.runtime,
            &interrupted,
            &rejected,
            bindings,
            staged,
        )
    }

    pub fn lifecycle(&self, paused: bool) -> Result<()> {
        let interrupted = arm_deadline(&self.runtime);
        let rejected = track_rejections(&self.runtime);
        let completion = self.context.with(|ctx| -> rquickjs::Result<_> {
            let callbacks: Object = ctx.globals().get("registeredClock")?;
            let callback: Function = callbacks.get(if paused { "pause" } else { "resume" })?;
            let value: Value = callback.call(())?;
            Ok(Persistent::save(&ctx, value))
        })?;
        drain_jobs(&self.runtime, &interrupted)?;
        self.context
            .with(|ctx| check_completion(completion.restore(&ctx)?))?;
        check_callback_budget(&self.runtime, &interrupted)?;
        check_rejections(&rejected)
    }
}

fn check_callback_budget(runtime: &Runtime, deadline: &Deadline) -> Result<()> {
    check_deadline(deadline)?;
    if runtime.is_job_pending() {
        return Err("Scene callback left pending jobs beyond its execution budget".into());
    }
    Ok(())
}

fn check_deadline(deadline: &Deadline) -> Result<()> {
    if deadline.flag.load(Ordering::Relaxed) || Instant::now() >= deadline.at {
        return Err("Scene callback exceeded its execution budget".into());
    }
    Ok(())
}

fn track_rejections(runtime: &Runtime) -> Arc<AtomicBool> {
    let rejected = Arc::new(AtomicBool::new(false));
    let observed = Arc::clone(&rejected);
    runtime.set_host_promise_rejection_tracker(Some(Box::new(move |_, _, _, handled| {
        if !handled {
            observed.store(true, Ordering::Relaxed);
        }
    })));
    rejected
}

fn check_rejections(rejected: &AtomicBool) -> Result<()> {
    if rejected.load(Ordering::Relaxed) {
        return Err("Scene callback had an unhandled promise rejection".into());
    }
    Ok(())
}

fn drain_jobs(runtime: &Runtime, deadline: &Deadline) -> Result<()> {
    for _ in 0..JOB_LIMIT {
        check_deadline(deadline)?;
        if !runtime
            .execute_pending_job()
            .map_err(|_| "Scene Promise job failed")?
        {
            break;
        }
    }
    check_callback_budget(runtime, deadline)
}

fn check_completion(completion: Value<'_>) -> Result<()> {
    if completion
        .into_promise()
        .is_some_and(|promise| promise.state() != PromiseState::Resolved)
    {
        return Err("Scene callback promise did not fulfill".into());
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
    let context = Context::custom::<(intrinsic::Eval, intrinsic::Promise)>(&runtime)?;
    let rejected = track_rejections(&runtime);
    let staged = context.with(|ctx| {
        let callback: Function = ctx
            .eval(package_source.unwrap_or(include_str!("clock_fixture.js")))
            .map_err(|_| "Scene source did not evaluate to a callback")?;
        stage_scene_callback(&ctx, bindings, callback, display_time)
    })?;
    finish_scene_callback(
        &context,
        &runtime,
        &interrupted,
        &rejected,
        bindings,
        staged,
    )
}

fn finish_scene_callback(
    context: &Context,
    runtime: &Runtime,
    deadline: &Deadline,
    rejected: &AtomicBool,
    bindings: &[Binding],
    staged: StagedScene,
) -> Result<Vec<Edit>> {
    drain_jobs(runtime, deadline)?;
    let edits = context.with(|ctx| collect_scene_edits(&ctx, bindings, staged))?;
    check_callback_budget(runtime, deadline)?;
    check_rejections(rejected)?;
    Ok(edits)
}

struct StagedScene {
    completion: Persistent<Value<'static>>,
    collect: Persistent<Function<'static>>,
}

fn stage_scene_callback<'js>(
    ctx: &rquickjs::Ctx<'js>,
    bindings: &[Binding],
    callback: Function<'js>,
    value: &str,
) -> Result<StagedScene> {
    use wallpaper_scene::transaction::MAX_EDITS;

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
    Ok(StagedScene {
        completion: Persistent::save(ctx, staged.get::<Value>(0)?),
        collect: Persistent::save(ctx, staged.get::<Function>(1)?),
    })
}

fn collect_scene_edits(
    ctx: &rquickjs::Ctx<'_>,
    bindings: &[Binding],
    staged: StagedScene,
) -> Result<Vec<Edit>> {
    use wallpaper_scene::{
        MAX_TEXT_BYTES,
        transaction::{MAX_BATCH_TEXT_BYTES, MAX_EDITS},
    };
    check_completion(staged.completion.restore(ctx)?)?;
    let edits_array: Array = staged.collect.restore(ctx)?.call(())?;
    let count = edits_array.len();
    if !(1..=MAX_EDITS).contains(&count) {
        return Err("Clock fixture edit count outside bounds".into());
    }
    let mut edits = Vec::with_capacity(count);
    let mut total = 0;
    for index in 0..count {
        let edit: Array = edits_array.get(index)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_host_calls_reject_repeats_and_invalid_arguments() {
        let controls = ClockControls::default();
        assert_eq!(controls.register(32.0), 1);
        controls.phase.set(IntervalPhase::Tick);
        assert!(controls.clear(1.0));
        assert!(controls.check().is_ok());
        assert!(!controls.clear(1.0));
        assert!(controls.check().is_err());

        let controls = ClockControls::default();
        assert_eq!(controls.register(32.0), 1);
        assert_eq!(controls.register(32.0), 0);
        assert!(controls.check().is_err());

        for delay in [f64::NAN, 31.5, 15.0, 65.0] {
            let controls = ClockControls::default();
            assert_eq!(controls.register(delay), 0);
            assert!(controls.check().is_err());
        }
        let controls = ClockControls::default();
        assert_eq!(controls.register(32.0), 1);
        controls.phase.set(IntervalPhase::Tick);
        assert!(!controls.clear(1.5));
        assert!(!controls.cancelled.get());
        assert!(controls.check().is_err());
    }

    #[test]
    fn interval_host_calls_respect_callback_phase_without_changing_timer_state() {
        for phase in [IntervalPhase::Setup, IntervalPhase::Idle] {
            let controls = ClockControls::default();
            assert_eq!(controls.register(32.0), 1);
            controls.phase.set(phase);
            assert!(!controls.clear(1.0));
            assert!(!controls.cancelled.get());
            assert!(controls.check().is_err());
        }
        for phase in [IntervalPhase::Idle, IntervalPhase::Tick] {
            let controls = ClockControls::default();
            controls.phase.set(phase);
            assert_eq!(controls.register(32.0), 0);
            assert_eq!(controls.delay.get(), None);
            assert!(controls.check().is_err());
        }
    }

    #[test]
    fn retained_dom_objects_expire_after_collection() {
        let runtime = Runtime::new().unwrap();
        let context = Context::custom::<(intrinsic::Eval,)>(&runtime).unwrap();
        let bindings = [Binding {
            name: "status".into(),
            node: 1,
            text: Some("Ready".into()),
            color: Color([255; 4]),
        }];
        let run = |source| {
            context.with(|ctx| {
                let callback = ctx.eval::<Function, _>(source).unwrap();
                let staged = stage_scene_callback(&ctx, &bindings, callback, "").unwrap();
                collect_scene_edits(&ctx, &bindings, staged).unwrap()
            })
        };
        run(r#"(document) => {
            globalThis.saved = document.getElementById("status");
            saved.textContent = "First";
        }"#);
        let edits = run(r#"(document) => {
            let expired = false;
            try { saved.textContent = "Stale"; } catch (_) { expired = true; }
            if (!expired) throw Error("Old element remained writable");
            document.getElementById("status").textContent = "Second";
        }"#);
        assert_eq!(
            edits,
            [Edit::Text {
                node: 1,
                value: "Second".into()
            }]
        );
    }

    #[test]
    fn elapsed_callback_is_rejected_even_without_a_quickjs_interrupt() {
        let runtime = Runtime::new().unwrap();
        let deadline = Deadline {
            at: Instant::now(),
            flag: Arc::new(AtomicBool::new(false)),
        };
        assert!(check_callback_budget(&runtime, &deadline).is_err());
    }
}
