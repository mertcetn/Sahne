mod clock;
mod compiled;
mod groups;
mod lifecycle;
mod live_clock;
mod motion;
mod pixels;
mod script_package;
use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache,
    TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use std::{
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
#[derive(Debug)]
struct DeviceLost {
    generation: u64,
    reason: String,
    explicitly_destroyed: bool,
}

enum AppEvent {
    DeviceLost(DeviceLost),
    ClockUpdate,
    WindowsLifecycle {
        generation: u64,
        event: wallpaper_platform::session_lifecycle::Event,
    },
}

fn main() -> Result<()> {
    let (mut seconds, mut desktop_mode, mut recreate) = (None, false, 0u32);
    let mut test_device_loss = false;
    let mut test_pixels = false;
    let mut reference_pose = false;
    let mut test_clock_rollover = false;
    let mut worker_snapshot = None;
    let mut check_worker_snapshot = false;
    let mut retained_scene_folder = None;
    let mut check_retained_scene = false;
    let mut script_package_folder = None;
    let mut check_script_package = false;
    let mut worker_live = None;
    let mut check_worker_live = false;
    let mut compiled_example = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                println!(
                    "Usage: wallpaper-preview [--desktop] [--example static|image|groups|groups-effects|animated-shapes|clock] [--seconds 1..3600] [--test-recreate 1..10 | --test-device-loss | --test-pixels]\nTrusted Windows D3D12 scenes only. --example compiles original embedded sources; no external file path. Escape closes window mode. Test recreation does not restart Explorer. Pixel test requires groups-effects, window mode, --seconds 1..30 and no other test mode; it exits after checking 18 fixed root-texture pixels. animated-shapes runs its fixed native program at a 30 FPS cap. Use --desktop to attach behind Windows icons or Space to pause/resume in window mode. --reference-pose freezes animated-shapes at exactly 1 s for visual reference; use it in window mode without other test flags. clock shows host-provided UTC time in window mode. --test-clock-rollover requires clock and --seconds 5..30; it demonstrates 23:59 to 00:00. --clock-worker-snapshot <run-folder> displays a retained worker value with --example clock. --check-clock-worker-snapshot <run-folder> verifies that value without opening a window. --clock-worker-live requires --example clock --seconds 8..30 and the sibling sandbox probe. --check-clock-worker-live runs the same bridge without a window. --clock-worker-utc uses current UTC minutes with --example clock. --check-clock-worker-utc verifies two live UTC updates without opening a window."
                );
                println!(
                    "--scene-worker-snapshot <run-folder> displays a retained flat scene. --check-scene-worker-snapshot <run-folder> validates it headlessly."
                );
                println!(
                    "--trusted-script-preview <absolute-folder> displays a trusted flat package and its AppContainer update. Add --desktop to attach it behind Windows icons. --check-trusted-script-preview <absolute-folder> checks that exchange without a window."
                );
                return Ok(());
            }
            "--desktop" => desktop_mode = true,
            "--example" => {
                compiled_example =
                    Some(match args.next().as_deref() {
                        Some("static") => compiled::Example::Static,
                        Some("image") => compiled::Example::Image,
                        Some("groups") => compiled::Example::Groups,
                        Some("groups-effects") => compiled::Example::GroupEffects,
                        Some("animated-shapes") => compiled::Example::Animated,
                        Some("clock") => compiled::Example::Clock,
                        _ => return Err(
                            "Choose an original static, image, groups, groups-effects, animated-shapes or clock example"
                                .into(),
                        ),
                    });
            }
            "--test-device-loss" => test_device_loss = true,
            "--test-pixels" => test_pixels = true,
            "--reference-pose" => reference_pose = true,
            "--test-clock-rollover" => test_clock_rollover = true,
            "--clock-worker-live"
            | "--check-clock-worker-live"
            | "--clock-worker-utc"
            | "--check-clock-worker-utc" => {
                if worker_live.is_some() {
                    return Err("Select only one clock worker mode".into());
                }
                worker_live = Some(if arg.ends_with("-utc") {
                    live_clock::Mode::Utc
                } else {
                    live_clock::Mode::Fixture
                });
                check_worker_live = arg.starts_with("--check-");
            }
            "--clock-worker-snapshot" | "--check-clock-worker-snapshot" => {
                if worker_snapshot.is_some() {
                    return Err("Choose one clock snapshot input".into());
                }
                check_worker_snapshot = arg == "--check-clock-worker-snapshot";
                worker_snapshot = Some(args.next().ok_or("Missing clock snapshot run folder")?);
            }
            "--scene-worker-snapshot" | "--check-scene-worker-snapshot" => {
                if retained_scene_folder.is_some() {
                    return Err("Choose one retained scene input".into());
                }
                check_retained_scene = arg.starts_with("--check-");
                retained_scene_folder =
                    Some(args.next().ok_or("Missing scene snapshot run folder")?);
            }
            "--trusted-script-preview" | "--check-trusted-script-preview" => {
                if script_package_folder.is_some() {
                    return Err("Choose one script preview input".into());
                }
                check_script_package = arg.starts_with("--check-");
                script_package_folder =
                    Some(args.next().ok_or("Missing trusted script package folder")?);
            }
            "--seconds" => {
                let n = args.next().ok_or("Missing seconds")?.parse::<u64>()?;
                if !(1..=3600).contains(&n) {
                    return Err("Seconds must be 1..3600".into());
                }
                seconds = Some(n);
            }
            "--test-recreate" => {
                recreate = args.next().ok_or("Missing recreation count")?.parse()?;
                if !(1..=10).contains(&recreate) {
                    return Err("Recreation count must be 1..10".into());
                }
            }
            _ => return Err(format!("Unsupported argument: {arg}").into()),
        }
    }
    if (retained_scene_folder.is_some() || script_package_folder.is_some())
        && (compiled_example.is_some()
            || worker_snapshot.is_some()
            || worker_live.is_some()
            || (desktop_mode && (script_package_folder.is_none() || check_script_package))
            || recreate != 0
            || test_device_loss
            || test_pixels
            || test_clock_rollover
            || reference_pose
            || ((check_retained_scene || check_script_package) && seconds.is_some())
            || (retained_scene_folder.is_some() && script_package_folder.is_some()))
    {
        return Err("Conflicting options for scripted or retained scene preview".into());
    }
    let script_package = script_package_folder
        .as_deref()
        .map(script_package::prepare)
        .transpose()?;
    if check_script_package {
        return live_clock::check_package(script_package.ok_or("Missing script preview package")?);
    }
    let retained_scene = if let Some(folder) = retained_scene_folder.as_deref() {
        let scene = compiled::load_retained(folder)?;
        if check_retained_scene {
            println!(
                "Accepted retained scene: {} nodes, {}x{} canvas.",
                scene.scene().nodes.len(),
                scene.scene().width,
                scene.scene().height
            );
            return Ok(());
        }
        Some(scene)
    } else {
        script_package
            .as_ref()
            .map(|package| Arc::clone(&package.scene))
    };
    if test_device_loss && recreate > 0 {
        return Err("Choose one test mode per run".into());
    }
    validate_pixel_mode(
        test_pixels,
        compiled_example,
        desktop_mode,
        recreate,
        test_device_loss,
        seconds,
    )?;
    if reference_pose
        && (compiled_example != Some(compiled::Example::Animated)
            || desktop_mode
            || recreate != 0
            || test_device_loss
            || test_pixels)
    {
        return Err(
            "Reference pose requires animated-shapes in window mode without other test flags"
                .into(),
        );
    }
    let clock_mode = compiled_example == Some(compiled::Example::Clock);
    if clock_mode && desktop_mode {
        return Err("clock currently requires window mode".into());
    }
    if test_clock_rollover
        && (!clock_mode
            || !matches!(seconds, Some(5..=30))
            || recreate != 0
            || test_device_loss
            || test_pixels
            || reference_pose)
    {
        return Err(
            "Clock rollover requires --example clock --seconds 5..30 without other test flags"
                .into(),
        );
    }
    if let Some(mode) = worker_live {
        if worker_snapshot.is_some()
            || test_clock_rollover
            || recreate != 0
            || test_device_loss
            || test_pixels
            || reference_pose
            || desktop_mode
            || (check_worker_live && (compiled_example.is_some() || seconds.is_some()))
            || (!check_worker_live
                && (!clock_mode
                    || (mode == live_clock::Mode::Fixture && !matches!(seconds, Some(8..=30)))))
        {
            return Err("Clock worker requires its own mode. Window modes require --example clock. The fixed fixture also requires --seconds 8..30.".into());
        }
        if check_worker_live {
            return live_clock::check(mode);
        }
    }
    let clock = if let Some(folder) = worker_snapshot.as_deref() {
        if test_clock_rollover
            || recreate != 0
            || test_device_loss
            || test_pixels
            || reference_pose
            || desktop_mode
        {
            return Err("Clock snapshot requires window mode without other test flags".into());
        }
        if check_worker_snapshot {
            if compiled_example.is_some() || seconds.is_some() {
                return Err("Clock snapshot check accepts only a run folder".into());
            }
            let clock = clock::Clock::from_worker(Instant::now(), folder)?;
            println!("Accepted clock snapshot: {} UTC", clock.text());
            return Ok(());
        }
        if !clock_mode {
            return Err("Worker snapshot requires --example clock".into());
        }
        Some(clock::Clock::from_worker(Instant::now(), folder)?)
    } else if worker_live.is_some() {
        Some(clock::Clock::live_fallback(Instant::now())?)
    } else if clock_mode {
        Some(clock::Clock::new(Instant::now(), test_clock_rollover)?)
    } else {
        None
    };
    let motion = if compiled_example == Some(compiled::Example::Animated) {
        let mut playback = motion::Playback::new(Instant::now())?;
        if reference_pose {
            playback.freeze_at_reference_pose();
        }
        Some(playback)
    } else {
        None
    };
    let events = EventLoop::<AppEvent>::with_user_event().build()?;
    events.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        motion,
        clock,
        live_clock: None,
        clock_pause: live_clock::PauseState::default(),
        start_live_clock: worker_live,
        state: None,
        desktop: None,
        desktop_mode,
        compiled_example,
        retained_scene,
        start_script_package: script_package,
        test_pixels,
        pixels_passed: false,
        next_probe: Instant::now() + Duration::from_secs(2),
        next_retry: None,
        budget: Default::default(),
        generation: 0,
        proxy: events.create_proxy(),
        deadline: seconds.map(|s| Instant::now() + Duration::from_secs(s)),
        error: None,
        frames: 0,
        recreate_remaining: recreate,
        next_recreate: None,
        loss_test: lifecycle::DeviceLossTest::new(test_device_loss),
        next_device_test: None,
    };
    events.run_app(&mut app)?;
    eprintln!(
        "Preview stopped; presented frames={}; generations={}; pending recreations={}",
        app.frames, app.generation, app.recreate_remaining
    );
    eprintln!("Device loss test: {:?}", app.loss_test);
    if app.test_pixels && !app.pixels_passed {
        return Err(app
            .error
            .unwrap_or_else(|| "GPU pixel test interrupted before successful comparison".into())
            .into());
    }
    if !app.loss_test.complete() {
        return Err("Device loss test did not observe callback and restored frame".into());
    }
    if app.recreate_remaining > 0 || app.next_recreate.is_some() {
        return Err("Recreation test interrupted before completion".into());
    }
    if let Some(e) = app.error {
        return Err(e.into());
    }
    Ok(())
}
fn validate_pixel_mode(
    enabled: bool,
    example: Option<compiled::Example>,
    desktop: bool,
    recreate: u32,
    loss: bool,
    seconds: Option<u64>,
) -> Result<()> {
    if enabled
        && (example != Some(compiled::Example::GroupEffects)
            || desktop
            || recreate != 0
            || loss
            || !matches!(seconds, Some(1..=30)))
    {
        return Err("Pixel test requires --example groups-effects --seconds 1..30, window mode and no other test mode".into());
    }
    Ok(())
}
struct App {
    motion: Option<motion::Playback>,
    clock: Option<clock::Clock>,
    live_clock: Option<live_clock::Bridge>,
    clock_pause: live_clock::PauseState,
    start_live_clock: Option<live_clock::Mode>,
    state: Option<State>,
    desktop: Option<wallpaper_platform::Desktop>,
    desktop_mode: bool,
    compiled_example: Option<compiled::Example>,
    retained_scene: Option<Arc<wallpaper_scene::ValidatedScene>>,
    start_script_package: Option<script_package::Prepared>,
    test_pixels: bool,
    pixels_passed: bool,
    next_probe: Instant,
    next_retry: Option<Instant>,
    budget: lifecycle::RecoveryBudget,
    generation: u64,
    proxy: EventLoopProxy<AppEvent>,
    deadline: Option<Instant>,
    error: Option<String>,
    frames: u64,
    recreate_remaining: u32,
    next_recreate: Option<Instant>,
    loss_test: lifecycle::DeviceLossTest,
    next_device_test: Option<Instant>,
}
impl App {
    fn apply_package_update(&mut self, events: &ActiveEventLoop) {
        let Some(bridge) = &self.live_clock else {
            return;
        };
        let update = match bridge.poll() {
            Ok(update) => update,
            Err(error) => {
                self.live_clock = None;
                self.fail(events, error);
                return;
            }
        };
        if let Some(snapshot) = update.snapshot {
            let scene = Arc::clone(&snapshot.scene);
            if let Some(state) = &mut self.state {
                let result = state
                    .compiled
                    .as_mut()
                    .ok_or_else(|| "Missing package renderer".into())
                    .and_then(|renderer| renderer.sync_snapshot(snapshot, &mut state.fonts));
                if let Err(error) = result {
                    self.fail(events, error);
                    return;
                }
                state.window.request_redraw();
            }
            self.retained_scene = Some(scene);
            eprintln!("Accepted worker package snapshot");
        }
        if let Some(terminal) = update.terminal {
            self.live_clock = None;
            match terminal {
                Ok(()) => eprintln!("Package worker completed; scene retained"),
                Err(error) => eprintln!("Package worker stopped: {error}; scene retained"),
            }
        }
    }

