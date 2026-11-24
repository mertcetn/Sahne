# M3 PNG dependency review

Reviewed: November 24, 2026. Decision: exact png 0.18.1 with default features disabled and crc32fast 1.5.5 for the bounded original-image fixtures. This is a targeted adoption review, not a full source audit or approval for arbitrary image processing in the host.

## Necessity, graph and platform

A maintained PNG/DEFLATE implementation avoids inventing a decompressor. Direct crc32fast verifies every accepted container chunk, including IEND, before decoder entry. A handwritten preflight only implements the documented narrow container contract; upstream decoding/checksums still run. The `image` umbrella crate, other codecs, optional zlib-rs backend, native zlib and APNG transformations are not enabled.

The active media graph has ten registry package versions:

| Package | Version | License |
| --- | --- | --- |
| png | 0.18.1 | MIT OR Apache-2.0 |
| crc32fast | 1.5.5 | MIT OR Apache-2.0 |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 |
| flate2 | 1.1.11 | MIT OR Apache-2.0 |
| miniz_oxide | 0.8.9 and 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| simd-adler32 | 0.3.10 | MIT |
| bitflags (existing) | 2.13.2 | MIT OR Apache-2.0 |
| cfg-if (existing) | 1.0.5 | MIT OR Apache-2.0 |

png and flate2 currently resolve different miniz_oxide versions; both are included in the review. Cargo.lock also contains optional, inactive zlib-rs 0.6.8 (Zlib, MSRV 1.75), absent from the active normal/build tree. Nine registry blocks were added; existing package versions/checksums remain unchanged. Use `cargo tree -p wallpaper-media --locked -e normal,build,features` to inspect enabled paths. flate2's default Rust backend and miniz SIMD/Adler paths are active; these include architecture-specific low-level code, not covered by the workspace prohibition on unsafe project code.

Declared MSRVs are at most 1.73 for active packages declaring one, below pinned Rust 1.99.1. This is Rust code without a new OS binding or native compiler dependency; only local Windows x64/MSVC build and fixture execution are validated here. Linux/Android support is not established by portability expectations.

## Maintenance, origin and build execution

Current crates.io owner/version API metadata agrees with the declared upstream projects: png owners fintelia, 197g and image-rs publish team, publisher 197g; fdeflate owners fintelia/image-rs team, publisher fintelia; crc32fast owner srijs; flate2 owner rust-lang-owner; miniz_oxide owners oyvindln/Frommi; adler2 owner oyvindln; simd-adler32 owner/publisher mcountryman. Public publisher fields for crc32fast/flate2 returned null; owner/repository alignment is available, but per-release publisher identity is unresolved. None of the queried selected releases was yanked.

png 0.18.1 was published February 14, 2026; crc32fast 1.5.5 November 10, 2026; flate2 1.1.11 November 6, 2026; simd-adler32 0.3.10 July 14, 2026; miniz_oxide 0.9.1 March 13, 2026 with publisher oyvindln. Older fdeflate 0.3.7 was published December 6, 2024, miniz_oxide 0.8.9 and adler2 2.0.1 June 9, 2025. Release age and recognizable owners are maintenance signals, not proof against compromise or abandonment. Exact Cargo source manifests, license files and custom-build targets were inspected. The only active custom build script is crc32fast's: invoke Cargo's RUSTC with `--version` and emit CPU-intrinsic compatibility cfgs. No network download, native compilation or shell hook occurs in that script. Runtime CPU detection/SIMD paths remain a dependency trust assumption.

## Security findings and uncertainty

Direct OSV querybatch requests on November 24 returned empty advisory results for all eleven exact resolved versions, including inactive zlib-rs. No known advisory was returned; unpublished bugs, excessive resource use, panics, CPU-dispatch errors or a compromised source/publisher remain possible. This review did not independently audit the entire decoder, compressors, SIMD implementations or build toolchain.

The image-rs PNG project offers private vulnerability reporting, but explicitly treats crashes/panics and resource exhaustion as best-effort bugs outside its security policy. Therefore its policy and empty advisory results cannot substitute for a restricted worker, OS resource limits or timeout monitoring. The [media contract](m3-images.md) bounds input/chunks/output, rejects metadata/animation and retains upstream checksum checking. The 8 MiB decoder-internal limit is best effort. Existing QuickJS provenance/maintenance uncertainty, lru 0.16.4 RUSTSEC-2026-0253 and ttf-parser 0.25.1 RUSTSEC-2026-0192 findings are unchanged by this adoption.

## Notices and primary references

Exact selected MIT notices for the eight newly active package versions are retained under docs/licenses: [png](licenses/png-0.18.1-MIT.txt), [crc32fast](licenses/crc32fast-1.5.5-MIT.txt), [fdeflate](licenses/fdeflate-0.3.7-MIT.txt), [flate2](licenses/flate2-1.1.11-MIT.txt), [miniz_oxide 0.8.9](licenses/miniz_oxide-0.8.9-MIT.txt), [miniz_oxide 0.9.1](licenses/miniz_oxide-0.9.1-MIT.txt), [adler2](licenses/adler2-2.0.1-MIT.txt) and [simd-adler32](licenses/simd-adler32-0.3.10-MIT.txt). These additions do not replace the full distributed dependency/license inventory.

Consulted November 24, 2026:

- [png 0.18.1 API](https://docs.rs/png/0.18.1/png/), exact downloaded Decoder/Reader/Limits source and [image-rs security policy](https://github.com/image-rs/image-png/security).
- [crc32fast 1.5.5 API](https://docs.rs/crc32fast/1.5.5/crc32fast/) and exact downloaded build script/source manifests.
- Public crates.io version/owner APIs for each new active package, the resolved Cargo metadata/tree and exact Cargo.lock checksums.
- [OSV query API](https://google.github.io/osv.dev/post-v1-query/) and its querybatch endpoint for exact crates.io versions. Local raw results are ignored under target/png-dependency-review/osv-2026-11-24.json.
- [wgpu 30.0.1 texture upload](https://docs.rs/wgpu/30.0.1/wgpu/struct.Queue.html#method.write_texture) and [texture formats](https://docs.rs/wgpu/30.0.1/wgpu/enum.TextureFormat.html) for the fixed preview's sRGB upload path; these references do not constitute runtime evidence.
