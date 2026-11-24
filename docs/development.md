# Development and Release Guide

## Local setup

Use Rust 1.99.1 (pinned in rust-toolchain.toml). Windows development requires MSVC x64 build tools and the Windows SDK. The workspace contains the development CLI, trusted renderer preview, control codec and trusted Windows sandbox probe. It does not execute wallpaper packages.

Run from the repository root:

```powershell
cargo build --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -p wallpaper-cli -- --help
cargo run -p wallpaper-cli -- --version
```

The CLI prints help with no arguments, accepts --help/-h and --version/-V, and rejects other arguments with exit code 2. It does not accept package paths. Workspace tests include protocol-boundary and preview-lifecycle regressions; real AppContainer tests are explicit manual commands, not ordinary unit tests.

## Branches and publication

main is development; release is the publication branch. Only these two branches are maintained. Changes are committed after user testing and approval. A release push requires explicit user instruction.

The workflow checks formatting, Clippy, tests, release compilation, and CLI execution on Windows. A release push publishes a Windows x64 development ZIP and SHA256SUMS through GitHub Releases after required checks succeed. The tag contains the full source commit, workflow run ID, and attempt. Reruns are distinct releases. Until product readiness, these are prereleases containing a scaffold, not usable wallpapers. Linux artifacts are added only after Linux build validation.

The workflow uses read-only repository permissions except the publishing job. Checkout is pinned to a full commit with credential persistence disabled. The first main CI and authorized release publication passed; the downloaded archive checksum and CLI execution were verified. See [M0 validation record](m0-validation.md) for exact commit/run/artifact evidence. The runner emitted a checkout Node-runtime deprecation warning tracked in that record.

For an authorized release: review the exact commit and checks, update the release branch without rewriting history, push it, inspect Actions, download the ZIP, verify SHA256SUMS, and run the bundled CLI on a clean target machine. Record results before claiming the publication path is validated. The first explicitly authorized development release push has been validated; future release pushes still require explicit instruction.

## M1 preview

The separate wallpaper-preview executable provides a trusted Windows/D3D12 scene; it does not extend wallpaper-cli package support. See [M1 preview](m1-preview.md) for launch/stop commands, expected appearance and open validation cases. A future authorized release build packages both executables; the existing M0 release remains unchanged. Review M1 dependency risks and prepare required dependency notices before publishing the new artifact.

## Local research samples

The root samples/ directory is Git-ignored and optional. Do not add it to release archives, Actions inputs or default test requirements. Derived sample artifacts stay under ignored samples/ or target/. Tracked docs/ records may describe observed capabilities without copying third-party source/media. Future importer CI uses original synthetic fixtures so a clean checkout remains self-contained. See [Import plan](wallpaper-engine-import-plan.md).

## M3 fixed scene-data check

Run `cargo run --release -p wallpaper-cli --locked -- --scene-check` to validate/encode/decode one original built-in scene without opening any file or window. `cargo test -p wallpaper-scene --locked` checks its bounded decoder and semantic validation. This does not compile HTML/CSS or load packages; see [Scene schema, budgets and command behavior](m3-scene.md).

Run `cargo run --release -p wallpaper-cli --locked -- --compile-check` for original embedded HTML/CSS compilation and independent scene decode. `cargo test -p wallpaper-compiler --locked` checks the strict grammar, cascade/layout, diagnostics and limits. The `compiler-basic`/`compiler-invalid` sandbox cases exercise that code in AppContainer. No external input or rendering is enabled by these commands; see [Compiler subset and commands](m3-compiler.md).

Run `cargo run --release -p wallpaper-cli --locked -- --compile-image-check` for the separate original PNG fixture, pixel decoding and independent scene/asset validation. `cargo test -p wallpaper-media --locked` covers the bounded PNG contract; the `image-basic`/`image-invalid` cases are included in the native sandbox runner. These commands open no window. The implemented `wallpaper-preview --example image` requires a window and still awaits runtime/visual verification; see [PNG resource limits and pending checks](m3-images.md).

Run `cargo run --release -p wallpaper-preview --locked -- --example static --seconds 30` for the original compiled HTML/CSS window. It accepts only the fixed embedded example, with no external paths or JS. See [Visual reference, fit/text rules and remaining lifecycle validation](m3-preview.md). The original M1 fixture remains the default without `--example`.

## Protocol boundary checks

The M2 std-only codec can be checked without opening windows or running samples:

```powershell
cargo test -p wallpaper-protocol --locked
```

See [M2 control-frame foundation](m2-protocol.md) for wire format. This adds no user-facing package execution support.

## Trusted Windows sandbox probe

See [M2 sandbox commands and results](m2-sandbox.md) before running the separate wallpaper-sandbox-probe executable. Its fixed native/JS cases exercise a real AppContainer. The native loopback timeout still fails by itself; a separate [read-only WFP diagnostic](m2-loopback.md) confirms the receiving-side block on the recorded local machine. Each attempt creates a temporary profile and retains a unique test directory under target/sandbox-runs; normal completion requires explicit cleanup verification. No package or script path is accepted, and release archives do not distribute this experimental executable. See [M2 completion and remaining G2 requirements](m2-acceptance.md).

