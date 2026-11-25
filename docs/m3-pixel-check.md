# M3 fixed GPU pixel check

Updated: November 25, 2026. Status: CPU checks and the fixed GPU comparison passed locally on Windows D3D12, with window/maximize/restore observations recorded below. This is a quantitative check for the original effects fixture, not a completed M3/G3 or broader hardware/platform claim.

## Command and completion

After building `cargo build --release -p wallpaper-preview --locked`, use:

```powershell
target/release/wallpaper-preview.exe --example groups-effects --test-pixels --seconds 6
```

This opens a trusted Windows D3D12 window. It requires this exact example, window mode, an explicit deadline of 1–30 seconds and no recreation/device-loss mode. Invalid combinations fail before event-loop/window creation. It exits after the first successfully checked and presented frame. Closing early, initialization/render/device/map errors, missing callback, timeout or mismatching samples fails with a nonzero exit code. Recovery retries are disabled so a later attempt cannot hide a failure. Other preview modes retain their normal behavior.

Run under `Invoke-SahneGuardedProcess` from `tools/sandbox-process.ps1`, with a 25-second outer timeout and 1 MiB combined log limit. Require `Completion=Exited`, exit code zero, the `GPU pixel test passed: samples=18` marker and a presented frame. The event-loop deadline does not interrupt driver initialization; the outer guard is necessary. The GPU submission wait has a separate two-second limit.

## Readback boundary and reference

Only the effects root render texture gains `COPY_SRC`, and only when this test is requested. Its native size/format must be 1920×1080 `Rgba8UnormSrgb`. After group compositing, 18 separate 1×1 copies select the coordinates below into one fixed 4,608-byte `COPY_DST | MAP_READ` buffer (256-byte stride). These inspect the root before fitted window presentation. No desktop/swapchain capture, arbitrary coordinates, file path, script or runtime resource API is exposed. Existing scene/texture/sandbox budgets remain in place; no dependency is added.

| Samples (native integer pixel coordinates) | Expected result |
| --- | --- |
| (190,270), (110,190) | Blue/red at quarter contribution over canvas background |
| (410,190) | White badge at half contribution over canvas background |
| (190,410) | Canvas background outside inner clip |
| (670,240), (820,240), (670,290) | Opaque PNG top-left, top-right, bottom-left quadrants |
| (820,290) | Bottom-right PNG at alpha 128/255 over panel |
| (1240,181), (1500,200), (1250,180), (1410,280) | White inside fractional clips |
| (1400,181), (1340,200), (1410,180), (1250,280) | Panel outside fractional clips |
| (1560,200) | Panel under empty clip |
| (20,500) | Canvas background control |

Canvas RGB is (12,17,22), panel RGB (31,42,51). PNG colors are (32,160,128), (230,160,64), (40,64,128), (230,64,96). Expected bytes are independently calculated from these fixed source colors: decode sRGB, apply analytic linear source-over with the specified alpha, encode sRGB and round. The oracle does not reuse renderer plans, uniforms, shaders or CPU compositing output.

Expected alpha is exactly 255, with zero tolerance. RGB allows one byte for opaque/reference samples and two for blended samples, fixed before GPU execution to accommodate 8-bit intermediate sRGB/alpha quantization and conversion rounding. This cannot accept opposite white/panel coverage, a missing nested opacity stage or encoded-space half-white blending. Each accepted sample logs actual/expected RGBA and tolerance; the first mismatch reports label/coordinates and fails. The mapped view is dropped and the buffer unmapped on completion/error; no retry or tolerance adaptation occurs.

## Validation and remaining scope

Four new CPU tests cover bounded coordinates/stride/size, padded extraction, short/zero buffer rejection, fractional/empty coverage inversions, exact alpha, independent half-white sRGB reference, fixed tolerance rejection, missing opacity stage and argument restrictions. They create no device/window. All 113 workspace tests passed. Formatting, Clippy and release build are checked with the implementation. The authorized local GPU comparison and window checks subsequently passed; their scope is recorded below.

