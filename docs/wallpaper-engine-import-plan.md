# Wallpaper Engine Import Plan

Approved direction: November 11, 2026. Status: planned; no importer or compatibility runtime exists.

## Scope and timing

Plan compatibility alongside native engine development; implement it after the compiler and sandboxed runtime work end to end in M4. The initial goal is a defined subset of local web-project import, with explicit compatibility reports. Universal Wallpaper Engine playback is not a delivery promise. Scene PKG compatibility is a separate later track.

| Track | Timing / dependency | Deliverable | Acceptance |
| --- | --- | --- | --- |
| I0 — Corpus and boundaries | Now; independent of M1 implementation | Local read-only inventory, blockers, supported-subset intent | Sources remain unexecuted/unmodified; findings distinguish evidence from inference |
| I1 — Contract preparation | Alongside M2/M3 | Capability/diagnostic model, metadata/resource mappings, settings and pause semantics | Native package/IPC/sandbox contracts remain authoritative; no source paths or limits gain trust through import |
| I2 — Web importer | After M4 passes; can proceed alongside M5 | Bounded local project inspection, compatibility report, supported package conversion and small API adapter | Positive synthetic fixtures match behavior; unsupported real examples are reported accurately; no silent feature loss |
| I3 — Scene assets | Later, separately scoped after I2 evaluation | Bounded PKG inventory/extraction and selected texture conversion investigation | Validate versions, ranges, archive paths and budgets; asset extraction is labeled separately from playback |
| I4 — Broader playback | Future scope decision only | Optional scene behavior or browser backend assessment | Independent renderer/security/performance evidence; no implied full compatibility |

I0's two local examples have an initial static assessment in [Sample assessment](wallpaper-engine-samples.md). Neither has been runtime-tested or converted. I1 remains pending; I2 is gated on M4. I2 does not block the first native Windows product or Linux platform work unless a later release scope explicitly requires it. Android remains deferred.

## Proposed boundary

The importer is a frontend translating approved input into Sahne's versioned package contract; it is not a second trusted runtime. Keep it separate from core scene/layout/renderer behavior. Do not create the implementation crate before the compiler/format responsibilities are clear. A Wallpaper Engine API adapter exposes only explicitly mapped callbacks through Sahne's existing worker and host permissions.

Candidate initial mappings: primitive user settings, host-capped FPS, and pause/resume. Define initial property delivery, change-only updates, types/defaults and timer/animation semantics before claiming parity. Dynamic file/directory properties, media, audio, RGB, network, Canvas/WebGL, scene materials and SceneScript are excluded until separately approved. Inert or no-op stubs must not masquerade as capability support.

## Compatibility report contract to specify in I1

For each input, record detected type and manifest hints, observed required capabilities, file/source evidence, unsupported or unknown features, missing/unsafe resources, budget violations, and proposed conversion actions. Detection must inspect content rather than trust project.json alone. Classify as convertible, needs explicit manual/lossy work, unsupported, or unknown; report coverage and uncertainty rather than a binary success for all scripts.

Do not execute JavaScript, evaluate manifest conditions, launch native applications, load shaders on the GPU, fetch URLs or decode unbounded media while discovering requirements. Computed property access, minification, eval and dynamic resource selection limit static analysis; unknown is a valid result. Any later behavioral test runs only under the validated worker/helper isolation.

## I2 acceptance — import-specific gate

- Positive original fixtures cover metadata conversion, initial/settings updates, pause/resume, resource IDs and compiled-package validation in Sahne's worker.
- Negative fixtures cover unsupported elements/CSS/browser APIs, malformed metadata, condition expressions, unknown/dynamic scripts, missing resources, traversal and size limits.
- The supplied complex web example produces honest Canvas/WebGL/media/audio/network/budget diagnostics; it need not render to pass this reporting case.
- Converted output is written separately, carries source provenance and adapter version, and passes the same independent host validation as native packages.
- Partial/static fallback requires explicit selection and a visible loss report. It is never equivalent scene/web playback by default.
- CI uses tracked original synthetic fixtures; optional ignored samples are not required on a clean checkout. No third-party sample assets are shipped in releases.

## Decisions still open

Exact web subset and adapter versioning; report schema; safe condition grammar; static JS analysis tools and their security/license review; I2 release placement; I3 format/texture versions. Resolve through fixtures and measurements, not by relaxing native isolation or assuming a full browser. Steam Workshop acquisition/distribution is outside local import scope.
