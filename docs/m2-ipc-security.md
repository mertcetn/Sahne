# M2 Control Channel Security

Date: November 12, 2026. Status: trusted Windows experiment after 32ace38; partial G2 evidence. No arbitrary content, package loading or production worker API is enabled.

## Run and expected result

```powershell
cargo run --release -p wallpaper-sandbox-probe --locked -- --case ipc-rights
```

The host must verify the actual pipe DACLs before launching the worker. The worker must complete Hello/Ping/Pong, receive access-denied error 5 for both DACL queries, both DACL mutation attempts, six handle-rights upgrade attempts and both wrong-direction data operations, report exactly 0x817, close its output and exit zero. Baseline sentinel read/write and host-process VM-access denials must also pass. No visual or physical user input is required.

## Creation and granted rights

Both anonymous pipes now receive an explicit protected DACL containing one non-inherited allow ACE for the current host user's SID, with FILE_ALL_ACCESS. There are no Everyone, anonymous, all-packages or profile-SID grants. A missing/null ACL or any creation/security error aborts launch; there is no default-DACL fallback. The unrelated endpoint fixture shares this ACL builder, with the same effective host-user-only policy.

After CreatePipe, GetSecurityInfo reads back both endpoints' actual security descriptors. The host requires a present protected DACL containing exactly one ACE whose bytes match the requested trustee, flags and specific rights. FILE_ALL_ACCESS is specified directly so generic-right mapping cannot change the expected ACE. These checks are part of every launch, not just ipc-rights. Descriptor/ACL storage remains owned through synchronous API use and is freed afterward.

The original creation handles are non-inheritable. DuplicateHandle creates replacement read handles with FILE_READ_DATA | SYNCHRONIZE and write handles with FILE_WRITE_DATA | SYNCHRONIZE, using explicit desired access and no DUPLICATE_SAME_ACCESS or source-close option. Original handles are then closed before process creation. Metadata, ACL-read/change, write-attributes and pipe-instance-creation rights are not requested for these replacement handles.

Parent endpoints have inheritance cleared; the existing explicit process handle list contains only worker stdin and worker stdout/stderr. No descriptor, token, Job, host-file or unrelated endpoint handle is added to that list. Host copies of worker endpoints are closed before resume. The reader/attempt owns its endpoints through stop/wait/join; replacement attempts receive fresh pipes and sessions. Inheriting the listed handles supplies the worker's permitted data path without granting new namespace access.

The host user is trusted in this experiment. This DACL does not isolate the host from other unsandboxed code running as that user or an administrator. It is not a protected-process guarantee or an audit of all Windows brokers. Anonymous-pipe names are assigned by Windows and are not exposed as application APIs; this fixture does not enumerate/reopen their internal names or test handle duplication into other processes.

## Direct worker checks

Native helper code first requires AppContainer execution. It borrows the actual stdin/stdout handles from the safe Rust application. DuplicateHandle attempts to regain READ_CONTROL, WRITE_DAC and FILE_CREATE_PIPE_INSTANCE separately on each handle must return ERROR_ACCESS_DENIED; any unexpected duplicate is closed without exercising that right and fails the case. On each handle, GetSecurityInfo must also return ERROR_ACCESS_DENIED, despite the same API succeeding on the original host handles. SetSecurityInfo must likewise return ERROR_ACCESS_DENIED. The attempted replacement is a valid DWORD-aligned, non-null empty protected DACL: even an unexpected success would restrict access and fail the case, rather than grant access. No null or permissive DACL is used as a negative test.

Owned same-access clones let File perform a one-byte write on stdin and a one-byte read on stdout, bypassing stdio buffering. Each must fail with error 5. Unexpected success or another error fails; a stalled operation is terminated by the independent host deadline. Normal framed output after these checks supplies the positive data-path control. No correct-direction bytes are consumed or injected by the negative operations. Failure diagnostics use a bounded Reject and never count as a successful result.

## Bounds and ownership

The existing versioned schema rejects bodies above 64 KiB before allocating/reading them, caps diagnostics at 256 UTF-8 bytes and checks session, sequence, revision and nonce. The host writes one small fixed Ping, closes its input and accepts one fixed response conversation. Handshake and response waits each have a three-second deadline; a missing handshake does not acquire a second wait. The ready/result channels each have one slot. The host requires clean EOF after the final result and rejects the first unsolicited trailing byte without draining a flood. The 4096-byte CreatePipe buffer argument is an OS sizing hint, not an exact queue-memory guarantee.

The worker remains in the unchanged one-process, 256 MiB, kill-on-close Job. Existing stall, malformed/flooded IPC and bounded recovery tests exercise cleanup and host responsiveness. This evidence covers the current one-shot conversation. Persistent valid-message rate limits, arbitrary source transport, scene transactions, write-side pressure under larger inputs and production IPC peer policy remain open. Broader G2 isolation, permitted system-read inventory and the inconclusive loopback test are unchanged. Untrusted execution remains disabled.

## Validation and dependencies

The final release regression passed all twenty-six expected-success scenarios, totaling thirty-two restricted worker launches across ordinary and recovery cases. The ipc-rights run `13820-fcc513cee19dc30e` reported 0x817, all twelve expected access denials, clean EOF and exit zero. Every launch also passed the actual DACL readback checks. The complete local transcript is retained under ignored target/sandbox-runs/ipc-rights-regression.log.

All nineteen Rust tests, formatting, Clippy with warnings denied and the workspace release build passed locally. A concurrent build initially encountered Windows' executable-file lock while the regression was running; rebuilding separately after process completion passed. Existing incremental-cache hardlink/copy notices remain. Local documentation links, CLI help and diff checks were also verified. Cargo manifests and lockfile are unchanged. No hosted CI, other Windows configuration or non-Windows target was tested. The loopback case remains inconclusive and is not among the expected-success cases. Changes remain uncommitted; no release or push was performed.

No Cargo package, version, feature or lockfile changes are required. All calls use already-enabled windows-sys 0.61.2 bindings and Rust standard-library handle/I/O APIs. Existing dependency/security assessments and unresolved advisories remain applicable; this change does not resolve them. Native FFI remains confined to wallpaper-sandbox; application unsafe rules, capabilities and resource limits are unchanged.

Primary references consulted November 12, 2026:

- [CreatePipe security, lifetime and buffer semantics](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createpipe)
- [Pipe DACLs and specific versus generic access rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)
- [DuplicateHandle desired access and inheritance](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-duplicatehandle)
- [GetSecurityInfo access requirements and descriptor ownership](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-getsecurityinfo)
- [SetSecurityInfo and null-DACL consequences](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setsecurityinfo)
- [GetAce storage](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-getace)
- [InitializeAcl alignment and empty ACLs](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-initializeacl)
- [Rust StdinLock borrowed Windows handles](https://doc.rust-lang.org/std/io/struct.StdinLock.html)
