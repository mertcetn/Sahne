# Sahne — Development Roadmap

Updated: November 11, 2026.

## 1. Scope and current state

Deliver a restricted HTML/CSS/JavaScript wallpaper engine on Windows first, then stable Linux support on selected mainstream configurations. Android is deferred until desktop milestones are complete. macOS requires a future scope decision.

The repository has working rules, an architecture, source-based feasibility research, and a Linux support plan. The repository has been created and the existing documentation committed. No implementation, build validation, sandbox validation, platform certification, or performance measurement is complete. All implementation milestones below remain pending.

This roadmap sequences the existing design; it does not introduce a new architecture. Technical decisions belong in [Project architecture](architecture.md), research and gate definitions in [Feasibility research](feasibility-research.md), and Linux acceptance details in [Linux support plan](linux-support.md).

Dates and effort estimates will be added after the first technical spikes establish integration difficulty. Milestone completion requires recorded evidence, not just merged code.

## 2. Milestone overview

| Milestone | Deliverable | Prerequisites | Exit gate | Status |
| --- | --- | --- | --- | --- |
| M0 | Reproducible development foundation and initial contracts | Existing design documents | Workspace builds; contracts and security tests are reviewable | Pending |
| M1 | Windows rendering and desktop attachment spike | M0 foundation | G1 | Pending |
| M2 | Windows sandbox and IPC spike | M0 foundation and threat model | Initial G2 evidence | Pending |
| M3 | Restricted content compiler and native animation | M1; M0 content contract | G3 | Pending |
| M4 | Sandboxed scripted end-to-end Windows prototype | M1, M2, M3 | Integrated G2 regression and prototype acceptance | Pending |
| M5 | Windows product foundations and performance report | M4 | G5 and Windows release checklist | Pending |
| M6 | Stable Linux configurations | M5; early Linux investigation | G6 for each supported configuration | Pending |
| M7 | Deferred Android evaluation | Windows and selected Linux milestones complete | G4 if Android work is activated | Deferred |

M1 and M2 are independent once their M0 prerequisites are ready. The sandbox spike happens early to expose feasibility problems; full script/scene integration happens in M4. Linux desktop integration risks are investigated in M0 and revisited before M6, rather than being discovered only after Windows product work.

## 3. M0 — Development foundation and contracts

### Work

- [ ] Record the development toolchain and target Windows/GPU test configuration.
- [ ] Recheck current primary documentation for adopted technologies. Review direct/transitive dependency security, maintenance, licenses, provenance, and platform compatibility before adoption; record retrieval dates for cached references.
- [ ] Establish a minimal Cargo workspace, lockfile, formatting, linting, and relevant automated checks. Keep the initial crate count small where practical.
- [ ] Create the planned content-feature, scripting-API, package-format, and threat-model documents under docs/. Specify supported behavior and explicit exclusions before implementing it.
- [ ] Specify IPC envelopes, transaction/acknowledgement semantics, resource ownership, and provisional host-enforced limits in project documentation.
- [ ] Define direct OS access tests, worker/native-stall tests, malformed-input cases, and evidence recording for G1–G3.
- [ ] Investigate GNOME, Plasma, and Cinnamon integration options and record unresolved blockers against the Linux candidate matrix. Do not claim tested support.
- [ ] Establish CI on main and the release publication workflow required by AGENTS.md. Every release push must build and publish uniquely identifiable artifacts through GitHub Releases; failed required checks must prevent publication. Add platform artifacts only as their builds become available.
- [ ] Document local build/check commands and the release procedure. Any end-to-end publication test requires an explicit user instruction to push to release.

### Completion evidence

A clean checkout builds the initial workspace and runs its declared checks. Contracts and the threat model can be reviewed independently of implementation. CI configuration is checked, dependency decisions are recorded, and the release workflow's untested portions are clearly identified. Do not block the first spike on complete product documentation; specify the behavior needed by that spike first.

## 4. M1 — Windows rendering and desktop attachment

### Work

- [ ] Draw a shape, bundled-font text, and an image in a normal development window.
- [ ] Add an isolated Windows desktop attachment adapter and explicit native surface lifetimes.
- [ ] Implement change-driven redraw, resize/DPI handling, and surface/device recovery as needed for the spike.
- [ ] Handle Explorer restart with bounded reattachment and a clear failure state.
- [ ] Record results on the target Windows build, graphics backend, GPU, and driver.

### Completion evidence — G1

Pass the research report's Windows surface gate, including 10 Explorer restarts and the declared monitor/DPI transitions. Static scenes have no continuous application rendering loop. Record unavailable hardware cases as untested. A normal-window demo alone does not complete this milestone.

## 5. M2 — Windows sandbox and bounded IPC

### Work

- [ ] Launch a minimal worker in the selected AppContainer/LPAC policy with Job limits and restricted handles.
- [ ] Establish the actual permitted IPC channel with schema, size, ownership, and queue limits.
- [ ] Embed the selected engine with recorded source revision and minimal features; accept source only.
- [ ] Add runtime memory/stack limits, cooperative interruption, and an independent host watchdog.
- [ ] Exercise direct OS denials through trusted test code inside the sandbox, not merely missing JavaScript APIs.
- [ ] Exercise infinite JavaScript, native stalls, memory exhaustion, IPC floods, worker crashes, and bounded restart behavior.

### Completion evidence — initial G2

Allowed IPC succeeds, prohibited access fails as specified, and the host remains responsive through failure cases. Record permitted executable/library/system reads separately. Pass the G2 spike criteria before enabling untrusted content. If isolation fails, resolve the policy or implementation instead of weakening it.

## 6. M3 — Restricted compiler and native animation

### Work

