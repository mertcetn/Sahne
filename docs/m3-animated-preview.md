# M3 native animation integration and fixed preview

Updated: November 25, 2026. Status: bounded animation wire format, typed author-ID binding and fixed animated preview code with CPU checks. Local Windows D3D12 visual, pause/resume and GPU recovery checks passed. Full M3/G3 remains open.

## Commands and original fixture

Build with `cargo build --release -p wallpaper-preview --locked`. When window testing is authorized, run:

```powershell
target/release/wallpaper-preview.exe --example animated-shapes --seconds 30
```

Space toggles manual pause/resume in window mode. Key repeat does not toggle repeatedly. Escape closes the window. The effects-only `--test-pixels` flag is unavailable for this example. The `--test-recreate` and `--test-device-loss` flags select the existing explicit reconstruction tests. Both combinations passed in the local window runs below.

For the M4 desktop integration check, run `wallpaper-preview.exe --desktop --example animated-shapes --seconds 90`. It uses the same native animation and the existing Explorer attachment. The preview retains its playback state across GPU or desktop surface recreation.

On November 26, 2026, the user observed motion behind Windows desktop icons with `f3e2675`. The 3840×1080 desktop surface spanned two 1920×1080 displays. The preview stopped at its configured 90-second deadline with 2,457 presentations in one GPU generation, averaging 27.3 frames per second over the run. Local log at `target/m4-animation-desktop-visual/567a02d12d0742318e81bafec06e1285/stderr.log`.

The original `examples/animated-shapes/` package contains a manifest and static HTML/CSS for three clipped panels. Its fixed native program lives in `wallpaper_compiler::fixtures::animated_shapes_spec`; CSS `@keyframes`/`animation-*` remain unsupported. It has five linear tracks over two seconds, alternating forever (four seconds per full out-and-back cycle):

| Author group | Property | First → last value |
| --- | --- | --- |
| `move` | Translate X / Y | 0 → 280 px / 0 → 180 px |
| `scale` | Scale X / Y, top-left origin | 0.5 → 2.5 / 0.5 → 2 |
| `fade` | Group opacity | 0 → 1 |

The fade group contains overlapping opaque red/blue children; blue must overwrite red before the group is blended once. At one second the moving rectangle's world bounds are (260,370,120,100), the scale rectangle is (720,280,180,125), and the fade group has opacity 0.5. At zero/two seconds, opacity is exactly 0/1. Static labels remain outside animated groups. The package requests 30 FPS; the host accepts only 1–60 FPS and rounds its frame interval upward (30 FPS gives 33,333,334 ns).

## Source association and independent decoding

`compile_animated(package, viewport, Spec)` explicitly opts into the existing v2 compiler and binds typed `AuthorTrack` target strings to its author/group symbol table. Target names follow the existing bounded identifier grammar (at most 64 ASCII bytes). Unknown/invalid names return a bounded track-index error; source HTML/CSS errors retain their existing diagnostic path/span. Duplicate properties and invalid values/times/envelopes are rejected by the [native model](m3-animation.md). The resulting `CompiledAnimation` exposes immutable animation, symbol, asset and deferred-script references; it executes no script. This API does not add source animation syntax or claim source locations for separately supplied typed keys.

`ValidatedAnimation::encode()` emits a self-contained base v2 scene plus native animation program. `animation::decode(bytes)` bounds input and counts, independently decodes/validates the base and rebuilds/validates target bindings and all animation envelopes. The base travels with the program; numerical target IDs cannot be rebound to an unrelated later scene through this format. Image pixels are not included: the receiver must independently match immutable assets to decoded scene declarations. The fixed preview does so before use.

| Wire field, in order | Encoding |
| --- | --- |
| Magic / animation version | 7 ASCII bytes `SAHNANM` plus NUL / u16 version 1 |
| Base length / base | u32 / complete v2 scene bytes |
| Duration / delay / iterations | Three u32 values in milliseconds / count; iteration 0 means forever |
| Alternate / track count | u8 restricted to 0 or 1 / u16 |
| Each track | u32 group ID, u8 property tag, u16 key count |
| Each key | u32 millisecond time, f32 IEEE bits |

