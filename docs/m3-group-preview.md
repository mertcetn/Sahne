# M3 group renderer implementation — runtime validation pending

Updated: November 24, 2026. Status: the original version-2 group fixture is wired to a bounded multi-pass renderer and passes CPU planning/shader tests and compilation. No adapter/device/window was opened for this continuation. GPU initialization, actual drawing, visual agreement, resize/idle/recovery and old-mode runtime regression remain unverified on this revision. M3/G3 are incomplete; do not describe group rendering as validated support yet.

## Entry point and authorization checkpoint

The new literal `groups` example selects only the original embedded examples/group-preview source package. It compiles through `compile_groups`, independently decodes the scene and validates immutable image assets before planning GPU resources. There is no external path, shader input, script execution or new permission. Existing default M1, `--example static` and `--example image` retain their previous paths.

The original examples/static-groups compiler fixture remains unchanged. The separate preview keeps its overlapping red/blue shapes but extends a long text box beyond the parent's right edge and moves the PNG beyond its right/bottom edges. The text world rectangle reaches x=950 while the parent clip ends at x=730; the PNG spans `(650,484)` through `(778,612)` against parent clip `(90,100)` through `(730,580)`. These cases make actual clipping observable rather than merely setting a clip flag on content that already fits. Its checker.png is an unchanged copy of the project's original quadrant pattern. The CLI compile-groups check still selects the original compiler fixture; preview CPU tests independently check this additional source package and its assets.

Nonvisual checks:

```powershell
cargo test -p wallpaper-preview --locked
cargo run --release -p wallpaper-cli --locked -- --compile-groups-check
```

The following command opens a window and is the next validation step, deferred until the user is ready for screen/window access:

```powershell
cargo run --release -p wallpaper-preview --locked -- --example groups --seconds 10
```

The existing recreation and device-loss flags are carried through to group mode, but have not been exercised in it. `--desktop` combinations are also unverified. Help explicitly labels group runtime/visual validation as pending. The user requested a pause before this stage; no headless device run or alternate screen-access mechanism was used to bypass it.

## Ordered rendering and opacity

A CPU-only planner consumes the validated BeginGroup/Draw/EndGroup sequence. The root paints into a fixed canvas-sized RGBA8 sRGB texture. Groups with opacity other than one receive individual full-canvas transparent targets, including zero-opacity and sequential groups, consistent with the [v2 admission contract](m3-groups.md). Opaque groups preserve command/clip order without allocating a redundant intermediate. Targets are unique for the scene; no pass samples its own destination.

Children paint in source order into the current opacity target. At group end, the child's texture is sampled and its already-premultiplied linear RGB and alpha are both multiplied by that group's opacity, then source-over composited once into the parent with premultiplied blending. Rectangle colors and source PNG textures use straight-alpha blending when first painted into an intermediate. Nested group composites close from the inside out; leaf alpha is never multiplied by cumulative parent opacity. The fixed WGSL shader uses explicit mip-zero sampling, not source-provided instructions.

The root clear color is converted from straight sRGB to premultiplied linear values. The final root texture is copied with replacement blending into an aspect-preserving, centered quad in the physical client area, avoiding a second blend over its own background. Letterbox regions use the scene background. All intermediate painting uses native canvas resolution; resizing updates only the final quad. Nearest/clamped sampling is explicit throughout. Large presentation magnification can expose the fixed canvas raster; high-DPI quality equivalence with the v1 direct text path is not claimed.

The initial implementation opens a load-preserving render pass per paint/composite operation, with separate clears and presentation. The fixture plans six paint/composite operations, two canvas targets and one text surface: ten passes on its first frame including text preparation, nine on later redraws. This favors inspectable order over batching and is not a performance claim. The existing change-driven event loop introduces no per-frame animation timer; actual idle behavior still needs measurement.

## Bounds, clipping and text

Validated accumulated transforms determine rectangle/image world-space quads. Every operation receives its effective canvas/ancestor clip. A conservative integer scissor encloses that clip, while the fragment shader rejects pixel centers outside the exact fractional rectangle. Empty/outside clips and zero-area draws are skipped; their source data and resource declarations have already been validated. This is hard axis-aligned clipping, not rounded/path clipping or edge anti-aliasing.

Glyphon's scalar text-area scale cannot directly express distinct X/Y scales. Each visible text node therefore gets a separate transparent, tightly bounded raster target. Text is shaped with the existing bundled Noto Sans, no wrap and 1.25 line height; raster scale is the larger accumulated axis scale, with a minimum of one. Its premultiplied texture is then mapped to the independently transformed world rectangle, preserving anisotropic scale and source paint order. UV extent accounts for ceil-rounded raster dimensions, and the world quad/ancestor clip bounds the final result. This is an implementation awaiting visual validation, including fractional clips and stretched text.

