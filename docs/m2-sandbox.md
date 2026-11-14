# M2 Windows Sandbox Spike

Date: November 11, 2026. Status: trusted Windows probe, partial G2 evidence. The native foundation below is now complemented by [fixed JavaScript fixtures](m2-js.md). No arbitrary script/package execution, production sandbox certification or Linux isolation is provided.

## Run and review

Run from the repository root on Windows:

```powershell
cargo build --release -p wallpaper-sandbox-probe --locked
.\target\release\wallpaper-sandbox-probe.exe --help
.\target\release\wallpaper-sandbox-probe.exe --case healthy
.\target\release\wallpaper-sandbox-probe.exe --case hang
.\target\release\wallpaper-sandbox-probe.exe --case oversized
.\target\release\wallpaper-sandbox-probe.exe --case crash
```

Each case must print `Probe passed` and exit zero. Healthy checks the restricted identity, sentinel read/write denial, host-process VM-access denial and a valid nonce response. Hang verifies a successful handshake before requiring the independent host deadline to stop a sleeping native worker. Oversized emits a u32::MAX frame prefix; only the specific codec length error satisfies the case. Crash exits the worker with code 42; the host must survive and observe that exact code. These are synthetic trusted fixtures, not arbitrary content.

The separate network investigation is deliberately stricter:

```powershell
.\target\release\wallpaper-sandbox-probe.exe --case network
```

It tries a TCP connection to a listener created by the host on 127.0.0.1 and an ephemeral port, after a verified host positive connection. Only explicit Windows access denial (5 or 10013) satisfies the native case. A timeout, refused connection or successful connection fails that case. On the current machine this returns a timeout with no raw OS error, so the native case still exits nonzero. A later [read-only WFP diagnostic](m2-loopback.md) independently confirmed a receiving-side AppContainer Loopback block for the local fixture; this supplemental evidence does not turn a bare timeout into a native pass or complete G2. No external network service or third-party wallpaper is contacted. Failure must not be worked around by granting network capabilities or a loopback exemption.

## Launch boundary

`apps/wallpaper-sandbox-probe` contains safe host/probe logic. `crates/wallpaper-sandbox` contains the Windows FFI and ownership guards; like the existing platform crate it explicitly permits narrowly documented unsafe blocks and denies unsafe operations outside such blocks. The ordinary CLI and preview still prohibit unsafe code. The non-Windows probe exits with a Windows-only explanation; no non-Windows build/runtime validation was performed.

For each launch the host generates nonzero session/nonce values with Windows BCryptGenRandom, creates a unique AppContainer profile, and creates a fresh `target/sandbox-runs/<pid>-<session>/` directory. Only its `bin/` subdirectory receives read/execute rights for that exact profile SID; existing ACEs are preserved. A copy of the current trusted probe executable is staged there. The source executable, repository directories, user files and broad ALL APPLICATION PACKAGES principal receive no new grants. A sentinel beside `bin/` receives no profile grant. File testing attempts a read and opening for write without truncation or modification; the host checks its contents afterward.

The worker starts suspended with SECURITY_CAPABILITIES and no capability entries. Before resuming it, the host assigns a Job and independently queries the process token for AppContainer identity, the exact profile SID and zero capabilities. Failure ends the launch; there is no unrestricted fallback. This is a standard AppContainer experiment, not LPAC. Windows library/system reads and AppContainer-owned storage remain possible under OS policy; this is not a claim of zero filesystem access.

The Job sets kill-on-close, one active process and a 256 MiB per-process committed-memory limit. No breakaway flag is enabled. The continuations below test a single over-limit commit and a suspended child-creation request; incremental exhaustion and JS allocator behavior remain untested. CPU limits, host-crash kill-on-close behavior and broader nested-Job compatibility have not yet been exercised. Configuration alone does not complete those tests.

