# M3 opt-in group source compiler

Updated: November 24, 2026. Status: `compile_groups` compiles the strict native source subset into independently validated version-2 groups and image assets. The existing `compile` API, default CSS parser and preview still use version 1. GPU group drawing, effect visuals, animation, production source transport and full G3 remain open. No arbitrary package path or script execution is enabled.

## Checks and original sources

```powershell
cargo run --release -p wallpaper-cli --locked -- --compile-groups-check
cargo test -p wallpaper-compiler -p wallpaper-media --locked
./tools/test-sandbox.ps1 -Cases groups-basic,groups-invalid
```

The CLI embeds examples/static-groups and compiles its HTML/CSS/PNG in memory. It independently decodes the scene and matches immutable image assets to the decoded declarations. Expected output: version 2, ten layers, fifteen commands, one image and 439 encoded scene bytes. Image pixels remain separate: 256 RGBA bytes for the same original 8x8 quadrant pattern as examples/static-image, copied without changes. The example has a half-opacity clipped/transformed panel, overlapping opaque red/blue boxes, a text box and the PNG. It is a compiler fixture; there is no visual reference for it yet. The original static-scene/static-image fixtures remain unchanged.

`groups-basic` and `groups-invalid` are fixed Windows AppContainer cases, using the existing zero capabilities, Job limits, watchdog and channel policy. Normal combined report is `0x400017`; only this bounded result is sent, not a scene/image payload. Positive checks include transformed coordinates, group identity and pixel bytes. Negative checks require rejection of rotation, out-of-range opacity, scrolling overflow and a missing explicit transform origin. The default native runner includes both cases; production transport remains M4 work.

## Explicit compiler modes

`compile` and `css::parse`/`css::declarations` retain the original flat contract, including rejection of all effect properties. `compile_groups` uses `css::parse_groups`/`css::declarations_groups`, sharing the existing grammar, source/stylesheet/declaration limits, cascade, text/image handling and diagnostics. Mode selection is an explicit trusted API decision, not a package manifest field or permission. Neither mode executes the optional script; its validated source reference is only retained.

The group result exposes an immutable `ValidatedHierarchy`, decoded images, a deferred script association and author symbols. Each author ID maps to one generated group ID. Version-1 symbols still map to their existing primitive-ID lists. These are scene-local compiler associations, not live host handles, cross-session IDs or a completed mutation/import API.

## Additional properties in group mode only

| Property | Accepted values and defaults |
| --- | --- |
| opacity | Plain finite decimal in 0..1; default 1; out-of-range values are rejected, not clamped |
| transform | `none`; `translate(tx,ty)`; `scale(s)` or `scale(sx,sy)`; optionally translate followed by scale; default none |
| transform-origin | Exactly two zero-valued lengths using the existing px/literal-zero grammar, such as `0 0` or `0px 0px` |
| overflow | `visible` (default) or `clip`; no scrolling container or scroll API |

Transform translation uses existing fixed-pixel numbers in -32,768..32,768; scale uses unitless finite decimals in 0..16. Numeric spelling remains deliberately narrow: at most 32 bytes per number, no exponent, trailing dot, percentage, calc/var or other units. A transform value is at most 160 bytes, with at most one translate and one scale function. Function names must immediately precede `(`; comma-separated arguments are required for translate and two-axis scale, and separate functions require ASCII whitespace. Reversed order, duplicates, adjacent functions without whitespace, extra arguments, rotation/skew/matrix, negative scale and arbitrary function lists fail. This is a documented CSS subset, not a complete CSS token/function parser.

A winning non-`none` transform requires an explicit accepted transform-origin on that same element, even for identity scale/translation. The compiler never guesses CSS's default center or silently changes it to the native top-left origin. Missing origin reports `MissingTransformOrigin` at the HTML element. `none` does not require an origin. Only the top-left origin is accepted; keywords, percentages, nonzero values and a third coordinate fail. These four effect properties are not inherited. Color/font-size retain the existing inheritance; all properties retain per-property specificity and source-order rules, with inline declarations taking precedence.

## Tree, layout and resource mapping

