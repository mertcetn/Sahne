# M2 completion record

Reviewed: November 14, 2026. Decision: the Windows sandbox and bounded IPC **technical spike is complete for the fixed trusted fixtures on the recorded local machine**. This delivers the roadmap's initial G2 evidence. The full integrated G2 gate remains open; arbitrary content stays disabled.

## Acceptance against the planned milestone

The [roadmap](roadmap.md#5-m2--windows-sandbox-and-bounded-ipc) places production source/scene/callback integration in M3/M4. This record evaluates M2's minimal-worker feasibility work, without treating missing product APIs as tested or removing their validation requirements.

| M2 requirement | Implemented and exercised evidence | Limits |
| --- | --- | --- |
| Actual restricted worker | Fresh exact AppContainer SID, independently queried token, zero capabilities, explicit two-handle inheritance, curated environment | Standard AppContainer; production/LPAC policy and broker inventory remain open |
| Job limits | 256 MiB process commit, one active process, kill-on-close configured, CPU hard cap read back; direct child quota denial and single-worker CPU membership checked | Abrupt host-death kill/cleanup and all nested-Job environments not exercised |
| Permitted IPC | Protected host-user-only pipe DACLs, data-only inherited handle rights, bounded codec, exact session/sequence/revision/nonce validation | No source/package or scene protocol yet |
| Queue and progress bounds | One-slot result/handshake/command queues, one outstanding request, at most 128 roundtrips, cumulative response deadline, EOF in both directions | Production pipelines, callback traffic and scene ACK scheduling remain M4 work |
| Direct filesystem denial | Sixteen synthetic access/create/listing denials, host positive controls, unchanged post-stop contents/namespace | Selected staged/system reads allowed and documented; not an exhaustive system-read inventory |
| Direct registry denial | Ten synthetic rights/create/delete denials with host controls and exact owned-key cleanup | Other namespaces and independent AppContainer hive/registration deletion audit open |
| Network evidence | Explicit outbound TCP denial to IPv4 and IPv6 documentation addresses. Controlled IPv4 and IPv6 TCP and UDP loopback accepted locally with WFP blocks correlated to the worker's source port. Worker-correlated WFP records confirmed IPv4 UDP egress blocking to a documentation address and IPv6 UDP blocking to a reachable non-loopback WSL peer. | Public IPv6 UDP destinations, DNS, brokers, other Windows builds and firewall configurations remain open. |
| Process/endpoint/handle denial | Host VM access denied, child creation rejected by quota, excluded handle denied or strict-policy terminated, unrelated named pipe denied, channel ACL changes/reverse I/O denied | Synthetic objects and selected APIs only |
| Memory/stack and JS | Native single and cumulative commit tests, release/recovery allocation, host peak accounting; fixed JS heap/stack/interruption/Promise-job cases | QuickJS maintenance uncertainty remains; no arbitrary scripts or host callbacks |
| Failure containment | Native stalls, malformed/truncated/replayed/overlong/flooded/open IPC, crashes, blocked host writes, mid-session stalls/replay | Every accepted attempt retains the one-second classification/Job-stop/I/O-join check; OS construction/profile-call timing is separate |
| Bounded recovery | Three-attempt run-wide budget, backoff, fresh identities/channels, stale completion protection, permanent protocol failures | Explicit fixtures, not a complete production supervisor |
| Cleanup | Explicit Job stop/wait, reader/writer joins, populated profile directory absence; registry sentinel/namespace and owned-key absence | Drop fallback and abrupt host-death/profile-hive audit remain open |
| Import boundary | No imported capability escalation, raw worker paths, sample execution or inspection-time execution | I1 overall mappings/callback semantics and actual importers remain later work |

Implementation details and source references are in [native foundation](m2-sandbox.md), [channel security](m2-ipc-security.md), [filesystem](m2-filesystem.md), [registry](m2-registry.md), [loopback diagnosis](m2-loopback.md), [incremental memory](m2-incremental-memory.md), [roundtrip IPC](m2-roundtrip.md), [JS](m2-js.md), [recovery](m2-recovery.md) and [runner](m2-validation-runner.md).

## Reproducible local evidence

- Final corpus report: ignored `target/sandbox-validation/0bc8f4624c404d7d9a262a0bcbdbf17c/summary.json`, `Status=Passed`, thirty-five cases, forty-one launches/profile cleanups and one registry cleanup. Its metadata records base commit 5b71ff1 with a dirty working tree; the validated implementation was subsequently committed in 06a253a. It is not a clean-checkout or hosted-CI claim.
- Binary SHA256: `efc2d18f2e7629b463c30317da2bf45fa403f22e0665673e404c7c0d7aa9dcb7`.
- Local platform: Windows 11 build 26200, x64, pinned Rust 1.99.1 MSVC. CPU usage observations apply to this eight-logical-processor machine.
- Twenty-one workspace Rust tests, Clippy with warnings denied, formatting and local documentation-link checks passed. Runner self-checks exercised four profile-free helper processes and nine evidence checks; loopback matcher checks passed one positive and twenty negative cases. The queue-full reader regression also passed independently.
- Existing Cargo incremental-cache hardlink/copy notices remain. Manual sandbox CI configuration exists; hosted execution and non-Windows runtime checks for this spike remain unverified.

`G2Complete=false` in runner reports remains intentional: a fixed-corpus run is not an automatic decision to enable untrusted content. M2 completion does not change the security gate, capabilities, limits or release contents. No additional user hardware/visual test is needed for this developer-only transport/worker milestone.

## Mandatory follow-up before untrusted content

These requirements remain active, rather than being declared passed by the fixed corpus:

| Stage | Remaining work |
| --- | --- |
| M3 / G3 | Bounded restricted compiler and scene data, hostile syntax/resources, finite numeric validation, tree/image/archive budgets and source diagnostics; parsing/decoding isolation before untrusted input |
| M4 / integrated G2 | Actual production source/scene transport; IDs/revisions/transactions/ACKs; invalid floats, stale/reordered updates and ACK/log floods; last accepted scene preservation |
| M4 / integrated G2 | Timers, callbacks, Promise/host-callback fairness, pause/resume and their count/time/memory/queue limits; none exists merely because fixed JS executes |
| M4 / isolation review | Final allowed-read/capability/broker policy, broader OS/network endpoint coverage, host-death lifecycle/cleanup and profile registry/hive audit; rerun failures through the real content path |
| M4 / dependency review | Reassess the adopted engine snapshot and full active dependency graph using current advisories and source review before arbitrary source is enabled; preserve unresolved risks in the gate decision |
| M5 / M6 | Product/session/performance and declared Windows/Linux target matrices; no untested platform support claims |

Known dependency findings, including QuickJS source/maintenance uncertainty and the separately assessed renderer lru/ttf-parser issues, remain in [JS dependency review](m2-js-dependencies.md) and [M1 dependency review](m1-dependencies.md). This task changes no dependency or feature and does not resolve those risks. No third-party sample has been executed by these fixtures.

## M4 integrated evidence checkpoint

Reviewed November 26, 2026. The Windows headless package regression passed 34 local cases. Its results cover the development paths below. The G2 gate stays open.

| Area | Evidence | Remaining verification |
| --- | --- | --- |
| Native intake | The restricted helper checks the manifest, compiles flat HTML/CSS and decodes supported PNGs. The host validates returned scene and pixel data. The [isolated script preview](m4-scene-transactions.md#isolated-script-package-preview) also verifies bindings before starting a separate execution worker. The isolated live mode was visually verified on the Windows desktop. | Custom font processing, broader input handling and visual coverage of other isolated script display modes remain pending. |
| Live transactions | The preview accepts bounded script updates and retains the accepted revision after callback or scene rejection. | Run the transport failure suite through the integrated path, including stale/reordered frames and independent deadline and cleanup cases. |
| Callback limits | Promise and DOM limits, staged edit limits, callback deadline, and pause/resume have local AppContainer evidence. | Broader callback and resource failure coverage, including host-death cleanup and OS isolation review. |

See [Windows AppContainer intake check](m3-manifest.md#windows-appcontainer-intake-check), [integrated script regression](m4-scene-transactions.md#integrated-script-preview-regression), and [live trusted package lifecycle](m4-timers.md#live-trusted-package-lifecycle-preview).

The [M4 advisory refresh](m2-js-dependencies.md#m4-advisory-refresh) checked all 243 locked crates.io versions and the embedded engine's upstream advisories. It confirmed the existing renderer findings and the need to retain the G2 decision until the remaining source and isolation work is complete.

The preview's package response reader has a local byte-stream regression after the host has accepted revision 1. Responses with a different session, repeated or skipped sequence, stale base revision, unexpected message kind or truncated frame fail without changing the accepted revision or snapshot. Real AppContainer fixtures cover a different session, skipped sequence, unexpected message kind, a repeated batch with its original base revision and an incomplete response payload.

The November 27, 2026 run passed all 55 package cases, including the three real-process response context cases. Each preserved the first snapshot and verified both profile cleanups. Integrated pressure and acknowledgement traffic remain open. Local report: `target/script-preview-validation/753e1278ce2746b9a4ac7e26a8eb8232/summary.json`.

A subsequent run passed all 56 package cases and recorded 128 consecutive live acknowledgements and both profile cleanups at the 16 ms minimum interval. The fixture checked that later callbacks read previously acknowledged values. This is sustained valid traffic; integrated unsolicited traffic and backpressure checks remain open. Local report: `target/script-preview-validation/8eb0651111414af480e47e75369939b9/summary.json`.

A follow-up run passed all 57 package cases. When an isolated execution worker completed the handshake and stopped reading, the 32 KiB script program write was still pending at the host deadline. The preview stopped the worker, joined the I/O thread, verified both profile cleanups and published no updates. This covers host-side program-write backpressure. Local report: `target/script-preview-validation/60dadb166b5a4b4ea865197889abac59/summary.json`.

On 2026-11-27, all 57 package cases passed with the expanded unexpected-message fixture. The worker sent 16 unexpected control messages after a valid frame and kept the pipe open. The preview rejected the first unexpected message, retained the single accepted snapshot, and verified cleanup of both AppContainer profiles. The case exited successfully after 1.22 seconds, before the fixture's 15-second wait could end. Local report: `target/script-preview-validation/67feb26e8cab4071ae4cef4a3dc5f739/summary.json`.

On 2026-11-28, all 59 package cases passed. The two new cases verified that AppContainer intake rejects package-defined font faces and font-family selection during compilation and cleans up the helper profile. The font-face fixture included the referenced TrueType file. Local report: `target/script-preview-validation/c4dee13c1009437794a4280b2b4f61d7/summary.json`.

A subsequent isolated live fixture verified that `console`, `print`, `fetch`, `require`, `std` and `os` were absent during package callbacks while the bounded DOM update path remained usable. All 60 cases passed when the suite ran on its own. Two earlier concurrent validation runs each recorded one lifecycle initialization rejection while other heavy checks were running. The standalone result does not establish behavior under system-wide load. Local report: `target/script-preview-validation/36092ee0ddcd4c45a65ac0dc3eab1504/summary.json`.

The November 27, 2026 package regression passed all 49 cases. Its stale-batch case observed one accepted snapshot, a terminal context rejection and both AppContainer profile cleanups. Local report: `target/script-preview-validation/a0a163885e044380838b98afef58e944/summary.json`.

A follow-up run passed all 50 cases. Its truncated-response case distinguished incomplete data from clean end of stream and preserved the first accepted snapshot while cleaning up both profiles. Local report: `target/script-preview-validation/be50656d4e77491d9c3a6ce57a90e2dd/summary.json`.

The November 27, 2026 resource run passed 52 package cases. In two isolated live cases the second tick staged a change before exercising the heap or stack limit. Both returned a generic worker rejection, preserved revision 1 and cleaned up both profiles. The separate `js-memory` and `js-stack` probes confirmed the engine's specific allocation and stack errors without a deadline interrupt. Local package report: `target/script-preview-validation/87ca357f6f4c4e10ad2643bbbaea5d97/summary.json`.

### Baseline sandbox revalidation

On November 27, 2026, the full default sandbox suite passed 50 cases and confirmed 56 profile cleanups on the local Windows machine. The run used the existing release binary with `-SkipBuild`. The source tree was clean and the latest commit only changed tests and documentation. Local report: `target/sandbox-validation/75449bcbcb5c421c8d04c2e3e6060bf0/summary.json`.

The separate strict `network` loopback case failed. The host connected to its own local fixture and the AppContainer worker's connection timed out without an explicit access-denied result. The profile was cleaned up. The timeout remains inconclusive under the existing acceptance rule. The default suite includes the distinct `network-egress` case, which passed, and reports `G2Complete=false`. Separate loopback report: `target/sandbox-validation/d16feee6575f408a9e2d594ee1795972/summary.json`.

A separate elevated read-only rerun matched two receiving-side WFP AppContainer Loopback drops for its own test port. A subsequent run correlated both drops with the source port of the verified worker in the Windows TCP table. The updated acceptance rule accepts that captured evidence for controlled IPv4 TCP loopback. Overall G2 stays open. See [Loopback acceptance rule](m2-loopback.md#acceptance-rule).

The isolated live package path now has a real-worker watchdog regression. The test worker publishes one valid snapshot and stops responding at the second tick. The host deadline terminates the job and verifies both profile cleanups. This covers an unresponsive worker after startup. Host-death cleanup and the remaining process-level fault cases stay open. See [Integrated package watchdog check](m4-timers.md#integrated-package-watchdog-check).

### Abrupt preview termination

On November 27, 2026, a headless test using the isolated live package path verified that killing the preview process also terminated its live AppContainer worker. The test identified the child by its parent process ID and exact staged executable path, then waited on the child's process handle. The execution worker's AppContainer storage remained after both processes exited. The test explicitly removed its own profile using the Windows API and confirmed the storage was gone. Local report: `target/host-death-validation/fb468288994f4cacb7cbeaa4c7959e6e/result.json`.

The Windows job terminates the worker when the host's last job handle closes. Abrupt exit prevents the host's Rust cleanup from deleting the profile. G2 still requires recovery of test-owned or application-owned orphaned profiles with proof of ownership and liveness checks. [Windows job objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) · [DeleteAppContainerProfile](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-deleteappcontainerprofile)

The launcher writes a per-run ownership record and holds it open exclusively during the profile lifetime. A headless check observed Windows reject a concurrent open, then verified that the record matched the run after both profiles were cleaned up. All 47 package regression cases also passed. Local report: `target/profile-lease-validation/a5742c7fd843454f8476be87b9cee878/result.json`. [Windows file sharing modes](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)

From the preview's working directory, run `wallpaper-preview.exe --recover-preview-profile <run-name>` with one `preview-<pid>-<session>` or `intake-<pid>-<session>` name from `target/sandbox-runs`. Use the same Windows user that launched the preview. The command checks the ownership record and both recorded process IDs before removing the exact matching profile. Incomplete records, reused or inaccessible PIDs, and already missing profile storage cause the command to stop. Recovery is explicit and does not scan other runs. [OpenProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openprocess)

On November 27, 2026, a follow-up process test confirmed that recovery refused an active lease, a live PID and a mismatched ownership token. After the test host and worker exited, the command deleted the test-owned profile with both original records present and verified that its storage was gone. Local report: `target/profile-recovery-validation/0d55edeef78a4a9595ae752bcca3e218/result.json`.

When a preview process starts its first sandbox worker, it checks up to 128 entries in `target/sandbox-runs/pending-profiles` under its working directory. Clean exits remove their markers. The same lease, ownership and liveness checks protect startup recovery. Earlier runs without index markers remain available to the explicit command if their records are complete. A version 2 record can also establish that the host stopped before starting worker creation. If launch intent is present and the worker PID is missing, the record stays in place for inspection.

On November 27, 2026, two separate preview launches verified that startup skipped the live test worker and recovered its orphaned profile on the next launch after forced host exit. The unrelated index marker remained in place. Local report: `target/auto-profile-validation/df0801fe60f245f6b79c0a9a1e163436/result.json`.

A separate run used a real test-owned AppContainer profile to verify the version 2 journal decisions. After both test processes exited, the test supplied a launch-intent-only record and observed that automatic recovery retained the profile. It then supplied the pre-launch record and verified deletion on the next startup. This simulates the journal phases. It does not reproduce a host crash during the Windows process creation call. Local report: `target/profile-phase-validation/c4c493fe5e514eed8fbe2950736ab2e6/result.json`.

With Job membership established at process creation, a November 27, 2026 follow-up again observed the live test worker terminate when its host was killed and its profile recover on the next launch. The healthy probe also verified zero capabilities, the Job limits and the baseline direct OS denials. This run exercised abrupt exit after worker launch. Local report: `target/atomic-job-validation/f34961a4221144e99165b2748af97691/result.json`.