The explicit inherited-handle list contains only the child ends of two anonymous pipes. Parent ends are non-inheritable. The worker receives no Job/process handle or source-file handle. Standard error shares the protocol output pipe, so unexpected diagnostics invalidate the stream rather than forming an unbounded diagnostic channel. One intentionally inheritable but excluded host file is now tested below; unrelated endpoints and broader handle classes remain pending. Concurrent host launches and inheritable-handle races have not been audited for production use.

The child receives a curated Unicode environment containing APPDATA, LOCALAPPDATA, SystemRoot and USERPROFILE location metadata, not the host's complete environment, PATH, proxy settings or tokens. With only SystemRoot the initial launch failed with Windows error 203; adding the OS location metadata enabled launch without changing ACL/capability policy. Windows may adjust these locations for the AppContainer. This observation is specific to this host environment.

## IPC and lifetime

The [bounded codec](m2-protocol.md) supplies exact wire parsing. Normally the host requires Hello(sequence 1), matching Pong(sequence 2) and ProbeResult(sequence 3), with its selected session and revision zero. The handles fixture adds the explicitly validated stage/final exchange described below. Host Ping uses sequence 1 in the other direction. ProbeResult is trusted test telemetry, not an authoritative security assertion from untrusted content.

One reader thread and one-slot result/handshake queues serve one worker. Handshake and subsequent response each have a three-second deadline; a failed Hello no longer waits for a second response deadline. On failure or completion the host terminates the Job and waits at most one second before joining the pipe reader; child pipe copies in the parent are closed before resume so worker termination releases the reader. The fixture writes one small Ping. A later continuation adds required EOF and trailing-message/partial-frame cases; see [IPC fault validation](m2-ipc-validation.md). Explicit [restart fixtures](m2-recovery.md) now test bounded recovery; ordinary cases still run once. Blocked large host writes, sustained valid-message pressure and production backpressure remain open.

Owned handles, process guards and profile guards attempt cleanup on all normal error paths. Profile deletion failures are reported. Successful runs printed no cleanup errors; independent registry/profile enumeration and abrupt host-death cleanup remain untested. Test directories and executable copies are deliberately retained under ignored `target/` for review; no recursive cleanup command is run. No third-party samples are executed or copied into the probe.

## Recorded local evidence

Windows 11 Pro build 26200, pinned Rust 1.99.1/MSVC; same machine as [M1 evidence](m1-lifecycle-validation.md). Final release build tested November 11, 2026:

| Case | Result | Evidence |
| --- | --- | --- |
| Healthy | Pass | Host verified token/profile/zero capabilities; OS error 5 for sentinel read/write and host VM-open; mask 0x17; worker exit 0; sentinel unchanged |
| Native stall | Pass | Valid Hello, three-second response deadline, Job termination 1.6955 ms after detection; host survives |
| Oversized prefix | Pass | u32::MAX rejected before payload allocation/read; Job stopped; host survives |
| Controlled crash | Pass | Worker exit 42 observed; host survives |
| Loopback TCP | Inconclusive/fails acceptance | Two-second connect timeout without raw access-denied error; no network permission added |

Representative retained runs end in `7104-8427d6da631d096b`, `22876-2d6e3afc44cdbd08`, `25952-fb36f56b03b78ec3` and `8784-d2d4e38f3b5216ff`. These single-machine measurements are not latency guarantees. G2 remains open, including full network coverage, unrelated handles/endpoints, process creation, incremental/JS memory exhaustion, IPC floods and every JS-engine test. No untrusted input is enabled by this task.

Workspace release build, all eleven tests (nine protocol and two preview lifecycle), formatting and Clippy with warnings denied passed. Local documentation links resolve and git diff --check passed. Cargo emitted the previously recorded incremental-cache hardlink/copy notices. Help, invalid-case exit 1 and unrestricted worker refusal exit 2 were also checked. Real AppContainer tests required execution outside the coding tool's own sandbox; no application policy was relaxed. Hosted CI and non-Windows targets were not run for this change. Source awaits user testing/approval before commit.

## User validation of the initial spike

