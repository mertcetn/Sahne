# M3 JSON dependency review

Reviewed: November 14, 2026, before adopting the new parser. Decision: exact serde_json 1.0.154 with only `std`, plus existing exact serde 1.0.229 with `derive`, for bounded typed manifest parsing in original fixed fixtures. This is not approval for arbitrary package execution.

## Necessity and version/feature selection

A maintained JSON parser avoids inventing a new general JSON/Unicode/escape implementation for native manifests. Typed Deserialize plus `deny_unknown_fields` rejects unknown and duplicate keys. The implementation bounds the input at 16 KiB first, validates its shallow field schema and redacts library error messages. Empty permissions use a custom bounded sequence visitor; script absence and explicit null remain distinct.

Only three registry packages are newly added to Cargo.lock: serde_json 1.0.154, itoa 1.0.19 and zmij 1.0.23. Existing versions remain unchanged. The initial offline candidate used itoa 1.0.18; before final validation it was updated to the current compatible 1.0.19 after checking its publication/advisory metadata. serde's derive feature is newly requested explicitly; its versions and macro dependencies already existed in the workspace lockfile.

The active format-crate graph has eleven registry packages:

| Package | Exact version | License |
| --- | --- | --- |
| serde / serde_core / serde_derive | 1.0.229 each | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | MIT OR Apache-2.0 |
| itoa | 1.0.19 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| zmij | 1.0.23 | MIT |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| syn | 3.0.10 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |

Cargo's resolved tree confirms no serde_json `unbounded_depth`, `preserve_order`, `arbitrary_precision`, `raw_value`, `float_roundtrip`, indexmap or no-panic integration. The standard recursion limit is retained. Macro dependencies are build-time code, not runtime authority for a wallpaper. Lockfile pinning and `--locked` retain the reviewed versions.

## Publisher, maintenance and build review

The primary crates.io version/owner endpoints identify dtolnay as publisher of serde_json, itoa and zmij; serde_json owners also include the serde-rs publish team, consistent with their upstream repositories. The selected releases are not yanked. serde_json 1.0.154 was published October 11, 2026, itoa 1.0.19 November 9, 2026 and zmij 1.0.23 July 13, 2026. Recent publication and matching identities are positive maintenance/provenance evidence, not protection against account compromise.

Downloaded exact Cargo manifests and build scripts were inspected. serde_json's build script selects an arithmetic configuration from Cargo target metadata. zmij's script invokes Cargo's RUSTC with `--version`, reads OPT_LEVEL and emits compatibility configuration. No download, shell hook, native compiler or network access occurs in those two scripts. itoa declares no build script. Existing Serde derive/proc-macro source remains a trusted build dependency with the maintenance/security context recorded in the earlier dependency inventory.

The new parsers use Rust APIs with MSRVs at most 1.71, below pinned Rust 1.99.1. Local Windows x64/MSVC compilation and actual AppContainer fixtures are the exercised targets. The graph is portable Rust, but non-Windows compilation/runtime support is not asserted from that fact alone.

## Current security assessment and uncertainty

Direct OSV API queries on November 14, 2026 returned empty advisory results for each of the eleven exact active versions above (including unicode-ident 1.0.27 and the final itoa 1.0.19). Package-version lookups used the crates.io ecosystem. No known advisory was returned; this is not a claim that these packages or their unsafe internals are safe.

The upstream serde-rs/json security page reports no SECURITY.md and no published GitHub security advisories at review time. That leaves a less explicit security-reporting process and is recorded as uncertainty. A source/publisher compromise, unpublished parser bug, excessive resource use within permitted input or macro/build-time compromise is still possible. Our review is targeted, not a full source audit. Keep byte/count/schema bounds, the default recursion limit and helper isolation; recheck before production untrusted input and dependency updates.

Existing QuickJS maintenance/source uncertainty and renderer lru/ttf-parser findings remain unchanged in their prior reviews. This JSON adoption does not resolve them or complete G2.

## Notices and sources

Exact selected runtime MIT notices are retained under docs/licenses: [serde](licenses/serde-1.0.229-MIT.txt), [serde_core](licenses/serde_core-1.0.229-MIT.txt), [serde_json](licenses/serde_json-1.0.154-MIT.txt), [itoa](licenses/itoa-1.0.19-MIT.txt), [memchr](licenses/memchr-2.8.3-MIT.txt) and [zmij](licenses/zmij-1.0.23-MIT.txt). The release workflow already includes docs/. Review the full distributed artifact and existing licenses separately; this list covers the selected runtime graph additions/features, not every workspace dependency.

Primary references consulted November 14, 2026:

- [serde_json API and feature documentation](https://docs.rs/serde_json/1.0.154/serde_json/)
- [Serde strict container attributes](https://serde.rs/container-attrs.html)
- [Upstream JSON security page](https://github.com/serde-rs/json/security)
- [serde_json version](https://crates.io/crates/serde_json/1.0.154), [itoa version](https://crates.io/crates/itoa/1.0.19), [zmij version](https://crates.io/crates/zmij/1.0.23) and their public crates.io version/owner API metadata
- [OSV query API](https://google.github.io/osv.dev/post-v1-query/) and querybatch endpoint, queried for the exact active versions

Downloaded Cargo sources supplied exact manifests, build scripts and notices; Cargo tree supplied active versions/features/licenses. No wallpaper script or input metadata condition was executed. Normal reviewed Cargo build scripts run during build validation.
