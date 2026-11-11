# M2 Unrelated Endpoint Validation

Date: November 11, 2026. Status: trusted Windows fixture after 0d88774, partial G2 evidence. This adds a denial test, not a new worker service or permitted channel.

## Run and expected result

```powershell
cargo run --release -p wallpaper-sandbox-probe --locked -- --case endpoint
```

Run from the repository root. The case must print a successful host positive control, require access-denied error 5 for all four worker requests, report mask 0x417, observe worker exit zero, leave the sentinel unchanged, and confirm the pipe name is absent with error 2 after owner destruction. It must end with `Probe passed`. Missing/busy endpoints, other error codes and unexpected connections are failures.

## Construction and ownership

The host creates a fresh local named pipe whose name contains only its PID and random session ID. First-instance mode prevents silently adopting an existing object. A protected non-null DACL is constructed from exactly one explicit full-access ACE for the actual host process user SID, obtained with TOKEN_QUERY. No default pipe DACL, inherited ACE, Everyone, anonymous, package SID or broad application-package principal is granted access. A null ACL result is rejected. Remote clients are rejected by the pipe mode.

The host opens a duplex client to the same name as a positive control and keeps that connection/server instance open. A second instance at the same name, with the same DACL/mode and a maximum of two instances, remains available for the worker's attempts. This avoids treating a nonexistent or fully occupied endpoint as evidence of isolation. No ConnectNamedPipe wait, message read or message write is required: Windows permits a client connection between pipe creation and the server connect call. There are no overlapped operations or borrowed asynchronous buffers.

All three host handles are non-inheritable and owned by the native Sandbox guard. They are omitted from the existing explicit list of two child pipe endpoints. The only permissions applied are on these new temporary kernel objects; no existing file, registry, endpoint, firewall or system ACL is changed. Token/ACL/descriptor backing storage stays alive through synchronous creation, then kernel object security owns its copy. Native FFI remains in wallpaper-sandbox; the safe probe application gets no new unsafe exception.

## Worker and cleanup checks

After normal AppContainer identity/zero-capability verification and handshake, trusted worker code derives the fixed synthetic name from its host PID/session arguments. It makes four CreateFileW attempts: GENERIC_READ, GENERIC_WRITE, both, and zero desired access. Each must return exactly ERROR_ACCESS_DENIED (5). On unexpected success the worker immediately closes its new handle without reading or writing and reports a bounded failure. File-not-found, pipe-busy, timeout or an unrelated failure never counts as access denial. Existing allowed anonymous-pipe IPC must still carry its valid nonce/report/EOF.

The endpoint bit (10) is set only after all four direct OS calls were denied. The host requires exact mask 0x417 and normal process completion, stops/joins the attempt, and destroys its owner. It then probes only its own unique name with zero access: only ERROR_FILE_NOT_FOUND (2) confirms cleanup. A still-open, busy or inaccessible object fails that check. This confirms this named object disappeared; it does not independently audit all kernel handles or AppContainer profile deletion.

The observed denial is the combined Windows access/namespace behavior for this unrelated host pipe. It is not a captured attribution to one ACE or filter. AppContainer named-pipe namespace restrictions may also apply. Do not infer that every system broker, RPC service, pipe name, protocol, access mode or Windows configuration is inaccessible from this one fixture. The permitted production channel's ACL/rate/ownership review and broader G2 coverage remain open. No arbitrary packages/scripts are enabled.

## Evidence and dependencies

The complete case passed locally in release mode on the recorded Windows 11 Pro build 26200/MSVC machine. Initial full cleanup evidence: `20852-48f63c70dfd5408d`; host connection succeeded, worker mask 0x417/exit 0, four access-denied results and post-owner file-not-found result were observed. The final release regression repeated the complete result in `21188-28b6190d2699c34b`.

All twenty-five expected-success actual-process scenarios passed, totaling thirty-one restricted worker launches across individual and recovery fixtures. The full local transcript is retained under ignored target/sandbox-runs/endpoint-regression.log. All nineteen Rust tests, formatting, Clippy with warnings denied, workspace release build, help output, local documentation links and diff checks passed. Cargo manifests and lockfile are unchanged from 0d88774. Existing incremental-cache hardlink/copy notices were emitted. Hosted CI, other Windows configurations and non-Windows targets were not run; the loopback network case remains inconclusive. New endpoint/contract changes remain uncommitted and no publication was performed.

No Cargo package, version, feature or lockfile change is required. The new calls use already-enabled windows-sys 0.61.2 Security/Authorization, FileSystem and Pipes bindings. Their existing source/license/security assessment still applies; native pointer/handle lifetimes and null-DACL semantics were reviewed for this change. Existing underlying dependency advisories are not reclassified as resolved.

Primary references consulted November 11, 2026:

- [CreateNamedPipeW modes, instances and security](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew)
- [Named-pipe access checks and default DACL risks](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)
- [Client connection before ConnectNamedPipe and AppContainer namespace restrictions](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-connectnamedpipe)
- [ACL allocation and lifetime](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setentriesinaclw)
- [Absolute descriptor/DACL lifetime and null-DACL meaning](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-setsecuritydescriptordacl)
- [User token information](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-gettokeninformation)
- [CreateFileW access modes](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)
