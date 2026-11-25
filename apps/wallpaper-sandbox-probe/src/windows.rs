use std::{
    error::Error,
    fs,
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
use wallpaper_protocol::{Body, Message, read_frame, write_frame};
mod attempt;
mod package;
mod retry;
mod roundtrip;
mod scene_ipc;
mod script;
use attempt::ProbeAttempt;
type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
const EXPECTED: u32 = 23;
const HANDLES_ARMED: u32 = EXPECTED | 256;
const INVALID_HANDLE_EXIT: u32 = 0xc0000008;
#[derive(Debug, PartialEq, Eq)]
enum ProbeReply {
    Report(u32),
    InvalidHandleTermination,
}
#[derive(Debug)]
struct TrailingData;
impl std::fmt::Display for TrailingData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Unsolicited data after the final probe result")
    }
}
impl Error for TrailingData {}
#[derive(Debug)]
struct MissingFrame(&'static str);
impl std::fmt::Display for MissingFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Missing {}", self.0)
    }
}
impl Error for MissingFrame {}

fn require_eof(output: &mut impl io::Read) -> Result<()> {
    let mut byte = [0];
    loop {
        match output.read(&mut byte) {
            Ok(0) => return Ok(()),
            Ok(_) => return Err(TrailingData.into()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
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
    let hello = read_frame(output)?.ok_or(MissingFrame("Hello"))?;
    hello.check_context(session, 1, 0)?;
    if hello.body != (Body::Hello { api_version: 1 }) {
        return Err("Invalid handshake".into());
    }
    ready.send(())?;
    let pong = read_frame(output)?.ok_or(MissingFrame("Pong"))?;
    pong.check_context(session, 2, 0)?;
    if pong.body != (Body::Pong { nonce }) {
        return Err(format!("Invalid Pong nonce/type: {:?}", pong.body).into());
    }
    let report = read_frame(output)?.ok_or(MissingFrame("probe result"))?;
    report.check_context(session, 3, 0)?;
    if report.body
        == (Body::ProbeResult {
            denied: EXPECTED | 16384,
        })
    {
        ready.send(())?;
    }
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
        let reply = match final_report.body {
            Body::ProbeResult { denied } => ProbeReply::Report(denied),
            other => return Err(format!("Handle probe failed: {other:?}").into()),
        };
        require_eof(output)?;
        return Ok(reply);
    }
    let reply = match report.body {
        Body::ProbeResult { denied } => ProbeReply::Report(denied),
        _ => return Err("Unexpected report".into()),
    };
    require_eof(output)?;
    Ok(reply)
}
fn worker(args: &[String]) -> Result<()> {
    if !wallpaper_sandbox::is_appcontainer()? {
        return Err("Worker refuses non-AppContainer execution".into());
    }
    if args.len() != 8 {
        return Err("Invalid trusted probe arguments".into());
    }
    let session = args[1].parse()?;
    let case = &args[2];
    let marker = PathBuf::from(&args[3]);
    let hostpid = args[4].parse()?;
    let port = args[5].parse::<u16>()?;
    let excluded_handle = args[6].parse::<usize>()?;
    let previous_session = args[7].parse::<u64>()?;
    if case == "no-hello" {
        loop {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    write_frame(
        &mut output,
        &envelope(session, 1, Body::Hello { api_version: 1 }),
    )?;
    output.flush()?;
    if case == "ipc-write-stall" {
        // Send Hello but never consume input. The host writer must fill the pipe
        // while its independent control thread remains able to time out/kill us.
        loop {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let request = read_frame(&mut input)?.ok_or("Missing Ping")?;
    request.check_context(session, 1, 0)?;
    let Body::Ping { nonce } = request.body else {
        return Err("Expected Ping".into());
    };
    if case == "ipc-write-drain" {
        let mut block = [0; attempt::PRESSURE_CHUNK_BYTES];
        for _ in 0..attempt::PRESSURE_CHUNKS {
            input.read_exact(&mut block)?;
            if block.iter().any(|&byte| byte != 0x5a) {
                return Err("Pressure fixture contents mismatch".into());
            }
        }
        require_eof(&mut input)?;
    }
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
        "ipc-stall" | "ipc-truncated" => {
            // Declare a Pong-sized payload, emit one byte, then stop mid-body.
            output.write_all(&35u32.to_le_bytes())?;
            output.write_all(&[1])?;
            output.flush()?;
            if case == "ipc-truncated" {
                return Ok(());
            }
            loop {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        "ipc-replay" => {
            write_frame(&mut output, &envelope(session, 1, Body::Pong { nonce }))?;
            output.flush()?;
            loop {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        "ipc-old-session" => {
            if previous_session == 0 || previous_session == session {
                return Err("Invalid old-session fixture".into());
            }
            write_frame(
                &mut output,
                &envelope(previous_session, 2, Body::Pong { nonce }),
            )?;
            output.flush()?;
            loop {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        "healthy"
        | "package-basic"
        | "package-invalid"
        | "compiler-basic"
        | "compiler-invalid"
        | "image-basic"
        | "image-invalid"
        | "hierarchy-basic"
        | "hierarchy-invalid"
        | "groups-basic"
        | "groups-invalid"
        | "network"
        | "network-egress"
        | "memory"
        | "memory-incremental"
        | "cpu-busy"
        | "process"
        | "handles"
        | "endpoint"
        | "ipc-rights"
        | "ipc-roundtrip"
        | "ipc-roundtrip-replay"
        | "ipc-roundtrip-stall"
        | "scene-transaction"
        | "scene-script"
        | "scene-timer"
        | "ipc-write-drain"
        | "filesystem"
        | "registry"
        | "ipc-flood"
        | "ipc-open"
        | "js-basic"
        | "js-interrupt"
        | "js-memory"
        | "js-stack"
        | "js-jobs" => {}
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
    if case == "memory-incremental" {
        wallpaper_sandbox::probe_incremental_memory()?;
    }
    if case == "process" {
        wallpaper_sandbox::probe_child_process_limit()?;
    }
    if case == "endpoint"
        && let Err(error) = wallpaper_sandbox::probe_host_endpoint(hostpid, session)
    {
        write_frame(
            &mut output,
            &envelope(
                session,
                2,
                Body::Reject {
                    code: 3,
                    message: error.to_string().chars().take(60).collect(),
                },
            ),
        )?;
        output.flush()?;
        return Ok(());
    }
    if case == "ipc-rights"
        && let Err(error) = wallpaper_sandbox::probe_channel_rights(&input, &output)
    {
        write_frame(
            &mut output,
            &envelope(
                session,
                2,
                Body::Reject {
                    code: 4,
                    message: error.to_string().chars().take(60).collect(),
                },
            ),
        )?;
        output.flush()?;
        return Ok(());
    }
    if case == "filesystem"
        && let Err(error) =
            wallpaper_sandbox::probe_filesystem(marker.parent().ok_or("Missing fixture root")?)
    {
        write_frame(
            &mut output,
            &envelope(
                session,
                2,
                Body::Reject {
                    code: 5,
                    message: error.to_string().chars().take(60).collect(),
                },
            ),
        )?;
        output.flush()?;
        return Ok(());
    }
    if case == "cpu-busy" {
        wallpaper_sandbox::probe_cpu_budget()?;
    }
    if case == "registry"
        && let Err(error) = wallpaper_sandbox::probe_registry(hostpid, session)
    {
        write_frame(
            &mut output,
            &envelope(
                session,
                2,
                Body::Reject {
                    code: 6,
                    message: error.to_string().chars().take(60).collect(),
                },
            ),
        )?;
        output.flush()?;
        return Ok(());
    }
    let package_result = if case.starts_with("package-") {
        package::run(case)?;
        package::RESULT_BIT
    } else {
        0
    };
    let compiler_result = if case.starts_with("compiler-") {
        package::run_compiler(case)?;
        package::COMPILER_RESULT_BIT
    } else {
        0
    };
    let script_result = if case.starts_with("js-") {
        script::run(case)?;
        script::RESULT_BIT
    } else {
        0
    };
    let image_result = if case.starts_with("image-") {
        package::run_image(case)?;
        package::IMAGE_RESULT_BIT
    } else {
        0
    };
    let hierarchy_result = if case.starts_with("hierarchy-") {
        package::run_hierarchy(case)?;
        package::HIERARCHY_RESULT_BIT
    } else {
        0
    };
    let group_compiler_result = if case.starts_with("groups-") {
        package::run_group_compiler(case)?;
        package::GROUP_COMPILER_RESULT_BIT
    } else {
        0
    };
    let denied = u32::from(read_denied)
        | (u32::from(write_denied) << 1)
        | (u32::from(process_denied) << 2)
        | 16
        | (u32::from(memory_denied) << 5)
        | image_result
        | hierarchy_result
        | group_compiler_result
        | (u32::from(case == "process") << 6)
        | script_result
        | package_result
        | compiler_result
        | (u32::from(case == "endpoint") << 10)
        | (u32::from(case == "ipc-rights") << 11)
        | (u32::from(case == "filesystem") << 12)
        | (u32::from(case == "ipc-write-drain") << 13)
        | (u32::from(case == "cpu-busy") << 14)
        | (u32::from(case == "registry") << 15)
        | (u32::from(case == "memory-incremental") << 16);
    if case.starts_with("ipc-roundtrip") {
        return roundtrip::worker(&mut input, &mut output, session, nonce, case, denied);
    }
    if let Some(mode) = scene_ipc::Mode::for_case(case) {
        return scene_ipc::worker(&mut input, &mut output, session, nonce, denied, mode);
    }
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
    if case == "cpu-busy" {
        let mut value = 1u64;
        loop {
            value = std::hint::black_box(value.wrapping_mul(6364136223846793005).wrapping_add(1));
        }
    }
    if case == "ipc-flood" {
        // This one-shot channel permits no messages after the final report.
        // Keep trying small frames until the host rejects/closes/terminates us.
        loop {
            write_frame(&mut output, &envelope(session, 4, Body::Ping { nonce }))?;
            output.flush()?;
        }
    }
    if case == "ipc-open" {
        loop {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}
fn host(args: &[String]) -> Result<()> {
    if args == ["--help"] {
        println!(
            "Usage: wallpaper-sandbox-probe [--case CASE]\nCases: healthy, network, network-egress, memory, process, handles, hang, oversized, crash, ipc-flood, ipc-stall, ipc-truncated, ipc-replay, ipc-open.\nTrusted Windows AppContainer/Job/IPC spike; fixed native/JS fixtures only, no packages or script paths. Creates a unique test directory under target/sandbox-runs and a temporary AppContainer profile.\nNetwork tests require explicit access denial; timeout is inconclusive and fails. network uses loopback; network-egress tries TEST-NET-1 192.0.2.1:9 without DNS or application payload.\nmemory checks a small allocation succeeds and an over-limit commit fails. process requires Job quota denial of a suspended child. handles checks an excluded host handle, including verified strict-policy termination after arming; WER is suppressed only for that intentional fault.\nIPC cases exercise unsolicited trailing-message flood, stalled partial body, truncated body at EOF, replayed sequence, and an open stream after a valid report. Every successful one-shot response requires EOF; a peer that keeps the stream open is stopped by the host deadline."
        );
        println!(
            "JS cases: js-basic, js-interrupt, js-memory, js-stack, js-jobs. Fixed trusted source only, executed after the worker's AppContainer check. Heap=16 MiB, stack=256 KiB, cooperative deadline=100 ms, Promise job budget=64; the independent host watchdog remains enabled. No script path or external bytecode is accepted."
        );
        println!(
            "Restart cases: restart-recover, restart-exhaust, restart-timeout, restart-handshake, restart-stale, restart-protocol. At most three attempts per run, 100 ms backoff, fresh identity/channel each time. Only crashes/timeouts are retry candidates; protocol or launch failures stop without retry. These are fixed supervisor fixtures, not automatic product recovery."
        );
        println!(
            "Endpoint case: endpoint. Host positive control opens the same uniquely named local pipe; an available instance remains. AppContainer read/write/duplex/zero-access opens must all return access denied. After owner destruction, the name must be absent. No endpoint handle or permission is granted to the worker."
        );
        println!(
            "Channel case: ipc-rights. Actual inherited streams must reject DACL reads, DACL changes and wrong-direction data I/O with access denied, while the normal protocol succeeds. All cases verify protected host-user-only pipe DACLs and use data-only inherited handles."
        );
        println!(
            "Filesystem case: filesystem. Synthetic private/staged targets have host positive controls. Worker file-rights, private listing and file/directory creation requests must be denied. Staged data/executable and System32/kernel32.dll reads plus staging listing must succeed. No user files are inspected; post-stop contents/namespace are checked."
        );
        println!(
            "Host-write cases: ipc-write-drain and ipc-write-stall. A dedicated writer sends one Ping plus a fixed 1 MiB pressure fixture. Drain validates every byte and EOF; stall must keep the writer pending until the host deadline kills the worker and both I/O threads join. This is a transport-only fixture, not package/source input."
        );
        println!(
            "CPU case: cpu-busy. Probe Jobs have a verified 10% CPU hard cap; this fixture uses a stricter 1%. The armed native loop must consume measurable bounded CPU while remaining alive until the unchanged watchdog stops it. This is a scheduling-rate experiment, not a production policy certification."
        );
        println!(
            "Registry case: registry. A fresh volatile user-only synthetic key has host positive controls. Eight worker access requests plus direct create/delete must return access denied. After worker stop, its fixed sentinel/namespace must be unchanged and the owned key must be absent after cleanup. No existing user registry values are read or changed."
        );
        println!(
            "Incremental memory case: memory-incremental. Commit fixed 16 MiB regions up to the unchanged 256 MiB Job cap, require a memory denial after at least 192 MiB, release all regions, then allocate/release 32 MiB and exit normally. Host independently verifies Job peak committed memory and single-worker exit. No physical pages are touched."
        );
        println!(
            "Roundtrip cases: ipc-roundtrip, ipc-roundtrip-replay, ipc-roundtrip-stall. Up to 128 sequential Ping/Pong exchanges with refreshed host nonces and one outstanding request. Replay/stall follows eight validated replies. Progress does not reset the cumulative host response deadline. Fixed transport fixtures only."
        );
        println!(
            "Package cases: package-basic and package-invalid. Original fixed in-memory manifest/source entries are validated inside AppContainer. Invalid permissions, version, traversal, FPS and case collision must be rejected. No external path, archive extraction or JavaScript execution."
        );
        println!(
            "Compiler cases: compiler-basic and compiler-invalid. Image cases: image-basic and image-invalid. Original embedded HTML/CSS/PNG is compiled and its scene/assets revalidated inside AppContainer; malformed/unsupported source must fail with bounded diagnostics. No external path or script execution."
        );
        println!(
            "Hierarchy cases: hierarchy-basic and hierarchy-invalid. Original version-2 group data and derived transform/clip/command order are checked inside AppContainer; no source input or group rendering."
        );
        println!(
            "Group compiler cases: groups-basic and groups-invalid. Original HTML/CSS/PNG sources compile to version-2 groups with independent scene/asset checks; no group rendering or script execution."
        );
        return Ok(());
    }
    let case = match args {
        [] => "healthy",
        [flag, value] if flag == "--case" => value.as_str(),
        _ => return Err("Use --help for probe arguments".into()),
    };
    if [
        "restart-recover",
        "restart-exhaust",
        "restart-timeout",
        "restart-handshake",
        "restart-stale",
        "restart-protocol",
    ]
    .contains(&case)
    {
        return retry::run(case);
    }
    if ![
        "healthy",
        "package-basic",
        "package-invalid",
        "compiler-basic",
        "compiler-invalid",
        "image-basic",
        "image-invalid",
        "hierarchy-basic",
        "hierarchy-invalid",
        "groups-basic",
        "groups-invalid",
        "network",
        "network-egress",
        "memory",
        "memory-incremental",
        "cpu-busy",
        "process",
        "handles",
        "endpoint",
        "ipc-rights",
        "ipc-roundtrip",
        "ipc-roundtrip-replay",
        "ipc-roundtrip-stall",
        "scene-transaction",
        "scene-script",
        "scene-timer",
        "ipc-write-stall",
        "ipc-write-drain",
        "filesystem",
        "registry",
        "hang",
        "oversized",
        "crash",
        "ipc-flood",
        "ipc-stall",
        "ipc-truncated",
        "ipc-replay",
        "ipc-open",
        "js-basic",
        "js-interrupt",
        "js-memory",
        "js-stack",
        "js-jobs",
    ]
    .contains(&case)
    {
        return Err("Unknown case".into());
    }
    let mut attempt = ProbeAttempt::start(case, None)?;
    let root = attempt.root.clone();
    let sandbox = &attempt.sandbox;
    let result = attempt.response.take().ok_or("Missing attempt result")?;
    let outcome = (|| -> Result<()> {
        if !attempt.handshake {
            return Err("Worker did not complete handshake".into());
        }
        match (case, result) {
            ("cpu-busy", Err(mpsc::RecvTimeoutError::Timeout)) if attempt.cpu_armed => {
                sandbox.verify_cpu_rate()?;
                let detection = Instant::now();
                sandbox.stop()?;
                println!(
                    "Watchdog terminated throttled native loop in {:?} after detection; armed report=0x4017.",
                    detection.elapsed()
                );
            }
            (
                "healthy" | "package-basic" | "package-invalid" | "compiler-basic"
                | "compiler-invalid" | "image-basic" | "image-invalid" | "hierarchy-basic"
                | "hierarchy-invalid" | "groups-basic" | "groups-invalid" | "network"
                | "network-egress" | "memory" | "memory-incremental" | "process" | "handles"
                | "endpoint" | "ipc-rights" | "ipc-roundtrip" | "scene-transaction"
                | "scene-script" | "scene-timer" | "ipc-write-drain" | "filesystem" | "registry"
                | "js-basic" | "js-interrupt" | "js-memory" | "js-stack" | "js-jobs",
                Ok(Ok(ProbeReply::Report(bits))),
            ) if bits
                == (EXPECTED
                    | match case {
                        "memory" => 32,
                        "memory-incremental" => 65536,
                        "package-basic" | "package-invalid" => package::RESULT_BIT,
                        "compiler-basic" | "compiler-invalid" => package::COMPILER_RESULT_BIT,
                        "image-basic" | "image-invalid" => package::IMAGE_RESULT_BIT,
                        "hierarchy-basic" | "hierarchy-invalid" => package::HIERARCHY_RESULT_BIT,
                        "groups-basic" | "groups-invalid" => package::GROUP_COMPILER_RESULT_BIT,
                        "ipc-roundtrip" => roundtrip::RESULT_BIT,
                        "scene-transaction" | "scene-script" | "scene-timer" => {
                            scene_ipc::RESULT_BIT
                        }
                        "process" => 64,
                        "handles" => 128,
                        "endpoint" => 1024,
                        "ipc-rights" => 2048,
                        "filesystem" => 4096,
                        "registry" => 32768,
                        "ipc-write-drain" => 8192,
                        "js-basic" | "js-interrupt" | "js-memory" | "js-stack" | "js-jobs" => {
                            script::RESULT_BIT
                        }
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
                if case == "memory-incremental" {
                    sandbox.verify_incremental_memory()?;
                }
                if case.starts_with("package-") {
                    println!(
                        "Fixed in-memory package manifest/source validation passed inside AppContainer; no file loading, script execution or imported capability."
                    );
                }
                if case.starts_with("compiler-") {
                    println!(
                        "Fixed HTML/CSS compiler fixture passed in AppContainer with scene revalidation; no external files or script execution."
                    );
                }
                if case == "ipc-roundtrip" {
                    if attempt.roundtrips_completed() != roundtrip::ROUNDS
                        || sandbox.wait(1000)? != Some(0)
                    {
                        return Err("Roundtrip completion/normal exit missing".into());
                    }
                    println!(
                        "Bounded roundtrip channel verified: 128 replies, one outstanding request, writer command capacity=1, host/worker EOF and normal exit."
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
                if case == "endpoint" {
                    println!(
                        "Unrelated host named-pipe endpoint denied with error 5 for read, write, duplex and zero-access connection requests."
                    );
                }
                if case == "ipc-rights" {
                    println!(
                        "Inherited channel rights verified: DACL reads/changes and reverse-direction I/O denied with error 5; normal framed I/O and EOF succeeded."
                    );
                }
                if case == "filesystem" {
                    println!(
                        "Filesystem matrix verified: sixteen direct OS denials (error 5); staged data/executable and System32/kernel32.dll reads plus staging listing permitted."
                    );
                }
                if case == "registry" {
                    println!(
                        "Registry matrix verified: eight access opens and direct child creation/key deletion denied with error 5; mask=0x8017."
                    );
                }
                if case.starts_with("js-") {
                    println!(
                        "Trusted {case} fixture passed: heap=16 MiB, stack=256 KiB, cooperative deadline=100 ms, Promise jobs<=64; host watchdog remains enabled."
                    );
                }
                if sandbox.wait(1000)? != Some(0) {
                    return Err("Healthy worker did not exit cleanly".into());
                }
            }
            (
                "hang" | "ipc-stall" | "ipc-open" | "ipc-write-stall",
                Err(mpsc::RecvTimeoutError::Timeout),
            ) => {
                if case == "ipc-write-stall" && !attempt.write_pressure_pending() {
                    return Err("Host writer did not remain pending under pressure".into());
                }
                let detection = Instant::now();
                sandbox.stop()?;
                println!(
                    "Watchdog terminated {case} in {:?} after detection",
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
            ("ipc-flood", Ok(Err(error))) if error.downcast_ref::<TrailingData>().is_some() => {
                let detection = Instant::now();
                sandbox.stop()?;
                println!(
                    "Unsolicited-message flood rejected after one trailing byte; Job stopped in {:?} after detection.",
                    detection.elapsed()
                );
            }
            ("ipc-replay" | "ipc-roundtrip-replay", Ok(Err(error)))
                if matches!(
                    error.downcast_ref::<wallpaper_protocol::Error>(),
                    Some(wallpaper_protocol::Error::Invalid(
                        "unexpected session, sequence or revision"
                    ))
                ) =>
            {
                if case == "ipc-roundtrip-replay"
                    && attempt.roundtrips_completed() != roundtrip::FAULT_AFTER
                {
                    return Err(
                        "Roundtrip replay was not preceded by eight validated replies".into(),
                    );
                }
                let detection = Instant::now();
                sandbox.stop()?;
                println!(
                    "Replayed sequence rejected; Job stopped in {:?} after detection.",
                    detection.elapsed()
                );
            }
            ("ipc-roundtrip-stall", Err(mpsc::RecvTimeoutError::Timeout))
                if attempt.roundtrips_completed() == roundtrip::FAULT_AFTER =>
            {
                if sandbox.wait(0)?.is_some() {
                    return Err("Roundtrip stall worker already exited".into());
                }
                let detection = Instant::now();
                sandbox.stop()?;
                println!(
                    "Roundtrip stall after eight replies; cumulative deadline stopped worker in {:?} after detection.",
                    detection.elapsed()
                );
            }
            ("ipc-truncated", Ok(Err(error)))
                if matches!(error.downcast_ref::<wallpaper_protocol::Error>(),
                Some(wallpaper_protocol::Error::Io(io_error)) if io_error.kind() == io::ErrorKind::UnexpectedEof) =>
            {
                if sandbox.wait(1000)? != Some(0) {
                    return Err("Truncation fixture did not exit cleanly".into());
                }
                println!("Truncated worker frame rejected as UnexpectedEof; host survived.");
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
    attempt.finish()?;
    outcome?;
    if case == "filesystem" {
        wallpaper_sandbox::verify_filesystem_fixture(&root)?;
        println!("Filesystem post-stop control: synthetic file contents and namespace unchanged.");
    }
    if case == "endpoint" {
        let session = attempt.session;
        drop(attempt);
        wallpaper_sandbox::verify_host_endpoint_closed(session)?;
        println!("Endpoint cleanup verified: name absent after owner destruction (error 2).");
    }
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
    #[test]
    fn cpu_arming_requires_complete_current_context_and_nonce() {
        for fault in 0..6 {
            let mut messages = prefix();
            let mut stage = envelope(
                42,
                3,
                Body::ProbeResult {
                    denied: EXPECTED | 16384,
                },
            );
            match fault {
                1 => stage.session = 43,
                2 => stage.sequence = 4,
                3 => stage.base_revision = 1,
                4 => messages[1].body = Body::Pong { nonce: 78 },
                _ => {}
            }
            messages.push(stage);
            let mut wire = bytes(&messages);
            if fault == 5 {
                wire.pop();
            }
            let (send, receive) = mpsc::sync_channel(2);
            let result = read_response(&mut io::Cursor::new(wire), 42, 77, false, &send);
            assert_eq!(receive.try_iter().count(), if fault == 0 { 2 } else { 1 });
            assert_eq!(result.is_ok(), fault == 0);
        }
    }
    #[test]
    fn completed_response_rejects_flood_after_only_one_extra_byte() {
        for handles in [false, true] {
            let mut messages = prefix();
            messages.push(envelope(
                42,
                3,
                Body::ProbeResult {
                    denied: if handles { HANDLES_ARMED } else { EXPECTED },
                },
            ));
            if handles {
                messages.push(envelope(
                    42,
                    4,
                    Body::ProbeResult {
                        denied: EXPECTED | 128,
                    },
                ));
            }
            let valid = bytes(&messages);
            let (ready, _receive) = mpsc::sync_channel(1);
            assert_eq!(
                read_response(&mut io::Cursor::new(&valid), 42, 77, handles, &ready).unwrap(),
                ProbeReply::Report(EXPECTED | if handles { 128 } else { 0 })
            );
            let mut flood = valid.clone();
            flood.extend_from_slice(&[0; 64 * 1024]);
            let mut stream = io::Cursor::new(flood);
            let (ready, _receive) = mpsc::sync_channel(1);
            let error = read_response(&mut stream, 42, 77, handles, &ready).unwrap_err();
            assert!(error.downcast_ref::<TrailingData>().is_some());
            assert_eq!(stream.position() as usize, valid.len() + 1);
        }
    }
}