The release binary also passed eight guarded non-GPU process checks: help (exit 0), missing/wrong example, missing/too-long deadline, desktop, recreation and device-loss combinations (each exit 1 with the expected argument diagnostic). No adapter/generation/presentation marker appeared. Local report: `target/m3-pixel-arguments/debf0db152bf4a3cb633dfec72640de7/summary.json`; workspace test/build/Clippy logs: `target/m3-pixel-{tests,build,clippy}.log`. Compiler cache hard-link warnings fell back to copying and did not fail the checks. The native sandbox corpus was not rerun because native sandbox code/policy did not change.

This sparse native-root check does not quantitatively verify text glyphs, swapchain scaling, window color management, animation, all image edges or other GPU/driver/display configurations. Prior [visual/lifecycle evidence](m3-effect-fixtures.md) remains separately scoped. Native animation/pause, hashes/provenance/resource loading, I1 mappings and full G3 remain open.

## Local GPU and window evidence

On November 25, 2026, the user authorized visual testing of source `4576360a5c9f4a0632d3bec44653d3368f9b799c`. The release binary SHA256 was `351B3E4D8EBAA7F9EDDC7A3905160CB4D77D387E66682CBC04B709C18CCEE0D5`. Hardware: NVIDIA GeForce RTX 3050 Laptop GPU, driver 32.0.16.1062, D3D12; two active 1920×1080 displays at scale 1, each reporting 120 Hz. No display/DPI changes were made.

The guarded pixel run exited zero in approximately 1.22 seconds, with one generation, one presented frame and no pending recreations. All 18 samples passed; all alpha bytes were exactly 255. Fourteen samples matched every expected byte exactly, including every fractional/empty-clip sample and all three opaque PNG quadrants. Only these four blended samples differed:

| Sample | Actual RGBA | Expected RGBA | Largest RGB difference |
| --- | --- | --- | --- |
| Nested blue overlap | (9,14,139,255) | (9,13,138,255) | 1 |
| Nested red | (139,14,18,255) | (138,13,18,255) | 1 |
| Outer white badge | (189,189,189,255) | (188,188,188,255) | 1 |
| PNG bottom-right alpha | (171,54,78,255) | (170,54,78,255) | 1 |

No tolerances or renderer algorithms were changed after observing results. Local report and complete sample/adapter logs: `target/m3-pixel-gpu/70225e08ba0f49db90c421ee55b1dac5/{summary.json,stdout.log,stderr.log}`.

A separate guarded effects run, without the pixel flag, was observed through Computer Use. The 960×600 client showed nested dim red/blue with a brighter one-level badge, anisotropic text/PNG beside normal text, two white fractional-clip boxes and an empty third clip region. Maximizing to a 1920×1009 client and restoring to 960×600 preserved layout/text/image appearance. Escape closed the window; the run exited zero after approximately 34.62 seconds, with three presented frames, one generation and no pending recreations. Logs: `target/m3-pixel-visual/6f24a8884b484a41bfe8662146bd0f31/{summary.json,stdout.log,stderr.log}`. The existing [visual reference](images/m3-effects-reference.jpg) remains the earlier reference; no new screenshot artifact is claimed. This is not a lifecycle/long-soak rerun or evidence for other hardware, DPI, desktop attachment or monitors being moved/disconnected.

The subsequent source edit only removes the stale pending-GPU-validation sentence from help; the tested algorithms, shader, sampling coordinates, expectations and tolerances remain unchanged. The fixed effect checkpoint is complete for this recorded environment; full G3 is not.

## Primary API references

Consulted November 25, 2026: wgpu 30.0.1 [texture-to-buffer copies](https://docs.rs/wgpu/30.0.1/wgpu/struct.CommandEncoder.html#method.copy_texture_to_buffer), [mapping](https://docs.rs/wgpu/30.0.1/wgpu/struct.BufferSlice.html#method.map_async), [mapped ranges](https://docs.rs/wgpu/30.0.1/wgpu/struct.BufferSlice.html#method.get_mapped_range), and [bounded submission polling](https://docs.rs/wgpu/30.0.1/wgpu/type.PollType.html). The pinned local API is used without dependency changes.