The continuation adds network-egress (explicit TCP denial to a reserved test address) and memory (small allocation succeeds, one over-limit commit fails) as separate cases. The original network case still checks loopback and does not count a timeout as success. See the sandbox guide for commands and remaining G2 limitations.

Additional process and handles cases test active-process quota and exclusion of an inheritable synthetic file handle. On a strict-handle Windows policy the handles worker intentionally terminates with 0xc0000008; only the verified armed-stage/policy/exit combination passes and the host must still exit zero. WER is suppressed locally in that intentional-fault worker. See the sandbox guide before running these developer-only checks.

IPC cases ipc-flood, ipc-stall, ipc-truncated, ipc-replay and ipc-open exercise the one-shot channel's trailing-data, deadline, early-EOF and sequence policy. See [IPC commands and scoped evidence](m2-ipc-validation.md). All successful responses now require EOF; a valid report followed by more messages or an indefinitely open stream fails the ordinary conversation. These checks require no visual inspection or user interaction and are run autonomously.

The Windows probe additionally has js-basic, js-interrupt, js-memory, js-stack and js-jobs. These evaluate only fixed trusted source literals after the AppContainer check. See [JS commands and budgets](m2-js.md) and [engine dependency review](m2-js-dependencies.md). Engine evaluation never occurs in the host. No script path, package, DOM or timer interface is exposed, and the probe remains excluded from release ZIPs. Its native C build requires the reviewed MSVC/C11 toolchain; hosted verification of this dependency remains pending.

Explicit restart-recover, restart-exhaust, restart-timeout, restart-handshake, restart-stale and restart-protocol cases exercise a three-attempt supervisor, fresh session ownership, backoff and permanent protocol rejection. They do not change ordinary single-case runs into automatic retry loops. See [Recovery commands and expected terminal states](m2-recovery.md). Agent-executable validation is run without asking for duplicate manual checks.

The endpoint case checks a host-created, unrelated named pipe with an actual host positive connection and a free instance. Four worker access requests must return error 5; after owner destruction the name must return error 2. See [Endpoint command and evidence](m2-endpoint.md). This introduces no user-configurable endpoint, external data transfer or broader worker API.

The ipc-rights case checks the actual inherited control handles: DACL reads, DACL changes, ACL/pipe-instance rights upgrades through duplication and reverse-direction I/O must return access denied while the normal protocol/EOF/exit succeeds. Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case ipc-rights`. All probe launches now verify protected host-user-only pipe DACLs and transfer data-only handles. See [Channel security and limits](m2-ipc-security.md). These tests require no manual approval of their results.

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case filesystem` for the synthetic filesystem matrix. It requires sixteen direct access denials, successful selected staged/system reads and unchanged post-stop test files. Host controls use the same synthetic targets; no user files are inspected. See [Filesystem command, access matrix and limits](m2-filesystem.md).

All completed probe attempts now require explicit AppContainer deletion and absence of the previously populated profile directory after stop/reader join. Each prints `Profile cleanup verified` with a separate duration; failure prohibits recovery. Existing healthy, hang, crash and restart cases exercise this without new arguments. See [Profile storage controls and remaining audit scope](m2-profile-cleanup.md).

The ipc-write-drain and ipc-write-stall cases test bounded host write pressure. Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case ipc-write-drain` or replace the case with ipc-write-stall. A dedicated writer sends one fixed 1 MiB transport fixture; drain requires full contents/EOF, while stall requires the host deadline to stop the worker and release/join the writer. All cases now use owned reader/writer threads. See [Commands, acceptance and timing scope](m2-host-write.md).

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case cpu-busy` for fixed native CPU stress. Probe Jobs now have a read-back-verified 10% CPU hard cap, with a stricter 1% in this measurement case. The worker must arm, consume measurable bounded CPU and remain alive until the existing watchdog stop; the host also verifies exact Job membership. Workers now launch detached from consoles with their same pipe handles. See [CPU command, hardware scope and console finding](m2-cpu-rate.md).

For reproducible M2/M3 validation on Windows/PowerShell 7, run `pwsh -NoProfile -File tools/test-sandbox-runner.ps1`, then `pwsh -NoProfile -File tools/test-sandbox.ps1`. The default run builds the probe, checks all thirty-nine expected-success cases and writes JSON/Markdown plus case logs under target/sandbox-validation/. The manual Sandbox validation (experimental) workflow provides the same checks when available on GitHub; hosted execution is not yet verified. See [Runner acceptance, outer timeout and report scope](m2-validation-runner.md).

The `memory-incremental` case checks cumulative native commit, denial, release/recovery and host Job peak accounting; see [Incremental memory](m2-incremental-memory.md). The `ipc-roundtrip`, `ipc-roundtrip-replay` and `ipc-roundtrip-stall` cases check 128 sequential exchanges, a one-slot writer-command queue and faults after valid progress; see [Multi-message transport](m2-roundtrip.md). M3's fixed package/compiler cases bring the default corpus to thirty-nine cases and forty-five profile cleanups. These are developer fixtures, not production source/scene APIs.

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case registry` for the synthetic registry matrix. Ten worker requests must be denied while host controls succeed. The host verifies its fixed value/namespace and deletes only its own newly created volatile key, requiring post-cleanup absence. Existing registry values are not inspected. See [Registry operations, evidence and limits](m2-registry.md).