All integers/bits are little-endian. Property tags are 1 translate-X, 2 translate-Y, 3 scale-X, 4 scale-Y, 5 opacity. The maximum is 2,114,461 bytes: existing 2 MiB base budget, 29 header bytes, at most 128 seven-byte track headers and 2,048 eight-byte keys. Individual tracks retain the 2–64 key limit. Unknown versions/tags/flags, truncated/trailing bytes, hostile counts, duplicate targets/properties, invalid scene/key values and over-budget data fail closed. A complete key payload is checked before its vector is allocated. This is not a new control-protocol kind or permission to send it through the existing 64 KiB control frame; production IPC/worker integration is still deferred.

## Stable renderer resources

The animated plan retains culled/empty/zero-opacity draw slots and forces a reserved intermediate for every animated opacity group. Thus passing through opacity 0 or 1 does not remove/add targets or renumber draw bindings. Existing scene intermediate, renderer texture/text, uniform and device limits apply before allocation. The fixture owns two full-canvas targets and five static text raster surfaces; it uses no image assets.

Each frame independently samples/revalidates the hierarchy and builds a bounded CPU plan. Before any uniform update, the renderer requires identical canvas/background, operation order, target count, texture reservation, quad count and text raster specifications. Mismatches are errors; no GPU texture, uniform buffer or text raster is resized/recreated to conceal them. Only draw uniform contents and the fitted presentation uniform change during steady playback. CPU scene/plan vectors and wgpu's write-buffer staging remain implementation costs; no performance gain or allocation-free rendering claim is made.

Animated text translation/opacity can reuse a raster when its specifications stay identical. Scale changes that would change text raster dimensions/scale are deliberately rejected by the compatibility check; this fixed example animates shapes only. General animated text-scale raster admission/preallocation remains future work. Native transform/clip budgets are not relaxed to accommodate animation.

## Scheduling and pause ownership

Playback belongs to the application, across GPU generations. The initial clock is paused during initialization. Explicit pause reasons are independent bits: manual, occluded, zero client size, application suspension and renderer recovery. Clearing one reason cannot resume while another remains. Successful reconstruction clears transient recovery/suspension/visibility state for the newly created window while preserving manual pause and active elapsed time. This logical behavior has CPU tests; local manual pause and controlled recovery observations are recorded below.

The event loop uses `WaitUntil` for the next animation deadline, queues one redraw and schedules the next interval after a draw attempt. It skips missed frames using absolute active elapsed time; there is no catch-up loop or polling thread. Paused or finitely completed playback contributes no animation wake deadline. Static examples have no playback instance and retain their existing event-driven path. Surface resize/expose events may request extra presentations outside the animation-driven FPS cadence; this is not an absolute cap on every presentation during interaction. Existing duration/recovery deadlines still operate.

The clock excludes paused intervals. Manual pause requests one final redraw at the frozen pose; paused resize/expose redraws use that same pose. Clearing the final reason schedules an immediate current-pose redraw and resumes the capped cadence. OS lock, display-off, fullscreen detection, suspend notification reliability and user pause-policy settings are not implemented/certified by this change. It does not infer those policies from monotonic time or claim sleep behavior across platforms.

## Validation and reproduction

CPU tests cover wire roundtrip/every truncation/trailing data/hostile fields/mutations, author target binding and asset retention, invalid names/duplicates/keys, fixed plan topology through full transparency/opacity and alternate boundaries, independent midpoint group compositing, retained hidden draws, text-raster reallocation rejection, multiple overlapping pause reasons, recovery clock preservation, finite completion, FPS bounds and a single future deadline after a large stall. They create no GPU/window. Existing static scene/group/effect tests remain part of the workspace checks.

Local checkpoint: all 139 workspace tests, formatting, Clippy and the release workspace build passed. Logs: `target/m3-motion-{tests,clippy,build}.log`. Three guarded release-process checks passed without adapter/window initialization: help exited 0; animated desktop and animated pixel-test combinations exited 1 with their expected diagnostic. Report: `target/m3-motion-arguments/b7fc11884d454b19a6d55988148373d1/summary.json`. Dependencies/lockfile and native sandbox policy are unchanged; the sandbox runtime corpus was not rerun. The subsequent local validation is recorded below.

