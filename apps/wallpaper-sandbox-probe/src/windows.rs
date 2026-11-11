use std::{
    error::Error,
    fs,
    io::{self, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
use wallpaper_protocol::{Body, Message, read_frame, write_frame};
type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
const EXPECTED: u32 = 23;
const HANDLES_ARMED: u32 = EXPECTED | 256;
const INVALID_HANDLE_EXIT: u32 = 0xc0000008;
#[derive(Debug, PartialEq, Eq)]
enum ProbeReply {
    Report(u32),
    InvalidHandleTermination,
}
pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--worker") {
        if worker(&args).is_err() {
            std::process::exit(2);
        }
        return;
    }
    if let Err(error) = host(&args) {
        eprintln!("Sandbox probe failed: {error}");
        std::process::exit(1);
    }
}
fn envelope(session: u64, sequence: u64, body: Body) -> Message {
    Message {
        session,
        sequence,
        base_revision: 0,
        body,
    }
}
fn read_response(
    output: &mut impl io::Read,
    session: u64,
    nonce: u64,
    handles_case: bool,
    ready: &mpsc::SyncSender<()>,
) -> Result<ProbeReply> {
    let hello = read_frame(output)?.ok_or("Missing Hello")?;
    hello.check_context(session, 1, 0)?;
    if hello.body != (Body::Hello { api_version: 1 }) {
        return Err("Invalid handshake".into());
    }
    ready.send(())?;
    let pong = read_frame(output)?.ok_or("Missing Pong")?;
    pong.check_context(session, 2, 0)?;
    if pong.body != (Body::Pong { nonce }) {
        return Err(format!("Invalid Pong nonce/type: {:?}", pong.body).into());
    }
    let report = read_frame(output)?.ok_or("Missing probe result")?;
    report.check_context(session, 3, 0)?;
    if handles_case {
        if report.body
            != (Body::ProbeResult {
                denied: HANDLES_ARMED,
            })
        {
            return Err("Handle probe was not armed after successful baseline checks".into());
        }
        let Some(final_report) = read_frame(output)? else {
            // Candidate only: host must still verify its own policy query and exact exit code.
            return Ok(ProbeReply::InvalidHandleTermination);
        };
        final_report.check_context(session, 4, 0)?;
        return match final_report.body {
            Body::ProbeResult { denied } => Ok(ProbeReply::Report(denied)),
            other => Err(format!("Handle probe failed: {other:?}").into()),
        };
    }
    match report.body {
        Body::ProbeResult { denied } => Ok(ProbeReply::Report(denied)),
        _ => Err("Unexpected report".into()),
    }
}
fn worker(args: &[String]) -> Result<()> {
    if !wallpaper_sandbox::is_appcontainer()? {
        return Err("Worker refuses non-AppContainer execution".into());
    }
    if args.len() != 7 {
        return Err("Invalid trusted probe arguments".into());
    }
    let session = args[1].parse()?;
    let case = &args[2];
    let marker = PathBuf::from(&args[3]);
    let hostpid = args[4].parse()?;
    let port = args[5].parse::<u16>()?;
    let excluded_handle = args[6].parse::<usize>()?;
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    write_frame(
        &mut output,
        &envelope(session, 1, Body::Hello { api_version: 1 }),
    )?;
    output.flush()?;
    let request = read_frame(&mut input)?.ok_or("Missing Ping")?;
    request.check_context(session, 1, 0)?;
    let Body::Ping { nonce } = request.body else {
        return Err("Expected Ping".into());
    };
    match case.as_str() {
        "hang" => loop {
            std::thread::sleep(Duration::from_millis(100));
        },
        "oversized" => {
            output.write_all(&u32::MAX.to_le_bytes())?;
            output.flush()?;
            std::thread::sleep(Duration::from_secs(10));
            return Ok(());
        }
        "crash" => std::process::exit(42),
        "healthy" | "network" | "network-egress" | "memory" | "process" | "handles" => {}
        _ => return Err("Unknown probe case".into()),
    }
    let read_denied = fs::read(&marker).is_err_and(|e| e.raw_os_error() == Some(5));
    let write_denied = fs::OpenOptions::new()
        .write(true)
        .open(&marker)
        .is_err_and(|e| e.raw_os_error() == Some(5));
    let process_denied = wallpaper_sandbox::host_process_access_denied(hostpid)?;
    if case == "network" || case == "network-egress" {
        // TEST-NET-1 is reserved for documentation; use no DNS or application payload.
        // A timeout or routing failure never counts as proof of sandbox denial.
        let socket = if case == "network-egress" {
            SocketAddr::from(([192, 0, 2, 1], 9))
        } else {
            SocketAddr::from(([127, 0, 0, 1], port))
        };
        let connection = TcpStream::connect_timeout(&socket, Duration::from_secs(2));
        let network_denied = connection
            .as_ref()
            .is_err_and(|e| matches!(e.raw_os_error(), Some(10013) | Some(5)));
        if !network_denied {
            write_frame(
                &mut output,
                &envelope(
                    session,
                    2,
                    Body::Reject {
                        code: 1,
                        message: format!(
                            "Network test inconclusive or allowed: {:?}",
                            connection
                                .as_ref()
                                .map(|_| ())
                                .map_err(|e| (e.kind(), e.raw_os_error()))
                        ),
                    },
                ),
            )?;
            output.flush()?;
            return Ok(());
        }
    }
    let memory_denied = if case == "memory" {
        wallpaper_sandbox::probe_memory_limit()?;
        true
    } else {
        false
    };
    if case == "process" {
        wallpaper_sandbox::probe_child_process_limit()?;
    }
    let denied = u32::from(read_denied)
        | (u32::from(write_denied) << 1)
        | (u32::from(process_denied) << 2)
        | 16
        | (u32::from(memory_denied) << 5)
        | (u32::from(case == "process") << 6);
    write_frame(&mut output, &envelope(session, 2, Body::Pong { nonce }))?;
    if case == "handles" {
        // The host must observe normal checks and an explicit armed stage before
        // classifying the strict-handle OS exception as this fixture's outcome.
        write_frame(
            &mut output,
            &envelope(
                session,
                3,
                Body::ProbeResult {
                    denied: denied | 256,
                },
            ),
        )?;
        output.flush()?;
        let body = match wallpaper_sandbox::probe_excluded_handle(excluded_handle) {
            Ok(()) => Body::ProbeResult {
                denied: denied | 128,
            },
            Err(error) => Body::Reject {
                code: 2,
                message: error.to_string().chars().take(60).collect(),
            },
        };
        write_frame(&mut output, &envelope(session, 4, body))?;
        output.flush()?;
        return Ok(());
    }
    write_frame(
        &mut output,
        &envelope(session, 3, Body::ProbeResult { denied }),
    )?;
    output.flush()?;
    Ok(())
}
fn host(args: &[String]) -> Result<()> {
    if args == ["--help"] {
        println!(
            "Usage: wallpaper-sandbox-probe [--case healthy|network|network-egress|memory|process|handles|hang|oversized|crash]\nTrusted Windows AppContainer/Job/IPC spike; no JavaScript or packages. Creates a unique test directory under target/sandbox-runs and a temporary AppContainer profile. Network tests require explicit access denial; timeout is inconclusive and fails. network uses loopback; network-egress tries TEST-NET-1 192.0.2.1:9 with no DNS or application payload. memory checks a small allocation succeeds and a 257 MiB commit fails under the 256 MiB Job limit; pages are never touched. process requires Job quota denial of a suspended child. handles requires INVALID_HANDLE for a host file omitted from the explicit inheritance list, or verified strict-policy termination after the armed IPC stage; WER is suppressed only for this intentional worker fault."
        );
        return Ok(());
    }
    let case = match args {
        [] => "healthy",
        [flag, value] if flag == "--case" => value.as_str(),
        _ => return Err("Use --help for probe arguments".into()),
    };
    if ![
        "healthy",
        "network",
        "network-egress",
        "memory",
        "process",
        "handles",
        "hang",
        "oversized",
        "crash",
    ]
    .contains(&case)
    {
        return Err("Unknown case".into());
    }
    let session = wallpaper_sandbox::random_id()?;
    let nonce = wallpaper_sandbox::random_id()?;
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
    let ready = ready_receive.recv_timeout(Duration::from_secs(3));
    let result = receive.recv_timeout(Duration::from_secs(3));
    let outcome = (|| -> Result<()> {
        ready.map_err(|_| "Worker did not complete handshake")?;
        match (case, result) {
            (
                "healthy" | "network" | "network-egress" | "memory" | "process" | "handles",
                Ok(Ok(ProbeReply::Report(bits))),
            ) if bits
                == (EXPECTED
                    | match case {
                        "memory" => 32,
                        "process" => 64,
                        "handles" => 128,
                        _ => 0,
                    }) =>
            {
                println!(
                    "Direct OS denials confirmed: sentinel read, sentinel write, host process VM access. Probe bits={bits:#x}"
                );
                if case == "network" || case == "network-egress" {
                    println!("Explicit TCP access denial confirmed for {case}.");
                }
                if case == "memory" {
                    println!(
                        "Job limit verified: 64 KiB commit succeeded; 257 MiB commit rejected; worker remained responsive."
                    );
                }
                if case == "process" {
                    println!(
                        "Worker Job limit verified: suspended child creation rejected with ERROR_NOT_ENOUGH_QUOTA (1816)."
                    );
                }
                if case == "handles" {
                    println!(
                        "Explicit handle list verified: inheritable host file absent from worker (ERROR_INVALID_HANDLE, 6)."
                    );
                }
                if sandbox.wait(1000)? != Some(0) {
                    return Err("Healthy worker did not exit cleanly".into());
                }
            }
            ("hang", Err(mpsc::RecvTimeoutError::Timeout)) => {
                let detection = Instant::now();
                sandbox.stop()?;
                println!(
                    "Watchdog terminated native stall in {:?} after detection",
                    detection.elapsed()
                );
            }
            ("handles", Ok(Ok(ProbeReply::InvalidHandleTermination)))
                if sandbox.strict_handle_checks()
                    && sandbox.wait(1000)? == Some(INVALID_HANDLE_EXIT) =>
            {
                println!(
                    "Explicit handle list verified: armed invalid-handle reference terminated only the strict-policy worker (0xc0000008)."
                );
            }
            ("oversized", Ok(Err(error))) => {
                if !matches!(
                    error.downcast_ref::<wallpaper_protocol::Error>(),
                    Some(wallpaper_protocol::Error::Invalid(
                        "frame length outside bounds"
                    ))
                ) {
                    return Err(error);
                }
                sandbox.stop()?;
                println!(
                    "Oversized worker frame rejected before payload allocation; worker stopped."
                );
            }
            ("crash", Ok(Err(_))) => {
                if sandbox.wait(1000)? != Some(42) {
                    return Err("Crash probe exit was not 42".into());
                }
                println!("Worker crash contained; host survived and recorded exit 42.");
            }
            (_, other) => {
                return Err(format!(
                    "Unexpected probe outcome: {other:?}; worker exit={:?}",
                    sandbox.wait(100)?
                )
                .into());
            }
        }
        if fs::read(root.join("denied.txt"))? != b"Sahne sandbox sentinel" {
            return Err("Test sentinel changed".into());
        }
        Ok(())
    })();
    // Kill even on errors before joining a potentially blocked pipe reader.
    let stopped = sandbox.stop();
    reader.join().map_err(|_| "Reader thread panicked")?;
    stopped?;
    outcome?;
    println!(
        "Probe passed; test artifacts retained at {}",
        root.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prefix() -> Vec<Message> {
        vec![
            envelope(42, 1, Body::Hello { api_version: 1 }),
            envelope(42, 2, Body::Pong { nonce: 77 }),
        ]
    }
    fn bytes(messages: &[Message]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for message in messages {
            write_frame(&mut bytes, message).unwrap();
        }
        bytes
    }
    fn parse(bytes: Vec<u8>) -> Result<ProbeReply> {
        let (ready, _receive) = mpsc::sync_channel(1);
        read_response(&mut io::Cursor::new(bytes), 42, 77, true, &ready)
    }
    #[test]
    fn eof_before_armed_stage_is_never_an_expected_handle_fault() {
        let messages = prefix();
        assert!(parse(bytes(&messages)).is_err());
        for mask in [EXPECTED, EXPECTED | 128, HANDLES_ARMED | 1 << 15] {
            let mut messages = prefix();
            messages.push(envelope(42, 3, Body::ProbeResult { denied: mask }));
            assert!(parse(bytes(&messages)).is_err());
        }
    }
    #[test]
    fn only_clean_eof_after_arming_becomes_a_termination_candidate() {
        let mut messages = prefix();
        messages.push(envelope(
            42,
            3,
            Body::ProbeResult {
                denied: HANDLES_ARMED,
            },
        ));
        assert_eq!(
            parse(bytes(&messages)).unwrap(),
            ProbeReply::InvalidHandleTermination
        );
        let mut truncated = bytes(&messages);
        truncated.push(1);
        assert!(parse(truncated).is_err());
        let mut rejected = messages.clone();
        rejected.push(envelope(
            42,
            4,
            Body::Reject {
                code: 2,
                message: "collision".into(),
            },
        ));
        assert!(parse(bytes(&rejected)).is_err());
        messages.push(envelope(
            42,
            4,
            Body::ProbeResult {
                denied: EXPECTED | 128,
            },
        ));
        assert_eq!(
            parse(bytes(&messages)).unwrap(),
            ProbeReply::Report(EXPECTED | 128)
        );
    }
    #[test]
    fn arming_requires_matching_session_sequence_revision_and_nonce() {
        for (session, sequence, revision) in [(43, 3, 0), (42, 4, 0), (42, 3, 1)] {
            let mut messages = prefix();
            let mut stage = envelope(
                session,
                sequence,
                Body::ProbeResult {
                    denied: HANDLES_ARMED,
                },
            );
            stage.base_revision = revision;
            messages.push(stage);
            assert!(parse(bytes(&messages)).is_err());
        }
        let mut messages = prefix();
        messages[1].body = Body::Pong { nonce: 78 };
        messages.push(envelope(
            42,
            3,
            Body::ProbeResult {
                denied: HANDLES_ARMED,
            },
        ));
        assert!(parse(bytes(&messages)).is_err());
    }
}
