# M2 Profile Storage Cleanup

## Integrated package registry mapping check

An approved Windows Performance Recorder Registry capture covered the headless `--check-isolated-script-live` fixture. The test passed and reported successful filesystem cleanup for both AppContainer profiles. The analysis derived each AppContainer SID from the profile name in its ownership record and found three successful registry opens of each exact per-user AppContainer mapping by the preview process.

After the test exited, a read-only check opened the same user's mapping parent as a positive control and confirmed that neither test SID's mapping remained. This verifies the two mapping keys for a normal package run. The trace also showed profile-linked registry value operations in other processes. Attribution of those processes, separate profile registry storage and hive lifetime, and the abrupt-exit path remain open. The local `mapping-readback.json` and `profile-registry-summary.json` are under `target/package-trace-validation/65dd9ea0db7a481ebcb254e28a4eccfe/`. The original ETL remains local and the temporary system-wide XML export was removed. [Registry ETW event fields](https://learn.microsoft.com/en-us/windows/win32/etw/registry-typegroup1) · [DeleteAppContainerProfile](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-deleteappcontainerprofile)

A separate headless test observed the execution profile's exact per-user mapping while its preview host was alive. It terminated only the preview process it had started, verified that the orphan's ownership record contained the recorded worker PID, and confirmed that the same mapping still existed. The explicit `--recover-preview-profile` command then succeeded and a new read-only open confirmed the mapping was absent. This covers the registry mapping for explicit orphan recovery after an abrupt host exit. Local report: `target/registry-recovery-validation/438ede7375924a0b939bbfa07aa8c367/result.json`.

Date: November 12, 2026. Status: trusted Windows continuation after 544fa56; partial G2 evidence. This strengthens host cleanup acceptance without granting worker access.

## Run and acceptance

Use existing cases; no new worker selector or wire message is needed:

```powershell
cargo run --release -p wallpaper-sandbox-probe --locked -- --case healthy
cargo run --release -p wallpaper-sandbox-probe --locked -- --case hang
cargo run --release -p wallpaper-sandbox-probe --locked -- --case restart-exhaust
```

Each completed attempt must print `Profile cleanup verified` after worker stop/wait and reader join. Recovery verifies cleanup before admitting another launch. A failed deletion, surviving directory or unexpected filesystem error fails the attempt and prohibits replacement.

## Positive controls and ownership

After creating its unique profile, the native Profile owner converts its actual SID to a string and queries GetAppContainerFolderPath. It copies the returned UTF-16 path with a bounded length check and frees the API allocation with CoTaskMemFree. The SID string uses LocalFree; the original SID retains FreeSid ownership.

On this machine the API returns the profile's AC local-data child. The host accepts either the exact profile root or that immediate AC child, requiring an absolute root whose final component matches the generated unique profile name. It does not search ancestors or substitute shared local-data storage. Both returned directory and root must exist as directories and must not be symbolic links. An unrecognized layout/lookup failure aborts launch.

The host creates a new fixed sahne-cleanup-control.txt in that owned local-data directory, writes original contents, closes it and reads it back before launch. This makes the deletion control nonempty. No prior profile or user data is enumerated or removed.

## Explicit deletion and failure handling

Previously deletion ran only from Drop and logged an HRESULT failure. Normal finish now stops/waits the worker, joins the reader, then calls DeleteAppContainerProfile for the owned unique name and propagates failure. After successful API return, symlink_metadata on the previously observed root must report NotFound. An existing object, access denial or another error fails. Only successful API deletion plus observed directory absence mark cleanup complete; subsequent calls are idempotent.

Microsoft documents that open storage handles can prevent complete removal and that deleting a nonexistent profile also returns success. The live pre-launch content/directory control and post-delete absence check supply separate evidence. The implementation uses the OS profile-deletion API, with no recursive filesystem-removal workaround.

Recovery finishes this check before leaving the old owner scope or advancing its policy. Drop still attempts best-effort deletion after partial construction, early launch failure or failed explicit cleanup, and always frees the owned SID. These fallback paths do not independently certify directory absence. No prior-run sweep or deletion retry loop is added.

## Timing and limits

Existing response-classification/Job-stop/reader-join acceptance retains its observational one-second bound. Its returned duration excludes profile deletion. Profile-cleanup duration is printed separately and includes an additional already-stopped worker check. Synchronous OS creation/deletion/path calls have no new hard wall-clock deadline; this is not bounded end-to-end recovery.

The audit proves deletion API success and absence of the selected profile directory tree, including its synthetic file. It does not independently enumerate registry registrations/hives, all handles, other worker-written locations, private-profile permissions or abnormal host death. Wider G2/production policy remains open. Ignored target/sandbox-runs artifacts remain deliberately retained; they are separate from AppContainer storage.

## Dependency and security review

Only Win32_System_Com is added to the existing exact windows-sys 0.61.2 dependency, for the allocator required by GetAppContainerFolderPath. This generated FFI binding gate does not initialize/activate COM or grant capabilities. In the inspected source its feature enables only the existing Win32_System parent, adding no package or transitive dependency. Cargo tree remains windows-sys 0.61.2 -> windows-link 0.2.1. Cargo.lock is byte-identical, SHA256 `8b02b145ca5b7adb6d0cffbd5a9bcaaa85ca343e9683964d6015e41ec4b74745`.

Microsoft upstream, published signatures, feature definition and MIT OR Apache-2.0 licensing were checked. The narrow binding built and ran on the current Windows/MSVC toolchain. A fresh OSV query on November 12 returned no advisories for windows-sys 0.61.2 or windows-link 0.2.1. This is limited to those versions/database results, not proof of safety or a fresh full-workspace audit. Existing lru/ttf-parser advisories and QuickJS maintenance risks remain as previously documented. Added risks include allocator/path lifetime mistakes and synchronous cleanup failures; ownership and failure acceptance are explicit above.

## Validation

All twenty-seven expected-success release scenarios passed, including crashes, watchdog stops, IPC faults, JS limits and recovery. All thirty-three worker launches produced explicit profile-storage cleanup verification before success/replacement. The transcript is retained under ignored target/sandbox-runs/profile-cleanup-regression.log. The loopback-network case remains inconclusive and outside this expected-success set.

For example, final healthy attempt `18628-ffdcd68ee88abaab` verified deletion/storage absence with a separately reported cleanup duration of 5.4765 ms. This observation is not a platform-wide maximum or new timeout guarantee.

All nineteen Rust tests, formatting, Clippy with warnings denied and the workspace release build passed. Existing incremental-cache hardlink/copy notices remain. Local documentation links, CLI help and diff checks were verified. No hosted CI, other Windows configurations or non-Windows targets were tested. Changes remain uncommitted; no push, release or untrusted execution was performed.

Primary references consulted November 12, 2026:

- [CreateAppContainerProfile ownership and storage](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-createappcontainerprofile)
- [DeleteAppContainerProfile and incomplete-storage warning](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-deleteappcontainerprofile)
- [GetAppContainerFolderPath allocator ownership](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-getappcontainerfolderpath)
- [ConvertSidToStringSidW and LocalFree](https://learn.microsoft.com/en-us/windows/win32/api/sddl/nf-sddl-convertsidtostringsidw)
- [CoTaskMemFree](https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-cotaskmemfree)
- [Microsoft Windows bindings upstream](https://github.com/microsoft/windows-rs)
- [OSV query endpoint](https://api.osv.dev/v1/querybatch)
