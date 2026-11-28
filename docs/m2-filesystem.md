# M2 Filesystem Access Matrix

Date: November 12, 2026. Status: trusted Windows experiment after a149196; partial G2 evidence. No package or user-provided script is executed.

## Command and acceptance

```powershell
cargo run --release -p wallpaper-sandbox-probe --locked -- --case filesystem
```

Run from the repository root. Host positive controls must succeed before worker launch. The worker must complete the permitted-read controls, receive error 5 for all sixteen negative operations, send exact mask 0x1017, finish the normal protocol/EOF and exit zero. The host stops/joins the attempt, then independently checks fixture contents and directory entries. Failure, missing paths, sharing violations, timeouts and other errors do not count as access denial. These tests require no visual judgment or physical interaction.

## Synthetic fixtures and host controls

The existing unique target/sandbox-runs directory receives private/secret.txt and bin/readable.txt with fixed original test contents. The private directory and its file each receive an explicit protected DACL granting FILE_ALL_ACCESS only to the host user. No null DACL, all-packages grant or profile grant is added there. Both objects are freshly created by this run; source files, user documents, existing directories and OS ACLs are untouched.

The public text file inherits the existing exact-profile read/execute staging grant. The native launcher still copies only its own trusted executable to bin/probe.exe. Before launch, the host successfully opens the same eleven file-right requests that the worker must be denied, enumerates the private directory, and creates/removes a separate fresh control file and empty directory in each tested directory. This excludes absent targets or generally unusable directories as explanations for worker denial. All handles close before launch, and native file opens allow read/write/delete sharing.

The fixture does not strengthen the sandbox by modifying user/system permissions. Private synthetic objects model host-owned private data; the read-only staging objects exercise the existing executable-directory grant. Testing private data here does not involve reading real user secrets.

## Direct worker matrix

The native helper requires AppContainer execution. It derives fixed child paths from the already supplied trusted fixture root; there is no new script filesystem API or configurable input path.

| Target | Worker operation | Required result |
| --- | --- | --- |
| private/secret.txt | Open for FILE_READ_DATA | Access denied, 5 |
| private/secret.txt | Open separately for FILE_WRITE_DATA, FILE_APPEND_DATA, DELETE, WRITE_DAC and WRITE_OWNER | Five access denials, 5 |
| bin/readable.txt | Open separately for those same five modifying rights | Five access denials, 5 |
| private/ | Enumerate entries | Access denied, 5 |
| private/ and bin/ | Create a new worker-created.txt in each | Two access denials, 5 |
| private/ and bin/ | Create a new worker-created-dir in each | Two access denials, 5 |

The eleven rights checks request OPEN_EXISTING without truncation, delete-on-close or actual data/ACL mutations. DELETE, WRITE_DAC and WRITE_OWNER checks prove denial of those handle-access requests; they do not claim every delete/rename/ownership API or path alias was exercised. Unexpected handles are immediately owned/closed and fail the case. Creation checks use new fixed names. No unexpected object is executed, recursively removed or treated as successful isolation.

After the worker is stopped, the host checks both fixed text files byte for byte, requires the staged executable to retain its MZ header, and requires exactly the expected private/staging entries. This checks the synthetic namespace and data; it is not a full executable digest or system-wide filesystem audit. Existing sentinel-integrity checks also remain active.

## Observed permitted-read inventory

The same worker must perform these positive checks before it can report success:

| Path | Observation required | Scope |
| --- | --- | --- |
| bin/readable.txt | Read exact fixed contents | Exact-profile staging read grant works |
| bin/probe.exe | Open and read the two-byte MZ header | The staged executable can be read as data |
| bin/ | Enumerate both expected entries | Staging names are visible to the worker |
| SystemRoot/System32/kernel32.dll | Open and read the two-byte MZ header | A selected public Windows system file can be read |
| SystemRoot/System32/KernelBase.dll | Open and read the two-byte MZ header | A second Windows API library can be read |
| SystemRoot/System32/ntdll.dll | Open and read the two-byte MZ header | A Windows native API library can be read |

SystemRoot comes from the existing curated child environment. The system-file check never loads a new library or modifies system data. These are selected observations, not exhaustive tracing of executable imports, loaded DLLs, fonts, registry, all system directories or AppContainer-private storage. A standard AppContainer is not a filesystem with only two readable files. This experiment must not be described as blanket filesystem invisibility or a fully read-only private profile.

## Validation and remaining work

On November 28, 2026, the Windows probe confirmed the three System32 header reads, all sixteen synthetic denials and unchanged synthetic fixtures after worker exit. The rebuilt default suite passed all 50 cases with 56 profile cleanups. Local report: `target/sandbox-validation/8d26130427d94497a9e90515ed7718f2/summary.json`.

