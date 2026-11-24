# M3 bounded PNG resources

Updated: November 24, 2026. Status: fixed original PNG decoding, compiler mapping and independent scene/asset validation. The image preview is implemented but its GPU/window/visual validation is pending. Arbitrary package loading and production helper integration remain disabled; M3/G3 are incomplete.

## Accepted input and limits

The `wallpaper-media` crate has no filesystem, network, script or GPU access and inherits the workspace unsafe-code prohibition. `decode_png` borrows encoded bytes and accepts a remaining decoded-byte budget. Output is immutable, tightly packed, straight-alpha sRGB RGBA8, with private dimensions/pixels and checked construction.

The deliberately narrow PNG subset is non-interlaced, 8-bit RGBA (color type 6), compression/filter methods zero. Require the PNG signature, one first 13-byte IHDR, optionally one sRGB chunk with intent 0..3 before image data, consecutive IDAT chunks and one empty final IEND. Verify every chunk's CRC; reject trailing container bytes and all other chunks, including APNG, palettes, text, ICC, gamma and HDR metadata. Untagged input is interpreted as sRGB by this native contract. Other valid PNG formats are rejected explicitly; they are not converted or silently stripped of metadata. CRC detects corruption, not origin or authenticity.

Limits are 16 MiB encoded input, 4,096 chunks, 1..4,096 pixels per dimension, at most 64 MiB RGBA per image and 128 MiB aggregate decoded images. Check dimensions and the caller's remaining budget, capped at the aggregate policy, before pixel allocation or decompression. The upstream decoder additionally receives an 8 MiB internal allocation limit with checksum checking enabled. That limit is best effort and can reject otherwise valid files; it is not a process-memory or execution-time guarantee. Frame output dimensions, pixel mode and exact length must agree with preflight, and decoder completion must succeed. No conversion buffer, animation frame or metadata expansion is requested.

`validate_assets` compares an independently decoded validated scene to the exact immutable image table. Missing/extra IDs, substituted dimensions and aggregate overflow fail, including unused declarations. Scene IDs remain local data, not authenticated host handles. No source hash, cryptographic content identity or production asset IPC is added by this work.

## Source mapping and original fixture

`img src` must resolve to an exact-case retained package path ending in lowercase `.png`. No URL or external file lookup is performed. Repeated references to the same path share one decoded asset and scene resource; distinct paths do not deduplicate by content. Resource IDs are sequential, scene-local and bounded by the existing 1,024-resource limit. Image nodes use authored width/height; intrinsic dimensions do not implement automatic layout. An image background, when present, is emitted immediately before its image and both primitives share the author-ID binding. Errors point at the referring HTML element with a bounded image error code.

The separate examples/static-image package embeds panel/title/image sources and an original algorithmic 8x8 RGBA PNG. Each 4x4 quadrant has a constant byte color: top-left `(32,160,128,255)`, top-right `(230,160,64,255)`, bottom-left `(40,64,128,255)`, bottom-right `(230,64,96,128)`. It was generated using the standard PNG signature, a big-endian IHDR, filter-zero rows compressed with .NET ZLibStream, IEEE CRC32 per chunk and IEND; it contains no third-party media or metadata. The decoded output is 256 bytes. The existing examples/static-scene and its visual reference remain unchanged.

Nonvisual checks:

```powershell
cargo test -p wallpaper-media -p wallpaper-compiler --locked
cargo run --release -p wallpaper-cli --locked -- --compile-image-check
./tools/test-sandbox.ps1 -Cases image-basic,image-invalid
```

When invoking the PowerShell runner from another shell, supply its `Cases` as a real PowerShell string array. Its default suite includes both cases. They require an actual zero-capability AppContainer, retain the 256 MiB Job limit/watchdogs, and report `0x100017` only after the fixed result checks. They transfer no image payload over IPC. The invalid fixture checks truncation, CRC, insufficient decoded budget and a compiler diagnostic; Rust tests additionally cover unsupported metadata/pixel modes, chunk counts/order, hostile lengths and exact asset matching. This finite corpus is not a complete fuzz/security audit.

## Preview implementation awaiting runtime evidence

`wallpaper-preview --example image --seconds 10` selects only the original embedded fixture. It independently validates scene/assets before GPU allocation, checks texture dimensions against device limits, uploads RGBA8 to sRGB textures and draws images in scene order using straight-alpha blending, nearest sampling and clamp-to-edge. The whole image stretches to the authored rectangle; no object-fit, mipmaps, ancestor clip, group opacity, transform or animation is implied. Multiple references share one GPU binding. The existing fixed-canvas letterboxing/text rules apply. Recreation constructs fresh scene/assets/textures.

Shader parsing/validation and Rust checks require no GPU/window. Actual upload, blending, visual output, static-mode regression, idle behavior and recovery of the modified renderer must still be exercised with a window. Those runs and screen access were deliberately deferred at the user's request. Do not interpret earlier static-mode evidence as validation of this revision or the image mode. See [preview history](m3-preview.md).

Before accepting arbitrary input, put media processing behind the production restricted helper and retain OS memory/CPU/watchdog protection, independent asset checks and bounded transport. Host-side fixed-fixture decoding is not authorization to parse untrusted images in the host. [Dependency review](m3-png-dependencies.md) records the current advisories, licenses and remaining uncertainty.

## Local validation — November 24, 2026

All 78 workspace Rust tests passed, including nine media tests, nineteen compiler tests and five preview pure/lifecycle tests. Shader parsing and Naga validation used no device/window. Workspace release build, Clippy with warnings denied, formatting, documentation links and nonvisual CLI/help/invalid-argument checks passed. The static compiler fixture still produces three nodes and 123 encoded bytes with no images; the new image fixture produces three nodes, one image and 136 scene bytes plus 256 decoded pixel bytes.

The full default native suite passed 41 cases, 47 worker launches/profile cleanups and one registry cleanup. Report: ignored target/sandbox-validation/02396ae919b742fd8a60aa529b198666/summary.json, base revision 4469f5e with the PNG changes uncommitted during validation. It records exact binary/lock hashes and keeps `G2Complete=false`. No GPU preview/window was opened and no screen capture/interaction occurred. Existing Cargo cache hardlink/copy notices were emitted. Hosted CI, non-Windows targets, image GPU/visual validation and renderer runtime regression checks were not run.
