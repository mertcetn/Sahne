# M2 IPC Fault and Pressure Validation

Date: November 11, 2026. Status: trusted Windows probe continuation after 9d53b81; partial G2 evidence. No packages or JavaScript are executed. This is a one-shot test channel, not the persistent M4 transaction channel.

## Behavior

The probe now requires clean EOF after the final accepted report. It reads at most one extra byte to check for unsolicited trailing traffic and rejects that byte immediately, without parsing, draining, queueing or logging subsequent messages. This applies to ordinary responses and the normally returning handle fixture. The strict-handle fatal path already requires clean EOF after its armed stage and retains the independent policy/exit checks.

A peer that sends a complete report but leaves stdout open cannot complete the conversation. The independent three-second host response deadline terminates its Job. A declared but incomplete frame remains subject to the same deadline; receiving some bytes does not reset it. Unexpected body types are rejected before waiting for EOF. The host does not retry malformed traffic or restart a failed worker.

The existing codec still caps one payload at 64 KiB before allocation, validates exact body consumption and checks expected session/sequence/revision. One reader thread processes the fixed conversation sequentially, with one-slot handshake/result queues. There is no accumulating queue per incoming message. The host writes only one small Ping; large host writes and a persistent bidirectional channel remain untested. CreatePipe's requested 4096-byte buffer is an OS sizing hint, not a claimed exact kernel-memory limit. Existing AppContainer capabilities, Job memory/process limits, handle list and deadlines are unchanged.

## Cases and commands

Run from the repository root on Windows:

```powershell
cargo run --release -p wallpaper-sandbox-probe --locked -- --case ipc-flood
cargo run --release -p wallpaper-sandbox-probe --locked -- --case ipc-stall
cargo run --release -p wallpaper-sandbox-probe --locked -- --case ipc-truncated
cargo run --release -p wallpaper-sandbox-probe --locked -- --case ipc-replay
cargo run --release -p wallpaper-sandbox-probe --locked -- --case ipc-open
```

Each case must print `Probe passed` and exit zero, meaning its intentionally invalid/stalled worker behavior was contained. A success from one fault selector cannot substitute for another error kind:

| Case | Trusted worker behavior | Required host observation |
| --- | --- | --- |
| ipc-flood | Valid Hello/Pong/report, then repeated small Ping frames | Typed trailing-data rejection after one extra byte; Job stopped |
| ipc-stall | After Hello/Ping, declare a 35-byte Pong payload, emit one byte and stall | Response timeout; Job termination within one second after detection |
| ipc-truncated | Same incomplete frame, then normal worker exit | Codec I/O UnexpectedEof and worker exit 0; host survives |
| ipc-replay | Send Pong with sequence 1 when sequence 2 is expected, then stall | Exact envelope-context rejection; Job stopped |
| ipc-open | Valid Hello/Pong/report, then keep the stream/process open | Response timeout, not successful completion; Job stopped |

The partial-frame fixtures have a valid Hello before the deliberate fault. No per-byte progress metric or packet capture is collected. Timings below start when the host supervisor observes the rejection/timeout, not at the first transmitted byte. The response deadline is separate from termination time.

## Regression coverage

A new reader regression supplies a valid completed response followed by 64 KiB of trailing bytes. For both normal and normally returning handle responses it checks the typed rejection and cursor position: exactly one byte beyond the valid conversation is consumed. A valid response ending at EOF still succeeds. Existing reader tests retain arming/context/truncation distinctions for strict-handle failure; codec tests retain malformed size/version/body and fragmented-I/O coverage.

The full actual-process regression also reruns healthy, network-egress, memory, process, handles, hang, oversized and crash, since EOF completion and the shared reader changed. The loopback network investigation remains inconclusive and is not reclassified by IPC results.

## Local evidence

Same Windows 11 Pro build 26200 / Rust 1.99.1 machine as the [sandbox record](m2-sandbox.md). Final release results:

| Case | Result | Retained run / measurement |
| --- | --- | --- |
| ipc-flood | Pass | `19884-9e52f17f415e6933`; Job stopped 1.4568 ms after detection |
| ipc-stall | Pass | `6664-95b665ec1143248d`; stopped 4.9237 ms after timeout detection |
| ipc-truncated | Pass | `16868-6b4d742606689dab`; required UnexpectedEof and clean worker exit |
| ipc-replay | Pass | `28928-e87646c7ba024307`; stopped 1.9307 ms after detection |
| ipc-open | Pass | `28640-775c537c3d549d12`; stopped 2.4406 ms after timeout detection |

All thirteen expected-success native cases passed on the final release binary, including the previous eight cases. All fifteen workspace tests (nine codec, two preview lifecycle, four host-reader), workspace release build, formatting, Clippy with warnings denied, local documentation links and diff checks passed. Cargo manifests and lockfile are unchanged from 9d53b81. The existing incremental-cache hardlink/copy notices were emitted. Hosted CI, non-Windows targets and additional Windows configurations were not run. These measurements are not cross-machine latency guarantees or a sustained-load benchmark. The validated IPC continuation remains in the working tree; no release was pushed.

## Remaining G2 work

This demonstrates fail-fast handling of unsolicited trailing-message pressure on one short-lived channel, not a production rate limiter for permitted persistent traffic. Per-window CPU/rate budgets, sustained valid-message pressure, blocked large host writes, multiple connections, ACK/log floods, repeated restarts and long-duration memory measurements remain open. Scene transactions, atomic acceptance and retained renderer state belong to the M4 integration. JS interruption, Promise/timer floods, runtime allocation/stack limits and engine security review remain unimplemented. Broader network, unrelated endpoint and process/handle escape coverage also remains open.

No dependency, feature, registry version, resource limit or native permission was added by this task. The worker-launch whitelist adds only these trusted fixtures. Native tests still create unique temporary AppContainer profiles and retain ignored target/sandbox-runs artifacts; cleanup behavior is unchanged. Agent-executable checks are run autonomously under the user's updated workflow; no duplicate user test run is required.

Primary documentation consulted November 11, 2026:

- [Rust bounded synchronous channels](https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html)
- [Rust Read, EOF and partial reads](https://doc.rust-lang.org/std/io/trait.Read.html)
- [Windows anonymous pipes, buffering and blocking](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createpipe)
