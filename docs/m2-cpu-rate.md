# M2 Native CPU Rate and Console-Free Launch

Date: November 12, 2026. Status: trusted Windows experiment after 52be1d5; partial G2 evidence.

## Command and acceptance

```powershell
cargo run --release -p wallpaper-sandbox-probe --locked -- --case cpu-busy
```

The host must read back the configured CPU hard cap before process creation. The worker verifies its immediate Job policy, completes baseline file/host-process denials, sends its valid Pong and armed report 0x4017, then executes a fixed native arithmetic loop using black_box. The host requires the validated armed stage, the existing response timeout, measurable CPU within the observation bound, a live worker and exactly its expected PID in the Job. It then stops the Job, joins both I/O threads and verifies profile storage deletion. Missing/malformed arming, early exit, failed queries, unexpected processes or out-of-bound accounting fail the case.

## Enforced scheduling policy

All probe Jobs now set JobObjectCpuRateControlInformation with exactly ENABLE | HARD_CAP before launching the suspended worker. Ordinary cases use CpuRate=1000 (10%); cpu-busy uses the stricter CpuRate=100 (1%). SetInformationJobObject failure or mismatched QueryInformationJobObject readback aborts launch. The worker self-query requires the same 1% flags/value for its own immediate Job; no Job handle is inherited.

Windows interprets this value as cycles per 10,000 cycles in each scheduling interval. Threads stop running for the remainder of an interval once the hard cap is consumed. In a capped Job hierarchy the share is relative to the immediate capped parent; otherwise it is a share of system capacity. This percentage therefore depends on scheduling hierarchy and processor capacity. It is not a measured percentage of one logical core.

The one-active-process limit, 256 MiB process-memory limit, kill-on-close policy, explicit handle list, zero AppContainer capabilities, JS budgets and three-second control deadlines remain unchanged. The new policy adds rate limiting to the previously approved baseline. It is an experimental probe policy; production wallpaper scheduling/callback budgets remain a separate design/validation step.

## Measurement and arming

The CPU armed notification is produced only after complete envelope/body/context/nonce validation. The existing one-slot ready channel carries Hello and, only for the exact CPU report, a second stage notification. The control thread consumes Hello first and captures the second notification after the response wait. Ordinary timeout alone cannot satisfy cpu-busy. A reader regression checks wrong session, sequence, revision, nonce and truncated report never produce the armed notification.

The host measures Job user+kernel execution time in 100 ns ticks and wall time from just before resume. The native loop must have consumed at least 20 ms user CPU and still be alive. The observation upper bound is wall_seconds * active_logical_processors * 0.01 * 2 + 0.1 seconds. This allows for scheduling intervals, cycle/time differences and accounting granularity; it does not change the exact 1% configured policy. GetActiveProcessorCount covers all groups. This measurement fixture deliberately requires 1..32 logical processors; other configurations fail with an unsupported-measurement error rather than claiming a useful single-thread stress result.

The host also reads back the one-process limit and queries a bounded Job PID list, requiring exactly the owned worker PID and one active/accounted process. It checks these while the worker is alive, before its ordinary watchdog stop. After termination, existing stop/join and profile cleanup acceptance applies. This is fixed single-thread native stress with observed accounting, not certification of every workload, CPU frequency, multi-thread attack or Windows configuration.

## Console helper finding and launch change

The first PID-list check found both the worker and Windows conhost.exe under the previous CREATE_NO_WINDOW launch, despite the configured active-process limit remaining 1. Host-only inspection identified that extra image in this Job; the test failed instead of treating it as a single-process result. Historical child-creation-denial evidence remains scoped to its exact CreateProcess fixture and does not prove there was no OS console helper.

The launcher now uses DETACHED_PROCESS, with the same suspended startup, STARTF_USESTDHANDLES and explicit inherited pipe list. The suspended child-creation negative fixture uses the same console-free flag. Subsequent CPU inspection found only the expected worker PID while pipe communication and cleanup continued to pass. The test retains its strict membership assertion. This does not prove that a compromised worker cannot invoke AllocConsole or another OS broker later; broader console/broker/escape coverage remains open.

## Total CPU-time investigation

An earlier local experiment configured one/two-second total user-time limits. Those failed to establish quota termination inside the unchanged three-second host window: one sample recorded 2.09375 seconds user CPU after a two-second limit while still alive. Microsoft documents periodic checks for these limits. The final implementation uses interval-based CPU hard caps; it does not advertise immediate total-time quota termination. Watchdog timing was not increased to make that experiment pass.

## Validation and dependencies

All thirty expected-success release scenarios passed with thirty-six explicit profile-storage cleanups under the CPU caps and detached launch. The transcript is retained under ignored target/sandbox-runs/cpu-rate-regression.log. The inconclusive loopback case is excluded from the expected-success set.

Final regression CPU attempt `38356-7b863603a9573ec6` observed one expected worker PID, one active/accounted process, eight logical processors, 3.0365 s wall time and 0.3125 s user+kernel CPU, below the separate 0.5858 s observation allowance. Its watchdog stop took 1.7595 ms and profile cleanup passed. A subsequent targeted run, `37092-53a22eadebeb2a5f`, passed the additional complete-PID-list guard with 0.2656 s CPU over 3.0323 s wall time; its transcript is retained under ignored target/sandbox-runs/cpu-rate-final.log. These are local observations, not universal timing limits.

All twenty Rust tests (including the new CPU-arming regression), formatting, Clippy with warnings denied, workspace release build, CLI help, local documentation links and diff checks passed. Existing incremental-cache hardlink/copy notices remain. No hosted CI, other Windows configuration or non-Windows target was tested. New changes remain uncommitted; no push or release was performed.

No package, version, Cargo feature or lockfile change is needed. The new calls use already-enabled windows-sys 0.61.2 JobObjects/Threading bindings; native FFI stays in wallpaper-sandbox. The fixed loop, stage notification and regression use Rust std only. Current primary API documentation was reviewed. Existing dependency advisories/maintenance uncertainty remain as previously recorded; no new safety claim or full dependency re-audit is inferred.

Broader G2 remains open: sustained valid-message CPU/rate behavior, multiple/persistent sessions, console and other OS brokers, full permitted-read inventory and inconclusive loopback denial. No arbitrary source/package, untrusted execution or release publication is enabled.

Primary references consulted November 12, 2026:

- [Job CPU-rate flags, units, hard caps and nested behavior](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_cpu_rate_control_information)
- [Job accounting and active-process counters](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_accounting_information)
- [QueryInformationJobObject classes and immediate Job lookup](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-queryinformationjobobject)
- [Complete Job process-ID list semantics](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_process_id_list)
- [Active processor count across groups](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getactiveprocessorcount)
- [Periodic total user-time limit semantics](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_limit_information)
- [Process creation flags](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags)
- [Console creation and detached processes](https://learn.microsoft.com/en-us/windows/console/creation-of-a-console)