Every authored div/span/img becomes a v2 group with local authored bounds and its own effects. The group contains its generated background/text/image primitives at local `(0,0)` with the authored width/height. A div's background paints before its child groups; a span/image background paints immediately before its content. Zero-alpha div backgrounds remain explicit, preserving the existing primitive behavior. Source preorder and parser parent indices produce contiguous subtrees; positions are not accumulated into flat child coordinates. The [v2 validator](m3-groups.md) independently computes ancestor transforms/clips and preserves group-opacity boundaries. Primitive colors/alpha are never multiplied by parent opacity.

Groups and draws share the existing total 4,096-layer budget. For example, 2,048 divs with one background each fit, while 2,049 do not; text/image backgrounds may consume additional layers. Existing HTML/source/CSS/text/resource limits apply as well. Accumulated world-scale/coordinate and conservative intermediate reservations are validated after generation, even though GPU allocation is not implemented yet. New source syntax does not weaken any existing limit.

PNG lookup, exact-case lowercase `.png` references, path deduplication and decoded-byte budgeting share the existing media path. `validate_hierarchy_assets` checks the exact resource table of an independently decoded v2 scene against immutable RGBA images: missing/extra IDs and dimension substitutions fail, including unused declarations. It shares the same bounded table checks as v1, without copying pixels or treating IDs as authenticated authority. File/archive collection, content hashes and production resource IPC remain open.

## Diagnostics, security and remaining work

Malformed/unsupported effect declarations fail even in unmatched stylesheet rules and retain stylesheet file/line/byte-span diagnostics; inline declaration offsets stay in the original HTML. Missing transform origin and final scene/effect budget failures map generated group/draw indices back to the originating HTML element. The latter identify the affected element, not necessarily the stylesheet declaration responsible for the accumulated failure. No raw source text or upstream image error string is included.

No external dependency, feature/version, native binding, unsafe exemption or license change is needed. Cargo.lock remains unchanged. Shared parser refactoring preserves version-1 checks and fixed encoded outputs; existing dependency advisory/maintenance uncertainty remains as previously documented. Original source fixtures and finite mutation tests do not establish arbitrary-input security. Keep restricted helper processing, OS limits and bounded diagnostics before production untrusted input.

Primary documentation consulted November 24, 2026: [CSS transform functions](https://www.w3.org/TR/css-transforms-1/#transform-functions), [CSS opacity](https://www.w3.org/TR/css-color-4/#transparency), [CSS overflow control](https://www.w3.org/TR/css-overflow-3/#overflow-control) and the existing [native group contract](m3-groups.md). The explicit syntax/origin/range restrictions above govern this prototype; full browser behavior is not implied.

The later [group renderer implementation](m3-group-preview.md) consumes v2 output in a separate experimental mode, with independent asset checks, bounded intermediates and ancestor clipping. Its GPU/window/visual validation is still pending: overlap opacity, transformed text/images, resize, idle and recovery must pass before claiming rendered support. Existing v1 preview modes remain separate. Native animation/timing, compiled hashes, I1 mappings and full G3 remain unfinished work.

## Local evidence — November 24, 2026

All 98 workspace Rust tests passed: the compiler now has 27 tests (eight new group tests) and media has ten (one new independent v2 asset-table test). New coverage includes exact original fixture output, v1 mode separation/regression, non-inherited effect cascade, transform grammar/origin rejection, unmatched-rule stylesheet locations, accumulated-effect HTML locations, the shared group/draw limit, 256 deterministic source mutations and missing/extra/substituted v2 image assets. This is finite regression coverage, not an exhaustive fuzz or visual/security proof.

Workspace release build, Clippy with warnings denied, formatting and the five release CLI checks passed. The original typed hierarchy check remains 185 bytes; v1 scene/static/PNG checks remain 147/123/136 bytes. The default native suite passed 45 cases, 51 worker launches/profile cleanups and one registry cleanup. Report: ignored target/sandbox-validation/f84d2cfe1d38474dab50dda857cc7435/summary.json, base revision 5292a48 with this continuation uncommitted during validation. It retains exact binary/lock hashes and `G2Complete=false`.

Cargo.lock is unchanged from the base revision. Existing incremental-cache hardlink/copy notices remain. No window or screen access occurred; group GPU/visual checks, hosted CI and non-Windows builds were not run. The source/compiler behavior is reviewable through the nonvisual command above; no duplicate user testing is required for these automated checks.
