//! Bounded development bridge to the existing AppContainer clock fixture.
use super::*;
use script_package::Lifecycle;
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Write},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};
use wallpaper_protocol::{
    Body, Message, SCENE_REJECT_CODE, SCRIPT_FAILURE_CODE, accept_scene_batch, read_frame,
    write_frame,
};
use wallpaper_scene::{
    Color, Content,
    transaction::{Edit, HostScene, Snapshot},
};

const POLL_INTERVAL: Duration = Duration::from_millis(25);
const RESPONSE_LIMIT: Duration = Duration::from_secs(8);
const PACKAGE_WORKER_TIMEOUT: &str = "Package worker exchange timed out";
const PACKAGE_WORKER_EOF: &str = "Package worker closed before sending a scene batch";
const PACKAGE_WORKER_CONTEXT: &str = "Package worker response context did not match";
const TICK_GAP: Duration = Duration::from_millis(1400);
const MINUTES: [u16; 3] = [1439, 0, 0];
type WorkerResult<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Fixture,
    Utc,
}

#[derive(Default)]
pub struct PauseState {
    pub minimized: bool,
    pub suspended: bool,
    session_locked: bool,
    system_suspended: bool,
    display_off: bool,
}

impl PauseState {
    pub fn apply(&mut self, event: wallpaper_platform::session_lifecycle::Event) {
        use wallpaper_platform::session_lifecycle::Event;
        match event {
            Event::SessionLocked(locked) => self.session_locked = locked,
            Event::SystemSuspended(suspended) => self.system_suspended = suspended,
            Event::SessionDisplayOff(off) => self.display_off = off,
        }
    }

    pub fn paused(&self) -> bool {
        self.minimized
            || self.suspended
            || self.session_locked
            || self.system_suspended
            || self.display_off
    }
}

#[cfg(test)]
mod pause_tests {
    use super::PauseState;
    use wallpaper_platform::session_lifecycle::Event;

    #[test]
    fn clearing_one_pause_reason_does_not_resume_clock() {
        let mut state = PauseState {
            minimized: true,
            ..PauseState::default()
        };
        state.apply(Event::SessionLocked(true));
        state.apply(Event::SystemSuspended(true));
        state.apply(Event::SessionDisplayOff(true));
        state.apply(Event::SystemSuspended(false));
        assert!(state.paused());
        state.minimized = false;
        assert!(state.paused());
        state.suspended = true;
        state.apply(Event::SessionLocked(false));
        assert!(state.paused());
        state.suspended = false;
        assert!(state.paused());
        state.apply(Event::SessionDisplayOff(false));
        assert!(!state.paused());
    }
}

enum Signal {
    Deadline(Option<Instant>),
    Finished(WorkerResult<()>),
    Cancel,
}

struct ChannelState {
    update: Mutex<Update>,
    cancel: AtomicBool,
    paused: AtomicBool,
    channel_thread: OnceLock<thread::Thread>,
    notify: Box<dyn Fn() + Send + Sync>,
    signal: mpsc::SyncSender<Signal>,
    mode: Mode,
    handles_lifecycle: bool,
}

#[derive(Default)]
pub struct Update {
    pub snapshot: Option<Snapshot>,
    pub paused: Option<bool>,
    pub terminal: Option<std::result::Result<(), String>>,
}

pub struct Bridge {
    state: Arc<ChannelState>,
    supervisor: Option<JoinHandle<()>>,
}

impl Bridge {
    pub fn handles_lifecycle(&self) -> bool {
        self.state.handles_lifecycle
    }
    pub fn start(mode: Mode, notify: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        Self::start_inner(mode, None, false, notify)
    }