The user confirmed both requested release-mode checks before authorizing the commit: healthy passed with mask 0x17 (`28452-8e037f109ed494b0`), and hang passed with termination 1.489 ms after detection (`12528-565dd177576c6aaa`). These results validate the requested initial cases only; the network and remaining G2 items stay open.

This initial spike and user evidence were committed in 49c0a3f. The continuation below is a new change awaiting user testing and approval.

## Dependency and source review

No registry package or version was added or changed; all 227 external lockfile package blocks match the approved commit. The two new workspace packages reuse exact windows-sys 0.61.2, with additional Security/Authorization/Isolation/Cryptography, FileSystem, Threading/JobObjects/Pipes API features. Cargo.lock adds only local package entries. Microsoft maintains these generated Windows bindings under MIT OR Apache-2.0; the new feature surface was compiled on Windows x64. Native pointer/handle lifetime correctness is our responsibility, not a property guaranteed by the bindings.

The same-day complete registry-version advisory assessment in [M1 dependency review](m1-dependencies.md) remains applicable to the unchanged external graph, including its unresolved lru and ttf-parser risks. No absence-of-advisories safety claim is made. JS-engine dependencies require a separate current review before adoption. The upstream Microsoft repository and security reporting policy were consulted again for this task.

Primary documentation consulted November 11, 2026:

