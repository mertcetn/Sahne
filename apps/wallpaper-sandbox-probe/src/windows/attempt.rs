//! One owned launch/channel lifetime, shared by individual fixtures and restart tests.
use super::{Body, ProbeReply, Result, envelope, read_response, write_frame};
use std::{
    fs,
    io::Write,
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    sync::mpsc,
    thread::JoinHandle,
    time::{Duration, Instant},
};

type Response = std::result::Result<Result<ProbeReply>, mpsc::RecvTimeoutError>;
pub(super) struct ProbeAttempt {
    pub session: u64,
    pub nonce: u64,
    pub root: PathBuf,
    pub sandbox: wallpaper_sandbox::Sandbox,
    pub handshake: bool,
    pub response: Option<Response>,
    pub channel_wait: Duration,
    observed_at: Instant,
    reader: Option<JoinHandle<()>>,
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
        write_frame(&mut input, &envelope(session, 1, Body::Ping { nonce }))?;
        input.flush()?;
        drop(input);
        let (send, receive) = mpsc::sync_channel(1);
        let (ready_send, ready_receive) = mpsc::sync_channel(1);
        let handles_case = case == "handles";
        let reader = std::thread::spawn(move || {
            let response = read_response(&mut output, session, nonce, handles_case, &ready_send);
            let _ = send.send(response);
        });
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
        Ok(Self {
            session,
            nonce,
            root,
            sandbox,
            handshake,
            response: Some(response),
            channel_wait,
            observed_at,
            reader: Some(reader),
            _listener: listener,
        })
    }
    /// Kill/wait and join before any replacement may be launched. Drop the owner afterward
    /// to release its remaining handles/profile; cleanup errors forbid retry.
    pub fn finish(&mut self) -> Result<Duration> {
        let detection = Instant::now();
        self.sandbox.stop()?;
        if let Some(reader) = self.reader.take() {
            reader.join().map_err(|_| "Reader thread panicked")?;
        }
        let elapsed = detection.elapsed();
        if self.observed_at.elapsed() > Duration::from_secs(1) {
            return Err(
                "Response classification and Job stop/reader join exceeded one second".into(),
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
        if self.reader.is_some()
            && let Err(error) = self.finish()
        {
            eprintln!("Attempt cleanup failed: {error}");
        }
    }
}
