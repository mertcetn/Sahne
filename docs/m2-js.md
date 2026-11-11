# M2 Fixed JavaScript Sandbox Fixtures

Date: November 11, 2026. Status: experimental fixed trusted source only. No script file, package, imported wallpaper, external bytecode, DOM or timer API is accepted. M2/G2 remain incomplete.

## Boundary and budgets

The Windows sandbox probe now statically links exact rquickjs 0.14.0 with its reviewed QuickJS-NG snapshot. The host does not construct or evaluate an engine. Its worker first passes the existing AppContainer check and Ping exchange, then selects one of five source literals compiled into the executable. The private script module repeats the AppContainer self-check before runtime creation. The host still verifies token identity/capabilities before resuming the worker. See [source, dependency and security review](m2-js-dependencies.md).

The fixture installs base language objects plus Eval and Promise intrinsics. It installs no module resolver/loader, native functions, browser/system objects, logging callbacks, document, timers, typed arrays, RegExp, Date, performance or additional optional intrinsics. This is an initial test surface, not a promised general ECMAScript compatibility contract. Missing APIs are checked in JS and direct OS denials remain separately tested by native probes; missing globals do not prove OS isolation.

| Budget | Spike value | Scope |
| --- | --- | --- |
| Engine allocation | 16 MiB | QuickJS default C allocator; other native/process allocations still rely on the Job |
| JS stack | 256 KiB | Engine stack checks, not an OS thread stack replacement |
| Cooperative deadline | 100 ms | One cumulative monotonic deadline set before context construction; engine polling must return control |
| Promise work | 64 jobs | At most 64 explicit execute_pending_job calls in the jobs fixture; pending work is discarded on teardown |
| Process memory/processes | 256 MiB / one | Existing Job policy, unchanged |
| Independent host deadline | Three seconds | Existing response deadline includes engine setup, execution, teardown and stream EOF |
| Termination wait | One second | Existing Job stop/wait requirement after detection |

No custom allocator features are enabled, so the engine memory setter remains effective. No budget is supplied or raised by a script. Interrupt callbacks do only a monotonic time check and atomic flag update. These are experimental constants, not benchmarked product defaults or a hard 100 ms latency guarantee. Native calls/teardown may not poll interrupts; the independent watchdog remains mandatory.

## Commands and expected results

Run from the repository root on Windows:

```powershell
cargo run --release -p wallpaper-sandbox-probe --locked -- --case js-basic
cargo run --release -p wallpaper-sandbox-probe --locked -- --case js-interrupt
cargo run --release -p wallpaper-sandbox-probe --locked -- --case js-memory
cargo run --release -p wallpaper-sandbox-probe --locked -- --case js-stack
cargo run --release -p wallpaper-sandbox-probe --locked -- --case js-jobs
```

Each must print `Probe passed`, report mask 0x217, reach worker exit zero and leave the sentinel unchanged. The private fixture checks the actual result before setting report bit 9; the host selects the expected case, requires valid nonce/context/EOF and verifies the final process exit. This report is trusted test telemetry, not an assertion an untrusted worker could use to grant itself privileges.

| Case | Required result |
| --- | --- |
| js-basic | Arithmetic produces 42; listed OS/browser/loader/optional globals are absent |
| js-interrupt | Infinite loop returns engine Exception after the interrupt handler actually marked its deadline expired; exceptions are discarded without formatting a script-controlled error |
| js-memory | Retained arrays exhaust the configured engine allocator; the fixed script releases them and catches InternalError with the expected out-of-memory message; an interrupt does not count |
| js-stack | Fixed recursion produces the expected stack-related RangeError; an interrupt does not count |
| js-jobs | Self-replenishing Promise work executes exactly 64 engine jobs and still has pending work; an interrupt or job exception does not count; no further jobs are run |

OOM/stack exception checks are for these fixed original scripts, not a general exception-classification or byte-accurate memory benchmark. All ordinary IPC/OS-access checks still run around the fixtures. A stalled/crashed engine cannot turn into a successful result merely because another budget was hit.

## Initial local evidence

All five cases passed in release mode on the recorded Windows 11 Pro/MSVC machine. Retained initial runs are js-basic `14240-f93edfd6ec2f3e99`, js-interrupt `2912-17187f10309b2bd8`, js-memory `3444-5b22ba620a7e49ea`, js-stack `21212-a1bf238c7bc60b1a`, and js-jobs `4644-72a4868988d23352`. Final verification after expanding absent-global checks is recorded below. These runs add no Linux, alternate Windows build or hosted-run evidence.

Final verification passed all eighteen expected-success actual-process cases (the previous thirteen native/IPC cases and these five JS cases) with the expanded API-absence checks. Final JS runs were `22372-fe06de7e8b50f7ae`, `15420-9b39f2710274c917`, `14348-ca0a61ef1a414f27`, `15888-93974dd1d7be3e7f`, and `26792-8eae03c448cce2c1`, in the same case order. All fifteen Rust tests, workspace Clippy with warnings denied, formatting and workspace release build passed. Cargo emitted the existing incremental-cache hardlink/copy notices. Help output was updated and checked separately; no external script input was opened.

Because hashbrown feature unification changed, the release renderer was also run for two seconds in normal-window mode: RTX 3050/DX12, one frame presented, generation one, clean exit. This is a dependency smoke test, not renewed visual/lifecycle certification or a performance measurement. Hosted CI, other Windows configurations and non-Windows targets were not run. The new JS continuation remains uncommitted; no publication was performed and no duplicate user-run test is needed.

## Limitations and next work

Untrusted packages and sample wallpapers remain disabled. The compiler/DOM/clock/timer API still belongs to M3/M4; JavaScript cannot render a wallpaper through this probe. Multiple callbacks, per-callback/job deadlines, timer/ACK floods, source transport/validation, bounded restart policy, persistent IPC rate budgets and preservation of a rendered scene after worker failure remain open. Full network/endpoint/escape and platform coverage also remain open. The native stall and IPC tests exercise the independent watchdog separately; this task does not simulate a novel native engine exploit.

No additional unsafe exception is added to application code. Build/check commands are in [Development guide](development.md); actual IPC faults remain documented in [IPC validation](m2-ipc-validation.md). The release workflow builds the workspace but continues to package only CLI, preview and docs, excluding this developer probe. No release was pushed.
