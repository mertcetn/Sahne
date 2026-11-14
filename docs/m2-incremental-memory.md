# M2 incremental committed-memory validation

Date: November 14, 2026. Status: fixed trusted Windows fixture; partial G2 evidence.

## Run

From the repository root on Windows with PowerShell 7:

```powershell
./tools/test-sandbox.ps1 -Cases memory-incremental
```

The case must exit zero with one launch, profile cleanup and `Probe passed` marker. It is included in the default runner alongside the original single-allocation `memory` case.

## Contract

The worker first verifies AppContainer identity and its immediate Job's exact 256 MiB process-commit limit. A 64 KiB reserve/commit/release control must succeed. It then retains up to sixteen separate 16 MiB `VirtualAlloc` regions, using `MEM_RESERVE | MEM_COMMIT` and `PAGE_READWRITE`. Bookkeeping is allocated before pressure. No pointer is dereferenced, physical page population is not forced, and no executable allocation is requested.

Acceptance requires a native memory error 8 or 1455 after at least twelve successful regions (192 MiB). Success for all sixteen regions, early failure or another error fails the case. The fixed attempt count and existing Job/watchdog bound the experiment; this is not an unbounded host-memory exhaustion loop.

Every successful region has an owner. The normal path explicitly checks each `VirtualFree` using the original base, size zero and `MEM_RELEASE`; error paths retain best-effort ownership cleanup. After release, a 32 MiB allocation and checked release must succeed before normal Pong/result/EOF and worker exit zero. The trusted report mask is `0x10017`, reusing the existing result body and adding no wire type.

The host independently queries its retained Job after normal worker exit. It requires peak process committed memory in 192..256 MiB, the unchanged exact process-memory limit, exactly one associated process ever and zero active processes. Thus a small-allocation-only worker cannot satisfy acceptance merely by emitting the expected report. Existing sentinel checks, stop/join timing and explicit profile cleanup still apply.

## Evidence and limits

Initial selected run `4561b50abccd4fe791a207176e99d53c` passed on Windows 11 build 26200 / Rust 1.99.1 MSVC. The host observed peak committed memory 253,153,280 bytes below the 268,435,456-byte limit, normal worker exit and verified profile removal. Retained staging directory: `11412-e56a94d1f81146ed`.

Full report `26709353ad174dc8b23446dccdc10a23` passed all thirty-two cases, thirty-eight profile cleanups and one registry cleanup. All twenty Rust workspace tests, Clippy with warnings denied, formatting, local documentation links and diff checks passed. Existing incremental-cache hardlink/copy notices remain. Hosted CI and other platform configurations were not run.

This complements the engine's 16 MiB heap-limit tests and the original 257 MiB rejection. It tests committed virtual memory, not resident memory, physical-pressure paging, repeated allocator fragmentation or simultaneous wallpaper sessions. Errors 8/1455 can also reflect system commit pressure; the large successful control, exact policy readback and independent peak accounting strengthen the observation without uniquely attributing every possible failure to the Job. An early pressure failure remains a failed test rather than a reason to increase the limit.

No dependency, feature, version, Cargo.lock entry, capability or resource limit changes. Existing Windows Memory/JobObjects bindings suffice. The native ownership/error paths were reviewed against current Microsoft references; other engine/renderer dependency risks remain as previously recorded. G2 and untrusted-content enablement remain open.

## Primary sources reviewed November 14, 2026

- [VirtualAlloc and commit versus physical memory](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualalloc)
- [VirtualFree release ownership and size rules](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualfree)
- [Job process-memory limits and peak accounting](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_extended_limit_information)
- [QueryInformationJobObject and immediate worker Job selection](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-queryinformationjobobject)
