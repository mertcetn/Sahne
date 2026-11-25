# Sahne — Development Roadmap

Updated: November 25, 2026.

## 1. Scope and current state

Deliver a restricted HTML/CSS/JavaScript wallpaper engine on Windows first, then stable Linux support on selected mainstream configurations. Android is deferred until desktop milestones are complete. macOS requires a future scope decision.

M0 foundation validation is complete: the workspace, draft contracts, clean-source build, hosted CI and a downloaded development release have been verified. M1 has local Windows rendering/lifecycle evidence with an extended display matrix still open. M2's fixed trusted Windows sandbox/IPC spike is complete, with its initial G2 evidence and remaining production-gate work recorded in [M2 completion](m2-acceptance.md). Untrusted content and full integrated G2 remain disabled/open; content compilation, product integration, performance and Linux validation are later milestones. See [M0 validation record](m0-validation.md).

This roadmap sequences the existing design; it does not introduce a new architecture. Technical decisions belong in [Project architecture](architecture.md), research and gate definitions in [Feasibility research](feasibility-research.md), and Linux acceptance details in [Linux support plan](linux-support.md).

Dates and effort estimates will be added after the first technical spikes establish integration difficulty. Milestone completion requires recorded evidence, not just merged code.

## 2. Milestone overview

| Milestone | Deliverable | Prerequisites | Exit gate | Status |
| --- | --- | --- | --- | --- |
| M0 | Reproducible development foundation and initial contracts | Existing design documents | Workspace builds; contracts and security tests are reviewable | Complete; evidence committed |
| M1 | Windows rendering and desktop attachment spike | M0 foundation | G1 | Implementation and font/device follow-up approved; extended display matrix open |
| M2 | Windows sandbox and IPC spike | M0 foundation and threat model | Initial G2 evidence | Complete for local fixed trusted fixtures; full integrated G2 remains open |
| M3 | Restricted content compiler and native animation | M1; M0 content contract | G3 | In progress: bounded static/PNG examples and opt-in v2 source compiler validated locally; GPU group effects, animation and full G3 open |
| M4 | Sandboxed scripted end-to-end Windows prototype | M1, M2, M3 | Integrated G2 regression and prototype acceptance | Pending |
| I2 | Selective web import and compatibility report | M4; I1 contracts | Import-specific acceptance; does not block native M5/M6 by default | Planned |
| M5 | Windows product foundations and performance report | M4 | G5 and Windows release checklist | Pending |
| M6 | Stable Linux configurations | M5; early Linux investigation | G6 for each supported configuration | Pending |
| M7 | Deferred Android evaluation | Windows and selected Linux milestones complete | G4 if Android work is activated | Deferred |

M1 and M2 are independent once their M0 prerequisites are ready. The sandbox spike happens early to expose feasibility problems; full script/scene integration happens in M4. Linux desktop integration risks are investigated in M0 and revisited before M6, rather than being discovered only after Windows product work.

## 3. M0 — Development foundation and contracts

### Work

- [x] Record the development toolchain and target Windows/GPU test configuration.
- [x] Recheck current primary documentation for adopted technologies. Review direct/transitive dependency security, maintenance, licenses, provenance, and platform compatibility before adoption; record retrieval dates for cached references.
- [x] Establish a minimal Cargo workspace, lockfile, formatting, linting, and relevant automated checks. Keep the initial crate count small where practical.
- [x] Create the planned content-feature, scripting-API, package-format, and threat-model documents under docs/. Specify supported behavior and explicit exclusions before implementing it.
- [x] Specify IPC envelopes, transaction/acknowledgement semantics, resource ownership, and provisional host-enforced limits in project documentation.
- [x] Define direct OS access tests, worker/native-stall tests, malformed-input cases, and evidence recording for G1–G3.
- [x] Investigate GNOME, Plasma, and Cinnamon integration options and record unresolved blockers against the Linux candidate matrix. Do not claim tested support.
- [x] Establish CI on main and the release publication workflow required by AGENTS.md. Every release push must build and publish uniquely identifiable artifacts through GitHub Releases; failed required checks must prevent publication. Add platform artifacts only as their builds become available.
- [x] Document local build/check commands and the release procedure. Any end-to-end publication test requires an explicit user instruction to push to release.