- [Implementing an AppContainer](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer)
- [AppContainer isolation and default access](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation)
- [Process attributes and explicit inherited handles](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute)
- [CreateProcessW and environment blocks](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw)
- [GetTokenInformation](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-gettokeninformation)
- [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
- [Microsoft Rust for Windows](https://github.com/microsoft/windows-rs)
- [Microsoft security reporting policy](https://github.com/microsoft/windows-rs/security/policy)

## Continuation: outbound TCP and memory limit

The existing `network` loopback case still requires explicit denial and still fails on this machine with a timeout. Microsoft's current troubleshooting documentation describes default packet drops for AppContainer traffic and distinct loopback handling. Primary research by Project Zero explains how receiving-side loopback filtering can cause timeout rather than immediate access denial. This is consistent with our observation, not a captured-filter diagnosis of this run. No firewall setting, network capability or loopback exemption was changed.

Two additional independent cases are available:

```powershell
cargo run --release -p wallpaper-sandbox-probe --locked -- --case network-egress
cargo run --release -p wallpaper-sandbox-probe --locked -- --case memory
```

Both must print `Probe passed`, exit zero and leave the sentinel unchanged. Network-egress tries IPv4 TCP to 192.0.2.1:9 (RFC 5737 TEST-NET-1) with a two-second bound, no DNS and no application payload. Only error 5 or 10013 satisfies the trusted probe. A timeout/routing failure or an unexpectedly successful connection fails acceptance. This is one synthetic outbound destination/protocol, not a real Internet-service connectivity test or IPv6/UDP/DNS certification. The host does not run an unrestricted positive connection to this reserved destination; results cannot distinguish the exact AppContainer/WFP rule responsible for denial.

Memory queries the worker's associated Job and requires the process-memory flag and exact 256 MiB limit. It successfully reserves/commits and releases 64 KiB as a positive control, then attempts one 257 MiB reserve/commit. Only Windows memory errors 8 or 1455 satisfy the probe; a successful oversized allocation is immediately released and fails acceptance. No pages are touched, no executable memory is requested and no growing allocation loop runs. A normal Pong/result and worker exit zero are required afterward. Memory errors can also reflect overall system commit pressure; this single test, combined with the Job query and small-allocation control, is limited evidence and not a comprehensive exhaustion stress test.

| Case | Local release result, November 11, 2026 | Retained run |
| --- | --- | --- |
| network-egress | Pass; explicit Windows TCP access denial, normal response and worker exit 0 | `14548-b850c5a85ba1dad5` |
| memory | Pass; Job query, small commit succeeds, over-limit commit fails, mask 0x37 and worker exit 0 | `28580-1386f29e4fefdf5c` |

Final workspace release verification repeated both new cases successfully (`7804-0c07f5ac51ca024c`, `17296-81158e5b79211a51`). The original loopback case still reported TimedOut/no raw OS error and exited 1; its acceptance was not relaxed. All eleven workspace tests, release build, formatting, Clippy, documentation-link and diff checks passed. Cargo emitted the existing incremental-cache hardlink/copy notices. Hosted CI and non-Windows execution were not run. These continuation changes remain uncommitted pending user testing/approval.

The user subsequently confirmed that both network-egress and memory worked and explicitly authorized committing this continuation, recorded in 3b9f93f. No additional timing, error-code or platform evidence is inferred from that confirmation.

The memory helper adds only windows-sys's System_Memory binding feature; external package versions and lockfile remain unchanged. Existing source/license/security assessment applies; the native helper's pointer ownership and allocation release were reviewed against current VirtualAlloc/VirtualFree documentation. Resource limits and AppContainer policy are unchanged. Unrelated handles, child-process attempts, aggregate/incremental memory exhaustion, CPU budgets, protocol floods, restart behavior and JS remain future work.

Primary references consulted November 11, 2026:

- [Microsoft: troubleshooting AppContainer network drops](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/windows-firewall/troubleshooting-uwp-firewall)
- [Project Zero: original AppContainer network research](https://projectzero.google/2021/08/understanding-network-access-windows-app.html)
- [RFC 5737 reserved IPv4 documentation ranges](https://www.rfc-editor.org/rfc/rfc5737)
- [VirtualAlloc commit versus physical pages](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualalloc)
- [VirtualFree ownership and release rules](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualfree)
- [Job committed-memory limit semantics](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_extended_limit_information)
- [Querying the calling process's Job](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-queryinformationjobobject)

## Continuation: process quota and excluded handles

Later cumulative-commit validation complements the original single-allocation memory case: see [Incremental memory pressure, release and host accounting](m2-incremental-memory.md). Its fixed loop does not change the original case or the configured Job limit.

Run these new cases from the repository root on Windows:

```powershell
cargo run --release -p wallpaper-sandbox-probe --locked -- --case process
cargo run --release -p wallpaper-sandbox-probe --locked -- --case handles
```

Both must exit zero and print `Probe passed`. These changes await user testing/approval; they are not part of 3b9f93f.

Process queries the worker's actual Job and requires an active-process limit of exactly one. It calls CreateProcessW for its own staged trusted executable with CREATE_SUSPENDED, no inherited handles and no PATH search. Only ERROR_NOT_ENOUGH_QUOTA (1816) satisfies the case; access denied or an image/environment error does not. Unexpected success fails the case and an ownership guard terminates/waits for the suspended child without resuming it. A normal Pong, mask 0x57, worker exit zero and unchanged sentinel are then required. This tests this child-creation path, not brokered launches, every creation flag or every process API.

Handles opens the host's synthetic sentinel for read and marks that host-only file handle inheritable. The host verifies the inherit flag and keeps the file open through worker termination. It deliberately omits the handle from the existing two-pipe inheritance list and passes only its numeric value as a trusted test argument. No ACL or list permission is added for the worker. The worker attempts GetHandleInformation on that value without owning or closing it. A numeric handle collision or a successful query fails acceptance; the fixture does not guess another value until it passes.

Windows can apply strict invalid-handle checks, causing the attempted query to terminate the worker instead of returning error 6. The initial fixture exposed this as missing output/timeout; the observed exit was 0xc0000008. The host now queries the actual suspended worker's strict-handle mitigation before resume. The worker sends its successful baseline Pong and an exact armed ProbeResult (0x117, sequence 3), then flushes before the query. Only clean EOF after this stage, independently verified strict policy, and exactly 0xc0000008 satisfy the fatal path. If the API returns error 6 normally, a final mask 0x97 at sequence 4 and worker exit zero are required instead. Other errors, partial frames, unarmed EOF, mismatched context and timeouts remain failures. The fatal outcome is expected containment evidence for this trusted fixture; it does not turn arbitrary worker crashes into valid replies.

Only the intentional invalid-reference worker uses SetErrorMode to add SEM_NOGPFAULTERRORBOX while preserving previous flags. This suppresses Windows Error Reporting for that intentional fault and removes its observed delay; no host/system-wide error-reporting settings change. Strict-handle mitigation, AppContainer capabilities, Job limits and the three-second host deadlines stay unchanged. No third-party package is executed and no exception handler bypasses the mitigation.

| Case | Local release result, November 11, 2026 | Retained run |
| --- | --- | --- |
| process | Pass; quota error 1816, mask 0x57, worker exit 0 | `8420-59bf3578f335e637` |
| handles | Pass; host queried strict policy enabled, validated armed stage, worker exit 0xc0000008, host survives | `25852-bb1a67368b124798` |

Three host-reader regressions check that EOF before arming is rejected, only clean EOF after the exact armed stage becomes a candidate, truncated/rejected frames stay errors, and stage session/sequence/revision and Pong nonce must match. These tests do not replace real OS-policy/exit checks.

Final continuation validation passed the workspace release build, all fourteen tests (nine codec, two preview lifecycle, three host-reader), formatting, Clippy with warnings denied, documentation links and diff checks. The unchanged lockfile was verified against 3b9f93f. All eight expected-success native cases passed on the final binary: healthy, network-egress, memory, process, handles, hang, oversized and crash. Final process/handles runs were `25148-eae6893d8cd7d50e` and `10308-96a8cb8b35cc6e33`; hang termination measured 2.8905 ms after detection. The original loopback investigation remains open and was not reclassified. Existing incremental-cache hardlink/copy notices were emitted; hosted CI, non-Windows targets and broader Windows configurations were not run. New source remains uncommitted pending user testing/approval.

The user subsequently instructed the agent to verify executable tests autonomously rather than request duplicate user runs, and authorized committing this validated continuation. This is approval based on the recorded local evidence, not additional user-run hardware evidence.

The approved process/handle continuation and updated working rule were committed in 9d53b81. The next [IPC fault-validation change](m2-ipc-validation.md) requires EOF after the final report and adds flood, partial-frame, replay and open-stream fixtures. Its results do not close the full G2 gate.

Only SystemServices and Diagnostics_Debug features of the existing exact windows-sys 0.61.2 are added for the mitigation structure and process-local error-mode calls. Registry packages, versions, license/provenance assessment and lockfile remain unchanged. Native pointer/handle ownership and API preconditions were reviewed against the current primary references below. This fixture temporarily owns an inheritable host handle; production launch concurrency/races still require review. Network coverage, unrelated IPC endpoints, comprehensive handle/process escape attempts, CPU limits, floods, restart behavior and JS-engine gates remain open.

Primary references consulted November 11, 2026:

- [Active-process Job limits](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_limit_information)
- [CreateProcessW suspension, inheritance and executable selection](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw)
- [Explicit inherited-handle list](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute)
- [GetHandleInformation](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-gethandleinformation)
- [Strict invalid-handle mitigation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocessmitigationpolicy)
- [Strict-handle policy structure](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-process_mitigation_strict_handle_check_policy)
- [Process-local error mode and WER](https://learn.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-seterrormode)

## Continuation: control-channel permissions

On November 12, 2026, the actual anonymous-pipe transport was hardened with explicit protected host-user-only DACLs, per-launch readback checks and data-only inherited handles. The ipc-rights case exercises ACL read/change denial, handle-rights upgrade denial and reverse-direction I/O denial on the actual channel. Current policy, ownership/queue bounds, commands and results are recorded in [Control channel security](m2-ipc-security.md). Historical evidence above remains scoped to its recorded revision; broader G2 gaps are not closed by this change.