A separate administrator-approved WPR `FileIO` capture ran the same fixture successfully and saved a local ETL under `target/wpr-validation/c5d99545adab46ba8f5d0e1036118a9d/`. The worker PID came from the run's ownership record. An elevated event query selected 672 records for that PID. The query did not resolve usable file paths from the classic records, so these results establish capture and PID filtering only. The three direct DLL reads remain the verified positive controls. The raw ETL may contain unrelated machine activity. File object and path correlation, and broker activity attributed to other processes, remain open. [WPR command-line reference](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/wpr-command-line-options)

A second analysis reused the same ETL with Windows `tracerpt` and streamed its XML export, selecting 660 event envelopes with the worker's PID. This count is specific to the XML export. For each of the three direct DLL reads, the preceding open identified the path by file object, and the read and immediately following completion shared the same request pointer. Each completion returned zero status and two bytes. The local `correlated-system-reads.json` records those three results. The temporary system-wide XML export was removed. [File open event](https://learn.microsoft.com/en-us/windows/win32/etw/fileio-create) · [Operation completion event](https://learn.microsoft.com/en-us/windows/win32/etw/fileio-opend)

The same export showed image-load events for 16 distinct System32 libraries under the worker PID: `advapi32.dll`, `apphelp.dll`, `bcrypt.dll`, `bcryptprimitives.dll`, `combase.dll`, `kernel.appcore.dll`, `kernel32.dll`, `KernelBase.dll`, `msvcrt.dll`, `ntdll.dll`, `rpcrt4.dll`, `sechost.dll`, `ucrtbase.dll`, `userenv.dll`, `vcruntime140.dll` and `ws2_32.dll`. These are observed image loads for one native fixture on one Windows installation. The broader file-read inventory, attribution of work performed by brokers in other processes, and integrated package worker coverage remain open.

Correlating the same worker's open requests with their immediately following completions by request pointer also corroborated all 16 synthetic denials. Windows returned `STATUS_ACCESS_DENIED` for six opens of the private file, five opens of the staged file, one open of the private directory for listing, and four creation requests. The counts and paths match the fixture matrix. The probe validates the individual requested access rights. The local `correlated-fixture-denials.json` contains only fixture-relative paths and completion status. [Windows NTSTATUS values](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-erref/596a1078-e883-4972-9bbc-49e60bebca55)

The headless `--check-isolated-script-live` fixture also ran successfully from a fresh working directory. It verified pause, resume, two updates and both AppContainer profile cleanups. The two ownership records under `target/sandbox-runs` in that directory identified distinct intake and execution worker PIDs under the same preview PID. The staged executable paths in each run and those PIDs provide attribution controls for a future trace. This baseline run collected no ETW data. Its local logs and ownership records are under `target/package-trace-validation/5c3601e37f174cfc87ce548ded1d221f/`.

The initial release run passed with mask 0x1017 and unchanged post-stop fixtures in `6452-276a561563399a66`. The final release regression repeated that complete result in `33268-8cf70295a27aceec`. All twenty-seven expected-success scenarios passed, totaling thirty-three restricted worker launches including recovery fixtures. The complete transcript is retained under ignored target/sandbox-runs/filesystem-regression.log. The inconclusive loopback-network case is deliberately excluded from that expected-success set.

All nineteen Rust tests, formatting, Clippy with warnings denied, the workspace release build, CLI help, local documentation links and diff checks passed. Existing incremental-cache hardlink/copy notices remain. Hosted CI, other Windows configurations and non-Windows targets were not tested. The preceding IPC hardening was committed with user authorization in a149196; the filesystem continuation remains uncommitted. No push or release was performed.

The process, Job, profile, capabilities, IPC handle list, limits and watchdog policy are unchanged. Filesystem failures produce a bounded Reject instead of a success bit. No new dependency, Cargo feature or lockfile change is needed; already-enabled windows-sys 0.61.2 FileSystem/Security bindings and Rust standard-library I/O provide the operations. Previous dependency advisories and review limitations remain unresolved.

Broader G2 remains open: reparse/hard-link and path-alias cases, registry/broker/system access, complete permitted-read inventory, private-profile storage/cleanup audit, alternate Windows configurations, persistent channel pressure and the inconclusive loopback-network test. Current passing fixtures do not enable untrusted content or certify a production AppContainer/LPAC policy.

Primary documentation consulted November 12, 2026:

- [Windows file security, inheritance and specific rights](https://learn.microsoft.com/en-us/windows/win32/fileio/file-security-and-access-rights)
- [CreateFileW desired access, sharing and creation semantics](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)
- [SetNamedSecurityInfoW protected DACL application](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setnamedsecurityinfow)
- [AppContainer file isolation and its scope](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation)

References consulted on November 28, 2026 for the additional DLL observations are [default system resource access in a regular AppContainer](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer) and [the Windows PE file format](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format).