### Completion evidence

A clean checkout builds the initial workspace and runs its declared checks. Contracts and the threat model can be reviewed independently of implementation. CI configuration is checked, dependency decisions are recorded, and the release workflow's untested portions are clearly identified. Do not block the first spike on complete product documentation; specify the behavior needed by that spike first.

## 4. M1 — Windows rendering and desktop attachment

### Work

- [x] Draw a shape, bundled-font text, and an image in a normal development window.
- [x] Add an isolated Windows desktop attachment adapter and explicit native surface lifetimes.
- [x] Implement change-driven redraw, resize/DPI handling, and surface/device recovery as needed for the spike.
- [x] Handle Explorer restart with bounded reattachment and a clear failure state.
- [x] Record results on the target Windows build, graphics backend, GPU, and driver.

### Completion evidence — G1

Pass the research report's Windows surface gate, including 10 Explorer restarts and the declared monitor/DPI transitions. Static scenes have no continuous application rendering loop. Record unavailable hardware cases as untested. A normal-window demo alone does not complete this milestone.

## 5. M2 — Windows sandbox and bounded IPC

### Work

- [x] Launch a trusted minimal worker in an experimental AppContainer policy with configured Job limits and an explicit handle list; production AppContainer/LPAC policy and negative-handle tests remain open.
- [x] Specify I1's M2 capability/inspection boundary: no imported permission escalation, raw paths as worker APIs or execution during inspection. M3 mappings/callback semantics and the overall I1 gate remain open.
- [x] Establish the spike's actual permitted IPC channel with schema, size, ownership and queue limits; [128 sequential exchanges and mid-session faults](m2-roundtrip.md) complement the one-shot and write-pressure cases. Production source/scene transport remains M4 work.
- [x] Harden and validate the initial one-shot channel's explicit DACLs and data-only handles; record existing schema/deadline/one-slot bounds. Persistent source/scene channels and broader channel-pressure policy remain open; see [channel evidence](m2-ipc-security.md).
- [x] Embed the reviewed pinned engine with minimal features for fixed trusted source fixtures; general source/package transport remains disabled.
- [x] Add and exercise initial runtime memory/stack limits, cooperative interruption, bounded Promise jobs and an independent host watchdog; integrated callback policy remains M4 work.
- [x] Exercise direct OS denials through trusted native code inside the sandbox: selected files, registry, TCP, process, endpoint and handle operations; record permitted reads and the separate local loopback WFP evidence. This is the [documented spike matrix](m2-acceptance.md), not exhaustive OS isolation certification.
- [x] Exercise a synthetic filesystem matrix with host positive controls and sixteen worker denials; record selected permitted staged/system reads separately. Broader filesystem/system-read inventory and G2 remain open; see [filesystem evidence](m2-filesystem.md).
- [x] Exercise initial fixed-source infinite JS, native stalls, OOM, IPC faults/floods, crashes and bounded restart behavior; sustained/integrated pressure and broader G2 coverage remain open.

### Completion evidence — initial G2

Allowed IPC succeeds, prohibited access fails as specified, and the host remains responsive through failure cases. Record permitted executable/library/system reads separately. Pass the G2 spike criteria before enabling untrusted content. If isolation fails, resolve the policy or implementation instead of weakening it.

Completion reviewed November 14, 2026: thirty-five fixed cases and forty-one profile cleanups passed, with twenty-one Rust tests. See [M2 requirement/evidence matrix and mandatory follow-up](m2-acceptance.md). This closes the technical spike and records initial G2 evidence; it does not close integrated G2 or authorize untrusted content. Existing limits and the strict native loopback timeout failure remain unchanged.

## 6. M3 — Restricted compiler and native animation

### Work

