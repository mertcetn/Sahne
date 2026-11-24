# M3 version-2 group-data boundary

Updated: November 24, 2026. Status: typed hierarchical scene data, bounded codec, derived transform/clip validation and explicit ordered group commands. A later [opt-in source compiler](m3-group-compiler.md) maps a restricted effect subset to this data and validates its images; the default compiler and preview still use v1. No group GPU rendering, visual acceptance or animation is claimed by these steps.

## Why preserve groups

Applying an author's opacity separately to each child gives the wrong result where children overlap. A group must first paint its children in order, then apply its opacity once to the combined result. The experimental version 2 retains that boundary rather than reducing it to flat primitives. It adds a separate `wallpaper_scene::hierarchy` API; the existing version-1 API/codec and original static/PNG fixtures remain unchanged. Neither decoder silently accepts the other revision.

The new CLI check operates on original typed data only:

```powershell
cargo run --release -p wallpaper-cli --locked -- --hierarchy-check
cargo test -p wallpaper-scene --locked
./tools/test-sandbox.ps1 -Cases hierarchy-basic,hierarchy-invalid
```

The CLI validates, encodes, independently decodes and compares both scene and derived state. Expected fixture output: version 2, four layers, five commands and 185 encoded bytes. No window, file lookup, source parsing or script execution occurs. The two native cases run in the existing zero-capability AppContainer with unchanged Job/channel/watchdog policies; their exact result bit is `0x200000`, combined normal report `0x200017`. No hierarchical scene payload is transported over the control channel. Production source/scene IPC remains M4 work.

## Tree and paint order

Every layer has a unique nonzero scene-local ID, a parent ID, a local rectangle and either group properties or one existing rectangle/text/image primitive. Parent zero means the implicit canvas root. A nonzero parent must name a currently open group in depth-first preorder. Every subtree is contiguous; a layer cannot reopen a completed group, refer forward, parent itself, use a primitive as a parent or create a cycle. Root zero is never a declared layer.

Siblings paint back to front in their stored order. Group contents finish together before the following sibling paints. There is no z-index, browser stacking-context algorithm, blend mode or paint-order sorting. Validation derives a bounded command list of BeginGroup/Draw/EndGroup with original layer indices; the last group is explicitly closed. Opacity values stay on group records, and primitive colors/alphas remain unchanged. A future renderer must consume these boundaries; a flat draw list with multiplied child alpha does not satisfy this contract.

## Transform and clipping contract

The native transform is axis-aligned translate plus nonnegative scale, around the group's local top-left origin. In each axis, a local point maps to its parent's coordinates as: group rectangle origin + translation + scale times local point. A parent then maps that result to the canvas. Thus local scale acts before translation; parent scale also affects a child's origin and translation. There is no rotation, skew, reflection, perspective or general matrix input. This native origin differs from CSS's general default transform origin; source mapping must explicitly define the restricted behavior before enabling CSS properties.

A group's rectangle determines its local width/height and placement, not automatic content layout. Its clip flag, when true, intersects the transformed group rectangle with the existing canvas/ancestor clip. When false, descendants can overflow that group's rectangle but remain subject to inherited clips and the canvas boundary. Primitive bounds are local to their containing group and map through its transform. Empty intersections and zero scale produce empty bounds/clips; their remaining data still undergoes validation. Text's own-box clipping remains a separate renderer requirement, not a replacement for ancestor clips.

Immutable validated data exposes derived per-layer canvas bounds, local-to-canvas axis transform and effective rectangular clip. Derived state is recomputed after decoding and never trusted from the wire. Group opacity is deliberately absent from this accumulated transform/clip calculation; it is not reduced to a cumulative leaf alpha.

## Opacity/compositing contract and budgets

Group opacity is finite in 0..1 and affects the entire subtree, including a group's own background primitives. The intended renderer paints an isolated transparent intermediate, then composites that result once over the parent in normal source-over order. In the existing linear-light rendering contract, group opacity scales both premultiplied RGB and alpha at that composite step. Nested group opacity applies separately at each boundary. This specifies required semantics; no intermediate render target or reference image exists yet in this continuation.

The existing 4,096-node budget counts all groups and primitives together; the 1,024-resource, 2 MiB scene, local geometry, text, canvas and 128 MiB declared-image budgets remain. Additional v2 policy:

| Value | Limit |
| --- | --- |
| Group ancestors | At most 32; the implicit canvas root is not counted |
| Local translation | Finite, magnitude at most 32,768 px per axis |
| Local scale | Finite, 0..16 per axis |
| Accumulated scale | At most 64 per axis |
| Transformed rectangle origin and far edges | Finite, magnitude at most 1,048,576 canvas px |
| Groups with opacity other than 1 | At most eight in the entire scene, including zero-opacity and sequential groups |
| Conservative intermediate storage reservation | Canvas width × height × 4 × number of opacity groups, at most 128 MiB |
| Derived commands | At most twice the layer count |

