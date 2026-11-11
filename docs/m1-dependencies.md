# M1 Dependency Assessment

Reviewed: November 11, 2026. Scope: Windows trusted fixture only.

## Direct dependencies and purpose

- wgpu 30.0.1: GPU surface and rendering; gfx-rs project; MIT OR Apache-2.0. Enable only std, dx12 and wgsl directly. The resolved feature graph also includes glyphon's requirements.
- winit 0.30.13: native windows, DPI and event loop; rust-windowing project; Apache-2.0. Disable defaults, enable rwh_06. Non-Windows sessions are not supported by this build.
- glyphon 0.12.0: GPU text/atlas integration; grovesNL project; MIT OR Apache-2.0 OR Zlib. Resolves cosmic-text 0.19.0 (MIT OR Apache-2.0).
- pollster 0.4.0: drive GPU initialization futures without an async runtime; zesterer project; MIT/Apache-2.0 declaration. A newer major exists; this small API is pinned and audited with the resolved graph rather than assuming latest is required.
- windows-sys 0.61.2: Microsoft-generated Windows API bindings for the isolated platform module; MIT OR Apache-2.0. Enable Foundation and WindowsAndMessaging only directly.

All Rust registry packages use crates.io with lockfile checksums; no git dependency or remote build script is introduced by project code. Transitive crates include procedural macros, native bindings, build scripts and unsafe code; upstream ownership and registry checksums do not prove safety. Versions/repositories/licenses were inspected using Cargo metadata. Full package/license inventory: [Resolved dependencies](m1-dependency-inventory.md). Standard license alternatives include permissive selections; font discovery/shaping requires Unicode notices as applicable. The initial spike redistributed no external assets; the current preview embeds the reviewed Noto Sans font with its OFL notice. See [Font provenance](bundled-font.md). The checkerboard remains generated original RGBA data. Distribution NOTICE aggregation is still required before a public M1 artifact release.

## Security result

Queried the OSV batch API for all 227 registry package/version pairs from the resolved lockfile (including non-Windows lockfile entries). Two advisories were returned:

- [RUSTSEC-2026-0253](https://rustsec.org/advisories/RUSTSEC-2026-0253.html): lru 0.16.4 has a panic-safety/use-after-free issue; fixed in >=0.18.2. Glyphon requires the older semver line. Inspection of glyphon 0.12.0 text_render.rs/custom_glyph.rs found its atlas key is a Copy enum of Copy text/custom cache keys without a key Drop implementation, while the reported trigger requires a panicking key destructor and continued use after unwind. The built-in text path does not supply custom keys or catch/resume cache panics. Retained only for this trusted spike; this is an applicability assessment, not a claim the crate is patched or universally safe. Upstream compatible upgrade/replacement is required for broader adoption review.
- [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html): ttf-parser 0.25.1 is unmaintained, with no patched version. It enters through font discovery. The initial spike used installed fonts; the current preview uses only embedded, revision/hash-pinned Noto Sans bytes. Arbitrary package fonts and system-font discovery are disabled in this fixture. Migration of the font stack and restricted font processing must be resolved before untrusted fonts are accepted.

No advisory suppression or sandbox/limit relaxation was added. No hits for other resolved packages is not evidence of absence of vulnerabilities. Re-run the review when dependencies/features change. No production security endorsement is made.

## Primary references consulted

- [wgpu API](https://docs.rs/wgpu/30.0.1/wgpu/)
- [winit API and event model](https://docs.rs/winit/0.30.13/winit/)
- [glyphon API](https://docs.rs/glyphon/0.12.0/glyphon/)
- [Microsoft SetParent and DPI limitations](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setparent)
- [Lively desktop integration reference](https://github.com/rocksdanister/lively/blob/core-separation/src/Lively/Lively/Core/WinDesktopCore.cs)
- [OSV query API](https://google.github.io/osv.dev/api/)

Unsafe Win32 calls are confined to wallpaper-platform, with local safety comments and unsafe_op_in_unsafe_fn denied. This crate deliberately does not inherit the workspace unsafe_code=forbid lint because native window discovery requires FFI. Preview and CLI retain that lint. This exception affects trusted platform code only and is not a sandbox-policy exception. The native desktop handles are external, mutable state; G1 must validate lifecycle behavior.