- [x] Establish the initial resolved-scene data/validation boundary with an experimental versioned codec, finite numeric/ID/text/resource budgets and original fixed fixtures; [scope and checks](m3-scene.md). This is only the flat foundation, not the completed source compiler or G3.
- [x] Add strict native manifest validation and retained in-memory source entries, with fixed positive/negative AppContainer fixtures; [source ownership and limits](m3-manifest.md). Filesystem/archive collection, compiler resource mapping and compiled hashes remain open.
- [x] Implement the first strict fragment/CSS compiler, fixed-pixel cascade/layout, source diagnostics and original static source fixture; [implemented subset](m3-compiler.md). Broader visual/media/effect coverage and complete G3 remain open; the items below are not complete from this subset alone.
- [x] Render the original compiled static fixture in a Windows window and retain a [limited visual reference](m3-preview.md); controlled quiet-idle, reconstruction and device-loss checks also passed locally on November 15. Broader visual/platform checks, media/effects/animation and complete G3 remain open.
- [x] Add bounded original RGBA8 PNG decoding, retained resource mapping and independent scene/asset validation, including fixed AppContainer checks; [contract and limits](m3-images.md). Local image-window/resize and image/static idle/recreation/device-loss regression checks passed on November 24. Broader hardware/DPI/platform and full G3 remain open.
- [x] Establish a separate version-2 group-data/codec validation boundary with ordered subtrees, accumulated transform/clip checks and intermediate reservations; [implemented data scope](m3-groups.md). Compiler/GPU integration and visual effect acceptance remain open; no new source effects are enabled by this item.
- [x] Add explicit opt-in source-to-v2 group compilation, restricted effect parsing, author/group/source diagnostics and independent v2 image checks; [implemented compiler scope](m3-group-compiler.md). Default preview remains v1; GPU group rendering and visual effect acceptance remain open.
- [x] Implement the fixed group preview's bounded multi-pass code, text rasters and CPU plan/compositing/shader checks; [local window/lifecycle evidence](m3-group-preview.md). The original overlap/clip example passed Windows visual/resize checks and ten group/static/image/M1 lifecycle cases on November 24. Broader effect/configuration and full G3 acceptance remain open.
- [x] Prepare separate nested-opacity, anisotropic-scale and fractional/empty-clip sources with independent scene/assets and CPU reference samples; [effect evidence and scope](m3-effect-fixtures.md). On November 25, local window/maximize/restore and seven lifecycle/runtime cases passed. The subsequent fixed GPU pixel/color comparison passed locally; see [quantitative evidence](m3-pixel-check.md).
- [x] Produce a [versioned compiled provenance record](m3-compiled-provenance.md) and verify repeatable SHA-256 for the fixed v2 group fixture. Version 2 adds typed in-memory export for flat, grouped and animated compilation. Receiver-side exact comparison is implemented against separately supplied source and build inputs. Disk package collection and production integration remain open.
- [x] Specify [I1 web metadata, resource and diagnostic mappings](i1-web-mapping.md) with original synthetic cases. Runtime delivery remains gated on M4.
- [x] Run the [fixed 18-sample GPU check](m3-pixel-check.md) for fractional clipping, nested opacity and PNG colors. All samples passed locally on November 25; normal/maximize/restore window checks also passed. Broader configurations/full G3 remain open.
- [x] Load trusted native Windows folders and validate resource references and versioned scene output. Product helper integration and archive collection are tracked separately.
- [x] Add bounded package ingestion from open readers. Windows directory integration is recorded below.
- [x] Verify the Windows handle-relative child-open primitive against parent replacement, junctions and hard links. Followed by the Windows collector below.
- [x] Connect Windows fixed-drive root acquisition and handle-based enumeration to bounded in-memory package validation. Trusted CLI integration is recorded below.
- [x] Validate, export and verify records for trusted local Windows packages through the [developer CLI](m3-manifest.md#trusted-package-cli). Product/helper integration and archive collection remain open.
- [x] Parse the [agreed strict HTML/CSS subset](m3-compiler.md) and report unsupported constructs at their source locations.
- [x] Implement fixed-size absolute layout, bounded text boxes, colors/backgrounds, PNG images, and ID/class selection.
- [x] Define and implement [group paint order, transforms, clipping and opacity](m3-group-preview.md).
- [x] Add [typed native translate/scale/opacity animation and pause timing](m3-animated-preview.md) for the fixed animated example.
- [x] Establish [typed native animation admission, sampling and explicit pause/resume clock](m3-animation.md), with whole-run transform/opacity bounds and CPU regressions. Source/wire/renderer/FPS/platform integration remains open; this does not complete the parent animation task.
- [x] Add [typed author binding, independent animation codec and fixed animated preview code](m3-animated-preview.md), including stable opacity targets, raster compatibility and capped pause-aware scheduling. Local GPU/window and recovery acceptance passed. CSS animation grammar remains open.
- [x] Retain visual references for the [static](m3-preview.md) and [animated midpoint](m3-animated-preview.md#fixed-midpoint-reference-mode) fixtures. Local animated window, pause and recovery checks are also recorded.
- [x] Validate unsupported syntax, numeric bounds, strict malformed-source rejection and resource budgets. The parser rejects malformed input rather than applying browser recovery.

### Completion evidence — G3

See the [M3 acceptance record](m3-acceptance.md) for the local trusted Windows scope, evidence and M4 handoff.

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

At delivery, summarize changes, completed checks, untested cases, and useful review steps. Run agent-executable checks autonomously; request user testing only when necessary input, visual judgment or physical/unavailable hardware is required. Commit after validation and user authorization without requesting duplicate automatic test runs. Do not create extra branches or push to release without explicit instruction. The user's earlier repository-initialization commit does not approve future commits.

## 12. Next task

The [M3 acceptance record](m3-acceptance.md) closes the trusted Windows prototype scope. Start M4 with bounded host mutation transactions, revision checks and acknowledgements. Production source handling and integrated G2 validation remain part of M4. Extended M1 hardware coverage and later platform gates remain open.

Sections below are historical progress records; their earlier pending/uncommitted statements describe that stage, not the current status above.

## 13. M0 progress — November 11, 2026

Completed the foundation checks and recorded the Windows/GPU inventory, clean-source build, main CI, release workflow, downloaded artifact checksum, and CLI smoke results in [M0 validation record](m0-validation.md). Draft content/API/security contracts are ready for the spikes; they are not implemented functionality. Active display topology and renderer adapter choice are G1 measurements. No external Rust dependencies have been adopted. The checkout runtime deprecation notice is recorded for maintenance. This evidence/documentation update awaits user review before commit.

## 14. M1 progress — November 11, 2026

Implemented normal-window rendering, a WorkerW child-surface adapter, static event-driven submission, resize/DPI events, timed shutdown, bounded surface/reconnection attempts, and adapter/frame logging. Local smoke runs presented one frame each on RTX 3050 DX12 in window and desktop modes. G1 is not complete: visual confirmation, ten Explorer restarts, device loss and display/sleep lifecycle evidence remain open. See [M1 preview and user checks](m1-preview.md). Dependencies have two documented advisories; trusted fixtures only.

### M1 lifecycle follow-up

Prototype f0cf31a was committed after user confirmation of both visual modes. The current working tree adds owner/size validation, generation-tagged GPU-loss recovery, bounded reconnects and a synthetic teardown harness. Ten synthetic cycles and ten real user-authorized Explorer restarts each reached eleven generations; see [Lifecycle validation](m1-lifecycle-validation.md). Remaining G1 cases include recorded resize/DPI, display hotplug, lock/suspend/resume and hardware GPU-loss behavior. These are not inferred from baseline visuals.

### User lifecycle feedback — November 11, 2026

Resize/minimize, sleep/wake and lock/unlock were reported successful by the user. Remaining unverified cases include changed DPI, display hotplug, Show Desktop and forced GPU loss. No additional cycle counts or timings are inferred. Wallpaper Engine import was researched separately in [Import feasibility note](wallpaper-engine-import-research.md); selective planning and the post-M4 importer track have now been approved as recorded in section 15.

## 15. Wallpaper Engine compatibility track

The approved sequence is I0 static local sample assessment now, I1 contract preparation alongside M2/M3, and I2 selective web import after M4. I2 can proceed alongside product work and is not a prerequisite for native Windows/Linux releases by default. PKG resource extraction and scene playback remain separate later decisions. See [Import plan and acceptance](wallpaper-engine-import-plan.md).

- [x] I0: ignore the local samples/ corpus and record its initial file/feature inventory without execution.
- [ ] I1: finalize capability/report schema, metadata/settings/resource mapping and callback semantics against native contracts.
- [ ] I2: implement reporting, supported conversion and bounded API mapping after M4; test positive original fixtures and negative complex samples.
- [ ] I3: scope bounded PKG/TEX extraction separately after web-import evaluation.

The supplied web example is a complex incompatibility fixture, not a required full-fidelity success case. Two samples cannot establish broad compatibility coverage. No importer has been implemented. See [Sample assessment](wallpaper-engine-samples.md).

### M1 implementation checklist follow-up

The prototype implementation checklist is checked: a bundled fixed font replaces system-font discovery, and normal/desktop application-device destruction tests observed the real loss callback and successful reconstruction. This does not close G1's extended configuration matrix or imply driver-fault/hotplug/mixed-DPI certification. Source/font/license and test details are in [Lifecycle validation](m1-lifecycle-validation.md) and [Font record](bundled-font.md). New changes await user visual/test review before commit; next remaining G1 work is Show Desktop and available display/DPI transitions.

### Current continuation

Font and application-device recovery were approved and committed in d50eb30. Show Desktop, physical hotplug and changed DPI remain unverified; no new display support claim is made. M2 proceeds independently with a std-only bounded control codec and malformed-input regressions, documented in [M2 foundation](m2-protocol.md). A codec is not the actual sandbox channel, so the M2 IPC and isolation checklist items remain open.

### M2 actual-process continuation

The approved codec was committed in 5a245f8. The next working-tree change adds a trusted Windows AppContainer process, exact-profile executable ACL, zero-capability token checks, explicit inherited pipes, configured Job limits and a bounded host deadline. Healthy file/process-denial IPC, native stall termination, oversized-prefix rejection and controlled exit tests passed locally. The network investigation times out without explicit denial and deliberately fails acceptance; G2 remains open. No untrusted content or JS engine is enabled. See [M2 sandbox evidence and remaining work](m2-sandbox.md). These new changes await user testing/approval and have not been committed or published.

### M2 network/resource continuation

The user tested healthy and hang successfully and approved the initial sandbox, now committed in 49c0a3f. New separate network-egress and memory cases passed locally: outbound IPv4 TCP to a reserved test destination was explicitly denied, and a Job-verified 256 MiB memory policy rejected a single 257 MiB commit after a successful 64 KiB control allocation. The original loopback case remains inconclusive; full network, memory-exhaustion, handle, process-creation, flood and JS gates remain open. New continuation changes await user testing/approval. See [Commands and scoped evidence](m2-sandbox.md#continuation-outbound-tcp-and-memory-limit).

### M2 process/handle continuation

The user confirmed network-egress and memory and authorized commit 3b9f93f. The next change adds process (Job quota error 1816 for a suspended trusted child) and handles (excluded inheritable sentinel handle, with verified strict-policy fatal handling). Both passed locally; three new reader regressions prevent unarmed/truncated/mismatched output from satisfying the handle case. These are partial G2 checks, not blanket process/handle isolation certification. Broader network/endpoint/escape, CPU/flood and JS work remains. New changes await user testing/approval; see [Detailed boundary and commands](m2-sandbox.md#continuation-process-quota-and-excluded-handles).

### M2 IPC fault/pressure continuation

The validated process/handle changes were approved and committed in 9d53b81, with the user's instruction to run executable tests autonomously. The next change requires EOF after the final result and rejects unsolicited trailing data immediately. Actual restricted-process fixtures cover flood, partial-body stall/EOF, sequence replay and a peer that does not close after a valid report. The scope is fixed-conversation failure containment; persistent valid-message rate budgets, host-write backpressure, restart behavior, full OS isolation coverage and JS remain open. See [IPC fault validation](m2-ipc-validation.md).

### M2 fixed JavaScript engine continuation

IPC validation was approved and committed in 9d207b7. The next change adds reviewed exact rquickjs 0.14.0 with its identified QuickJS-NG snapshot and only std enabled. Fixed trusted scripts execute inside AppContainer with engine heap/stack/deadline/job budgets while the independent Job/watchdog remains active. Five initial real-worker cases passed for normal execution, interruption, OOM, stack and Promise jobs. No arbitrary script/package or creator API is enabled; remaining G2/integration work is explicit. See [JS fixture results](m2-js.md) and [dependency/security assessment](m2-js-dependencies.md). The new continuation is not yet committed.

### M2 bounded recovery continuation

The validated JS continuation was approved and committed in 1261c32. The next change adds a three-attempt, backoff-controlled supervisor for explicit fixed fixtures, fresh session/channel ownership, permanent protocol rejection and stale completion protection. A missing Hello no longer gets an extra response deadline. Initial actual-process recovery/exhaustion/stale-session tests passed; full evidence and remaining network/endpoint/persistent-channel/I1 gaps are in [Recovery validation](m2-recovery.md). M2 remains in progress; the new continuation is not yet committed.

### M2 endpoint and adapter-boundary continuation

Recovery validation was approved and committed in 0d88774. A new trusted named-pipe fixture now proves host positive connection, four worker access denials and disappearance of the endpoint after owner destruction on the recorded machine. The M2 portion of I1 is documented without implementing an importer or declaring callback parity. Broader network/endpoint/escape and production-channel coverage remain open; M2 is not complete. See [Endpoint evidence](m2-endpoint.md) and [I1 boundary](import-adapter-boundary.md). New changes are not yet committed.

### M2 control-channel hardening — November 12, 2026

The endpoint/I1 work was committed in 32ace38. The actual one-shot control pipes now have explicit host-user-only protected DACLs, host readback validation and data-only handles. A fixed worker case rejects ACL reads/changes, rights upgrades through handle duplication and reverse-direction I/O while normal IPC succeeds. All twenty-six expected-success process scenarios and nineteen Rust tests passed locally. See [Channel security, bounds and evidence](m2-ipc-security.md). Persistent source/scene transport, broader G2 coverage and permitted system-read inventory remain open; M2 is not complete. This continuation remains uncommitted.

### M2 filesystem continuation — November 12, 2026

Control-channel hardening was approved and committed in a149196. A new fixed filesystem case checks eleven file-right denials, private listing denial and four file/directory creation denials against synthetic objects with host positive controls. Selected staged/system reads are recorded separately, and post-stop contents/namespace are checked independently. All twenty-seven expected-success process scenarios and nineteen Rust tests passed locally. See [Filesystem evidence and remaining scope](m2-filesystem.md). Complete system-read inventory, broader OS isolation and sustained channel-pressure work remain open; M2 is not complete. These new changes remain uncommitted.

### M2 profile-storage cleanup — November 12, 2026

Filesystem validation was approved and committed in 544fa56. Normal attempt completion now propagates profile-deletion errors and requires absence of the exact profile directory after stop/wait/reader join. Pre-launch nonempty storage controls prevent a missing/empty target from satisfying the audit. All twenty-seven release scenarios passed with thirty-three verified profile-storage deletions; nineteen Rust tests and build/lint checks passed. Profile timing is separate from the existing stop/join bound; registry/hive, abnormal-host-death and early-failure fallback audits remain open. See [Profile cleanup evidence and limits](m2-profile-cleanup.md). M2 remains in progress; this continuation is uncommitted.

### M2 host write-pressure continuation — November 12, 2026

Profile-storage cleanup was approved and committed in 0052ca8. Host writes now belong to one owned writer thread per attempt, keeping the control/deadline thread free of blocking pipe writes. Fixed 1 MiB drain/stall controls verify normal transfer and deadline-triggered release of a pending writer; both I/O threads must join before cleanup/retry. All twenty-nine release scenarios passed with thirty-five verified profile cleanups; nineteen Rust tests and build/lint checks passed. See [Write-pressure evidence and remaining scope](m2-host-write.md). Sustained valid-message/CPU budgets, production transport and broader OS isolation remain open; M2 is not complete. This continuation remains uncommitted.

### M2 native CPU and console-free launch — November 12, 2026

Host write-pressure work was approved and committed in 52be1d5. Probe Jobs now apply and read back a 10% CPU hard cap, with 1% in an armed native-loop fixture. The host verifies measurable bounded CPU and exact live Job membership before the unchanged watchdog stop. PID inspection discovered an implicit conhost under the old launch flags; detached launch now preserves pipe IPC with only the expected worker in the CPU check. All thirty scenarios passed with thirty-six profile cleanups; twenty Rust tests and build/lint checks passed. See [CPU evidence, total-time investigation and limits](m2-cpu-rate.md). Sustained/multiple-session policy, broader OS/broker access and other G2 evidence remain open. M2 is not complete; this continuation is uncommitted.

### M2 reproducible acceptance runner — November 12, 2026

CPU/detached-launch work was approved and committed in 4e23cd9. A single Windows/PowerShell runner now builds and executes the thirty-case corpus, enforces an independent outer host guard and records per-case evidence/provenance in JSON/Markdown. The final default run passed thirty cases and thirty-six cleanups; real helper tests cover runner timeout/output/exit failures, and the inconclusive loopback case correctly produces a failed report. A manual-only read-only GitHub workflow is defined, with local CI-output smoke validation; hosted execution remains unverified. See [Runner evidence and limitations](m2-validation-runner.md). Native implementation/dependencies and release automation are unchanged. M2/G2 remain open; this continuation is uncommitted.

### M2 synthetic registry matrix — November 14, 2026

The runner/workflow was approved and committed in 61385cc. A fresh volatile host-user-only registry fixture now checks eight requested rights plus direct child creation/deletion from the AppContainer, with host positive controls and independent sentinel/namespace/absence checks. All thirty-one runner cases passed with thirty-seven profile cleanups and one mandatory registry cleanup; twenty Rust tests and build/lint checks passed. Runner schema 2 rejects missing registry cleanup evidence. See [Registry evidence and scope](m2-registry.md). This work was committed in 6f6b69e. AppContainer registry-hive deletion, broader OS/broker access and other G2 coverage remain open; M2 is not complete.

### M2 loopback diagnosis — November 14, 2026

A verified host listener control and user-authorized elevated read-only WFP queries established that the current machine's timeout corresponds to a receiving-side AppContainer Loopback block. A separate diagnostic validates the matching filter, host PID/image, endpoint and time; the native timeout still fails by itself. No isolation policy was weakened. See [Loopback evidence, commands and limitations](m2-loopback.md). This resolves the immediate unexplained-timeout investigation on this machine; broader network/G2 coverage remains open.

### M2 incremental memory — November 14, 2026

The new fixed `memory-incremental` fixture exercises bounded cumulative native commit, allocation denial, checked release, recovery allocation and normal exit. The host independently requires 192..256 MiB peak committed memory and exact single-worker Job accounting. See [Incremental memory evidence](m2-incremental-memory.md). The default runner now includes thirty-two cases and thirty-eight profile cleanups; broader G2 and production integration remain open.

### M2 bounded roundtrip channel — November 14, 2026

Incremental memory was committed in 5b71ff1. Three new fixed transport cases exercise 128 sequential validated Ping/Pong exchanges, replay after eight valid replies and native stall after the same progress. A one-slot nonblocking writer-command queue and one outstanding request keep memory bounded, with a cumulative response deadline and explicit sender shutdown before final EOF. The full default corpus passed thirty-five cases and forty-one cleanups; twenty-one Rust tests passed. See [Roundtrip evidence and scope](m2-roundtrip.md).