    fn apply_live_update(&mut self) {
        if let (Some(bridge), Some(clock)) = (self.live_clock.as_ref(), self.clock.as_mut()) {
            match bridge.poll() {
                Ok(update) => {
                    if let Some(paused) = update.paused {
                        eprintln!(
                            "Clock worker lifecycle acknowledged: {}",
                            if paused { "pause" } else { "resume" }
                        );
                    }
                    if let Some(snapshot) = update.snapshot {
                        clock.accept_worker(snapshot);
                        eprintln!("Accepted worker clock: {} UTC", clock.text());
                        if let Some(state) = self.state.as_ref() {
                            state.window.request_redraw();
                        }
                    }
                    if let Some(terminal) = update.terminal {
                        self.live_clock = None;
                        match terminal {
                            Ok(()) => {
                                eprintln!("Clock worker completed; retained {} UTC", clock.text())
                            }
                            Err(error) => eprintln!(
                                "Clock worker stopped: {error}; retained {} UTC",
                                clock.text()
                            ),
                        }
                    }
                }
                Err(error) => {
                    eprintln!("Clock worker updates unavailable: {error}");
                    self.live_clock = None;
                }
            }
        }
    }

    fn motion_reason(&mut self, events: &ActiveEventLoop, reason: u8, enabled: bool) -> bool {
        if let Some(motion) = self.motion.as_mut()
            && let Err(error) = motion.reason(Instant::now(), reason, enabled)
        {
            self.fail(events, error);
            return false;
        }
        true
    }
    fn fail(&mut self, events: &ActiveEventLoop, error: impl std::fmt::Display) {
        self.error = Some(error.to_string());
        self.state = None;
        self.desktop = None;
        events.exit();
    }
    fn recover(&mut self, events: &ActiveEventLoop, reason: &str) {
        if !self.motion_reason(events, motion::RECOVERY, true) {
            return;
        }
        if self.test_pixels {
            self.fail(events, reason);
            return;
        }
        match self.budget.consume() {
            Ok(attempt) => {
                eprintln!("Recovery {attempt}/10: {reason}");
                self.state = None;
                self.desktop = None;
                self.next_retry = Some(Instant::now() + Duration::from_millis(500));
                self.next_recreate = None;
            }
            Err(e) => self.fail(events, e),
        }
    }
    fn create(&mut self, events: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        self.next_retry = None;
        self.generation += 1;
        let result = (|| -> Result<State> {
            let desktop = if self.desktop_mode {
                Some(wallpaper_platform::Desktop::connect()?)
            } else {
                None
            };
            let attrs = Window::default_attributes()
                .with_title(match self.compiled_example {
                    Some(compiled::Example::Static) => "Sahne — M3 compiled static example",
                    Some(compiled::Example::Image) => "Sahne — M3 compiled image example",
                    Some(compiled::Example::Groups) => "Sahne — M3 compiled groups example",
                    Some(compiled::Example::GroupEffects) => "Sahne — M3 group effects example",
                    Some(compiled::Example::Animated) => "Sahne — M3 native animated shapes",
                    Some(compiled::Example::Clock) => "Sahne — M4 UTC clock",
                    None if self.retained_scene.is_some() => "Sahne — M4 retained scene",
                    None => "Sahne — M1 trusted scene",
                })
                .with_inner_size(LogicalSize::new(960.0, 600.0));
            let attrs = if let Some(ref d) = desktop {
                d.attributes(attrs)?
            } else {
                attrs
            };
            let window = Arc::new(events.create_window(attrs)?);
            self.desktop = desktop;
            let mut state = pollster::block_on(State::new(
                window,
                events,
                self.proxy.clone(),
                self.generation,
                !self.loss_test.complete(),
                (self.compiled_example, self.retained_scene.as_ref()),
                self.test_pixels,
            ))?;
            if self.start_live_clock == Some(live_clock::Mode::Utc)
                || self
                    .live_clock
                    .as_ref()
                    .is_some_and(live_clock::Bridge::is_utc)
            {
                let proxy = self.proxy.clone();
                let generation = self.generation;
                state._session_lifecycle =
                    Some(wallpaper_platform::session_lifecycle::Subscription::new(
                        Arc::clone(&state.window),
                        move |event| {
                            let _ =
                                proxy.send_event(AppEvent::WindowsLifecycle { generation, event });
                        },
                    )?);
            }
            Ok(state)
        })();
        match result {
            Ok(state) => {
                eprintln!(
                    "Generation {} created: size={:?}, scale={}",
                    self.generation,
                    state.window.inner_size(),
                    state.window.scale_factor()
                );
                for monitor in events.available_monitors() {
                    eprintln!(
                        "Monitor: {:?} {:?} position={:?} scale={} refresh={:?}",
                        monitor.name(),
                        monitor.size(),
                        monitor.position(),
                        monitor.scale_factor(),
                        monitor.refresh_rate_millihertz()
                    );
                }
                state.window.request_redraw();
                self.clock_pause.suspended = false;
                self.clock_pause.minimized = state.window.is_minimized() == Some(true);
                if let Some(bridge) = &self.live_clock {
                    bridge.set_paused(self.clock_pause.paused());
                }
                let zero =
                    state.window.inner_size().width == 0 || state.window.inner_size().height == 0;
                self.state = Some(state);
                for (reason, enabled) in [
                    (motion::HIDDEN, false),
                    (motion::ZERO_SIZE, zero),
                    (motion::SUSPENDED, false),
                    (motion::RECOVERY, false),
                ] {
                    if !self.motion_reason(events, reason, enabled) {
                        return;
                    }
                }
            }
            Err(e) => self.recover(events, &format!("Initialization failed: {e}")),
        }
    }
}
impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.next_retry.is_none() {
            self.create(events);
        }
    }
    fn suspended(&mut self, events: &ActiveEventLoop) {
        self.clock_pause.suspended = true;
        if let Some(bridge) = &self.live_clock {
            bridge.set_paused(true);
        }
        if !self.motion_reason(events, motion::SUSPENDED, true) {
            return;
        }
        self.state = None;
        self.desktop = None;
        self.next_recreate = None;
    }
    fn user_event(&mut self, events: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::ClockUpdate if self.clock.is_some() => self.apply_live_update(),
            AppEvent::ClockUpdate => self.apply_package_update(events),
            AppEvent::WindowsLifecycle { generation, event }
                if generation == self.generation && self.state.is_some() =>
            {
                self.clock_pause.apply(event);
                if let Some(bridge) = &self.live_clock {
                    bridge.set_paused(self.clock_pause.paused());
                }
                eprintln!("Windows clock lifecycle: {event:?}");
            }
            AppEvent::DeviceLost(loss)
                if loss.generation == self.generation && self.state.is_some() =>
            {
                self.loss_test
                    .callback(loss.generation, loss.explicitly_destroyed);
                self.recover(events, &loss.reason);
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        let now = Instant::now();
        if self.deadline.is_some_and(|d| now >= d) {
            if self.frames == 0
                || self.state.is_none()
                || self.next_retry.is_some()
                || self.recreate_remaining > 0
                || self.next_recreate.is_some()
            {
                self.error =
                    Some("Deadline reached before a healthy final scene/test completion".into());
            }
            events.exit();
            return;
        }
        if self.next_retry.is_some_and(|d| now >= d) {
            self.create(events);
        }
        if self.next_recreate.is_some_and(|d| now >= d) {
            self.recreate_remaining -= 1;
            self.recover(
                events,
                "Injected window/GPU teardown (not an Explorer restart)",
            );
        }
        if self.next_device_test.is_some_and(|d| now >= d) {
            self.next_device_test = None;
            if let Some(state) = self.state.as_ref() {
                self.loss_test.request(self.generation);
                eprintln!("Injecting device.destroy generation={}", self.generation);
                state.device.destroy();
                let _ = state.device.poll(wgpu::PollType::Poll);
            }
        }
        if self.desktop_mode && now >= self.next_probe {
            let size = self.desktop.as_ref().map(|d| d.size());
            match size {
                Some(Ok(size)) => {
                    if let Some(state) = self.state.as_ref()
                        && state.window.inner_size() != size
                    {
                        let _ = state.window.request_inner_size(size);
                        eprintln!("Desktop size changed: {size:?}");
                    }
                }
                Some(Err(e)) => self.recover(events, &format!("Desktop invalid: {e}")),
                None => {}
            }
            self.next_probe = Instant::now() + Duration::from_secs(2);
        }
        if let (Some(motion), Some(state)) = (self.motion.as_mut(), self.state.as_ref())
            && motion.wake().is_some_and(|wake| now >= wake)
        {
            state.window.request_redraw();
            motion.queued();
        }
        if let (Some(clock), Some(state)) = (self.clock.as_mut(), self.state.as_ref()) {
            match clock.tick(Instant::now()) {
                Ok(true) => state.window.request_redraw(),
                Ok(false) => {}
                Err(error) => {
                    self.fail(events, error);
                    return;
                }
            }
        }
        let wake = [
            self.state
                .as_ref()
                .and(self.clock.as_ref())
                .and_then(clock::Clock::wake),
            self.motion.as_ref().and_then(motion::Playback::wake),
            self.deadline,
            self.next_retry,
            self.next_recreate,
            self.next_device_test,
            self.desktop_mode.then_some(self.next_probe),
        ]
        .into_iter()
        .flatten()
        .min();
        events.set_control_flow(wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
    fn window_event(&mut self, events: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if !self.state.as_ref().is_some_and(|s| s.window.id() == id) {
            return;
        }
        if matches!(event, WindowEvent::Destroyed) {
            self.recover(events, "Native window destroyed");
            return;
        }
        if self
            .desktop
            .as_ref()
            .is_some_and(|desktop| !desktop.alive())
        {
            self.recover(events, "Desktop owner invalid before window event");
            return;
        }
        let state = self.state.as_mut().expect("checked window");
        let result = (|| -> Result<()> {
            match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    event:
                        winit::event::KeyEvent {
                            logical_key: Key::Named(NamedKey::Escape),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    events.exit();
                    Ok(())
                }
                WindowEvent::KeyboardInput {
                    event:
                        winit::event::KeyEvent {
                            logical_key: Key::Named(NamedKey::Space),
                            state: ElementState::Pressed,
                            repeat: false,
                            ..
                        },
                    ..
                } => {
                    if let Some(motion) = self.motion.as_mut() {
                        motion.toggle(Instant::now())?;
                        state.window.request_redraw();
                    }
                    Ok(())
                }
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    if let Some(minimized) = state.window.is_minimized() {
                        self.clock_pause.minimized = minimized;
                        if let Some(bridge) = &self.live_clock {
                            bridge.set_paused(self.clock_pause.paused());
                        }
                    }
                    eprintln!(
                        "Surface event: size={:?} scale={}",
                        state.window.inner_size(),
                        state.window.scale_factor()
                    );
                    state.resize();
                    if let Some(motion) = self.motion.as_mut() {
                        let size = state.window.inner_size();
                        motion.reason(
                            Instant::now(),
                            motion::ZERO_SIZE,
                            size.width == 0 || size.height == 0,
                        )?;
                    }
                    state.window.request_redraw();
                    Ok(())
                }
                WindowEvent::RedrawRequested => (|| -> Result<()> {
                    if let Some(motion) = self.motion.as_mut() {
                        let sample = motion.sample(Instant::now())?;
                        state
                            .groups
                            .as_mut()
                            .ok_or("Missing animated group renderer")?
                            .update_motion(&sample.scene, &state.device, &state.queue)?;
                    }
                    if let Some(clock) = self.clock.as_mut() {
                        clock.tick(Instant::now())?;
                        state
                            .compiled
                            .as_mut()
                            .ok_or("Missing clock renderer")?
                            .sync_clock(clock, &mut state.fonts)?;
                    }
                    let presented = state.draw()?;
                    if let Some(motion) = self.motion.as_mut() {
                        motion.rendered(Instant::now())?;
                    }
                    if presented {
                        if self.test_pixels {
                            self.pixels_passed = true;
                            events.exit();
                        }
                        self.loss_test.frame(self.generation);
                        if self.loss_test.needs_request() && self.next_device_test.is_none() {
                            self.next_device_test = Some(Instant::now() + Duration::from_secs(1));
                        }
                        if let Some(mode) = self.start_live_clock.take() {
                            let proxy = self.proxy.clone();
                            match live_clock::Bridge::start(mode, move || {
                                let _ = proxy.send_event(AppEvent::ClockUpdate);
                            }) {
                                Ok(bridge) => {
                                    bridge.set_paused(self.clock_pause.paused());
                                    self.live_clock = Some(bridge);
                                }
                                Err(error) => eprintln!("Clock worker could not start: {error}"),
                            }
                        }
                        if let Some(package) = self.start_script_package.take() {
                            let proxy = self.proxy.clone();
                            match live_clock::Bridge::start_package(package, move || {
                                let _ = proxy.send_event(AppEvent::ClockUpdate);
                            }) {
                                Ok(bridge) => self.live_clock = Some(bridge),
                                Err(error) => eprintln!("Package worker could not start: {error}"),
                            }
                        }
                        self.frames += 1;
                        eprintln!(
                            "Presented frame {} generation={}",
                            self.frames, self.generation
                        );
                        if self.recreate_remaining > 0 && self.next_recreate.is_none() {
                            self.next_recreate = Some(Instant::now() + Duration::from_secs(1));
                        }
                    }
                    Ok(())
                })(),
                WindowEvent::Occluded(hidden) => {
                    if let Some(motion) = self.motion.as_mut() {
                        motion.reason(Instant::now(), motion::HIDDEN, hidden)?;
                    }
                    if !hidden {
                        state.window.request_redraw();
                    }
                    Ok(())
                }
                _ => Ok(()),
            }
        })();
        if let Err(e) = result {
            self.recover(events, &format!("Renderer error: {e}"));
        }
    }
}
struct State {
    _session_lifecycle: Option<wallpaper_platform::session_lifecycle::Subscription>,
    pixel_pending: bool,
    surface_retries: u8,
    surface: wgpu::Surface<'static>,
    instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    image: wgpu::BindGroup,
    fonts: FontSystem,
    text: Buffer,
    swash: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    compiled: Option<compiled::Renderer>,
    groups: Option<groups::Renderer>,
    window: Arc<Window>,
}
impl State {
    async fn new(
        window: Arc<Window>,
        events: &ActiveEventLoop,
        proxy: EventLoopProxy<AppEvent>,
        generation: u64,
        report_destroyed: bool,
        scene_source: (
            Option<compiled::Example>,
            Option<&Arc<wallpaper_scene::ValidatedScene>>,
        ),
        test_pixels: bool,
    ) -> Result<Self> {
        let (compiled_example, retained_scene) = scene_source;
        let mut desc = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
            events.owned_display_handle(),
        ));
        desc.backends = wgpu::Backends::DX12;
        let instance = wgpu::Instance::new(desc);
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;
        eprintln!(
            "Adapter: {:?}; DPI scale={}",
            adapter.get_info(),
            window.scale_factor()
        );
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;
        device.set_device_lost_callback(move |reason, message| {
            if reason != wgpu::DeviceLostReason::Destroyed || report_destroyed {
                let _ = proxy.send_event(AppEvent::DeviceLost(DeviceLost {
                    generation,
                    reason: format!("Device lost: {reason:?}: {message}"),
                    explicitly_destroyed: reason == wgpu::DeviceLostReason::Destroyed,
                }));
            }
        });
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("Unsupported surface")?;
        if compiled_example.is_some() || retained_scene.is_some() {
            config.format = surface
                .get_capabilities(&adapter)
                .formats
                .into_iter()
                .find(wgpu::TextureFormat::is_srgb)
                .ok_or("Compiled example requires an sRGB surface")?;
        }
        surface.configure(&device, &config);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Trusted fixture"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Fixture"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(config.format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Built-in RGBA image"),
            size: wgpu::Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let pixels: Vec<u8> = (0..64)
            .flat_map(|p| {
                if (p / 8 + p % 8) % 2 == 0 {
                    [220, 150, 70, 255]
                } else {
                    [30, 95, 115, 255]
                }
            })
            .collect();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(32),
                rows_per_image: Some(8),
            },
            wgpu::Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&Default::default());
        let image = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let mut db = glyphon::fontdb::Database::new();
        db.load_font_data(include_bytes!("../../../assets/fonts/NotoSans.ttf").to_vec());
        db.set_sans_serif_family("Noto Sans");
        let mut fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
        let mut text = Buffer::new(&mut fonts, Metrics::new(32.0, 44.0));
        text.set_size(Some(820.0), Some(140.0));
        text.set_text(
            "Sahne / M1\nStatic GPU scene: shape + text + image",
            &Attrs::new().family(Family::Name("Noto Sans")),
            Shaping::Advanced,
            None,
        );
        text.shape_until_scroll(&mut fonts, false);
        if text.layout_runs().all(|run| run.glyphs.is_empty()) {
            return Err("Bundled font produced no text glyphs".into());
        }
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::new(&device, &queue, &cache, config.format);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let groups =
            if let Some(fixture) = compiled_example.and_then(compiled::Example::group_fixture) {
                let memory_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
                let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
                let result = groups::Renderer::new(
                    &device,
                    &queue,
                    config.format,
                    &mut fonts,
                    fixture,
                    test_pixels,
                );
                let validation = validation_scope.pop();
                let memory = memory_scope.pop();
                let validation = validation.await;
                let memory = memory.await;
                if let Some(error) = validation.or(memory) {
                    return Err(format!("Group GPU initialization failed: {error}").into());
                }
                Some(result?)
            } else {
                None
            };
        let flat_source = retained_scene.map(compiled::Source::Retained).or_else(|| {
            compiled_example
                .filter(|v| v.group_fixture().is_none())
                .map(compiled::Source::Example)
        });
        let compiled = if let Some(source) = flat_source {
            Some(compiled::Renderer::new(
                &device,
                &queue,
                config.format,
                &mut fonts,
                &mut atlas,
                source,
            )?)
        } else {
            None
        };
        Ok(Self {
            _session_lifecycle: None,
            pixel_pending: test_pixels,
            surface_retries: 0,
            surface,
            instance,
            device,
            queue,
            config,
            pipeline,
            image,
            fonts,
            text,
            swash: SwashCache::new(),
            viewport,
            atlas,
            text_renderer,
            compiled,
            groups,
            window,
        })
    }
    fn retry_surface(&mut self) -> Result<()> {
        self.surface_retries += 1;
        if self.surface_retries > 3 {
            return Err("Surface recovery exhausted".into());
        }
        self.window.request_redraw();
        Ok(())
    }
    fn resize(&mut self) {
        let size = self.window.inner_size();
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }
    fn draw(&mut self) -> Result<bool> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(false);
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.retry_surface()?;
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.retry_surface()?;
                self.resize();
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.retry_surface()?;
                self.surface = self.instance.create_surface(self.window.clone())?;
                self.resize();
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("Surface validation failed".into());
            }
        };
        self.surface_retries = 0;
        self.viewport.update(
            &self.queue,
            Resolution {
                width: self.config.width,
                height: self.config.height,
            },
        );
        if let Some(groups) = self.groups.as_mut() {
            groups.prepare(
                &self.device,
                &self.queue,
                &mut self.fonts,
                self.config.width,
                self.config.height,
            )?;
        } else if let Some(compiled) = self.compiled.as_mut() {
            compiled.prepare(&mut compiled::Prepare {
                device: &self.device,
                queue: &self.queue,
                fonts: &mut self.fonts,
                atlas: &mut self.atlas,
                viewport: &self.viewport,
                swash: &mut self.swash,
                width: self.config.width,
                height: self.config.height,
            })?;
        } else {
            self.text_renderer.prepare(
                &self.device,
                &self.queue,
                &mut self.fonts,
                &mut self.atlas,
                &self.viewport,
                [TextArea {
                    buffer: &self.text,
                    left: 40.0 * self.window.scale_factor() as f32,
                    top: 32.0 * self.window.scale_factor() as f32,
                    scale: self.window.scale_factor() as f32,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: self.config.width as i32,
                        bottom: self.config.height as i32,
                    },
                    default_color: Color::rgb(235, 238, 230),
                    custom_glyphs: &[],
                }],
                &mut self.swash,
            )?;
        }
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        if let Some(groups) = self.groups.as_mut() {
            groups.encode(&mut encoder, &view, self.config.width, self.config.height)?;
        } else {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(
                            self.compiled
                                .as_ref()
                                .map_or(wgpu::Color::BLACK, compiled::Renderer::clear_color),
                        ),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            if let Some(compiled) = self.compiled.as_ref() {
                compiled.render(&self.atlas, &self.viewport, &mut pass)?;
            } else {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.image, &[]);
                pass.draw(0..3, 0..1);
                self.text_renderer
                    .render(&self.atlas, &self.viewport, &mut pass)?;
            }
        }
        self.window.pre_present_notify();
        let readback = if self.pixel_pending {
            Some(pixels::Readback::encode(
                &self.device,
                &mut encoder,
                self.groups
                    .as_ref()
                    .ok_or("Missing pixel-test renderer")?
                    .pixel_source()?,
            )?)
        } else {
            None
        };
        let submission = self.queue.submit([encoder.finish()]);
        if let Some(readback) = readback {
            readback.verify(&self.device, submission)?;
            self.pixel_pending = false;
        }
        self.queue.present(frame);
        self.atlas.trim();
        Ok(true)
    }
}

#[cfg(test)]
mod argument_tests {
    use super::*;
    #[test]
    fn pixel_mode_rejects_other_sources_and_unbounded_or_combined_runs() {
        let valid = Some(compiled::Example::GroupEffects);
        assert!(validate_pixel_mode(true, valid, false, 0, false, Some(6)).is_ok());
        for example in [
            None,
            Some(compiled::Example::Static),
            Some(compiled::Example::Image),
            Some(compiled::Example::Groups),
            Some(compiled::Example::Animated),
        ] {
            assert!(validate_pixel_mode(true, example, false, 0, false, Some(6)).is_err());
        }
        for seconds in [None, Some(0), Some(31), Some(3600)] {
            assert!(validate_pixel_mode(true, valid, false, 0, false, seconds).is_err());
        }
        assert!(validate_pixel_mode(true, valid, true, 0, false, Some(6)).is_err());
        assert!(validate_pixel_mode(true, valid, false, 1, false, Some(6)).is_err());
        assert!(validate_pixel_mode(true, valid, false, 0, true, Some(6)).is_err());
        assert!(validate_pixel_mode(false, None, true, 1, false, None).is_ok());
    }
}
