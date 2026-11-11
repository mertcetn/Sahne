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

## Protocol boundary checks

The M2 std-only codec can be checked without opening windows or running samples:

```powershell
cargo test -p wallpaper-protocol --locked
```

See [M2 control-frame foundation](m2-protocol.md) for wire format. This adds no user-facing package execution support.

## Trusted Windows sandbox probe

See [M2 sandbox commands and results](m2-sandbox.md) before running the separate wallpaper-sandbox-probe executable. Its healthy, hang, oversized and crash cases exercise trusted native code in a real AppContainer; the loopback network investigation currently fails acceptance with an inconclusive timeout. Each run creates a temporary profile and retains a unique test directory under target/sandbox-runs. Profile cleanup is attempted on normal exit. No package or script path is accepted, and the release archive workflow has not been extended to distribute this experimental executable. Fixed embedded JS fixtures are described below.

The continuation adds network-egress (explicit TCP denial to a reserved test address) and memory (small allocation succeeds, one over-limit commit fails) as separate cases. The original network case still checks loopback and does not count a timeout as success. See the sandbox guide for commands and remaining G2 limitations.

Additional process and handles cases test active-process quota and exclusion of an inheritable synthetic file handle. On a strict-handle Windows policy the handles worker intentionally terminates with 0xc0000008; only the verified armed-stage/policy/exit combination passes and the host must still exit zero. WER is suppressed locally in that intentional-fault worker. See the sandbox guide before running these developer-only checks.

IPC cases ipc-flood, ipc-stall, ipc-truncated, ipc-replay and ipc-open exercise the one-shot channel's trailing-data, deadline, early-EOF and sequence policy. See [IPC commands and scoped evidence](m2-ipc-validation.md). All successful responses now require EOF; a valid report followed by more messages or an indefinitely open stream fails the ordinary conversation. These checks require no visual inspection or user interaction and are run autonomously.

The Windows probe additionally has js-basic, js-interrupt, js-memory, js-stack and js-jobs. These evaluate only fixed trusted source literals after the AppContainer check. See [JS commands and budgets](m2-js.md) and [engine dependency review](m2-js-dependencies.md). Engine evaluation never occurs in the host. No script path, package, DOM or timer interface is exposed, and the probe remains excluded from release ZIPs. Its native C build requires the reviewed MSVC/C11 toolchain; hosted verification of this dependency remains pending.

Explicit restart-recover, restart-exhaust, restart-timeout, restart-handshake, restart-stale and restart-protocol cases exercise a three-attempt supervisor, fresh session ownership, backoff and permanent protocol rejection. They do not change ordinary single-case runs into automatic retry loops. See [Recovery commands and expected terminal states](m2-recovery.md). Agent-executable validation is run without asking for duplicate manual checks.

The endpoint case checks a host-created, unrelated named pipe with an actual host positive connection and a free instance. Four worker access requests must return error 5; after owner destruction the name must return error 2. See [Endpoint command and evidence](m2-endpoint.md). This introduces no user-configurable endpoint, external data transfer or broader worker API.
