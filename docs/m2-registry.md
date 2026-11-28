# M2 Synthetic Registry Access Matrix

Date: November 14, 2026. Status: trusted Windows continuation after 61385cc; partial G2 evidence. No existing user registry values are inspected or modified.

## Run and acceptance

```powershell
cargo run --release -p wallpaper-sandbox-probe --locked -- --case registry
```

The case requires successful host positive controls, ten direct worker access denials with error 5, exact report mask 0x8017, ordinary protocol/EOF and exit zero. After worker stop and both I/O joins, the host requires unchanged synthetic contents/namespace, closes the fixture handle, deletes the owned key and requires reopening it to fail with error 2. Profile deletion remains independently required. All checks are agent-executable without visual judgment or physical input.

The worker also opens its own private registry storage through `GetAppContainerRegistryLocation`, writes a fixed binary sentinel, reads it back with an exact-size buffer, and closes the handle before reporting success. This establishes a positive control for observing private registry storage. The host's existing profile-directory check verifies filesystem cleanup. Independently confirming the private registry hive's lifetime and unload remains open. [GetAppContainerRegistryLocation](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-getappcontainerregistrylocation)

On November 28, 2026, the expanded registry case passed in the rebuilt default Windows suite. All 50 cases and 56 profile cleanups passed. Formatting, Clippy with warnings denied and 184 Rust tests also passed. Local report: `target/sandbox-validation/936f56cc213a47df883edaa2d64d8c0f/summary.json`.

## Owned volatile fixture

The host opens the existing HKEY_CURRENT_USER/Software container in the 64-bit registry view and creates one unique Sahne.Probe.<host-pid>.<session> child with REG_OPTION_VOLATILE. The cryptographic session is already part of the native launch identity. RegCreateKeyEx must report REG_CREATED_NEW_KEY before the instance assumes deletion ownership; an existing-key disposition fails without changing/deleting that key.

The new key receives an explicit protected DACL containing only the actual host user's SID with KEY_ALL_ACCESS. No AppContainer/package grant or inheritable registry handle is added. The existing SID/ACL builder now takes object-specific rights internally; its file/pipe callers still use their unchanged FILE_ALL_ACCESS mask. Registry handles have their own RegCloseKey owner and never use Foundation CloseHandle.

The only stored value is the fixed original binary Sentinel. The host successfully requests every tested access right on that same live key. It also creates a fixed host-control child with the same DACL, writes a synthetic sentinel, closes it and successfully deletes it before worker launch. This proves the operations are available to the host and the target exists. No HKLM operation, existing value query, arbitrary key selection or parent security change is performed. The Software container receives only this temporary child entry.

## Direct worker operations

Trusted native code first requires actual AppContainer execution, then derives the fixed synthetic path from the existing host PID/session arguments. No new argument, registry handle or general script registry API is introduced. All operations select the same 64-bit view.

| Worker request | Required outcome |
| --- | --- |
| Open with KEY_QUERY_VALUE | Access denied, 5 |
| Open with KEY_SET_VALUE | Access denied, 5 |
| Open with KEY_CREATE_SUB_KEY | Access denied, 5 |
| Open with KEY_ENUMERATE_SUB_KEYS | Access denied, 5 |
| Open with DELETE | Access denied, 5 |
| Open with READ_CONTROL | Access denied, 5 |
| Open with WRITE_DAC | Access denied, 5 |
| Open with WRITE_OWNER | Access denied, 5 |
| Direct RegCreateKeyEx for worker-created under the fixture | Access denied, 5 |
| Direct RegDeleteKeyEx for the fixture | Access denied, 5 |

Eight rows check requested handle rights without modifying values or permissions. Direct creation/deletion additionally exercise operation-time checks; registry APIs can check a key's security descriptor separately from an earlier handle's requested mask. Unexpected successful handles are closed and fail the case. Missing/redirected paths, other errors and timeouts are not treated as access denial. Diagnostics use a bounded Reject. The observed result covers Windows' combined namespace/access behavior, not attribution to one ACE alone.

