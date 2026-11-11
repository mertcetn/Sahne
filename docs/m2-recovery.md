# M2 Bounded Worker Recovery

Date: November 11, 2026. Status: fixed trusted Windows supervisor fixtures after 1261c32; partial G2 evidence. Ordinary single-case probes still run once. These new cases are not automatic product wallpaper recovery and do not enable arbitrary scripts/packages.

## Policy

One supervisor invocation has a run-wide budget of three attempts: initial launch plus at most two replacements. The attempt is reserved before launch. Only an observed failed process exit following missing/partial channel data, or a host deadline timeout, is eligible for retry. Malformed frames, wrong envelope/nonce/body, trailing traffic, clean exit without a complete validated response and launch/security-policy failures stop the run. A successful one-shot JS response also stops it; success does not reset the budget for another launch.

| Observation | Action |
| --- | --- |
| Valid selected JS report, nonce/envelope/EOF, exit 0 | Stop successfully |
| Channel timeout | Stop worker; retry if budget remains |
| Missing/truncated channel data plus an observed nonzero process exit | Stop/clean up; retry if budget remains |
| Bad schema/context/nonce/body, unsolicited traffic, unclassified channel error | Stop permanently, even if budget remains |
| Launch or explicit process/reader cleanup failure | End the supervisor; no fallback/retry |
| Third eligible failure | Exhaust budget; no fourth worker |

Retries have a fixed 100 ms minimum backoff from completion of the old owner scope, using Instant rather than wall-clock time. Waiting and valid handshakes do not replenish attempts. Concurrent start calls, stale/duplicate completions and attempts to reopen terminal state are rejected. A native exception can delay an observable exit; if an EOF failure cannot be paired with a failed exit within the short 100 ms observation window, it stops conservatively as a protocol failure rather than being retried speculatively.

## Ownership and deadlines

Individual probes and recovery fixtures now share one ProbeAttempt owner for process/Job/profile, pipe reader, one-slot queues and loopback listener. Before admitting a replacement, the supervisor stops/waits for the old Job, joins its reader and leaves that owner's scope, releasing the old process/channel handles. Fresh session/nonce values, Job, profile SID, staging directory and pipes are created for every attempt. The fixture rejects reuse of any earlier session or nonce in that run; OS numeric handle/PID reuse is not used as identity.

The previous response cannot be promoted after a new attempt starts: no reader/queue is reused, completions carry the current host attempt number, and every wire message is checked against the new session. The stale-session case deliberately injects a prior real session ID into an otherwise correctly formed Pong on a new pipe with the current nonce. A valid new Hello must already have been accepted, then the exact context error must end the run. This tests rejection of injected old identity; it does not intentionally carry old pipe buffers into a replacement.

The handshake and subsequent response each retain a three-second deadline. A failed/missing Hello now uses only a nonblocking check for any already-queued error; it does not acquire an additional response wait. The no-Hello regression requires channel waiting to end between three and four seconds, excluding OS launch/profile setup. Both old and new paths keep the independent watchdog and native Job limits unchanged.

After the supervisor observes a response result/timeout, classification plus explicit Job stop/reader join must complete within one second or the attempt fails. Printed stop/join durations measure only that explicit cleanup stage; they are not end-to-end recovery times. Successful joins depend on stopping the sole worker and closing its listed pipe writers; failed stop/join prevents policy advancement. Existing native guards still attempt cleanup on error. Profile deletion remains best-effort Drop cleanup that reports failures; this task does not add independent profile enumeration or propagation of deletion failures. Profile/OS creation and deletion do not have a newly claimed hard wall-clock deadline.

Ignored staging artifacts are retained as before. There is no concurrent replacement, permission expansion, watchdog bypass, persisted policy reset or repeated trial loop. A new explicit CLI invocation creates a new test run; product reset/retry UI is future work.

## Commands and exact fixture outcomes

Run from the repository root on Windows:

```powershell
cargo run --release -p wallpaper-sandbox-probe --locked -- --case restart-recover
cargo run --release -p wallpaper-sandbox-probe --locked -- --case restart-exhaust
cargo run --release -p wallpaper-sandbox-probe --locked -- --case restart-timeout
cargo run --release -p wallpaper-sandbox-probe --locked -- --case restart-handshake
cargo run --release -p wallpaper-sandbox-probe --locked -- --case restart-stale
cargo run --release -p wallpaper-sandbox-probe --locked -- --case restart-protocol
```

