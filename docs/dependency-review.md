# M0 Dependency and Environment Review

Reviewed: November 11, 2026.

The initial workspace has no external Rust dependencies or transitive crate dependencies. Cargo.lock must reflect only wallpaper-cli. Rust standard-library/toolchain security is separate from crate auditing. Pin Rust 1.99.1 with rustfmt/Clippy; do not interpret an empty external dependency graph as proof of safety.

wgpu, glyphon, cosmic-text, rquickjs and parsers remain candidates from the feasibility report, not adopted dependencies. Before adding them, inspect their current primary documentation, exact distributed sources/features, licenses, maintenance, RustSec/upstream advisories, and the full resolved graph. Do not reuse the earlier research snapshot as a completed current security audit.

GitHub checkout is pinned to the previously published v4.2.2 commit 11bd71901bbe5b1630ceea73d27597364c9af683. It is maintained by GitHub and provides repository checkout. The GitHub API tag lookup verified this exact revision on November 11, 2026. The public repository advisory endpoint returned no entries on that date; this is not proof of safety or a complete Node dependency audit. The hosted execution path remains unverified. Recheck action/runtime compatibility and advisories before publication. No third-party Rust installation/cache/release action is used; publication uses the runner's GitHub CLI with job-scoped permissions. Runner tools remain external trust inputs.

Local environment: Windows x64; Rust/Cargo 1.99.1; MSVC Build Tools under Visual Studio 18; Windows SDK 10.0.26100.0. Exact OS build, GPU/driver and real display test topology still require capture before G1; CIM queries were denied. No hardware results are implied.

Primary references consulted on the review date:

- [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [GitHub Rust CI](https://docs.github.com/en/actions/tutorials/build-and-test-code/rust)
- [GitHub secure workflow use](https://docs.github.com/en/actions/reference/security/secure-use)
- [GitHub CLI release creation](https://cli.github.com/manual/gh_release_create)
- [RustSec advisory database and audit tooling](https://rustsec.org/)

Hosted CI, branch policy and release publication require remote validation after an authorized push. No remote repository settings have been changed.

