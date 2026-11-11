# Development and Release Guide

## Local setup

Use Rust 1.99.1 (pinned in rust-toolchain.toml). Windows development requires MSVC x64 build tools and the Windows SDK. The initial workspace contains only a dependency-free development CLI; it does not render or execute wallpaper packages.

Run from the repository root:

```powershell
cargo build --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -p wallpaper-cli -- --help
cargo run -p wallpaper-cli -- --version
```

The CLI prints help with no arguments, accepts --help/-h and --version/-V, and rejects other arguments with exit code 2. It does not accept package paths. There are no behavioral unit tests yet; cargo test currently verifies test-target compilation only. Rendering, parser, and sandbox tests belong to the later spikes.

## Branches and publication

main is development; release is the publication branch. Only these two branches are maintained. Changes are committed after user testing and approval. A release push requires explicit user instruction.

The workflow checks formatting, Clippy, tests, release compilation, and CLI execution on Windows. A release push publishes a Windows x64 development ZIP and SHA256SUMS through GitHub Releases after required checks succeed. The tag contains the full source commit, workflow run ID, and attempt. Reruns are distinct releases. Until product readiness, these are prereleases containing a scaffold, not usable wallpapers. Linux artifacts are added only after Linux build validation.

The workflow uses read-only repository permissions except the publishing job. Checkout is pinned to a full commit with credential persistence disabled. The first main CI and authorized release publication passed; the downloaded archive checksum and CLI execution were verified. See [M0 validation record](m0-validation.md) for exact commit/run/artifact evidence. The runner emitted a checkout Node-runtime deprecation warning tracked in that record.

For an authorized release: review the exact commit and checks, update the release branch without rewriting history, push it, inspect Actions, download the ZIP, verify SHA256SUMS, and run the bundled CLI on a clean target machine. Record results before claiming the publication path is validated. The first explicitly authorized development release push has been validated; future release pushes still require explicit instruction.
