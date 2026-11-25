# M3 native animation foundation

Updated: November 25, 2026. Status: typed CPU data admission, sampling and explicit pause/resume clock implemented. **Source animation parsing, animated GPU preview, FPS scheduling and platform pause policy are not implemented by this checkpoint.** The existing static examples and scene wire formats are unchanged; full M3/G3 remains open.

## Typed contract

`wallpaper_scene::animation` accepts an experimental `Animation` with version 1, a shared positive duration, nonnegative delay, finite iteration count or `Forever`, an optional alternating direction and property tracks. This version identifies the typed animation contract; it is not a new scene wire version. There is no animation serializer/decoder or externally loadable animation file yet.

Each `Track` targets exactly one existing v2 group by scene-local ID and replaces one local property: `TranslateX`, `TranslateY`, `ScaleX`, `ScaleY` or `Opacity`. It never multiplies opacity into children. Multiple different properties may target a group; duplicate tracks for the same target/property are rejected. Missing IDs, zero IDs and primitive/draw targets are rejected. The group's bounds, origin, clipping flag, resources, content and order remain those of the bound base scene. No CSS/script precedence or mutation API is introduced.

`Animation::validate` consumes an independently validated hierarchy. The returned immutable `ValidatedAnimation` owns that base, private target bindings and animation data; it cannot accidentally sample the same numeric IDs against a different scene. Read-only `base()` and `animation()` accessors support inspection. Errors contain a bounded `Code` and optional track index, without echoing source text.

| Input | Limit |
| --- | --- |
| Tracks | 1–128 |
| Keys per track | 2–64 |
| Total keys | At most 2,048 |
| Shared duration | 1–86,400,000 ms |
| Delay | 0–86,400,000 ms |
| Finite iterations | 1–10,000 |
| Translate values | Finite, absolute value at most 32,768 local pixels |
| Scale values | Finite, 0–16 per axis; negative scale unsupported |
| Opacity values | Finite, 0–1 |

Every track must explicitly include time 0 and the shared duration. Intermediate integer-millisecond key times must be strictly increasing. Linear interpolation is the only interpolation; there are no implicit keys, easing curves, rotation, skew, percentage units or additive composition. These are admission limits on caller-owned typed vectors, not a claim that hostile byte payload allocation has already been bounded by a decoder.

## Whole-run admission and sampling

Admission computes conservative ranges from all keys and propagates parent/child translation and nonnegative scale intervals through the existing group order. This checks world scales (at most 64) and positions/edges (absolute value at most 1,048,576) for combinations between key timestamps as well as at endpoints. Correlations are deliberately ignored: a safe but correlated animation can be rejected if its independent interval bounds exceed policy. Checking only matching endpoint frames would miss unsafe combinations of independently moving ancestors and children.

Every animated opacity target reserves an intermediate even when its keys are all 1, alongside existing static opacity groups. The existing eight-group and 128 MiB conservative full-canvas reservation limits still apply. This is admission accounting; a future renderer must actually retain suitable intermediate topology and enforce its own text/texture/uniform/device budgets before upload. No GPU allocation or sandbox permission is changed here.

`sample(Duration)` uses absolute active elapsed time. Delay fills the first pose; finite completion fills the final pose. A normal iteration restarts at its first key at the exact next-iteration boundary. Alternate mode reverses odd zero-based iterations; after finite completion it holds the endpoint appropriate to the last iteration's direction. This explicit initial/final fill rule is the prototype's native rule, not a claim of CSS default fill behavior.

Submillisecond elapsed time is retained. Integer nanosecond arithmetic determines iteration and position before bounded floating-point interpolation. A long stall skips directly to the current pose; it does not replay missed frames or enumerate iterations. `Duration::MAX` is handled without multiplying an unbounded iteration count or converting total uptime to a float. Sampling returns a revalidated v2 hierarchy and an `active` flag (true during delay/running, false after finite completion). Every sample is independently scene-validated again before it may be used; rounding or an unexpected invariant failure returns an error.

The current CPU implementation clones/revalidates bounded scene metadata and text for each sample. It is a correctness foundation, not an optimized per-frame renderer or a performance result. Asset pixels are not owned/copied by this module. Existing v1/v2 binary scene codecs carry the sampled static scene only, not the animation program.

## Explicit clock

`Clock::new(Instant)` starts running at elapsed zero. `advance(now)` updates its active elapsed time. `set_paused(now, true)` first accounts for running time up to that instant and then freezes elapsed time. Further paused updates retain the pose. `set_paused(now, false)` resumes from that elapsed time and excludes the paused interval. Repeated pause/resume calls are idempotent with respect to the state; no interval is counted twice. Read-only `elapsed()` and `paused()` expose the recorded state.

Clock inputs must be nondecreasing monotonic instants. Backward instants and elapsed-duration overflow fail without changing the previous time, elapsed value or pause state. There is no wall-clock/calendar input, sleeping thread, timer callback, global pause flag or automatic OS subscription. A future host must invoke pause/resume at the appropriate lifecycle events and schedule frames at its FPS cap. Suspend-time behavior is not inferred from `Instant` alone, and platform lock/sleep/visibility policies remain unverified for animation.

## Validation and review

Run `cargo test -p wallpaper-scene --locked animation::tests` to review the 17 CPU animation regressions. They cover independent midpoint geometry/group opacity, multikey/submillisecond sampling, delay/repeat/alternate boundaries and final fill, huge/infinite elapsed time, invalid targets/duplicates/times/values, individual/aggregate key budgets, combined ancestor envelopes (including safe endpoints with an unsafe intermediate product), opacity reservations, zero scale, sampled-scene codec roundtrips, multiple/idempotent pauses, backward input and overflow atomicity. No window/device is created.

All 130 workspace tests, formatting, Clippy and a release workspace build passed for this checkpoint. Logs are retained locally under `target/m3-animation-{workspace-tests,clippy,build}.log`. Compiler cache hard-link warnings used the existing copy fallback. No dependency, lockfile, native sandbox code, release policy or external input surface changes. GPU/visual tests and the native sandbox corpus are not evidence for this CPU-only implementation and are not rerun here.

Next: source/author-ID association, a bounded animation encoding with independent decoding, frame scheduling, stable opacity targets and text-raster budgets, and an original animated-shapes preview with explicit pause/resume controls and lifecycle checks. Those steps must not be treated as complete from these tests. Screen/window work must wait for the user's availability under the current session instruction.

## Primary references

Consulted November 25, 2026: Rust [`Instant` and checked monotonic subtraction](https://doc.rust-lang.org/std/time/struct.Instant.html#method.checked_duration_since) and [checked duration addition](https://doc.rust-lang.org/std/time/struct.Duration.html#method.checked_add); W3C [CSS Animations play-state semantics](https://www.w3.org/TR/css-animations-1/#animation-play-state). The stopped-clock resume behavior follows that reference, while the narrow typed grammar and fill rules above are explicitly this prototype's own contract. No CSS animation support is claimed.
