# Sahne — Feasibility Research

Research date: November 10, 2026. Design reviewed: [Project architecture](architecture.md).

Scope update: Windows is the first implementation target. Stable support on selected mainstream Linux configurations follows. Android is deferred until after the desktop milestones; macOS remains a possible future target. See [Linux support plan](linux-support.md).

This is source research. No application build, real sandbox test, or performance measurement has been completed in this repository. “Feasible” means the required building blocks exist, not that Sahne has integrated or validated them. Recommendations and acceptance thresholds are engineering proposals derived from the research.

## 1. Decision

**A Windows product using restricted HTML/CSS, native animation, and isolated JavaScript is technically feasible. The architecture is not yet a validated implementation plan for every target platform.**

No fundamental impossibility was identified. Before broader feature development, validate the Windows desktop surface and sandbox in two small technical spikes. Linux should have an explicit distribution/desktop/session matrix and its own platform gate. Android investigation is retained as future reference and does not block desktop development.

| Claim or goal | Assessment | Evidence still required |
| --- | --- | --- |
| Rust core and wgpu rendering | Feasible | Surface tests on target GPUs and drivers |
| Restricted HTML/CSS compilation | Feasible; custom style engine required | Exact contract, diagnostics, and visual samples |
| QuickJS-family custom scripting API | Feasible | Pinned engine, callback budgets, and recovery |
| Rendering behind Windows desktop icons | Existing implementation evidence; OS behavior dependency | WorkerW + wgpu, Explorer, and DPI tests |
| Untrusted content on Windows | Conditional | Real AppContainer/LPAC + IPC + Job tests |
| Stable Linux on selected mainstream configurations | Scoped product target | Desktop-specific integration, sandbox, and stability matrix |
| One method for every Linux desktop | Not a supportable initial promise | Separate integration and validation per environment |
| Android live wallpaper | Platform API exists; deferred | Service, native surface, and isolated worker spike later |
| No continuous rendering for static content | Feasible design | Instrumented render and wakeup measurements |
| Lower RAM/CPU/energy than WebView | Unknown | Equivalent scenes; total process and GPU measurements |
| Unmodified React/Three.js content | Outside the current scope | A substantially broader web engine scope |

## 2. Windows desktop integration

Lively's platform code uses Progman, WorkerW, SHELLDLL_DefView, and message 0x052C. Separate paths for different desktop window arrangements demonstrate that this is more involved than a single SetParent call. It is evidence that the method works in an application, not a permanent Microsoft compatibility contract. [Lively implementation](https://github.com/rocksdanister/lively/blob/core-separation/src/Lively/Lively/Core/WinDesktopCore.cs)

Keep Explorer integration inside the platform adapter. First render into a normal HWND, then add desktop attachment. Make native window ownership and lifetime explicit; do not keep using a surface after Explorer destroys its window. Failed reattachment must not cause an unlimited restart loop.

The first spike should record target Windows versions, draw shapes/text/images through D3D12, restart Explorer, and exercise Show Desktop, mixed DPI, monitor removal, lock, and suspend/resume. Windows wallpaper support is not complete until these pass.

## 3. GPU and text stack