Each must print `Probe passed` and exit zero: this means the test's expected terminal state was reached, not that exhausted/rejected workers count as successful wallpapers.

| Case | Expected sequence | Required terminal state |
| --- | --- | --- |
| restart-recover | Exit-42 worker, then fixed js-basic | Healthy; two attempts |
| restart-exhaust | Three exit-42 workers | Exhausted; exactly three attempts and no further launch |
| restart-timeout | Valid Hello then native stall, then js-basic | Healthy; two attempts |
| restart-handshake | No Hello, then js-basic | Healthy; two attempts; no second wait after failed Hello |
| restart-stale | Exit-42 worker, then new Hello plus prior-session Pong | Protocol; two attempts; no third launch |
| restart-protocol | Valid report followed by trailing-message flood | Protocol; one attempt despite unused retry budget |

All recovery success paths require the fixed JS report mask 0x217, clean worker exit and unchanged sentinel. No creator scene is applied or retained by these fixtures; preserving a last accepted rendered scene remains M4 work.

## Verification record

Four policy/classification regressions cover the three-attempt cap and monotonic backoff boundary, terminal success/protocol/launch behavior, stale/duplicate completions during a newer attempt, and the distinction between EOF candidates and other protocol errors. Advancing the clock repeatedly cannot reopen exhausted state. Real fixtures exercise both retry and permanent-stop paths.

Final release verification passed all twenty-four expected-success scenarios: the previous eighteen native/IPC/JS cases and all six supervisor cases, totaling thirty restricted worker launches. The no-Hello case waited 3.0001066 seconds for the channel and stopped/joined in 2.0868 ms; the Hello-then-stall case stopped/joined in 1.8893 ms after its response timeout. These stop-stage timings exclude OS launch and classification and are not cross-machine guarantees. Every finish additionally checked the one-second window from the host's response observation through classification/stop/join.

| Final recovery scenario | Result | Retained attempt directories |
| --- | --- | --- |
| restart-recover | Healthy after exactly two attempts | `28416-b3dd483790c6cbeb`, `28416-538e97bdda548052` |
| restart-exhaust | Exhausted after exactly three attempts | `30664-0c9e0afb30cc8169`, `30664-fbf35c9c2cb77de5`, `30664-85a822c5adbc3d71` |
| restart-timeout | Healthy after timeout cleanup and a fresh attempt | `3792-e95d7c9ea2f97226`, `3792-1a8073ddd4517022` |
| restart-handshake | Healthy after a single failed-Hello deadline | `18472-bbef1ef46adfa980`, `18472-5575d005c94e245b` |
| restart-stale | Permanent protocol stop at attempt two | `23648-09482e298b72c14e`, `23648-d0bed97ef7eabbc2` |
| restart-protocol | Permanent protocol stop at attempt one | `26220-36e0ca537007e559` |

The full local transcript is retained under ignored target/sandbox-runs/restart-regression.log. All nineteen Rust tests (nine codec, two preview lifecycle, four existing reader and four new policy/classification), workspace release build, formatting and Clippy with warnings denied passed. Existing incremental-cache hardlink/copy notices were emitted. Hosted CI, other Windows configurations and non-Windows targets were not run. No user input is required to repeat these checks. New recovery source remains uncommitted.

No dependency, Cargo feature, source interpreter policy, native permission or resource limit changed. The only native launch additions are trusted internal old-session/no-Hello selectors and a numeric prior-session test argument.

Remaining M2/G2 gaps include broader network and unrelated endpoint/escape coverage, persistent allowed-message rate/CPU budgets, source transport, multiple long-lived sessions, profile-cleanup audit and hosted/alternate-platform validation. The native retry experiment does not close these or the I1 import-adapter contract work. Product callback/timer/scene recovery is still M4 integration. No untrusted content or release publication is enabled.

Primary documentation consulted November 11, 2026:

- [Rust monotonic Instant and platform behavior](https://doc.rust-lang.org/std/time/struct.Instant.html)
- [Rust channel receive/timeout/nonblocking semantics](https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html)
- [Windows Job ownership, limits and termination](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
