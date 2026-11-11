# M2 Windows Sandbox Spike

Date: November 11, 2026. Status: trusted native probe only, partial G2 evidence. No JavaScript engine, package execution, production sandbox certification or Linux isolation is provided.

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

It tries a TCP connection to a listener created by the host on 127.0.0.1 and an ephemeral port. Only explicit Windows access denial (5 or 10013) satisfies the case. A timeout, refused connection or successful connection fails the case. On the current machine this returns a timeout with no raw OS error, so the case exits nonzero and network isolation remains unverified. No external network service or third-party wallpaper is contacted. Failure must not be worked around by granting network capabilities or a loopback exemption.

## Launch boundary

`apps/wallpaper-sandbox-probe` contains safe host/probe logic. `crates/wallpaper-sandbox` contains the Windows FFI and ownership guards; like the existing platform crate it explicitly permits narrowly documented unsafe blocks and denies unsafe operations outside such blocks. The ordinary CLI and preview still prohibit unsafe code. The non-Windows probe exits with a Windows-only explanation; no non-Windows build/runtime validation was performed.

For each launch the host generates nonzero session/nonce values with Windows BCryptGenRandom, creates a unique AppContainer profile, and creates a fresh `target/sandbox-runs/<pid>-<session>/` directory. Only its `bin/` subdirectory receives read/execute rights for that exact profile SID; existing ACEs are preserved. A copy of the current trusted probe executable is staged there. The source executable, repository directories, user files and broad ALL APPLICATION PACKAGES principal receive no new grants. A sentinel beside `bin/` receives no profile grant. File testing attempts a read and opening for write without truncation or modification; the host checks its contents afterward.

The worker starts suspended with SECURITY_CAPABILITIES and no capability entries. Before resuming it, the host assigns a Job and independently queries the process token for AppContainer identity, the exact profile SID and zero capabilities. Failure ends the launch; there is no unrestricted fallback. This is a standard AppContainer experiment, not LPAC. Windows library/system reads and AppContainer-owned storage remain possible under OS policy; this is not a claim of zero filesystem access.

The Job sets kill-on-close, one active process and a 256 MiB per-process committed-memory limit. No breakaway flag is enabled. Memory exhaustion, attempted child creation, CPU limits, host-crash kill-on-close behavior and nested-Job compatibility have not yet been exercised. Configuration alone does not complete those tests.

The explicit inherited-handle list contains only the child ends of two anonymous pipes. Parent ends are non-inheritable. The worker receives no Job/process handle or source-file handle. Standard error shares the protocol output pipe, so unexpected diagnostics invalidate the stream rather than forming an unbounded diagnostic channel. Direct unrelated-handle/endpoint tests are still pending. Concurrent host launches and inheritable-handle races have not been audited for production use.

The child receives a curated Unicode environment containing APPDATA, LOCALAPPDATA, SystemRoot and USERPROFILE location metadata, not the host's complete environment, PATH, proxy settings or tokens. With only SystemRoot the initial launch failed with Windows error 203; adding the OS location metadata enabled launch without changing ACL/capability policy. Windows may adjust these locations for the AppContainer. This observation is specific to this host environment.

## IPC and lifetime

The [bounded codec](m2-protocol.md) supplies exact wire parsing. The host requires Hello(sequence 1), matching Pong(sequence 2) and ProbeResult(sequence 3), with its selected session and revision zero. Host Ping uses sequence 1 in the other direction. ProbeResult is trusted test telemetry, not an authoritative security assertion from untrusted content.

One reader thread and one-slot result/handshake queues serve one worker. Handshake and subsequent response each have a three-second deadline. On failure or completion the host terminates the Job and waits at most one second before joining the pipe reader; child pipe copies in the parent are closed before resume so worker termination releases the reader. The fixture writes one small Ping; blocked large writes, stream floods, trailing-frame floods, restart policy and production backpressure are not covered. There is no automatic restart.

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

Representative retained runs end in `7104-8427d6da631d096b`, `22876-2d6e3afc44cdbd08`, `25952-fb36f56b03b78ec3` and `8784-d2d4e38f3b5216ff`. These single-machine measurements are not latency guarantees. G2 remains open, including network/internet, unrelated handles/endpoints, process creation, memory exhaustion, IPC floods and every JS-engine test. No untrusted input is enabled by this task.

Workspace release build, all eleven tests (nine protocol and two preview lifecycle), formatting and Clippy with warnings denied passed. Local documentation links resolve and git diff --check passed. Cargo emitted the previously recorded incremental-cache hardlink/copy notices. Help, invalid-case exit 1 and unrestricted worker refusal exit 2 were also checked. Real AppContainer tests required execution outside the coding tool's own sandbox; no application policy was relaxed. Hosted CI and non-Windows targets were not run for this change. Source awaits user testing/approval before commit.

## Dependency and source review

### User validation of the initial spike

The user confirmed both requested release-mode checks before authorizing the commit: healthy passed with mask 0x17 (`28452-8e037f109ed494b0`), and hang passed with termination 1.489 ms after detection (`12528-565dd177576c6aaa`). These results validate the requested initial cases only; the network and remaining G2 items stay open.

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