wgpu supplies native Vulkan, Metal, D3D12, and OpenGL backends. It is a suitable renderer foundation, but does not supply HTML boxes, CSS painting, or font handling. [wgpu documentation](https://docs.rs/wgpu/30.0.1/wgpu/)

Published glyphon 0.12.0 depends on wgpu ^30.0.0 and cosmic-text ^0.19. Therefore **wgpu 30.0.1 + glyphon 0.12.0 + cosmic-text 0.19.0** is a dependency-compatible starting candidate. This is not a successful build result. [glyphon dependencies](https://docs.rs/crate/glyphon/0.12.0)

COSMIC Text provides shaping, text layout, rasterization, and fallback. Its documentation describes full-feature support on Linux, macOS, and Windows and notes that other platforms may need fallback work. Android font discovery/fallback remains future work. Starting with a bundled fixed font simplifies cross-platform visual comparisons. [COSMIC Text scope](https://github.com/pop-os/cosmic-text)

Start with a narrow renderer for rectangles, textures, scissor clipping, and text. Group opacity differs from applying opacity to each overlapping child. Specify overflow, rounded clipping, transform order, stacking, and alpha compositing explicitly; these remain Sahne's responsibilities.

Vello is an alternative. Its current repository distinguishes CPU, GPU, and experimental compute renderers, so “use Vello” is not a sufficiently precise choice. Select an implementation and published version before evaluating it. It need not be a mandatory dependency for the narrow prototype. [Vello options](https://github.com/linebender/vello)

## 4. HTML, CSS, and layout

html5ever is a parser and does not supply a DOM tree representation. HTML5 error recovery means inspecting only the resulting tree may not identify every unsupported source construct at its original location. Validate token/syntax diagnostics and source-position handling in a parser spike. Writing an incomplete custom HTML parser is not recommended. [html5ever description](https://github.com/servo/html5ever)

cssparser supplies CSS syntax infrastructure. Lightning CSS supplies higher-level property/value structures and transformations. Neither automatically supplies Sahne's cascade, inheritance, layout, and painting semantics. Prefer cssparser for a narrow allowlist; reconsider Lightning CSS if a broader property AST becomes useful. This is a scope recommendation. [cssparser API](https://docs.rs/cssparser/0.38.0/cssparser/), [Lightning CSS API](https://docs.rs/lightningcss/1.0.0-alpha.72/lightningcss/)

Taffy supplies Block, Flexbox, and Grid layout algorithms. It is a suitable candidate for later restricted Flexbox, but style mapping, text measurement callbacks, and painting remain application work. [Taffy scope](https://github.com/DioxusLabs/taffy)

The span element suggests browser inline flow. Decide whether the first version treats it as a text box or a fragment in a line. Mixed inline content, whitespace collapsing, line height, baselines, and wrapping substantially increase workload.

The content contract must define:

- Acceptance of html/head/body and style/link/script structures versus manifest-managed resources.
- UTF-8, unique IDs, allowed attributes, and rejection of inline event handlers and external URLs.
- Supported px, percentage, and unitless values; box sizing and coordinates.
- Selector specificity, source order, inline styles, inheritance, shorthands, and !important.
- Whitespace, wrapping, overflow, and font policy.
- Precedence when scripts and keyframes target the same property.

## 5. JavaScript engine and security boundary

QuickJS provides memory, stack, and interrupt mechanisms, and explicitly warns against loading untrusted bytecode. The first package format should accept JavaScript source only. Interrupt callbacks must not be treated as OS-level preemption. [QuickJS developer documentation](https://bellard.org/quickjs/quickjs.html)

The rquickjs main branch inspected during research points its engine submodule at **QuickJS-NG**, a fork of Bellard QuickJS. Record the distributed C source revision, upstream branch, and patches rather than saying only “QuickJS + rquickjs.” Main-branch state does not prove a published crate contains the same commit. [rquickjs submodule](https://github.com/DelSkayn/rquickjs/blob/master/.gitmodules), [QuickJS-NG](https://quickjs-ng.github.io/quickjs/)

rquickjs does not aim to supply web/system APIs; DOM and timers remain custom work. Its full feature group includes native-module loading. Select features individually instead of enabling broad convenience groups. [rquickjs README](https://github.com/DelSkayn/rquickjs), [feature definitions](https://github.com/DelSkayn/rquickjs/blob/master/Cargo.toml)

A BigInt/interrupt-delay report in the Bellard repository supports caution about relying solely on engine interruption. It does not establish that the same issue affects Sahne's eventual NG revision. An external watchdog must terminate a worker even if native engine code stalls. [Primary issue report](https://github.com/bellard/quickjs/issues/545)

Budget Promise/microtask queues as well as timers. A callback that continuously creates microtasks must not block the host. Rust functions called from JavaScript must also perform bounded work: large string processing and formatting are not controlled solely by a JavaScript instruction budget. Preserve the last valid scene after failure and limit restart attempts.

## 6. Windows sandbox

Microsoft documents native AppContainer process launch and the more restrictive LPAC option. Distinguish exposing no filesystem API to JavaScript from the OS permissions of a compromised worker. A normal AppContainer can access some system resources, so “zero file access” is not an accurate blanket guarantee. [AppContainer launch and LPAC](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer)

Job Objects manage process groups, limits, and termination; they do not replace AppContainer access policy. Grant no network capability and define the minimum executable/DLL and IPC access. [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects), [AppContainer isolation](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation)

AppContainer IPC has namespace and ACL requirements. A pipe working between normal user processes may not work in the sandbox. Test the chosen channel with explicit handle transfer and disable general handle inheritance. [Sharing named objects](https://learn.microsoft.com/en-us/windows/apps/develop/communication/sharing-named-objects)

Do not confuse the newer packaged Win32 app isolation feature with directly launching an AppContainer worker. The newer feature's documentation lists preview status and Windows 11 24H2 requirements; these are not automatically the minimum requirements for classic AppContainer. [Win32 app isolation requirements](https://learn.microsoft.com/en-us/windows/win32/secauthz/app-isolation-overview)

The critical test goes beyond checking that JavaScript APIs are absent. Trusted test code inside the sandbox must attempt direct OS operations for unauthorized file reads/writes, networking, process access/creation, and handle access. Record expected permitted system reads separately. These tests validate policy and limits, not absolute sandbox security.

## 7. Linux support

X11 EWMH defines _NET_WM_WINDOW_TYPE_DESKTOP, which is a useful starting point. Stacking and ownership interactions with desktop icons and the shell still need testing in each target environment. [EWMH window properties](https://specifications.freedesktop.org/wm/latest/ar01s05.html)

Wayland wlr-layer-shell supplies a background layer; this does not mean every compositor implements it. Detect the protocol at runtime. Opening an ordinary window must not be reported as successful wallpaper integration. GNOME and KDE integration must be investigated on target releases; this report does not certify them. [Layer-shell protocol](https://wayland.app/protocols/wlr-layer-shell-unstable-v1)

Kernel documentation explicitly says seccomp alone is not a sandbox. Landlock capabilities depend on ABI and kernel configuration. Combine namespace, syscall, filesystem/network, and resource policies. If mandatory isolation is unavailable, disable untrusted package execution. [Seccomp](https://docs.kernel.org/userspace-api/seccomp_filter.html), [Landlock](https://docs.kernel.org/userspace-api/landlock.html)

The agreed scope is stable operation on a practical set of mainstream distributions, rather than universal Linux compatibility. Initial candidates are Ubuntu LTS/GNOME, Fedora Workstation/GNOME, Linux Mint/Cinnamon, Debian stable, and Kubuntu LTS/Plasma. This is a planning selection, not a researched popularity ranking. Exact releases, sessions, integrations, and acceptance criteria are tracked in [Linux support plan](linux-support.md).

Distribution names alone do not resolve compositor compatibility. Default GNOME sessions remain explicit targets requiring investigation, even if another Wayland compositor already passes a layer-shell prototype. Publish support per validated configuration and retain visible gaps. No Linux configuration has been tested yet.

## 8. Android — deferred research

Android supplies WallpaperService and Engine for live wallpapers. Engine exposes surface, visibility, preview, size, and destruction callbacks. A shared renderer is plausible; the desktop window adapter cannot simply be reused. [WallpaperService](https://developer.android.com/reference/android/service/wallpaper/WallpaperService), [Engine lifecycle](https://developer.android.com/reference/android/service/wallpaper/WallpaperService.Engine)

Retain this outline for the later mobile phase:

1. A Kotlin/Java WallpaperService.Engine manages the system Surface lifetime.
2. A JNI/NDK bridge passes surfaces and events to Rust; native wgpu surface creation is tested on real devices.
3. JavaScript runs in a separate bound service, evaluating android:isolatedProcess="true". Desktop child-process launch code is not reused.
4. A Binder/service bridge carries the logical protocol, size limits, and resource identities.
5. Work pauses while invisible; process death and recreation are normal lifecycle events.

Android documents that an isolated service has no permissions of its own. Integration with Sahne's native runtime, Binder, and resource transfer is untested. Merely placing a service in another process through android:process is not equivalent isolation. [Service manifest attributes](https://developer.android.google.cn/guide/topics/manifest/service-element?hl=en)

Preview and active wallpapers may create multiple Engines. Avoid a single-global-scene assumption in shared code. Minimum API, CPU ABIs, launchers/vendors, font fallback, surface recreation, battery, and thermal testing belong to a separate future milestone. They are not desktop release gates.

## 9. Packages, IPC, and scene consistency

Serde provides serialization, not application schema validation or resource budgets. Enforce limits before decoding, during collection/depth allocation, and after semantic validation. [Serde](https://docs.rs/serde/1.0.229/serde/)

Atomic transaction acceptance is appropriate, but worker-local DOM changes can diverge when the host rejects a batch. Start with ordered, bounded pending batches, a baseRevision, and commit after host acknowledgement. On rejection, close the session cleanly instead of implementing complex optimistic rollback. Define a temporary local view for reading a script's own writes during a callback.

Because layout is on the host and the script DOM is in the worker, exclude synchronous getBoundingClientRect, offsetWidth, and computed-style APIs initially. A future asynchronous revision-tagged layout snapshot is possible. Synchronous cross-process reads would introduce blocking and consistency risks.

Also define generation/session checks for reused node IDs, limits for acknowledgement/error queues, order-preserving property coalescing, and log/diagnostic budgets.

Package validation should cover Windows drive-relative/UNC/alternate-stream paths, case collisions, and reparse points. Avoid validation/reopen races by using validated bytes or narrowly authorized handles. Budget font/image decoding inside restricted helper processes. Compiled packages remain untrusted.

## 10. Version, maintenance, and license snapshot

This is a publication snapshot from the research date, not a lockfile. Linked release histories show accessible projects and recent publications, not a security audit or maintenance guarantee.

| Candidate | Observed release | Initial decision / source |
| --- | --- | --- |
| wgpu | 30.0.1 | Renderer foundation; [documentation](https://docs.rs/wgpu/30.0.1/wgpu/) |
| glyphon | 0.12.0 | Compatible text renderer; [dependencies](https://docs.rs/crate/glyphon/0.12.0) |
| cosmic-text | 0.19.0 | Text measurement/shaping; [release](https://docs.rs/crate/cosmic-text/0.19.0) |
| rquickjs | 0.14.0 | Engine-source and target-build validation required; [release](https://docs.rs/crate/rquickjs/0.14.0) |
| html5ever | 0.40.1 | Parser candidate; [release](https://docs.rs/crate/html5ever/0.40.1) |
| cssparser | 0.38.0 | Recommended for restricted CSS; [release](https://docs.rs/crate/cssparser/0.38.0) |
| lightningcss | 1.0.0-alpha.72 | Alternative; account for alpha API changes; [release](https://docs.rs/crate/lightningcss/1.0.0-alpha.72) |
| taffy | 0.15.1 | Flexbox phase; [release](https://docs.rs/crate/taffy/0.15.1) |
| serde | 1.0.229 | Typed data exchange; [release](https://docs.rs/crate/serde/1.0.229) |

Main-branch manifests declare MIT OR Apache-2.0 for wgpu/html5ever/cosmic-text, MIT for Taffy/rquickjs, and MPL-2.0 for cssparser/Lightning CSS. Glyphon declares MIT OR Apache-2.0 OR Zlib. This is preliminary inspection of direct candidates, not a complete inventory of transitive dependencies, fonts, or distributed assets. [wgpu manifest](https://github.com/gfx-rs/wgpu/blob/trunk/Cargo.toml), [html5ever](https://github.com/servo/html5ever/blob/main/Cargo.toml), [cosmic-text](https://github.com/pop-os/cosmic-text/blob/main/Cargo.toml), [Taffy](https://github.com/DioxusLabs/taffy/blob/main/Cargo.toml), [rquickjs](https://github.com/DelSkayn/rquickjs/blob/master/Cargo.toml), [cssparser](https://github.com/servo/rust-cssparser/blob/master/Cargo.toml), [Lightning CSS](https://github.com/parcel-bundler/lightningcss/blob/master/Cargo.toml), [glyphon](https://github.com/grovesNL/glyphon/blob/main/Cargo.toml)

Pin published versions and record Cargo.lock, toolchain, C compiler, enabled features, and engine revision. Record the NDK only when Android work begins. Main-branch manifests can differ from published crates; validate MSRV and transitive compatibility through actual builds. Prepare a dependency/security inventory and license/NOTICE material before distribution.

## 11. Performance: what is known?

A narrow scene model and deadline-driven scheduler can avoid unnecessary work. They do not establish lower total memory or energy use than WebView. Measure textures, glyph atlases, drivers, and per-monitor worker overhead.

Proposed method: release builds on the same device, resolution, FPS, fonts/assets, and scenes; warm up and repeat at least three times. Use a static image, minute clock, transform/opacity scene, and dense animation. Record aggregate host/worker/helper memory and CPU, GPU duration, p50/p95/p99 frame times, startup, and wakeups. Android energy/thermal testing is deferred. These are proposed measurements, not results.

The architecture's introductory setInterval(..., 1000) wakes JavaScript every second even if a minute clock's text does not change. Schedule the next minute boundary for the production example. Measure no redraws separately from no CPU wakeups. Treat fullscreen/visibility detection as platform signals or heuristics.

## 12. Technical validation gates

The thresholds below are proposals, not achieved measurements or security guarantees. Gate identifiers remain stable for cross-references; G4 is deferred, so desktop execution order is G1/G2/G3, G5, then G6.

| Gate | Small technical spike | Acceptance | Failure response |
| --- | --- | --- | --- |
| G1 — Windows surface | Normal HWND + WorkerW + wgpu | Shapes/text/images; 10 Explorer restarts; no crashes in target DPI/monitor transitions; no animation render loop when static | Fix the adapter; do not claim support |
| G2 — Windows isolation | Minimal AppContainer/LPAC worker + real IPC + Job | Allowed IPC works; unauthorized direct OS access is denied; host survives infinite JS, native stalls, OOM, and IPC floods; watchdog terminates the worker within 1 second of detecting the violation in the spike | Keep untrusted content disabled; revise isolation |
| G3 — Content contract | Three examples and unsupported-syntax corpus | Consistent acceptance/rejection and source diagnostics; written text/style/animation rules | Narrow the contract or change parser integration |
| G4 — Android, deferred | Engine + native wgpu surface + isolated JS bound service | Later device matrix validates preview/active Engines, surface recreation, visibility, and worker failure | Revise Android design; no effect on desktop release |
| G5 — Performance | Equivalent scenes against WebView | Repeatable measurements; claims limited to measured advantages | Reconsider renderer or feature scope |
| G6 — Selected Linux configurations | Native wallpaper integration + sandbox + install/lifecycle tests | Pass the matrix and stability criteria in the Linux support plan | Keep failing combinations experimental/unsupported; fix them before claiming support |

G1–G3 form the smallest Windows end-to-end prototype. G5 precedes performance claims. G6 precedes stable Linux support claims and includes the separate GNOME/Plasma/Cinnamon integration work. Android G4 becomes active only after desktop milestones; macOS requires its own future gate.

## 13. Architecture changes and remaining decisions

The updated architecture incorporates the research recommendations: desktop-first rollout, deferred Android, explicit Linux matrix, pinned engine provenance, source-only JavaScript packages, restricted DOM/layout reads, transaction rejection handling, microtask/restart budgets, and the distinction between API exposure and OS permissions.

Detailed follow-up specifications remain necessary:

1. Exact supported HTML/CSS and text behavior.
2. Scripting API and timer/pause semantics.
3. Package and IPC schemas with concrete resource limits.
4. OS threat model and sandbox test harness.
5. Build-validated dependency versions and features.
6. Exact Linux release/session versions and desktop integration choices.

The research supports investing in a small Windows prototype, followed by stable Linux on the selected configurations. It does not establish security, performance, or platform guarantees in advance. The next implementation work should be the measurable G1 and G2 spikes.
