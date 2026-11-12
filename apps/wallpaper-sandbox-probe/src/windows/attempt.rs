//! One owned launch/channel lifetime, shared by individual fixtures and restart tests.
use super::{Body, ProbeReply, Result, envelope, read_response, write_frame};
use std::{
    fs,
    io::{self, Write},
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

type Response = std::result::Result<Result<ProbeReply>, mpsc::RecvTimeoutError>;
pub(super) const PRESSURE_CHUNKS: usize = 256;
pub(super) const PRESSURE_CHUNK_BYTES: usize = 4096;
pub(super) const PRESSURE_BYTES: usize = PRESSURE_CHUNKS * PRESSURE_CHUNK_BYTES;
pub(super) struct ProbeAttempt {
    pub session: u64,
    pub nonce: u64,
    pub root: PathBuf,
    pub sandbox: wallpaper_sandbox::Sandbox,
    pub handshake: bool,
    pub cpu_armed: bool,
    pub response: Option<Response>,
    pub channel_wait: Duration,
    observed_at: Instant,
    reader: Option<JoinHandle<()>>,
    writer: Option<JoinHandle<io::Result<()>>>,
    pressure_stall: bool,
    writing_pressure: Arc<AtomicBool>,
    written: Arc<AtomicUsize>,
    _listener: TcpListener,
}
impl ProbeAttempt {
    pub fn start(case: &str, previous_session: Option<u64>) -> Result<Self> {
        let session = wallpaper_sandbox::random_id()?;
        let nonce = wallpaper_sandbox::random_id()?;
        if previous_session == Some(session) {
            return Err("Session identity collision".into());
        }
        let base = std::env::current_dir()?.join("target/sandbox-runs");
        fs::create_dir_all(&base)?;
        let root = base.join(format!("{}-{session:016x}", std::process::id()));
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
        let wallpaper_sandbox::Channel {
            sandbox,
            mut input,
            mut output,
        } = wallpaper_sandbox::spawn_probe(
            &std::env::current_exe()?,
            &root,
            session,
            case,
            listener.local_addr()?.port(),
            previous_session,
        )?;
        println!(
            "Verified AppContainer identity and zero capabilities; Job memory=256 MiB, active processes=1. Case={case}"
        );
        if case == "handles" {
            println!(
                "Host-queried strict invalid-handle policy: {}",
                sandbox.strict_handle_checks()
            );
        }
        let (send, receive) = mpsc::sync_channel(1);
        let (ready_send, ready_receive) = mpsc::sync_channel(1);
        let handles_case = case == "handles";
        // Install ownership before starting either I/O thread. A later spawn failure
        // drops this owner, which stops the worker before joining an existing thread.
        let mut attempt = Self {
            session,
            nonce,
            root,
            sandbox,
            handshake: false,
            cpu_armed: false,
            response: None,
            channel_wait: Duration::ZERO,
            observed_at: Instant::now(),
            reader: None,
            writer: None,
            pressure_stall: case == "ipc-write-stall",
            writing_pressure: Arc::new(AtomicBool::new(false)),
            written: Arc::new(AtomicUsize::new(0)),
            _listener: listener,
        };
        let pressure = matches!(case, "ipc-write-stall" | "ipc-write-drain");
        let writing = Arc::clone(&attempt.writing_pressure);
        let written = Arc::clone(&attempt.written);
        attempt.writer = Some(
            std::thread::Builder::new()
                .name("probe-writer".into())
                .spawn(move || {
                    write_frame(&mut input, &envelope(session, 1, Body::Ping { nonce })).map_err(
                        |error| match error {
                            wallpaper_protocol::Error::Io(error) => error,
                            other => io::Error::other(other),
                        },
                    )?;
                    if pressure {
                        let block = [0x5a; PRESSURE_CHUNK_BYTES];
                        writing.store(true, Ordering::SeqCst);
                        for _ in 0..PRESSURE_CHUNKS {
                            input.write_all(&block)?;
                            written.fetch_add(block.len(), Ordering::SeqCst);
                        }
                    }
                    input.flush()?;
                    Ok(()) // File drops here, making EOF observable to a draining worker.
                })?,
        );
        attempt.reader = Some(
            std::thread::Builder::new()
                .name("probe-reader".into())
                .spawn(move || {
                    let response =
                        read_response(&mut output, session, nonce, handles_case, &ready_send);
                    let _ = send.send(response);
                })?,
        );
        let waiting_at = Instant::now();
        let handshake = ready_receive.recv_timeout(Duration::from_secs(3)).is_ok();
        let response = if handshake {
            receive.recv_timeout(Duration::from_secs(3))
        } else {
            // A failed handshake must not acquire an additional response deadline.
            match receive.try_recv() {
                Ok(response) => Ok(response),
                Err(mpsc::TryRecvError::Empty) => Err(mpsc::RecvTimeoutError::Timeout),
                Err(mpsc::TryRecvError::Disconnected) => Err(mpsc::RecvTimeoutError::Disconnected),
            }
        };
        let observed_at = Instant::now();
        let channel_wait = observed_at.duration_since(waiting_at);
        attempt.handshake = handshake;
        attempt.cpu_armed = case == "cpu-busy" && ready_receive.try_recv().is_ok();
        attempt.response = Some(response);
        attempt.channel_wait = channel_wait;
        attempt.observed_at = observed_at;
        Ok(attempt)
    }
    pub fn write_pressure_pending(&self) -> bool {
        self.pressure_stall
            && self.writing_pressure.load(Ordering::SeqCst)
            && self.written.load(Ordering::SeqCst) < PRESSURE_BYTES
            && self
                .writer
                .as_ref()
                .is_some_and(|writer| !writer.is_finished())
    }
    /// Kill/wait and join before any replacement may be launched. Drop the owner afterward
    /// to release its remaining handles/profile; cleanup errors forbid retry.
    pub fn finish(&mut self) -> Result<Duration> {
        let detection = Instant::now();
        self.sandbox.stop()?;
        if let Some(reader) = self.reader.take() {
            reader.join().map_err(|_| "Reader thread panicked")?;
        }
        if let Some(writer) = self.writer.take() {
            let result = writer.join().map_err(|_| "Writer thread panicked")?;
            if self.pressure_stall {
                match result {
                    Err(error) if matches!(error.raw_os_error(), Some(109 | 232)) => {
                        println!(
                            "Blocked host writer released by worker stop: pipe closed ({}); completed pressure bytes={}/{}.",
                            error.raw_os_error().unwrap(),
                            self.written.load(Ordering::SeqCst),
                            PRESSURE_BYTES
                        );
                    }
                    other => {
                        return Err(format!("Unexpected pressure writer result: {other:?}").into());
                    }
                }
            } else {
                result?;
                if self.writing_pressure.load(Ordering::SeqCst) {
                    if self.written.load(Ordering::SeqCst) != PRESSURE_BYTES {
                        return Err("Draining fixture host write budget incomplete".into());
                    }
                    println!(
                        "Host pressure writer completed all {PRESSURE_BYTES} bytes and joined."
                    );
                }
            }
        }
        let elapsed = detection.elapsed();
        if self.observed_at.elapsed() > Duration::from_secs(1) {
            return Err(
                "Response classification and Job stop/I/O thread joins exceeded one second".into(),
            );
        }
        let profile_cleanup = Instant::now();
        self.sandbox.cleanup_profile()?;
        println!(
            "Profile cleanup verified: storage existed before launch and is absent after OS deletion; elapsed={:?}",
            profile_cleanup.elapsed()
        );
        Ok(elapsed)
    }
}
impl Drop for ProbeAttempt {
    fn drop(&mut self) {
        if (self.reader.is_some() || self.writer.is_some())
            && let Err(error) = self.finish()
        {
            eprintln!("Attempt cleanup failed: {error}");
        }
    }
}
