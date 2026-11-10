# Sahne — Linux Support Plan

## 1. Scope and priority

Linux is the second desktop milestone after the Windows prototype and desktop foundations. The goal is stable wallpapers on a practical set of mainstream distributions and desktop environments. Android is deferred until after these desktop milestones.

The list below is an initial engineering selection, not a measured popularity ranking. Every entry is currently **planned and unverified**. It is not a claim that an integration, package, or sandbox already works.

## 2. Candidate validation matrix

| Distribution family | Desktop target | Session target | Integration work to establish | Current status |
| --- | --- | --- | --- | --- |
| Ubuntu LTS | GNOME | Default session of the chosen supported LTS release | GNOME-compatible background surface and shell lifecycle; no assumed layer-shell support | Planned; unverified |
| Fedora Workstation | GNOME | Default session of the chosen supported release | GNOME integration plus Fedora-specific packaging and sandbox policy | Planned; unverified |
| Linux Mint | Cinnamon | Default session of the chosen supported release | Desktop background/stacking, icons, and actual session backend | Planned; unverified |
| Debian stable | GNOME; KDE Plasma as an additional target | Explicitly selected available sessions | Validate each desktop/session independently and package dependencies | Planned; unverified |
| Kubuntu LTS | KDE Plasma | Default session of the chosen supported LTS release | Plasma-compatible background integration and compositor lifecycle | Planned; unverified |

At Linux milestone kickoff, record exact OS release, kernel, desktop/compositor version, X11 or Wayland session, graphics backend, GPU/driver, package format, and sandbox profile for each configuration. Freeze this matrix for the release; do not use floating labels such as “latest” in test results.

Prefer supported stable/LTS releases. Select additional or legacy X11 sessions only where offered and explicitly tested; do not assume every distribution still provides them. Rolling-release distributions and other desktop/compositor combinations can be added after the initial stable set.

Default GNOME sessions are explicit targets. A successful wlr-layer-shell prototype on another compositor does not satisfy them. Establish whether a desktop-specific adapter or shell component is required, and document its installation, permissions, compatibility, and recovery behavior before selecting the approach. Treat Plasma and Cinnamon integration the same way: validate actual desktop behavior rather than inferring support from protocol availability.

## 3. Support states

| State | Meaning |
| --- | --- |
| Planned | Selected target; implementation and validation incomplete |
| Experimental | Some functionality works; one or more stability/security criteria remain open |
| Supported | The declared release/configuration passes the complete acceptance checklist |
| Unsupported | No maintained integration or required isolation is unavailable |

Support can ship per configuration. Show gaps explicitly; do not describe the entire candidate list as supported after only one configuration passes. The selected set remains the Linux milestone goal, even when individual configurations are released incrementally.

## 4. Integration and security requirements

Keep X11, Wayland, and desktop-specific code behind the platform adapter. Preserve shared scene, package, and scripting semantics. Detect required protocols and sandbox capabilities at runtime and provide an actionable unsupported-configuration error.

The wallpaper must stay behind icons and normal windows, preserve desktop interaction, avoid stealing focus, and survive expected shell/compositor lifecycle changes through controlled recovery. An ordinary development window is not a successful wallpaper fallback.

Create a real Linux sandbox profile with bounded resources and IPC. Test direct OS access from trusted test code inside the sandbox. If mandatory isolation cannot be established, disable untrusted content rather than silently weakening the policy. Kernel and distribution security settings are part of the test configuration.

## 5. Stable-release acceptance checklist — gate G6

Run the following for every configuration claimed as supported. Proposed repetition/duration thresholds are initial engineering criteria, not existing test results.

- Install, launch, update, and uninstall using the declared distribution package method; verify required libraries and clean failure messages.
- Validate static image, clock script, native transform/opacity animation, and malformed-package rejection with shared reference scenes.
- Check wallpaper placement, icons, focus, Show Desktop/workspace behavior, and fullscreen pause policy.
- Exercise at least 10 lock/unlock cycles and 10 suspend/resume cycles; verify controlled recovery without host crashes or accumulating workers/resources.
- Test a single display and dual displays, monitor hotplug, resolution changes, and mixed scaling where supported. Record unavailable hardware cases instead of marking them passed.
- Test shell/compositor restart or session teardown/re-login as appropriate to that environment; no stale surface access or uncontrolled restart loops.
- Confirm static scenes have no continuous application render loop. Measure idle wakeups separately, including any shell component introduced by the integration.
- Verify unauthorized file/network/process access is denied; exercise IPC floods, script/native stalls, memory exhaustion, and worker crashes while the host remains responsive.
- Run a 24-hour mixed-scene soak and record process memory, GPU resources, worker counts, and errors; investigate sustained unbounded growth.
- Record release build, hardware/driver, exact configuration, known limitations, and reproducible results. Re-run affected checks after adapter, sandbox, desktop, or driver changes.

A normal-window test alone, VM-only graphics test, or distribution name without a session/version record cannot establish stable wallpaper support. Use real target hardware for final GPU/lifecycle acceptance; VMs can supplement installation and policy testing.

## 6. Release evidence

Add a result record for each validated configuration containing the matrix identifiers, build revision, installation method, sandbox capabilities, acceptance results, benchmark links, and open limitations. No configuration has a completed result record yet.

Architecture and research references: [Project architecture](architecture.md), [Feasibility research](feasibility-research.md). This plan defines product scope and acceptance; it does not add new verified platform research.
