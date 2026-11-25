//! Bounded development bridge to the existing AppContainer clock fixture.
use super::*;
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Write},
    sync::{
        Mutex,
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

pub const POLL_INTERVAL: Duration = Duration::from_millis(25);
const RESPONSE_LIMIT: Duration = Duration::from_secs(8);
const TICK_GAP: Duration = Duration::from_millis(1400);
const MINUTES: [u16; 3] = [1439, 0, 0];
type WorkerResult<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Default)]
pub struct Update {
    pub snapshot: Option<Snapshot>,
    pub terminal: Option<std::result::Result<(), String>>,
}

pub struct Bridge {
    update: Arc<Mutex<Update>>,
    cancel: Arc<AtomicBool>,
    supervisor: Option<JoinHandle<()>>,
}

impl Bridge {
    pub fn start() -> Result<Self> {
        let update = Arc::new(Mutex::new(Update::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        let shared = Arc::clone(&update);
        let stopped = Arc::clone(&cancel);
        let supervisor = thread::Builder::new()
            .name("clock-supervisor".into())
            .spawn(move || {
                let result = supervise(&shared, &stopped).map_err(|error| error.to_string());
                if let Ok(mut pending) = shared.lock() {
                    pending.terminal = Some(result);
                }
            })?;
        Ok(Self {
            update,
            cancel,
            supervisor: Some(supervisor),
        })
    }

    pub fn poll(&self) -> Result<Update> {
        Ok(std::mem::take(
            &mut *self
                .update
                .lock()
                .map_err(|_| "Clock mailbox unavailable")?,
        ))
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(supervisor) = self.supervisor.take() {
            let _ = supervisor.join();
        }
    }
}

fn supervise(update: &Arc<Mutex<Update>>, cancel: &Arc<AtomicBool>) -> WorkerResult<()> {
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
    let wallpaper_sandbox::Channel {
        mut sandbox,
        input,
        output,
    } = wallpaper_sandbox::spawn_probe(&exe, &root, session, "scene-timer", 0, None)?;
    let shared = Arc::clone(update);
    let stopped = Arc::clone(cancel);
    let (sender, receiver) = mpsc::sync_channel(1);
    let io_thread = thread::Builder::new()
        .name("clock-channel".into())
        .spawn(move || {
            let result = exchange(input, output, session, nonce, scene, &shared, &stopped);
            let _ = sender.send(result);
        });
    let io_thread = match io_thread {
        Ok(thread) => thread,
        Err(error) => {
            sandbox.cleanup_profile()?;
            return Err(error.into());
        }
    };
    let start = Instant::now();
    let response = loop {
        if cancel.load(Ordering::Acquire) || start.elapsed() >= RESPONSE_LIMIT {
            break Err("Clock worker stopped or timed out".into());
        }
        match receiver.recv_timeout(POLL_INTERVAL) {
            Ok(result) => break result,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Err("Clock worker channel disappeared".into());
            }
        }
    };
    let exit = if response.is_ok() {
        sandbox.wait(1000)
    } else {
        Ok(None)
    };
    sandbox.stop()?;
    let joined = io_thread
        .join()
        .map_err(|_| "Clock channel thread panicked");
    sandbox.cleanup_profile()?;
    joined?;
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
    update: &Mutex<Update>,
    cancel: &AtomicBool,
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
    for (index, minute) in MINUTES.into_iter().enumerate() {
        let base = index as u64;
        let sequence = base + 1;
        if index != 0 {
            if read_frame(&mut output)? != Some(frame(session, sequence, base, Body::ClockReady)) {
                return Err("Invalid clock worker readiness".into());
            }
            let start = Instant::now();
            while start.elapsed() < TICK_GAP {
                if cancel.load(Ordering::Acquire) {
                    return Err("Clock update cancelled".into());
                }
                thread::sleep(POLL_INTERVAL);
            }
        }
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
            update
                .lock()
                .map_err(|_| "Clock mailbox unavailable")?
                .snapshot = Some(snapshot);
        }
        send(&mut input, ack)?;
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

pub fn check() -> Result<()> {
    let mut current = clock::Clock::live_fallback(Instant::now())?;
    let bridge = Bridge::start()?;
    let mut seen = Vec::new();
    let start = Instant::now();
    while start.elapsed() < RESPONSE_LIMIT + Duration::from_secs(3) {
        let update = bridge.poll()?;
        if let Some(snapshot) = update.snapshot {
            current.accept_worker(snapshot);
            seen.push(current.text().to_owned());
        }
        if let Some(result) = update.terminal {
            result?;
            if seen != ["23:59", "00:00"] || current.text() != "00:00" {
                return Err("Clock publication sequence mismatch".into());
            }
            println!(
                "Live worker clock verified: 23:59 -> 00:00 UTC; terminal rejection preserved 00:00."
            );
            return Ok(());
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err("Live worker clock check timed out".into())
}