    pub fn start_package(
        package: script_package::Prepared,
        paused: bool,
        notify: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self> {
        Self::start_inner(Mode::Fixture, Some(package), paused, notify)
    }

    fn start_inner(
        mode: Mode,
        package: Option<script_package::Prepared>,
        paused: bool,
        notify: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self> {
        let (signal, receiver) = mpsc::sync_channel(2);
        let state = Arc::new(ChannelState {
            update: Mutex::new(Update::default()),
            cancel: AtomicBool::new(false),
            paused: AtomicBool::new(paused),
            channel_thread: OnceLock::new(),
            notify: Box::new(notify),
            signal,
            mode,
            handles_lifecycle: mode == Mode::Utc
                || package
                    .as_ref()
                    .is_some_and(|p| p.lifecycle.handles_live_events()),
        });
        let shared = Arc::clone(&state);
        let supervisor = thread::Builder::new()
            .name("clock-supervisor".into())
            .spawn(move || {
                let result =
                    supervise(&shared, receiver, package).map_err(|error| error.to_string());
                if let Ok(mut pending) = shared.update.lock() {
                    pending.terminal = Some(result);
                }
                (shared.notify)();
            })?;
        Ok(Self {
            state,
            supervisor: Some(supervisor),
        })
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.state.cancel.store(true, Ordering::Release);
        let _ = self.state.signal.try_send(Signal::Cancel);
        if let Some(supervisor) = self.supervisor.take() {
            supervisor.join().map_err(|_| "Clock supervisor panicked")?;
        }
        Ok(())
    }

    pub fn set_paused(&self, paused: bool) {
        if self.state.handles_lifecycle
            && self.state.paused.swap(paused, Ordering::AcqRel) != paused
            && let Some(channel) = self.state.channel_thread.get()
        {
            channel.unpark();
        }
    }

    pub fn poll(&self) -> Result<Update> {
        Ok(std::mem::take(
            &mut *self
                .state
                .update
                .lock()
                .map_err(|_| "Clock mailbox unavailable")?,
        ))
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn supervise(
    state: &Arc<ChannelState>,
    receiver: mpsc::Receiver<Signal>,
    package: Option<script_package::Prepared>,
) -> WorkerResult<()> {
    crate::package_intake::recover_profiles_on_startup();
    let exe = std::env::current_exe()?.with_file_name("wallpaper-sandbox-probe.exe");
    if !exe.metadata()?.is_file() {
        return Err("Missing sibling sandbox probe executable".into());
    }
    let session = wallpaper_sandbox::random_id()?;
    let nonce = wallpaper_sandbox::random_id()?;
    let (initial, writable, program) = if let Some(package) = package {
        let Body::SceneProgram { targets, .. } = &package.program else {
            return Err("Expected package scene program".into());
        };
        (
            wallpaper_scene::decode(&package.scene.encode())?,
            targets.values().copied().collect(),
            Some((package.program, package.lifecycle)),
        )
    } else {
        (
            clock::initial_scene().map_err(|error| error.to_string())?,
            BTreeSet::from([clock::CLOCK_ID, clock::ACCENT_ID]),
            None,
        )
    };
    let scene = HostScene::new(initial, session, &writable)
        .map_err(|_| "Could not initialize worker scene")?;
    let base = std::env::current_dir()?.join("target/sandbox-runs");
    fs::create_dir_all(&base)?;
    let root = base.join(format!("preview-{}-{session:016x}", std::process::id()));
    let package_mode = program.is_some();
    let case = if let Some((_, lifecycle)) = &program {
        match lifecycle {
            Lifecycle::OneShot => "local-script-package",
            Lifecycle::Check => "preview-lifecycle-package",
            Lifecycle::Live => "live-lifecycle-package",
            Lifecycle::WatchdogCheck => "live-lifecycle-watchdog-check",
            Lifecycle::WorkerExitCheck => "live-lifecycle-exit-check",
            Lifecycle::StaleBatchCheck => "live-lifecycle-stale-check",
        }
    } else {
        match state.mode {
            Mode::Fixture => "scene-timer",
            Mode::Utc => "preview-clock-utc",
        }
    };
    let wallpaper_sandbox::Channel {
        mut sandbox,
        input,
        output,
    } = wallpaper_sandbox::spawn_probe(&exe, &root, session, case, 0, None)?;
    let shared = Arc::clone(state);
    let io_thread = thread::Builder::new()
        .name("clock-channel".into())
        .spawn(move || {
            let _ = shared.channel_thread.set(thread::current());
            let result = exchange(input, output, session, nonce, scene, &shared, program);
            let _ = shared.signal.send(Signal::Finished(result));
        });
    let io_thread = match io_thread {
        Ok(thread) => thread,
        Err(error) => {
            sandbox.cleanup_profile()?;
            return Err(error.into());
        }
    };
    let mut deadline = Some(Instant::now() + RESPONSE_LIMIT);
    let response = loop {
        if state.cancel.load(Ordering::Acquire) {
            break None;
        }
        let signal = match deadline {
            Some(deadline) => {
                receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            }
            None => receiver
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        };
        match signal {
            Ok(Signal::Deadline(next)) => deadline = next,
            Ok(Signal::Finished(result)) => break Some(result),
            Ok(Signal::Cancel) => break None,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let reason = if package_mode {
                    PACKAGE_WORKER_TIMEOUT
                } else {
                    "Clock worker timed out"
                };
                break Some(Err(reason.into()));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Some(Err("Clock worker channel disappeared".into()));
            }
        }
    };
    drop(receiver);
    let exit = if matches!(response, Some(Ok(()))) {
        sandbox.wait(1000)
    } else {
        Ok(None)
    };
    state.cancel.store(true, Ordering::Release);
    io_thread.thread().unpark();
    let stopped = sandbox.stop();
    if let Err(error) = stopped {
        drop(sandbox);
        let _ = io_thread.join();
        return Err(error.into());
    }
    let joined = io_thread
        .join()
        .map_err(|_| "Clock channel thread panicked");
    sandbox.cleanup_profile()?;
    joined?;
    if package_mode {
        println!("Package worker profile cleanup verified.");
    }
    let Some(response) = response else {
        return Ok(());
    };
    response?;
    if exit? != Some(0) {
        return Err("Clock worker did not exit cleanly".into());
    }
    Ok(())
}

fn frame(session: u64, sequence: u64, base_revision: u64, body: Body) -> Message {
    Message {
        session,
        sequence,
        base_revision,
        body,
    }
}

fn send(input: &mut File, message: Message) -> WorkerResult<()> {
    write_frame(input, &message)?;
    input.flush()?;
    Ok(())
}

fn exchange(
    mut input: File,
    mut output: File,
    session: u64,
    nonce: u64,
    mut host: HostScene,
    state: &ChannelState,
    program: Option<(Body, Lifecycle)>,
) -> WorkerResult<()> {
    let mut key = host
        .snapshot_after(None)
        .ok_or("Missing initial clock state")?
        .key;
    let hello = read_frame(&mut output)?.ok_or("Missing clock worker hello")?;
    if hello != frame(session, 1, 0, Body::Hello { api_version: 1 }) {
        return Err("Invalid clock worker hello".into());
    }
    send(&mut input, frame(session, 1, 0, Body::Ping { nonce }))?;
    if read_frame(&mut output)? != Some(frame(session, 2, 0, Body::Pong { nonce })) {
        return Err("Invalid clock worker pong".into());
    }
    if let Some(program) = program {
        return exchange_package(input, output, session, host, state, program);
    }
    let mut sequence = 1u64;
    let mut lifecycle_sequence = 1u64;
    let mut acknowledged_pause = false;
    let mut wake = Instant::now();
    loop {
        let base = sequence - 1;
        let fixture_minute = MINUTES.get(base as usize).copied();
        if state.mode == Mode::Fixture && fixture_minute.is_none() {
            break;
        }
        if base != 0
            && read_frame(&mut output)? != Some(frame(session, sequence, base, Body::ClockReady))
        {
            return Err("Invalid clock worker readiness".into());
        }
        if state.mode == Mode::Utc {
            loop {
                if state.cancel.load(Ordering::Acquire) {
                    return Err("Clock update cancelled".into());
                }
                let desired = state.paused.load(Ordering::Acquire);
                if desired != acknowledged_pause {
                    state
                        .signal
                        .send(Signal::Deadline(Some(Instant::now() + RESPONSE_LIMIT)))?;
                    send(
                        &mut input,
                        frame(
                            session,
                            lifecycle_sequence,
                            base,
                            Body::Lifecycle { paused: desired },
                        ),
                    )?;
                    if read_frame(&mut output)?
                        != Some(frame(
                            session,
                            lifecycle_sequence,
                            base,
                            Body::LifecycleAck { paused: desired },
                        ))
                    {
                        return Err("Invalid clock lifecycle acknowledgement".into());
                    }
                    acknowledged_pause = desired;
                    lifecycle_sequence = lifecycle_sequence
                        .checked_add(1)
                        .ok_or("Clock lifecycle sequence exhausted")?;
                    state
                        .update
                        .lock()
                        .map_err(|_| "Clock mailbox unavailable")?
                        .paused = Some(desired);
                    (state.notify)();
                    if desired {
                        state.signal.send(Signal::Deadline(None))?;
                    } else {
                        wake = Instant::now();
                    }
                    continue;
                }
                if acknowledged_pause {
                    thread::park();
                } else if let Some(delay) = wake.checked_duration_since(Instant::now()) {
                    thread::park_timeout(delay);
                } else {
                    break;
                }
            }
        } else {
            while let Some(delay) = wake.checked_duration_since(Instant::now()) {
                if state.cancel.load(Ordering::Acquire) {
                    return Err("Clock update cancelled".into());
                }
                thread::park_timeout(delay);
            }
        }
        let minute = match state.mode {
            Mode::Fixture => {
                wake = Instant::now() + TICK_GAP;
                fixture_minute.ok_or("Missing clock fixture minute")?
            }
            Mode::Utc => {
                let sampled_at = Instant::now();
                let elapsed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
                let (minute, delay) = clock::utc_sample(elapsed);
                wake = sampled_at + delay;
                minute
            }
        };
        send(
            &mut input,
            frame(
                session,
                sequence,
                base,
                Body::ClockTick { utc_minute: minute },
            ),
        )?;
        let batch = read_frame(&mut output)?.ok_or("Missing clock worker batch")?;
        let expected = format!("{:02}:{:02}", minute / 60, minute % 60);
        let expected_batch = Body::SceneBatch {
            edits: vec![
                Edit::Text {
                    node: clock::CLOCK_ID,
                    value: expected.clone(),
                },
                Edit::Color {
                    node: clock::ACCENT_ID,
                    value: Color([36, 179, 144, 255]),
                },
            ],
        };
        if batch != frame(session, sequence, base, expected_batch) {
            host.close();
            return Err("Unexpected clock worker edit".into());
        }
        let ack = accept_scene_batch(&mut host, batch);
        if ack
            != frame(
                session,
                sequence,
                base,
                Body::SceneAck { revision: sequence },
            )
            || !matches!(&host.scene().scene().nodes[1].content, Content::Text { text, .. } if text == &expected)
        {
            return Err("Clock worker update rejected or mismatched".into());
        }
        if let Some(snapshot) = host.snapshot_after(Some(key)) {
            key = snapshot.key;
            state
                .update
                .lock()
                .map_err(|_| "Clock mailbox unavailable")?
                .snapshot = Some(snapshot);
            (state.notify)();
        }
        send(&mut input, ack)?;
        if state.mode == Mode::Utc {
            state
                .signal
                .send(Signal::Deadline(Some(wake + RESPONSE_LIMIT)))?;
        }
        sequence = sequence.checked_add(1).ok_or("Clock sequence exhausted")?;
    }
    let invalid = read_frame(&mut output)?.ok_or("Missing terminal scene batch")?;
    let sequence = MINUTES.len() as u64 + 1;
    invalid.check_context(session, sequence, sequence - 1)?;
    if invalid.body
        != (Body::SceneBatch {
            edits: vec![
                Edit::Color {
                    node: clock::CLOCK_ID,
                    value: Color([0; 4]),
                },
                Edit::Text {
                    node: clock::CLOCK_ID,
                    value: "invalid\0text".into(),
                },
            ],
        })
    {
        host.close();
        return Err("Unexpected terminal clock fixture batch".into());
    }
    let rejection = accept_scene_batch(&mut host, invalid);
    if !matches!(
        rejection.body,
        Body::Reject {
            code: SCENE_REJECT_CODE,
            ..
        }
    ) || host.snapshot_after(Some(key)).is_some()
        || !host.is_closed()
    {
        return Err("Clock worker terminal fixture changed the accepted scene".into());
    }
    send(&mut input, rejection)?;
    drop(input);
    const FIXTURE_RESULT: u32 = 23 | (1 << 23);
    if read_frame(&mut output)?
        != Some(frame(
            session,
            3,
            0,
            Body::ProbeResult {
                denied: FIXTURE_RESULT,
            },
        ))
        || output.read(&mut [0])? != 0
    {
        return Err("Invalid clock worker completion".into());
    }
    Ok(())
}

pub fn check(mode: Mode) -> Result<()> {
    let mut current = clock::Clock::live_fallback(Instant::now())?;
    let mut bridge = Bridge::start(mode, || {})?;
    let mut seen = Vec::new();
    let mut lifecycle = Vec::new();
    let mut resume_at = None;
    let start = Instant::now();
    let limit = match mode {
        Mode::Fixture => Duration::from_secs(11),
        Mode::Utc => Duration::from_secs(85),
    };
    while start.elapsed() < limit {
        let update = bridge.poll()?;
        let remained_paused = lifecycle == [true] && update.paused != Some(false);
        if let Some(paused) = update.paused {
            lifecycle.push(paused);
            if paused {
                resume_at = Some(Instant::now() + RESPONSE_LIMIT + Duration::from_millis(200));
            }
        }
        if let Some(snapshot) = update.snapshot {
            if remained_paused {
                return Err("UTC worker published a snapshot while paused".into());
            }
            current.accept_worker(snapshot);
            seen.push(current.text().to_owned());
            if mode == Mode::Utc && seen.len() == 1 {
                bridge.set_paused(true);
            }
        }
        if resume_at.is_some_and(|at| Instant::now() >= at) {
            resume_at = None;
            bridge.set_paused(false);
        }
        if let Some(result) = update.terminal {
            result?;
            if mode != Mode::Fixture || seen != ["23:59", "00:00"] || current.text() != "00:00" {
                return Err("Clock publication sequence mismatch".into());
            }
            println!(
                "Live worker clock verified: 23:59 -> 00:00 UTC; terminal rejection preserved 00:00."
            );
            return Ok(());
        }
        if mode == Mode::Utc && seen.len() >= 2 && lifecycle == [true, false] {
            bridge.shutdown()?;
            bridge
                .poll()?
                .terminal
                .ok_or("Missing UTC worker shutdown result")??;
            if seen[0] == seen[1] {
                return Err("UTC worker repeated a clock snapshot".into());
            }
            println!(
                "Continuous worker clock verified: {} -> {} UTC; pause and resume acknowledged; worker cleaned up.",
                seen[0], seen[1]
            );
            return Ok(());
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err("Live worker clock check timed out".into())
}

fn wait_package_tick(
    input: &mut File,
    output: &mut File,
    tick: Message,
    delay: Duration,
    state: &ChannelState,
    mut lifecycle: Option<&mut (bool, u64)>,
) -> WorkerResult<()> {
    let mut wake = Instant::now() + delay;
    if lifecycle.is_some() {
        state
            .signal
            .send(Signal::Deadline(Some(wake + RESPONSE_LIMIT)))?;
    }
    loop {
        if state.cancel.load(Ordering::Acquire) {
            return Err("Package tick cancelled".into());
        }
        if let Some((paused, sequence)) = lifecycle.as_deref_mut() {
            let desired = state.paused.load(Ordering::Acquire);
            if desired != *paused {
                state
                    .signal
                    .send(Signal::Deadline(Some(Instant::now() + RESPONSE_LIMIT)))?;
                let event = frame(
                    tick.session,
                    *sequence,
                    tick.base_revision,
                    Body::Lifecycle { paused: desired },
                );
                send(input, event)?;
                if read_frame(output)?
                    != Some(frame(
                        tick.session,
                        *sequence,
                        tick.base_revision,
                        Body::LifecycleAck { paused: desired },
                    ))
                {
                    return Err("Package lifecycle callback rejected".into());
                }
                *paused = desired;
                *sequence = sequence
                    .checked_add(1)
                    .ok_or("Package lifecycle sequence exhausted")?;
                state
                    .update
                    .lock()
                    .map_err(|_| "Package mailbox unavailable")?
                    .paused = Some(desired);
                (state.notify)();
                if desired {
                    state.signal.send(Signal::Deadline(None))?;
                } else {
                    wake = Instant::now() + delay;
                    state
                        .signal
                        .send(Signal::Deadline(Some(wake + RESPONSE_LIMIT)))?;
                }
                continue;
            }
            if *paused {
                state.signal.send(Signal::Deadline(None))?;
                thread::park();
                continue;
            }
        }
        if let Some(remaining) = wake.checked_duration_since(Instant::now()) {
            thread::park_timeout(remaining);
        } else {
            return send(input, tick);
        }
    }
}

fn accept_package_response(
    output: &mut impl Read,
    host: &mut HostScene,
    session: u64,
    sequence: u64,
    base: u64,
) -> WorkerResult<(Message, bool)> {
    let batch = read_frame(output)?.ok_or(PACKAGE_WORKER_EOF)?;
    batch
        .check_context(session, sequence, base)
        .map_err(|_| PACKAGE_WORKER_CONTEXT)?;
    let worker_rejected = match &batch.body {
        Body::Reject {
            code: SCRIPT_FAILURE_CODE,
            ..
        } => true,
        Body::SceneBatch { .. } => false,
        _ => return Err("Unexpected package worker response".into()),
    };
    Ok((accept_scene_batch(host, batch), worker_rejected))
}

fn exchange_package(
    mut input: File,
    mut output: File,
    session: u64,
    mut host: HostScene,
    state: &ChannelState,
    (program, lifecycle): (Body, Lifecycle),
) -> WorkerResult<()> {
    let mut accepted_key = host
        .snapshot_after(None)
        .ok_or("Missing initial package scene")?
        .key;
    send(&mut input, frame(session, 2, 0, program))?;
    let (delay, ticks) = if lifecycle != Lifecycle::OneShot {
        let ready = read_frame(&mut output)?.ok_or("Missing package lifecycle readiness")?;
        ready.check_context(session, 3, 0)?;
        let timing = match ready.body {
            Body::LifecycleReady if lifecycle == Lifecycle::Check => (None, 1),
            Body::PackageTimer { delay_ms, ticks } => (
                Some(Duration::from_millis(u64::from(delay_ms))),
                u64::from(ticks),
            ),
            _ => return Err("Package lifecycle initialization rejected".into()),
        };
        for (sequence, paused) in [(4, true), (5, false)]
            .into_iter()
            .filter(|_| lifecycle == Lifecycle::Check)
        {
            send(
                &mut input,
                frame(session, sequence, 0, Body::Lifecycle { paused }),
            )?;
            let reply = read_frame(&mut output)?;
            if reply != Some(frame(session, sequence, 0, Body::LifecycleAck { paused })) {
                return Err(if paused {
                    "Package pause callback rejected"
                } else {
                    "Package resume callback rejected"
                }
                .into());
            }
            println!(
                "Package lifecycle acknowledged: {}.",
                if paused { "pause" } else { "resume" }
            );
        }
        timing
    } else {
        (None, 1)
    };
    let live = lifecycle.handles_live_events();
    let mut lifecycle_state = (false, 4u64);
    for sequence in 1..=if live { u64::MAX - 5 } else { ticks } {
        let base = sequence - 1;
        if let Some(delay) = delay {
            wait_package_tick(
                &mut input,
                &mut output,
                frame(session, sequence + 5, base, Body::PackageTick),
                delay,
                state,
                live.then_some(&mut lifecycle_state),
            )?;
            if !live {
                println!("Host dispatched scheduled package tick.");
            }
        }
        let (ack, worker_rejected) =
            accept_package_response(&mut output, &mut host, session, sequence, base)?;
        if ack
            != frame(
                session,
                sequence,
                base,
                Body::SceneAck { revision: sequence },
            )
        {
            if !host.is_closed()
                || host.revision() != base
                || host.snapshot_after(Some(accepted_key)).is_some()
            {
                return Err("Package rejection changed the accepted scene".into());
            }
            if live {
                let rejection = if worker_rejected {
                    "worker callback rejection"
                } else {
                    "host scene rejection"
                };
                println!("Live package revision {base} preserved after {rejection}.");
            }
            // A worker reporting callback failure exits without waiting for a reply.
            if !worker_rejected {
                send(&mut input, ack)?;
            }
            return Err("Package scene batch rejected".into());
        }
        send(&mut input, ack)?;
        if delay.is_some() && !live {
            println!("Host acknowledged scheduled package revision {sequence}.");
        }
        if let Some(accepted) = host.snapshot_after(Some(accepted_key)) {
            accepted_key = accepted.key;
            state
                .update
                .lock()
                .map_err(|_| "Package scene mailbox unavailable")?
                .snapshot = Some(accepted);
            (state.notify)();
        }
    }
    if live {
        return Err("Package tick sequence exhausted".into());
    }
    let invalid = read_frame(&mut output)?.ok_or("Missing terminal package batch")?;
    invalid.check_context(session, ticks + 1, ticks)?;
    let rejection = accept_scene_batch(&mut host, invalid);
    if !matches!(
        rejection.body,
        Body::Reject {
            code: SCENE_REJECT_CODE,
            ..
        }
    ) || !host.is_closed()
        || host.revision() != ticks
        || host.snapshot_after(Some(accepted_key)).is_some()
    {
        return Err("Package snapshot changed during terminal rejection".into());
    }
    send(&mut input, rejection)?;
    drop(input);
    const FIXTURE_RESULT: u32 = 23 | (1 << 23);
    if read_frame(&mut output)?
        != Some(frame(
            session,
            3,
            0,
            Body::ProbeResult {
                denied: FIXTURE_RESULT,
            },
        ))
        || output.read(&mut [0])? != 0
    {
        return Err("Invalid package worker completion".into());
    }
    Ok(())
}

#[cfg(test)]
mod package_transport_tests {
    use super::*;

    #[test]
    fn package_bridge_rejects_wrong_context_and_truncation_after_an_accepted_update() {
        let session = 7;
        let mut host = HostScene::new(
            clock::initial_scene().unwrap(),
            session,
            &BTreeSet::from([clock::CLOCK_ID]),
        )
        .unwrap();
        let batch = |session, sequence, base, value: &str| {
            frame(
                session,
                sequence,
                base,
                Body::SceneBatch {
                    edits: vec![Edit::Text {
                        node: clock::CLOCK_ID,
                        value: value.into(),
                    }],
                },
            )
        };
        let mut first = Vec::new();
        write_frame(&mut first, &batch(session, 1, 0, "12:34")).unwrap();
        let result =
            accept_package_response(&mut first.as_slice(), &mut host, session, 1, 0).unwrap();
        assert_eq!(
            result,
            (frame(session, 1, 0, Body::SceneAck { revision: 1 }), false)
        );
        let accepted = host.snapshot_after(None).unwrap();
        for invalid in [
            batch(session + 1, 2, 1, "12:35"),
            batch(session, 1, 1, "12:35"),
            batch(session, 3, 1, "12:35"),
            batch(session, 2, 0, "12:35"),
            frame(session, 2, 1, Body::ClockReady),
        ] {
            let mut wire = Vec::new();
            write_frame(&mut wire, &invalid).unwrap();
            assert!(
                accept_package_response(&mut wire.as_slice(), &mut host, session, 2, 1).is_err()
            );
            assert_eq!(host.revision(), 1);
            assert!(host.snapshot_after(Some(accepted.key)).is_none());
        }
        let mut incomplete = Vec::new();
        write_frame(&mut incomplete, &batch(session, 2, 1, "12:35")).unwrap();
        incomplete.pop();
        assert!(
            accept_package_response(&mut incomplete.as_slice(), &mut host, session, 2, 1).is_err()
        );
        assert_eq!(host.revision(), 1);
        assert!(host.snapshot_after(Some(accepted.key)).is_none());
    }
}

pub fn check_package(package: script_package::Prepared) -> Result<()> {
    let bridge = Bridge::start_package(package, false, || {})?;
    let mut changed = false;
    let until = Instant::now() + RESPONSE_LIMIT + Duration::from_secs(3);
    while Instant::now() < until {
        let update = bridge.poll()?;
        changed |= update.snapshot.is_some();
        if let Some(result) = update.terminal {
            result?;
            println!(
                "Live package transaction verified; scene changed={changed}; worker cleaned up."
            );
            return Ok(());
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err("Live package worker check timed out".into())
}

pub fn check_live_package(package: script_package::Prepared) -> Result<()> {
    let expected_failure = match package.lifecycle {
        Lifecycle::WatchdogCheck => Some((
            PACKAGE_WORKER_TIMEOUT,
            "Live package watchdog stopped worker after one accepted snapshot.",
        )),
        Lifecycle::WorkerExitCheck => Some((
            PACKAGE_WORKER_EOF,
            "Live package worker exit preserved one accepted snapshot.",
        )),
        Lifecycle::StaleBatchCheck => Some((
            PACKAGE_WORKER_CONTEXT,
            "Live package rejected a stale batch and preserved one accepted snapshot.",
        )),
        _ => None,
    };
    let mut bridge = Bridge::start_package(package, true, || {})?;
    let until = Instant::now() + RESPONSE_LIMIT + Duration::from_secs(3);
    let mut resume_at = None;
    let mut resumed = false;
    let mut updates = 0;
    while Instant::now() < until {
        let update = bridge.poll()?;
        if let Some(result) = update.terminal {
            if let Some((expected_error, evidence)) = expected_failure
                && resumed
                && updates == 1
                && update.snapshot.is_none()
                && result
                    .as_ref()
                    .is_err_and(|reason| reason == expected_error)
            {
                println!("{evidence}");
                return Ok(());
            }
            result?;
            return Err("Live package ended before lifecycle verification".into());
        }
        match update.paused {
            Some(true) if resume_at.is_none() => {
                resume_at = Some(Instant::now() + Duration::from_millis(150));
            }
            Some(false) if resume_at.is_some() => resumed = true,
            _ => {}
        }
        if !resumed && resume_at.is_some_and(|at| Instant::now() >= at) {
            bridge.set_paused(false);
        }
        if update.snapshot.is_some() {
            if !resumed {
                return Err("Live package changed before resume".into());
            }
            updates += 1;
            if updates == 2 {
                if expected_failure.is_some() {
                    return Err("Failure fixture published an unexpected second snapshot".into());
                }
                bridge.shutdown()?;
                bridge
                    .poll()?
                    .terminal
                    .ok_or("Missing live package cleanup result")??;
                println!("Live package pause, resume and two updates verified.");
                return Ok(());
            }
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err("Live package lifecycle verification timed out".into())
}
