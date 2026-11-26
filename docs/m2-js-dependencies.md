# M2 JavaScript Engine Dependency Review

Reviewed: November 11, 2026, before building the new dependency. Decision: adopt exact rquickjs 0.14.0 for fixed trusted AppContainer fixtures only. This is not approval to enable arbitrary scripts/packages or a claim of engine safety.

## Necessity, features and build

M2 needs an embedded source interpreter to exercise cooperative interruption, runtime allocation/stack limits and bounded Promise jobs under the existing independent watchdog. rquickjs supplies a safe Rust interface without adding unsafe application code. The Windows-only probe dependency uses `default-features = false`, enabling only `std`. Cargo's actual feature tree confirms no loader, dyn-load, futures, parallel, macro, bindgen, allocator, rust-alloc, debugging dumps or disable-assertions feature. The C allocator is important: upstream documents that custom allocator/rust-alloc features make set_memory_limit ineffective.

The active engine graph contains ten registry packages: rquickjs/core/sys 0.14.0, hashbrown 0.17.1, allocator-api2 0.2.21, equivalent 1.0.3, foldhash 0.2.0, cc 1.11.1, find-msvc-tools 0.1.15 and shlex 2.0.1. The seven non-rquickjs packages already existed in the workspace lockfile. The lockfile adds the three engine crates and inactive relative-path 2.0.3; this optional loader-related package is absent from the active probe tree. hashbrown's default features additionally unify allocator-api2/equivalent into the existing graph; this is a feature change even though its version is unchanged.

rquickjs/core/sys and the bundled engine are MIT licensed; relative-path is MIT OR Apache-2.0. Existing transitive licenses remain as recorded in [M1 review](m1-dependencies.md), including foldhash's Zlib license. Exact engine notices are retained in [rquickjs notice](licenses/rquickjs-MIT.txt) and [QuickJS-NG notice](licenses/QuickJS-NG-MIT.txt). Review the complete artifact's licenses before distribution; the release workflow still excludes the probe executable.

The reviewed sys build script copies four C sources and fourteen headers and compiles them using cc, with packaged Windows x64 bindings instead of runtime bindgen. Its MSVC branch selects C11/experimental C11 atomics and overwrites CFLAGS; custom downstream compiler hardening flags therefore need separate review. Its WASI SDK download branch is not taken on this Windows target. No source-patching or disabling assertions was used to get the local build to work. Rust 1.99.1/MSVC builds successfully; non-Windows builds and hosted CI for this new dependency are not yet verified.

## Publisher and exact distributed source

crates.io lists DelSkayn (Mees Delzenne) and katyo as rquickjs owners, matching the upstream project identity. Version 0.14.0 was published September 18, 2026 and was not yanked at review time. The project has recent published work, a public issue tracker and active upstream engine development. Publisher/maintenance evidence does not replace code review or establish supply-chain integrity against a compromised account.

All three downloaded crate VCS records point to rquickjs commit `d7ef5eeae702fea24c03643064de454f1c1dd4b0`. Its engine gitlink is QuickJS-NG commit `0fdea21ff1090084e91dad812b343d92e79ba9d9`. The bundled header declares 0.16.2, but this is a pinned snapshot, not an assertion that it is identical to the v0.16.2 release tag. All eighteen compiled C/header files were compared using Git blob hashes against that exact upstream engine tree and matched. GitHub's compare API reports this snapshot contains v0.16.0, with 45 commits ahead and none behind.

| Published crate | SHA256 of downloaded .crate archive |
| --- | --- |
| rquickjs 0.14.0 | a0a22bf72f515cc4189dd7c63d016f6de92a11eee98ddf7dd64794198f6eb34e |
| rquickjs-core 0.14.0 | 6657979a24d543e5fc66f32cef2f47ca82ce8eb55cec5f7d59a12b7b570bbf24 |
| rquickjs-sys 0.14.0 | cee271d0eeba64f0915b846cb7ae02e16faf3dfdffdca91731101d9d30fe3423 |

These identities are recorded independently of the Cargo.lock checksums and remain traceable to the published source. The comparisons are provenance checks, not a full audit of the Rust wrapper or C implementation.

## Security findings and uncertainty

Before building, OSV batch queries covered all seventeen registry versions from a separate metadata-only minimal resolution, then all 231 registry versions in the actual workspace graph. No new engine/Rust dependency advisory was returned. The full graph still returned RUSTSEC-2026-0253 for lru 0.16.4 and RUSTSEC-2026-0192 for ttf-parser 0.25.1; their existing scoped assessment remains in [M1 dependency review](m1-dependencies.md). The rquickjs repository's public advisory API returned zero entries. None of these results establishes that a dependency is safe; crate databases do not comprehensively cover embedded C.

The separate QuickJS-NG repository exposed ten public advisories. Review considered the actual bundled C snapshot, not the unrelated Rust crate version number:

