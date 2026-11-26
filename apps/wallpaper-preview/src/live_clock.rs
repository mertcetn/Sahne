//! Bounded development bridge to the existing AppContainer clock fixture.
use super::*;
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
    Body, Message, SCENE_REJECT_CODE, accept_scene_batch, read_frame, write_frame,
};
use wallpaper_scene::{
    Color, Content,
    transaction::{Edit, HostScene, Snapshot},
};

const POLL_INTERVAL: Duration = Duration::from_millis(25);
const RESPONSE_LIMIT: Duration = Duration::from_secs(8);
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
}

impl PauseState {
    pub fn apply(&mut self, event: wallpaper_platform::session_lifecycle::Event) {
        use wallpaper_platform::session_lifecycle::Event;
        match event {
            Event::SessionLocked(locked) => self.session_locked = locked,
            Event::SystemSuspended(suspended) => self.system_suspended = suspended,
        }
    }

    pub fn paused(&self) -> bool {
        self.minimized || self.suspended || self.session_locked || self.system_suspended
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
        state.apply(Event::SystemSuspended(false));
        assert!(state.paused());
        state.minimized = false;
        assert!(state.paused());
        state.suspended = true;
        state.apply(Event::SessionLocked(false));
        assert!(state.paused());
        state.suspended = false;
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
    pub fn is_utc(&self) -> bool {
        self.state.mode == Mode::Utc
    }
    pub fn start(mode: Mode, notify: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        let (signal, receiver) = mpsc::sync_channel(2);
        let state = Arc::new(ChannelState {
            update: Mutex::new(Update::default()),
            cancel: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            channel_thread: OnceLock::new(),
            notify: Box::new(notify),
            signal,
            mode,
        });
        let shared = Arc::clone(&state);
        let supervisor = thread::Builder::new()
            .name("clock-supervisor".into())
            .spawn(move || {
                let result = supervise(&shared, receiver).map_err(|error| error.to_string());
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
        if self.state.mode == Mode::Utc
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

fn supervise(state: &Arc<ChannelState>, receiver: mpsc::Receiver<Signal>) -> WorkerResult<()> {
    let exe = std::env::current_exe()?.with_file_name("wallpaper-sandbox-probe.exe");
    if !exe.metadata()?.is_file() {
        return Err("Missing sibling sandbox probe executable".into());
    }
    let session = wallpaper_sandbox::random_id()?;
    let nonce = wallpaper_sandbox::random_id()?;
    let initial = clock::initial_scene().map_err(|error| error.to_string())?;
    let scene = HostScene::new(
        initial,
        session,
        &BTreeSet::from([clock::CLOCK_ID, clock::ACCENT_ID]),
    )
    .map_err(|_| "Could not initialize worker clock scene")?;
    let base = std::env::current_dir()?.join("target/sandbox-runs");
    fs::create_dir_all(&base)?;
    let root = base.join(format!("preview-{}-{session:016x}", std::process::id()));
    let case = match state.mode {
        Mode::Fixture => "scene-timer",
        Mode::Utc => "preview-clock-utc",
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
            let result = exchange(input, output, session, nonce, scene, &shared);
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
                break Some(Err("Clock worker timed out".into()));
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
