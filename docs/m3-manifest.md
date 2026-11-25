# M3 native manifest and retained source package

Date: November 14, 2026. Status: typed manifest and immutable in-memory entries implemented; fixed AppContainer fixtures only. Filesystem/archive collectors, HTML/CSS compilation, media decoding and arbitrary package execution are not enabled.

## Run the fixtures

On Windows/PowerShell 7 from the repository root:

```powershell
./tools/test-sandbox.ps1 -Cases @('package-basic', 'package-invalid')
cargo test -p wallpaper-format --locked
```

The first command uses original embedded manifest/HTML bytes inside the existing zero-capability AppContainer/Job. Each case must exit zero, report `Probe passed` and verify one profile cleanup. `package-basic` checks retained source and metadata; `package-invalid` requires rejection of permissions, unsupported version, traversal, excessive FPS and a case-colliding entry. Neither loads files nor interprets JavaScript. The trusted report mask is `0x40017`, using the existing result body without a new wire kind.

The pure library tests additionally cover missing/unknown/duplicate/escaped-duplicate fields, types, script null, malformed UTF-8/JSON, diagnostic redaction, path/device/URL forms, file-parent conflicts, missing sources and count/size limits. A fixture containing a throwing JavaScript string only verifies retained source; that string is never executed.

## Manifest v1

`wallpaper-format::Manifest::parse` accepts at most 16 KiB of UTF-8 JSON. The typed schema rejects unknown fields, duplicate fields, trailing data and missing required fields. No generic JSON value tree or field flattening is used. JSON syntax/schema failures retain parser line/column numbers, but not the parser's untrusted error text. Semantic errors identify a fixed field name; source-span refinement remains compiler work.

| Field | Requirement |
| --- | --- |
| `formatVersion`, `apiVersion` | Required integer 1; booleans, fractions, strings and unsupported versions rejected |
| `id` | 1..128 ASCII bytes; lowercase letters/digits plus dot, underscore, hyphen; first character alphanumeric. Opaque identity, never a filesystem path |
| `name` | Nonempty/non-whitespace UTF-8, at most 256 bytes, no Unicode control characters |
| `entry` | Required validated package-relative path ending in lowercase `.html` |
| `permissions` | Required empty JSON array; no capability can be requested |
| `preferredFps` | Required integer 1..60; a preference still subject to host policy, not a grant to change host limits |
| `script` | Optional validated path ending in lowercase `.js`; if present must be a string, not null |

## Portable paths and entry ownership

Paths are exact-case, slash-separated relative ASCII names, at most 240 bytes, 32 components and 64 bytes/component. Components allow ASCII letters/digits, dot, underscore and hyphen. Empty, dot/dot-dot and trailing-dot components are rejected, as are Windows reserved CON/PRN/AUX/NUL and COM0..9/LPT0..9 stems, including extension forms. COM0/LPT0 are conservatively excluded alongside the documented reserved range.

Absolute, drive-relative, UNC, alternate-stream, backslash, URL, percent-escape, query/fragment, whitespace and non-ASCII forms consequently fail. There is no path normalization, percent decoding or external URL resolution. This is a deliberately narrow portable native naming policy, not a promise that arbitrary imported names work unchanged. Future importers must map unsupported source names without expanding authority.

`Package::from_entries` consumes owned `(path, bytes)` entries and retains them immutably. It checks at most 1,024 entries, 16 MiB/entry and 64 MiB total, including the manifest and unused entries, with checked aggregate accounting before building indexes. Full-path ASCII case collisions and exact duplicates fail. A file cannot also be a parent of another entry, independent of order or case. Lookup is exact-case; it never silently corrects a reference to another entry.

Exactly named `manifest.json` must exist. The HTML entry and optional JS source must exist with exact case and valid UTF-8. Other entries remain bounded opaque bytes; this does not decode media or infer capabilities from a filename. Compiler resource references will use the retained bytes rather than reopening mutable paths.

This constructor does **not** establish filesystem provenance: it receives bytes, has no OS-path or extraction API and cannot certify that a caller rejected links/reparse points. A future collector must open/validate regular files safely, enforce budgets during reads and preserve handle/byte identity before claiming directory/archive import support. No user-facing external file path is added in this continuation. Package hashes/compiled asset indexing and source/compiler diagnostics remain pending M3 work.

## Isolation and validation scope

The two actual-process fixtures demonstrate this library under the unchanged Windows sandbox, not a general importer or production package validation service. AppContainer identity, Job limits, protected pipe handles, baseline direct denials, watchdogs and cleanup remain active. Input metadata cannot alter them. The ordinary CLI still accepts only its fixed scene self-check; there is no arbitrary JSON/HTML/CSS/JS path command.

The [JSON dependency/security review](m3-json-dependencies.md) records the exact active graph, advisories, provenance, build scripts, licenses and unresolved risks. Complete G2 and G3 remain open. Hosted CI and non-Windows runtime validation have not been performed.

Local validation passed all 47 workspace Rust tests and the full 37-case native corpus with 43 profile cleanups and one registry cleanup in report `bf43adbb42234baa9b356a8d03c3cc8e`. After replacing literal version checks with public version constants, the final release build and both package cases passed again in `7e7d51f7a50343ca86401c3ae2cebff7`. Formatting, Clippy and the unchanged scene CLI check passed. An initial concurrent release build encountered Windows' running-EXE lock; the sequential rerun succeeded. Existing Cargo incremental-cache notices remain.

## Bounded snapshot from open readers

`Package::from_readers` accepts entries supplied by an already authorized collector. It validates each relative path before reading. It counts bytes during reads and enforces the 16 KiB manifest, 16 MiB per-file, 64 MiB aggregate and 1,024-entry limits. One extra byte detects overflow at the exact limit. The completed snapshot goes through the existing manifest, reference and collision validation. Collector and read errors identify the entry index and I/O kind without carrying filesystem paths.

Callers establish regular-file and containment guarantees before passing readers and supply operational deadlines if needed. The Windows directory collector still needs handle-relative opens and reparse rejection. Path checks followed by a fresh open by name leave a window for directory replacement. The reader interface gives that future collector a single bounded ingestion path. [Windows file opening semantics](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)
