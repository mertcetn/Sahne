# M3 additional effect fixture — GPU validation pending

Updated: November 24, 2026. Status: original source fixture, independent scene/asset checks and CPU reference tests are implemented. The new `groups-effects` mode is wired to the existing group renderer, but has not opened a GPU/device/window or produced a visual reference. Existing [group-preview evidence](m3-group-preview.md) covers its earlier, simpler fixture only. These cases do not close G3 until the appropriate runtime/visual checks pass.

## Scope and entry point

examples/group-effects is a separate original source package. It preserves all previous examples and reuses an unchanged copy of the project's 8x8 RGBA quadrant PNG; there is no third-party sample media or script. Its three regions share a 1920x1080 canvas and the existing compiler/renderer policy. `groups` still selects examples/group-preview, while the new literal `groups-effects` selects this package. Neither accepts a path/URL, arbitrary input or new permission.

Nonvisual validation:

```powershell
cargo test -p wallpaper-preview --locked
```

The following opens a window and is deferred until the user is ready for screen/window access:

```powershell
cargo run --release -p wallpaper-preview --locked -- --example groups-effects --seconds 10
```

The existing resize/lifecycle/device-loss code is shared, but these combinations have not run for this new fixture. Help explicitly marks its GPU/window/visual validation as pending. The new mode does not expand accepted CSS or change rendering algorithms, limits, blending, shaders or sandbox policy.

## Case A: nested opacity and inner clipping

The outer group is at `(80,160)`, 400x280, opacity 0.5, with a transparent background and clipping. Its inner group begins at local `(20,20)`, is 300x220 and has another opacity 0.5. Two opaque red/blue 160x160 children overlap; blue is later in source order. Blue extends 20 pixels below the inner clip. A separate opaque white 60x60 badge at outer-local `(320,20)` belongs only to the outer opacity boundary.

For linear-light premultiplied source-over, the overlap must show 0.25 blue plus 0.75 canvas background, with no red contribution. Red-only content likewise contributes 0.25 red. The white badge contributes 0.5 white, distinguishing one opacity boundary from two. Blue outside the inner clip must disappear even while still inside the outer group's rectangle. These equations are CPU reference expectations, not GPU pixel measurements.

| Canvas pixel-center sample | Expected linear RGB, where B is the canvas background |
| --- | --- |
| `(190.5,270.5)` | `0.75 B + (0,0,0.25)` |
| `(110.5,190.5)` | `0.75 B + (0.25,0,0)` |
| `(410.5,190.5)` | `0.5 B + (0.5,0.5,0.5)` |
| `(190.5,410.5)` | B; outside the inner clip |

The CPU plan requires target 2 to composite into target 1, then target 1 into root target 0, each at opacity 0.5. Paint order is retained, rather than replacing the two groups with leaf alpha.

## Case B: nonuniform text and image scale

The middle panel starts at `(620,160)`. Its child at local `(20,20)` scales X by 1.5 and Y by 0.75, with explicit top-left origin. The same `Wide / short` text also appears untransformed below as a control, using the same authored 28 px font. The transformed and control text differ in color to identify them; color equivalence is not the comparison.

| Output | Required canvas rectangle |
| --- | --- |
| Transformed text | `(640,180)`, 270x30 |
| Transformed PNG | `(640,225)`, 240x90 |
| Untransformed text control | `(640,370)`, 400x40 |

The transformed text raster is 270x60 at raster scale 1.5, mapped to the 270x30 world quad; the control raster is 400x40 at scale 1. The PNG keeps all 256 original decoded bytes, including straight half-alpha pixels. The CPU tests check these dimensions/bytes and independent v2 asset matching. Actual stretched glyph appearance, multi-text atlas behavior and image compositing still need GPU/window observation.

## Case C: fractional clips and empty clipping

The right panel begins at `(1220,160)`. Two transparent 100x100 clip groups contain white 140x140 fills starting at local `(-10,-10)`, so every edge must be clipped. Their canvas clips are `(1240.25,180.25,100,100)` and `(1400.75,180.75,100,100)`. Both coarse scissors span 101x101 integer pixels; exact fragment pixel-center checks must distinguish the fractional boundaries.

| Pixel-center sample | Expected result |
| --- | --- |
| `(1240.5,181.5)` / `(1400.5,181.5)` | White / panel background |
| `(1340.5,200.5)` / `(1500.5,200.5)` | Panel background / white |
| `(1250.5,180.5)` / `(1410.5,180.5)` | White / panel background |
| `(1250.5,280.5)` / `(1410.5,280.5)` | Panel background / white |

A third group has zero width; its oversized white child must produce no paint operation and leave the panel unchanged. Its sample `(1560.5,200.5)` remains panel background. Empty clipping does not skip source validation or weaken admission limits.

These are exact native-canvas CPU reference checks. A scaled window or JPEG screenshot cannot establish exact GPU pixel/color agreement; retain that distinction when recording the future visual result. No GPU readback/golden comparison is implemented by this step.

## Budgets, validation and next checkpoint

The source produces 44 layers and 66 group/draw commands. Its CPU plan has three canvas targets, 22 paint/composite operations, seven text rasters and 25,643,936 known texture bytes. The zero-width clip is culled; all existing source, image, hierarchy, text-surface, uniform and device limits remain. Texture bytes exclude the same driver/font/cache/baseline costs documented in the [renderer contract](m3-group-preview.md).

Three added tests independently compile/decode the fixture and validate immutable images, then check nested source-over samples/order, nonuniform text/image/control geometry and fractional/empty clip samples. They use the CPU reference evaluator and create no device/window. They do not certify GPU results. No external dependency, feature/version, license or Cargo.lock change is introduced; existing dependency advisories and production-helper requirements remain unchanged.

Primary references consulted November 24, 2026: [W3C compositing groups](https://www.w3.org/TR/compositing-1/#groupcompositing) and [wgpu 30.0.1 scissors](https://docs.rs/wgpu/30.0.1/wgpu/struct.RenderPass.html#method.set_scissor_rect), alongside the existing native transform/clip and renderer contracts. The implementation still uses the project's deliberately restricted semantics, not browser conformance.

Next, with user authorization for screen/window access: observe all three regions, retain a limited visual reference, maximize/restore and run quiet-idle/recreate/device-loss checks for the new literal mode. Record fractional-pixel GPU accuracy as unverified unless a suitable quantitative GPU comparison is performed. Broader G3 acceptance, native animation/pause, hashes/I1 and production integration remain open.

## Local nonvisual evidence — November 24, 2026

All 109 workspace Rust tests passed, including the preview's sixteen tests and the three added effect-fixture checks. Workspace release build, Clippy with warnings denied and formatting passed. Base revision during validation: 7406452 with this continuation uncommitted. The original compiler/preview fixtures remain separate; no previously recorded visual reference was overwritten. Existing Cargo cache hardlink/copy notices remain.

Cargo.lock/external dependencies and native sandbox policy/code are unchanged; the native corpus was not repeated for this fixture/selection-only continuation. No window, adapter/device, screen capture or UI interaction occurred. Hosted CI, non-Windows builds and this new fixture's GPU/window/visual/lifecycle checks were not run. Work stops at the user's screen-access checkpoint; CPU samples alone do not satisfy G3 visual acceptance.
