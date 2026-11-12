# M2 Host Write-Pressure Validation

Date: November 12, 2026. Status: trusted Windows continuation after 0052ca8; partial G2 evidence. No arbitrary source, package transport or persistent scene channel is enabled.

## Commands and acceptance

```powershell
cargo run --release -p wallpaper-sandbox-probe --locked -- --case ipc-write-drain
cargo run --release -p wallpaper-sandbox-probe --locked -- --case ipc-write-stall
```

The drain case must validate all fixed pressure bytes, input EOF, normal nonce/context/report/EOF, exit zero and profile cleanup. Its exact report mask is 0x2017. The stall case must complete Hello but retain an unfinished host writer after entering the pressure loop at the response deadline. The host must stop the worker, join both I/O threads, observe a pipe-closure error from the writer and verify profile cleanup. A writer that never entered pressure, finished the entire budget, or returned an unrelated error fails the stall case.

## Host ownership and bounds

Previously the host wrote its small Ping synchronously before starting response deadlines. Although that fixed write passed, a larger write to a peer that does not read can block when the pipe fills. All cases now move input ownership into one dedicated writer thread. The main control thread starts the existing handshake/response waits without performing a blocking pipe write itself. One reader and one writer belong to each attempt; no writer thread or producer queue is created per message.

The attempt owner is installed before either thread is started. Fallible named thread creation ensures that failure to start the second thread drops an owner capable of stopping the worker before joining the first. Normal completion/failure cleanup stops/waits the sole worker, joins reader and writer, checks writer completion, then performs explicit profile deletion. Recovery cannot advance before this succeeds. Drop invokes the same cleanup while either thread remains owned.

The writer sends one ordinary Ping. Only the two fixed pressure cases then send exactly 256 blocks of 4096 bytes filled with 0x5a: 1 MiB total plus the 39-byte Ping frame. A single 4 KiB block is reused; there is no 1 MiB host allocation or unbounded outbound queue. All other cases still send only Ping. Closing the owned input makes EOF available to the worker. The existing one-slot ready/result channels, explicit two-handle worker list, protected DACLs, data-only handles, Job limits and zero capabilities remain unchanged.

## Positive and stalled workers

The drain worker validates its Ping, reads exactly the fixed number of blocks with a reusable 4 KiB buffer, checks every byte and requires input EOF before its ordinary response. The host also checks that its complete byte budget was written and that the writer joined successfully.

The stall worker emits and flushes Hello, then never reads stdin. At the three-second response timeout the host requires an active unfinished pressure writer with fewer than 1 MiB of completed blocks. It terminates the Job; closure of the worker's sole read endpoint releases the outstanding write. Only ERROR_BROKEN_PIPE (109) or ERROR_NO_DATA (232, pipe being closed) is an accepted writer outcome. Actual Windows testing returned 232; an initial test expecting only 109 correctly failed until the documented pipe-closing outcome was accounted for. Timeouts, arbitrary errors and successful full writes are not substituted for this evidence.

The progress counter counts fully completed write_all blocks. It does not expose partial writes or exact kernel-buffer occupancy. CreatePipe's 4096-byte buffer argument remains a sizing hint. The fixed raw tail is a transport-only test fixture selected by trusted host/worker case arguments; it is not a new control-message payload, script input API or production schema bypass.

## Timing and limits

Handshake and response waits retain their three-second deadlines; a missing handshake receives no extra response wait. The existing observational one-second classification/Job-stop/join acceptance now includes both I/O threads. Printed watchdog termination duration measures the stop call only; profile deletion has a separate duration. Joins depend on successful worker termination and sole ownership of the opposite pipe ends. This is observed failure containment, not a kernel-independent cancellation or hard real-time guarantee. Synchronous OS profile operations retain their documented timing limitation.

No dependency, Cargo feature, version, lockfile, application unsafe exception, capability or resource-limit changes are needed. This uses the existing reviewed native launch path and Rust std I/O/thread/atomic APIs. Dependency advisories remain as previously recorded. Remaining G2 work includes sustained valid-message rate/CPU limits, long-lived/multiple channels, production source/scene protocol, broader system access and inconclusive loopback denial. These fixtures do not certify untrusted execution.

## Validation

All twenty-nine expected-success release scenarios passed, totaling thirty-five restricted worker launches with thirty-five explicit profile-storage cleanups. The full local transcript is retained under ignored target/sandbox-runs/host-write-regression.log. The inconclusive loopback-network case remains outside this expected-success set.

Final drain attempt `6472-36237f4decd0948f` verified 0x2017, every byte/EOF, complete host write count, normal exit and cleanup. Final stall attempt `31736-ad4f1f8ff8a53841` remained pending at the deadline with 4096 completed block bytes out of 1048576, then returned 232 after the worker stop. Its printed stop duration was 2.585 ms; both threads joined and profile cleanup passed. These timings are observations, not universal limits.

All nineteen Rust tests, formatting, Clippy with warnings denied, the workspace release build, CLI help, local documentation links and diff checks passed. Existing incremental-cache hardlink/copy notices remain. No hosted CI, alternate Windows configuration or non-Windows target was tested. New changes remain uncommitted; no push or release was performed. All automated cases require no physical or visual user input.

Primary references consulted November 12, 2026:

- [CreatePipe write blocking and buffer semantics](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createpipe)
- [Windows pipe-closure error codes](https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--0-499-)
- [Rust JoinHandle ownership, join and detach behavior](https://doc.rust-lang.org/std/thread/struct.JoinHandle.html)
- [Bounded sync_channel behavior](https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html)