Before accepting the rendered checkpoint, run the command under `Invoke-SahneGuardedProcess` with bounded duration, a 45-second outer timeout and 1 MiB logs. Observe all three properties, Space freeze/resume, resize/maximize/restore, and Escape exit; correlate pause logs/frame cadence. Then run bounded animation recreation/device-loss cases and static/groups/effects/pixel regressions. Record actual adapter/driver/display data and exact binary/source provenance; do not infer success from CPU tests. See the local runtime evidence below.

## Primary references

Consulted November 25, 2026: winit 0.30.13 [`WaitUntil`](https://docs.rs/winit/0.30.13/winit/event_loop/enum.ControlFlow.html), [`request_redraw`](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html#method.request_redraw) and [`KeyEvent::repeat`](https://docs.rs/winit/0.30.13/winit/event/struct.KeyEvent.html#structfield.repeat); wgpu 30.0.1 [`Queue::write_buffer`](https://docs.rs/wgpu/30.0.1/wgpu/struct.Queue.html#method.write_buffer). Existing pinned dependencies are unchanged.

## Local runtime evidence

On November 25, 2026, source `ce1364512c3582d8cd305931aa753a509b38aee3` ran on Windows D3D12 with an NVIDIA GeForce RTX 3050 Laptop GPU, driver 32.0.16.1062 and DPI scale 1. The release binary SHA256 was `A255421223D7D8E973C79A19E8DEF12296E86A73AA1163001E2F216C6DD47497`.

- Visual inspection confirmed translation, X/Y scaling and changing group opacity. Space froze the pose across separate observations and a 960×600 → 1920×1009 → 960×600 resize. Pause and resume logged the same active elapsed value (19,262,758,100 ns). Motion resumed and Escape exited 0 with 736 frames in one generation. Logs: `target/m3-motion-visual/a96ff395210644e695492035dda80fa2/`.
- Requested device loss produced the callback and a restored frame in generation 2, with exit 0. Static, groups and effects each remained at one frame during the timed checks. All 18 effects GPU pixel samples passed. Report: `target/m3-motion-runtime/d0f2f40b81234b2d84f1e602dcda0874/summary.json`.
- The initial five-second double recreation run exited 1 with one recreation pending. A 12-second run completed both recreations, exited 0 and presented 175 frames across three generations. Both recoveries retained their active elapsed values exactly. Logs: `target/m3-motion-runtime/recreate-12s-8fcd7a1d2f4349f6922041f47d356045/`.

This validation covers manual pause and controlled recovery in the recorded window configuration. OS lock, display-off, other drivers, and quantitative animation cadence remain open.

## Fixed midpoint reference mode

Use this window-only command to reproduce the reference pose:

```powershell
target/release/wallpaper-preview.exe --example animated-shapes --reference-pose --seconds 30
```

The preview samples the existing scene at exactly 1 second on every redraw. It schedules no animation wakeups, and Space leaves the fixed pose in place. Resize and Escape behave as in the normal window. The flag requires the animated fixture and rejects desktop mode and other test flags. At this pose the green box is at native bounds (260, 370, 120, 100), the gold box at (720, 280, 180, 125), and the overlapping red/blue group has opacity 0.5. The [retained midpoint reference](images/m3-animated-reference.jpg) was captured on November 25, 2026 from source `0c8d32a`. The RTX 3050 Laptop GPU ran D3D12 at scale 1 with a 960×600 client. The capture includes the window frame (962×632). The three panels showed the expected midpoint appearance. Space left the pose visually unchanged and Escape exited 0. The guarded run lasted about 26 seconds and recorded two presentations in one generation. Logs are under `target/m3-midpoint/657456c874bf4aae814547d98631bd67/`. This is a window reference for the fixed pose. Quantitative pixel comparison remains scoped to the separate effects fixture.
