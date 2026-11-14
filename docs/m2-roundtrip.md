# M2 bounded multi-message IPC

Date: November 14, 2026. Status: trusted Windows transport fixtures, before source/scene integration.

## Run

From the repository root on Windows with PowerShell 7:

```powershell
./tools/test-sandbox.ps1 -Cases @('ipc-roundtrip', 'ipc-roundtrip-replay', 'ipc-roundtrip-stall')
```

Each case must exit zero, print `Probe passed` and verify its single profile cleanup. All three are included in the default corpus.

## Channel contract

These fixtures use the same protected, host-user-only, data-direction-only anonymous pipes and fresh AppContainer/Job/session as the ordinary probe. They add no wire kind, native binding, dependency, capability, source transport or scene/callback API.

After Hello at worker sequence 1, the host and worker perform at most 128 Ping/Pong exchanges. Host Ping sequences are 1..128; worker Pong sequences are 2..129, followed by a final result at sequence 130. Session and revision zero must match throughout. Every request after the first gets a fresh nonzero BCrypt nonce; adjacent collisions fail the fixture. A response must match the current nonce before progress is counted.

Only one request can be outstanding. The reader owns the sole sender for a one-slot writer-command queue; it submits the next fixed-size `(sequence, nonce)` command only after validating the preceding response. `try_send` rejects a full/disconnected queue immediately rather than blocking the reader. The dedicated writer consumes commands and writes each Ping; the main deadline thread performs no pipe I/O. Result and handshake queues retain their existing one-slot limits.

The three-second response wait is cumulative after the bounded Hello wait: validating a Pong does not restart it. On failure, the existing control thread stops the Job. Reader EOF/error drops the sole writer-command sender, releasing an idle writer; both threads must join before profile cleanup. Partial thread-start failures retain the existing attempt-owner cleanup. A full writer-command queue regression test verifies that reader shutdown does not block on that queue.

On normal completion, the reader drops the command sender before reading the final result. The writer drains the last issued command, closes input, and joins; the worker requires input EOF before sending the result and exiting. The host requires the exact result `0x20017`, 128 validated replies, output EOF, normal worker exit, unchanged sentinel and verified profile cleanup. Early EOF or a correct-looking final result without all replies cannot pass.

This is a bounded sequential control-channel experiment. It does not implement a persistent production session, source/package uploads, scene transactions, revision/ACK semantics, callback scheduling or concurrent request pipelines. Those remain tied to the M3/M4 contracts and their integrated validation.

## Fault cases and local evidence

Selected report `613f422a5f874624a1577437dc4517e3` passed on Windows 11 build 26200 / Rust 1.99.1 MSVC:

| Case | Required observation | Recorded result |
| --- | --- | --- |
| `ipc-roundtrip` | 128 validated nonce/context replies, both EOFs, normal exit | Passed, staging `6024-74abc006599a060e` |
| `ipc-roundtrip-replay` | Eight replies, then stale worker sequence with current nonce; exact context rejection | Passed; Job stop 1.2857 ms after detection |
| `ipc-roundtrip-stall` | Eight replies, then live native stall; cumulative response deadline | Passed; Job stop 1.3818 ms after detection |

These initial selected timings precede the final `try_send` hardening and are measurements, not guarantees. All three ran again in the final default corpus. The existing one-second classification/stop/join acceptance remains mandatory; profile-deletion timing remains separate. G2 production certification and untrusted execution remain disabled.

Final full report `0bc8f4624c404d7d9a262a0bcbdbf17c` passed thirty-five cases, forty-one worker launches/profile cleanups and one registry cleanup. All twenty-one workspace Rust tests, Clippy with warnings denied and formatting passed. Existing incremental-cache hardlink/copy notices remain. No hosted or non-Windows run was performed.

## Primary references reviewed November 14, 2026

- [Rust bounded synchronous channels](https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html): capacity, ordering and endpoint disconnection.
- [Rust receive timeout semantics](https://doc.rust-lang.org/std/sync/mpsc/struct.Receiver.html#method.recv_timeout): one cumulative wait in the main controller. Only APIs available in pinned Rust 1.99.1 are used; the current online standard-library docs describe a newer stable compiler and do not change the toolchain pin.
- [Microsoft Job termination and ownership](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).

The existing [codec](m2-protocol.md), [channel ACL/handle evidence](m2-ipc-security.md) and [host write-pressure tests](m2-host-write.md) remain applicable. Existing dependency security findings are unchanged; no new package or feature review is required for this standard-library-only extension.
