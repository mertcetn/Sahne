# M3 compiled preview

Updated: November 24, 2026. Status: original static, PNG and group examples rendered in local Windows D3D12 windows. [Group visual/resize, ten lifecycle/regression cases and exact scope](m3-group-preview.md) now have local evidence. M3/G3 remain incomplete.

## Run

```powershell
cargo run --release -p wallpaper-preview --locked -- --example static --seconds 30
```

Escape closes window mode. Literal example names `static`, `image` and `groups` are accepted; a path, URL, unknown example or missing example value fails. `image` selects the separately documented fixed PNG fixture. `groups` selects the [v2 fixed group/clipping fixture with limited local visual evidence](m3-group-preview.md). The existing default M1 fixture remains available without `--example`. The CLI's compile-check commands are nonvisual.

Static mode embeds examples/static-scene at build time, validates the in-memory package, compiles the restricted HTML/CSS and independently decodes/validates its scene before allocating drawing resources. It opens no source path and executes no JS. This example still has the unchanged panel, title and accent rectangle. Image mode separately embeds examples/static-image and validates decoded pixels against scene resources before upload; arbitrary package loading remains disabled.

## Rendering behavior

The fixed 1920x1080 canvas is uniformly scaled to fit the physical client area and centered; resizing does not reflow CSS or change authored pixel values. Surface/background color covers the letterbox area. At 960x600, scale is 0.5 with a 30-pixel vertical offset. The panel is 320x160 at (40,70), the title box starts at (52,82), and the accent rectangle is 120x32 at (52,138), all relative to the client area. Glyph ink has its normal font baseline inside its text box.

Rectangles use a bounded, aligned uniform record per node and an original WGSL quad shader. An sRGB surface is required; straight sRGB color bytes are converted to linear RGB for output, with straight-alpha blending. Text uses the existing bundled Noto Sans/Glyphon path. Each text item retains its own prepared renderer so rectangle/text commands can stay interleaved in scene order instead of rendering all text above every later rectangle.

Text has line height 1.25 times font size, no automatic wrapping and clipping to its own fixed box. Literal LF creates another shaped line. Parent boxes currently have visible overflow; this is not ancestor clipping or group compositing. The code adds no supported opacity/transform/animation source property and does not fake group opacity by changing every child alpha.

Drawing uses the existing change/recovery-driven event loop; it introduces no animation timer. The initial interactive run is not a quiet-idle benchmark. Compiled-mode quiet-idle, automated resize/reference comparisons, explicit recreation/device-loss regressions, additional DPI/hardware, desktop attachment and broader text/stacking fixtures remain follow-up validation. Existing M1 evidence must not be read as proof that those new-mode combinations have passed.

The November 15 controlled checks below now cover the quiet-idle and explicit recreation/device-loss items on this machine. Automated resize/reference comparisons, additional DPI/hardware, desktop attachment and broader text/stacking fixtures remain unverified.

## Observed local reference

![Original compiled static example at a 960x600 client area](images/m3-static-reference.jpg)

This is an unchanged JPEG capture of the actual preview window, observed with Computer Use and retained as a visual reference, not a pixel-exact golden image. Window chrome is included. Panel/title/accent geometry, colors and text matched the original source fixture at the captured size. It is a limited reference, not full G3 acceptance.

Observed environment: Windows 11 build 26200, NVIDIA GeForce RTX 3050 Laptop GPU, driver 32.0.16.1062, D3D12, 100% scaling; two 1920x1080 displays. The owned 120-second process exited zero with generation 1 and no pending recreations. Retained logs: ignored `target/m3-preview-runs/c56833e7ea894e848cfe98638f203ccc/`. During the interactive session the app received 123 surface-size/scale notifications and presented 265 frames; do not interpret that as a stationary idle measurement or attribute every event to a specific actor.

Two new pure checks cover canvas aspect/letterbox geometry and sRGB/alpha conversion; all 65 workspace Rust tests passed, including the preview's four tests. Preview Clippy, release build, formatting and local documentation links passed. Help and unknown-example rejection passed without opening a window. The new mode was visually exercised, but its explicit recreation/device-loss flags were not run during this phase. No user testing is requested for checks the agent can perform when work resumes.

The earlier static-preview continuation changed no external package versions/features. The November 24 image implementation adds the separately [reviewed PNG graph](m3-png-dependencies.md). Existing renderer/font dependency risks and source/asset isolation requirements remain unchanged. The shader is fixed project code, not a source-package shader feature. Arbitrary source, fonts, images and GPU instructions remain unavailable.

Primary documentation consulted November 14, 2026: [wgpu 30.0.1 bind group layout](https://docs.rs/wgpu/30.0.1/wgpu/struct.BindGroupLayoutEntry.html), [Glyphon 0.12.0 text preparation/rendering](https://docs.rs/glyphon/0.12.0/glyphon/struct.TextRenderer.html) and [WGSL uniform rules](https://www.w3.org/TR/WGSL/). Existing cached Cosmic Text source confirmed the pinned buffer wrap/size and RGBA APIs. Only local Windows runtime behavior is claimed.

## Resume point

Image upload/blending/window behavior and static-mode idle/recovery regression passed locally on November 24; see [image evidence](m3-images.md). The later [group reference and lifecycle regression](m3-group-preview.md) also passed locally. Broader effect fixtures (including nested opacity, nonuniform scale and fractional clips), native animation/pause, compiled provenance/hashes, display/platform checks, I1 mappings and full G3 remain open. The v1 compiler still rejects unsupported effects/media rather than silently approximating them.

## Controlled lifecycle checks — November 15, 2026

Same local Windows/D3D12 target, committed source 2951f4a, release build with `--locked`. Every process was owned, hidden-console, separately logged and bounded by the existing 25-second outer guard; cases ran sequentially, without UI automation or build/executable-lock contention. Report: ignored `target/m3-lifecycle/0fbe932bea4e40ba84b9de68f10dde3d/summary.json`.

| Case | Arguments | Result |
| --- | --- | --- |
| Compiled quiet idle | `--example static --seconds 6` | Exit 0, one frame, one generation, three startup surface notifications |
| Original M1 baseline | `--seconds 3` | Exit 0, one frame, one generation |
| Compiled reconstruction | `--example static --seconds 8 --test-recreate 2` | Exit 0, three frames/three generations, zero pending recreations |
| Compiled device loss | `--example static --seconds 8 --test-device-loss` | Exit 0, two frames/two generations, actual requested callback seen and restored frame confirmed |

Idle acceptance required at most three total startup presentations and a healthy timed exit; the observation was exactly one frame across six seconds. The loss fixture still requires the real callback from the requested generation before a later-generation frame, rather than equating any redraw with recovery. These observations do not prove every native fault, GPU/driver, lock/resume or monitor transition. No dependency, limit, sandbox permission or runtime code changed for this validation. The current pinned wgpu Device destroy/callback documentation was rechecked; no validation checks were disabled.