- [ ] Implement manifest/source loading, resource references, and versioned initial scene output.
- [ ] Parse the agreed HTML/CSS subset and report unsupported constructs with useful source locations.
- [ ] Implement fixed-size absolute layout, agreed text-box behavior, colors/backgrounds, static images, and ID/class selection.
- [ ] Define and implement supported stacking, transforms, clipping, and opacity semantics.
- [ ] Add native translate/scale/opacity animation and explicit pause/resume timing.
- [ ] Produce static-scene and animated-shapes examples with expected visual references.
- [ ] Validate unsupported-syntax cases, numeric bounds, parser error recovery, and resource budgets.

### Completion evidence — G3

The content contract agrees with accepted/rejected input and visual behavior. Supported examples render as specified, unsupported features fail clearly, and static scenes remain idle. Keep Flexbox and richer product features out of this prototype unless the documented scope is deliberately revised.

## 7. M4 — First end-to-end Windows prototype

### Work

- [ ] Connect script-visible DOM mutations to host transactions, revisions, acknowledgements, and controlled rejection.
- [ ] Add element lookup, text/style mutation, timers, pause/resume events, and the clock example.
- [ ] Enforce callback and Promise/microtask budgets, bounded host callbacks, and bounded diagnostic output.
- [ ] Preserve the last accepted scene or safe fallback after worker failure.
- [ ] Integrate package/resource validation and restricted helper processing for untrusted parsing/media/font operations before accepting those inputs.
- [ ] Re-run the complete sandbox failure suite through the integrated content path.

### Completion evidence

Run static-scene, clock, and animated-shapes wallpapers behind Windows icons. The clock updates through the sandboxed worker; native animation does not depend on per-frame script calls. Static output does not continuously redraw. G1/G3 remain passing and integrated G2 evidence covers the production worker path. Invalid batches and worker failures do not corrupt host state.

This is the first end-to-end prototype, not a general-purpose browser or a stable product release.

## 8. M5 — Windows product foundations and release readiness

### Work

- [ ] Add package import/validation, wallpaper selection, user settings, and actionable error reporting.
- [ ] Implement multiple-monitor session ownership, caches, fullscreen policy, lock/display-off pause, and suspend/resume recovery.
- [ ] Add percentage sizing, the agreed Flexbox subset, gradients, limited borders/rounding, selectors, keyframes, and restricted events/frame callbacks from the first-product feature contract. Validate and document each addition before marking it available.
- [ ] Test archive traversal, decompression limits, decoded asset limits, malformed compiled packages, and resource cleanup.
- [ ] Establish equivalent WebView reference scenes and reproducible release-build benchmarks for G5.
- [ ] Run long-duration resource monitoring and lifecycle regression tests on the declared Windows matrix.
- [ ] Prepare Windows distribution artifacts, checksums, license/NOTICE material, installation/update/removal instructions, creator documentation, and known limitations.
- [ ] Validate release artifact traceability and workflow behavior; publish only through an explicitly authorized release push after user testing/approval.

### Completion evidence — G5 and Windows release checklist

Record aggregate process/GPU measurements for all four benchmark scenarios; performance claims stay within demonstrated results. The declared Windows configurations pass lifecycle, sandbox, package, and resource-growth checks. Artifacts are installable and traceable to their build revision. User-facing functionality is documented, and release testing is complete before calling the build stable.

## 9. M6 — Stable Linux support

### Work

- [ ] Freeze exact release/kernel/desktop/session/GPU/backend combinations from the Linux support plan.
- [ ] Resolve the documented GNOME, Plasma, and Cinnamon integration choices; implement the required X11/Wayland/desktop adapters.
- [ ] Implement Linux process isolation and resource policies, runtime capability detection, and clear unsupported-configuration errors.
- [ ] Produce distribution-appropriate installation artifacts and add applicable build/validation jobs to CI and release automation.
- [ ] Reuse the shared scene examples, content contract, security corpus, and performance scenarios.
- [ ] Run the complete G6 checklist for each configuration, including lifecycle repetitions, display tests, and the 24-hour soak.
- [ ] Publish result records and installation/troubleshooting instructions per supported configuration.

### Completion evidence — G6

The initial target set is Ubuntu LTS/GNOME, Fedora Workstation/GNOME, Linux Mint/Cinnamon, Debian stable, and Kubuntu LTS/Plasma, with exact sessions specified at kickoff. Each supported configuration must independently satisfy the [Linux stability checklist](linux-support.md). Releases may be incremental; unfinished entries remain planned or experimental and the full Linux milestone remains open until its selected required matrix passes. A layer-shell success elsewhere does not satisfy a GNOME target.

## 10. M7 — Deferred platforms and scope expansion

Android work begins only after desktop milestones. Revisit service lifecycle, native surfaces, isolated bound workers, multiple Engines, device compatibility, fonts, battery, and thermals; activate G4 only then. macOS needs a separate proposal.

Canvas/WebGL/SVG, custom shaders, network access, audio analysis, additional media, and arbitrary website compatibility remain separate scope decisions. They are not implicit requirements for completing this roadmap.

## 11. Task completion and progress tracking

For each task, update its checkbox only after appropriate checks pass and evidence is recorded. Record blocked work with the missing evidence and next action; do not mark it complete because a workaround skips a gate. Keep architecture changes reflected in architecture.md and user-facing changes in the relevant docs/ document.

At delivery, summarize changes, completed checks, untested cases, and concrete user testing/review steps. Ask the user to test or review, and commit only after their approval. Do not create extra branches or push to release without explicit instruction. The user's earlier repository-initialization commit does not approve future commits.

## 12. Next task

Start M0 with the minimal workspace, dependency verification, and threat-model/contract sections needed to run G1 and G2. Then implement the two small Windows spikes before expanding the feature set. This roadmap does not itself authorize implementation, commits, or release publication.