## Cleanup and limits

Before deletion the host reads only the fixed Sentinel into a fixed-size buffer, checks its type/length/bytes, and requires zero subkeys and exactly one value. Oversized/changed data fails without allocating an untrusted length. It closes the key before RegDeleteKeyEx, checks absence through the still-live Software parent, then closes that parent. Cleanup errors propagate before normal attempt success; an explicit registry cleanup marker is printed only after these checks pass.

Partial-construction/failure Drop cleanup is best-effort and targets only this instance's unique key and its two fixed child names. There is no recursive deletion, profile sweep or deletion of a pre-existing fixture. Forced host death can bypass Drop and leave a volatile key until hive unload; the outer runner must fail such a run and cannot certify registry/profile cleanup. Registry cleanup timing is separate from the existing classification/stop/I/O-join bound, like the synchronous profile phase.

This validates one fresh private host key, not all HKCU/HKLM keys, 32-bit aliases, registry links, AppContainer-private hive permissions, system brokers or deletion of AppContainer's own registry registrations. Those broader G2 audits remain open. Capabilities, CPU/memory/process limits and input validation are unchanged; untrusted content stays disabled.

## Runner and dependencies

The default runner now includes registry: thirty-one cases and thirty-seven expected launches/profile cleanups. SchemaVersion=2 adds RegistryCleanups/ExpectedRegistryCleanups per case and a Markdown column. Registry requires exactly one cleanup marker; other cases require zero. Runner self-tests reject otherwise complete evidence missing that registry marker. Historical schema-1 reports and thirty-case evidence remain scoped to their recorded revisions.

Only Win32_System_Registry is added to the existing exact windows-sys 0.61.2 feature set. Its inspected feature definition enables the already-selected Win32_System parent and introduces no package/version/transitive change. Cargo.lock is unchanged; the native graph remains windows-sys 0.61.2 -> windows-link 0.2.1. These Microsoft-generated MIT OR Apache-2.0 bindings use the existing Windows/MSVC target; new native FFI stays in wallpaper-sandbox. Current registry API documentation and signatures were reviewed. A fresh November 14 OSV query returned no advisories for those two versions; that limited result is not proof of safety or a new whole-workspace audit. Existing dependency advisories and QuickJS maintenance uncertainty remain unresolved.

## Validation

The initial release case passed with ten access denials, unchanged sentinel/namespace and post-delete error 2 in `14244-d1d53d7591adb6e5`. The final default runner repeated the complete registry result in `20936-5c6bd62506c77b4f`, with registry cleanup reported at 110.8 microseconds; this is an observation, not a timing guarantee.

The schema-2 report under ignored target/sandbox-validation/667432ed41e04eddb59d0f8e52b61fd8/ passed all thirty-one cases, thirty-seven worker/profile cleanups and exactly one registry cleanup. Counts/schema were independently checked; G2Complete remains false. Runner self-tests passed four helper processes, bounded-log reading and nine evidence checks, including missing-registry-cleanup rejection.

All twenty Rust tests, formatting, Clippy with warnings denied and the workspace release build passed. Existing incremental-cache hardlink/copy notices remain. CLI help, local documentation links, PowerShell syntax and diff checks were verified. Hosted CI, other Windows configurations, 32-bit registry views and non-Windows targets were not run. New changes remain uncommitted; no push or release was performed.

Primary documentation consulted November 14, 2026:

- [Registry key security and specific rights](https://learn.microsoft.com/en-us/windows/win32/sysinfo/registry-key-security-and-access-rights)
- [RegCreateKeyEx disposition, security and volatile storage](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regcreatekeyexw)
- [RegOpenKeyEx access and operation-time checks](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regopenkeyexw)
- [RegQueryInfoKey bounded metadata outputs](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regqueryinfokeyw)
- [RegDeleteKeyEx non-recursive deletion and last-handle behavior](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regdeletekeyexw)
- [Registry virtualization scope](https://learn.microsoft.com/en-us/windows/win32/sysinfo/registry-virtualization)