Text preparation/raster painting is cached after the first successful command encoding; later window resizes reuse the fixed scene/text textures. A new renderer/device generation rebuilds everything, including this cache. Group mode owns an RGBA8-target Glyphon atlas/viewports rather than reusing the differently formatted swapchain text pipeline. Text shaping, atlas allocation and driver failures still require runtime checks; cached text is not a native animation implementation.

## Admission limits before allocation

The planner first checks every declared image and the canvas against device texture dimensions, then accounts for all root, opacity, image and text textures. Existing source/scene/media/node/depth/opacity-intermediate limits are retained. Additional renderer policy:

| Resource | Bound |
| --- | --- |
| Known texture storage combined | At most 256 MiB, using checked width × height × 4 byte accounting |
| Text raster targets | At most 128 visible text nodes and 64 MiB total |
| One text raster dimension | At most 4,096 and the device limit |
| Rasterized font size | At most 512 px after raster scale |
| Uniform record | 80 bytes, stride rounded to the device's required dynamic-offset alignment |
| Uniform allocation/offsets | Checked against max buffer size and u32 offset range |

All applicable limits must pass; a scene accepted by the data codec may still be rejected by this renderer admission policy. Values are not silently clamped or downscaled to evade budgets. Text rectangles are conservatively budgeted before ancestor cropping. Even invisible opacity groups reserve targets. The preview fixture's known texture bytes are 16,804,096: two 8,294,400-byte canvas targets, one 215,040-byte text raster (840x64) and the original 256-byte image. The unchanged compiler fixture's CPU plan separately remains 16,691,456 bytes with its smaller text box.

The texture estimate excludes driver padding/staging, Glyphon atlases, shaping/cache/vertex buffers and existing baseline preview allocations; it is not a total RAM/VRAM measurement or OS limit. Those costs and allocation failures remain material runtime/performance questions before arbitrary content. Initialization uses wgpu validation/OOM error-scope guards, popped in reverse order before awaiting results, then propagates captured errors through the existing bounded initialization recovery path. No adapter limits or sandbox policies are raised. Actual scope/error/recovery behavior has not yet been exercised for this mode.

## CPU checks and remaining acceptance

Eight new tests cover the original resource/pass plan, independent one-pixel rectangle/group source-over results, nested order/no self-sampling, zero-opacity and opaque-group behavior, exact fractional clip uniform values/coarse scissors, letterboxing, anisotropic text raster sizing, text/count/font/device/combined texture rejection and Naga parsing/validation of all shader entry points. The additional preview fixture is independently compiled/decoded/asset-checked, and text/PNG overflow against the parent clip is required. These tests use no device or window. The overlap reference distinguishes one group-opacity composite from separate per-child alpha, but it does not validate the GPU pipeline or captured pixels.

No new dependency, feature, license, native API binding or Cargo.lock change is needed. Existing renderer/font advisories and source/asset isolation uncertainty remain in their prior reviews. This fixed trusted preview does not authorize untrusted source/media/font processing in the host; production helper/G2 integration remains M4 work.

Primary documentation consulted November 24, 2026: [wgpu 30.0.1 blend states](https://docs.rs/wgpu/30.0.1/wgpu/struct.BlendState.html), [scissor rectangles](https://docs.rs/wgpu/30.0.1/wgpu/struct.RenderPass.html#method.set_scissor_rect), [sRGB formats](https://docs.rs/wgpu/30.0.1/wgpu/enum.TextureFormat.html), [error-scope guard lifecycle](https://docs.rs/wgpu/30.0.1/wgpu/struct.ErrorScopeGuard.html) and [Glyphon 0.12.0 preparation/rendering](https://docs.rs/glyphon/0.12.0/glyphon/struct.TextRenderer.html). Pinned Glyphon cache/atlas source confirms straight-alpha blending and accurate color conversion for the local raster target. These API/source checks are not GPU runtime evidence.

Next acceptance requires a user-authorized window run: verify overlap opacity, clipped transformed text/PNG and geometry; retain an actual reference; resize/maximize/restore; measure quiet idle; recreate and request real device-loss callbacks; and regress static/image/M1 modes. More adversarial/synthetic effect fixtures, display/DPI/hardware coverage, production input, animation/timing, hashes/I1 and full G3 remain open.

## Local nonvisual validation — November 24, 2026

All 106 workspace Rust tests passed, including the preview's thirteen tests (eight new group tests). Workspace release build, Clippy with warnings denied and formatting passed. Exact independent CPU overlap/compositing results and shader validation are recorded by those tests; they do not execute any GPU pipeline. Base revision during checks: 99814e4 with this continuation uncommitted. Existing Cargo incremental-cache hardlink/copy notices remain.

Cargo.lock and external dependencies are unchanged. Native sandbox code/policy/corpus are unchanged and its full suite was not repeated for this preview-only continuation. Hosted CI, non-Windows builds, actual group GPU/device/window/visual checks and existing-mode runtime regressions were not run. The implementation is intentionally stopped at the user's screen-access checkpoint, not marked as G3 acceptance.