These new provisional bounds prevent deep scale multiplication and unbounded offscreen storage while allowing zero-scale animation endpoints later. The reservation assumes full-canvas RGBA8 intermediates, deliberately counts invisible/sequential groups and does not depend on target reuse or culling. It is an admission estimate for future rendering, not proof of actual device memory usage. Other texture formats, staging, driver overhead, buffer reuse and allocation failure require explicit renderer checks; do not allocate unlimited targets or weaken this bound to make a fixture render. At the largest 32-million-pixel canvas, only one such full-canvas target fits this reservation.

Validation is iterative. Local primitive/resource/geometry/text rules reuse the unchanged v1 validator through a temporary bounded view with neutral group fills. Aggregate text is checked before cloning that view, so its extra text copy is limited to 1 MiB. That view is discarded and never rendered, encoded as v2 output or substituted for the hierarchical data. All layer IDs, including groups, participate in the same uniqueness budget. OS isolation/timeouts still matter before accepting untrusted input.

## Experimental wire layout

All numbers are little-endian, including IEEE binary32 values. The 30-byte header retains the existing magic `SAHNSCN` plus NUL, but requires u16 version 2; canvas/background/resource count are unchanged and the last count is the layer count. Image declarations remain 12 bytes each. Each layer begins with u32 ID, u32 parent ID, four f32 local rectangle values and one tag: a 25-byte prefix.

| Tag | Payload after prefix |
| --- | --- |
| 1 rectangle | RGBA4 |
| 2 text | RGBA4, f32 font size, u32 UTF-8 length, exact UTF-8 bytes |
| 3 image | u32 declared resource ID |
| 4 group | f32 translation x/y, f32 scale x/y, f32 opacity, one clip byte (exactly 0 or 1) |

A group record totals 46 bytes; the shortest primitive record totals 29. Count limits and minimum remaining bytes are checked before vector allocation; text lengths/aggregate budget are checked before string allocation. Truncation, unknown versions/tags/flags, nonfinite values and trailing bytes reject the whole scene. Even combined permitted node/resource/text maxima fit the existing 2 MiB envelope. This is an experimental scene revision, not a stable creator package format or a new permission to load arbitrary data.

## References, validation and remaining work

Primary references consulted November 24, 2026: [W3C group compositing](https://www.w3.org/TR/compositing-1/#groupcompositing), [CSS transform rendering](https://www.w3.org/TR/css-transforms-1/#transform-rendering) and [CSS overflow](https://www.w3.org/TR/css-overflow-3/). These inform compositing, transform order and overflow terminology, rather than claiming full CSS conformance. The narrower native grammar and origin above are deliberate. Rust implementation uses the existing bounded scene reader and long-stable finite-number, checked arithmetic and safe slice APIs.

Eleven new tests cover group order/opacity retention, nested transform order/clip intersection, parent/subtree failures, depth/node/effect/world/intermediate budgets, zero scale/opacity, exact minimal bytes, every truncated fixture prefix, bad flags/counts/text lengths, version separation, existing resource/text rules, combined maxima and 1,024 deterministic byte mutations. This is finite regression coverage, not exhaustive fuzzing or security certification.

No external dependency version/feature, native binding, unsafe exemption or license material is added. The new local scene `fixtures` feature only exposes original typed test data to CLI/probe builds; it performs no I/O. The exact existing external lockfile graph and its previously recorded advisories/maintenance uncertainty remain unchanged. Only local Windows validation is intended here; other targets remain untested.

The [opt-in group source compiler](m3-group-compiler.md) provides source-tree mapping, diagnostics and independent v2 image checks. A later [bounded renderer implementation](m3-group-preview.md) now needs GPU/window/visual acceptance, including overlapping-child references that distinguish group opacity from per-child alpha. Keep the existing v1 examples/regression checks until an explicit migration is validated. Animation/timing, I1 mappings, compiled hashes and full G3 remain open.

## Local evidence — November 24, 2026

All 89 workspace Rust tests passed, including 25 scene tests (fourteen existing flat-scene tests and eleven new hierarchy tests). Workspace release build, Clippy with warnings denied, formatting, documentation links and diff checks passed. Release CLI checks produced the expected v2 four-layer/five-command/185-byte fixture and unchanged v1 scene/static/PNG outputs (147/123/136 encoded bytes respectively). Preview help passed without opening a window. Cargo.lock is byte-for-byte unchanged from base revision 80fccea.

The full native suite passed 43 cases, 49 worker launches and profile cleanups, and one registry cleanup. Report: ignored target/sandbox-validation/f83b092f551b4597a00a6d3cb352cde8/summary.json, base revision 80fccea with this continuation uncommitted during validation. Exact binary/lock hashes are retained in that report, with `G2Complete=false`. Existing incremental-cache hardlink/copy notices remain. No window/screen access was performed; hosted CI, non-Windows builds and group rendering/visual checks were not run. No user rerun is required for these automated checks.