| Advisory | Issue category | Upstream patch/version information |
| --- | --- | --- |
| GHSA-m53g-qx47-3h38 | Array-copy GC memory corruption/RCE | Affected <0.14.1; patch marker 0.15.0 |
| GHSA-r6vf-p8wr-4vg9 | Array-length use-after-free | Affected <0.14.0; no patch version in API |
| GHSA-v647-xff9-jxf7 | Error stack allocation-failure use-after-free | Patch marker 0.16.0 |
| GHSA-crgj-9jv8-xfhq | Promise allocation-failure use-after-free | Patch marker 0.16.0 |
| GHSA-rxpx-wj25-jhqq | RegExp allocation-failure out-of-bounds write | Patch marker 0.16.0; affected metadata is unbounded >=0.15.1 |
| GHSA-3jf7-4qfx-xc2h | JSON indentation address disclosure | Patch marker 0.16.0 |
| GHSA-wm7w-r6qf-3c9w | Atomics/detached buffer memory leak | Patch marker 0.16.0 |
| GHSA-9wv5-5cgr-grj6 | 32-bit typed-array sort overflow | Patch marker 0.16.0 |
| GHSA-jp52-chwg-2p9p | 32-bit Proxy name-list overflow | Patch marker 0.16.0 |
| GHSA-52gx-967m-hxp3 | External ArrayBuffer transfer use-after-free | QuickJS-NG patch marker 0.16.0; advisory also lists other packages |

The snapshot includes the release identified by the latest patch markers. For the inconsistent RegExp range, the packaged re_parse_disjunction code was also inspected: it checks dbuf_error and returns on allocation failure before patching the branch offset. Do not silently reinterpret that advisory's metadata as a precise unaffected range. RegExp/typed-array/Proxy/JSON optional intrinsics are not installed in the current fixture context; this reduces this fixture's surface but is not a substitute for engine updates. No advisory proof-of-concept or sanitizer campaign was executed by this task.

Material maintenance risk: QuickJS-NG's current security policy directs reports to public issues and does not offer private security reporting or CVE issuance. Future disclosures may therefore appear in public issues/commits rather than a Rust advisory feed or CVE. Recheck these sources and the exact bundled C revision on every update. Source-only evaluation is mandatory: upstream explicitly excludes adversarial bytecode hardening; no external bytecode or native module is accepted here.

The wrapper and engine contain substantial native/unsafe code. Unknown memory-corruption defects, OOM cleanup bugs and missed interruption points remain credible risks. The OS sandbox must assume engine compromise; a 100 ms cooperative deadline is not OS preemption. Existing zero-capability AppContainer, 256 MiB Job limit, one-process limit, narrow executable access, explicit pipes and independent host deadline remain unchanged. Broader G2 gaps still prevent arbitrary content from being enabled. This limited trusted-fixture adoption is reviewable and reversible without widening the product's input surface.

## M4 advisory refresh

On November 26, 2026, the [OSV batch API](https://google.github.io/osv.dev/post-v1-querybatch/) returned a complete response for all 243 crates.io package versions in the workspace lockfile at `5f96690`. The lockfile SHA-256 was `5bc53c7d3b7657d735f8a1f1b3a6964fb1616df8d74a1dca4ad3157b4689affb`. The two returned findings were the same ones recorded in the earlier review.

| Locked crate | Windows dependency path and finding |
| --- | --- |
| `lru 0.16.4` | The preview uses it through `glyphon 0.12.0`. The existing cache-key applicability assessment is in the [renderer dependency review](m1-dependencies.md). [RUSTSEC-2026-0253](https://rustsec.org/advisories/RUSTSEC-2026-0253.html) |
| `ttf-parser 0.25.1` | The preview uses it through glyphon, cosmic-text and fontdb. The parser is unmaintained. Package fonts remain outside the preview contract. [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html) |

The Windows dependency tree still resolves `rquickjs-sys 0.14.0` for the sandbox worker. The upstream APIs exposed the same ten QuickJS-NG advisories and zero rquickjs repository advisories. The previously documented embedded C snapshot and advisory-range uncertainty still apply. The refresh checked public advisory records and the Windows dependency tree. Source re-audit, exploitability assessment for new runtime surfaces and integrated isolation acceptance remain open. [QuickJS-NG advisories](https://api.github.com/repos/quickjs-ng/quickjs/security-advisories), [rquickjs advisories](https://api.github.com/repos/DelSkayn/rquickjs/security-advisories), [QuickJS-NG security policy](https://github.com/quickjs-ng/quickjs/security/policy)

## Primary references consulted

- [Published rquickjs version](https://crates.io/crates/rquickjs/0.14.0), [owners API](https://crates.io/api/v1/crates/rquickjs/owners)
- [Pinned rquickjs source](https://github.com/DelSkayn/rquickjs/tree/d7ef5eeae702fea24c03643064de454f1c1dd4b0), [pinned engine source](https://github.com/quickjs-ng/quickjs/tree/0fdea21ff1090084e91dad812b343d92e79ba9d9)
- [Runtime limits and allocator caveat](https://docs.rs/rquickjs/latest/rquickjs/struct.Runtime.html), [context construction](https://docs.rs/rquickjs/latest/rquickjs/struct.Context.html)
- [QuickJS-NG security policy/advisories](https://github.com/quickjs-ng/quickjs/security), [all upstream advisories API](https://api.github.com/repos/quickjs-ng/quickjs/security-advisories)
- [RegExp advisory with inconsistent range](https://github.com/quickjs-ng/quickjs/security/advisories/GHSA-rxpx-wj25-jhqq)
- [Pinned RegExp guard](https://github.com/quickjs-ng/quickjs/blob/0fdea21ff1090084e91dad812b343d92e79ba9d9/libregexp.c#L2487)
- [v0.16.0 ancestry comparison](https://github.com/quickjs-ng/quickjs/compare/v0.16.0...0fdea21ff1090084e91dad812b343d92e79ba9d9)
- [OSV query API](https://google.github.io/osv.dev/api/)
