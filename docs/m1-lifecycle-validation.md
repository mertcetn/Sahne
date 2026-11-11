# M1 Lifecycle Validation Record

Date: November 11, 2026. Implementation: working tree based on prototype commit f0cf31a. The earlier lifecycle changes were approved and committed in 2ea71de; the bundled-font/device-test follow-up below awaits separate user review.

## Confirmed environment and visual feedback

The user confirmed the original preview works in both normal-window and desktop modes, using two active 1080p displays. Runtime enumeration reports:

| Display | Physical size | Position | Scale | Refresh |
| --- | --- | --- | --- | --- |
| DISPLAY1 | 1920x1080 | (-1920, 0) | 1.0 / 100% | 120 Hz |
| DISPLAY2 | 1920x1080 | (0, 0) | 1.0 / 100% | 120 Hz |

WorkerW client size: 3840x1080. Adapter: NVIDIA RTX 3050 Laptop GPU, D3D12, driver 32.0.16.1062. OS: Windows 11 Pro build 26200. Mixed scaling is not part of this observed configuration.

## Automated checks

| Check | Outcome | Scope |
| --- | --- | --- |
| Formatting / Clippy with warnings denied | Passed | Workspace; sandbox emitted incremental-cache hardlink/copy notices |
| Recovery budget test | Passed | Ten recovery attempts permitted; all further attempts rejected |
| Release compilation | Passed | Current workspace |
| Ten injected teardown/recreate cycles | Passed | 11 generations, 11 presented frames, clean timed exit, zero pending recreations |
| Ten actual Explorer restart cycles | Passed: 11 generations, 11 frames, clean timed exit code 0 | User explicitly authorized restart of the current-session Explorer; final timed shutdown passed with exit code 0 |

The synthetic test tears down the app's own window/GPU resources; it does not stand in for real Explorer testing. The real test stopped current-session Explorer, ensured a shell was running again, then waited for a new generation to present before the next cycle. It did not accept Explorer process existence alone as success. No screenshot-based post-restart correctness is claimed; the user confirmed baseline visuals before these tests.

Detailed local logs remain in ignored target/m1-desktop-recreate.log and target/m1-explorer-restart.log. Do not rely on those build-directory paths as permanent repository artifacts; this document preserves the summary. No new dependency or lockfile change was needed for lifecycle improvements.

## Implemented recovery changes

- Each native connection records its owner PID. Liveness checks require the same PID, a live HWND and WorkerW class; HWND presence alone is insufficient.
- Check desktop ownership before native-window events and query client size every two seconds. Request child resize only when physical dimensions change.
- Recreate window/GPU state after native-window loss, owner loss, renderer error or current-generation device-loss notification. Delayed events from older generations cannot invalidate a newer state.
- Share a run-wide ten-attempt recovery budget across real faults and injected teardown tests. Recovery waits 500 ms instead of spinning. Successful rendering does not reset this budget.
- Retry timeout/lost/outdated surface acquisition at most three times before escalating to bounded state recreation. Occluded scenes wait for a visibility event.
- Deadline expiry while recovery/testing is unfinished is a failure, even if an older generation rendered successfully. Interrupted synthetic tests also fail.
- Log generations, surface sizes, DPI scales, monitors and frame counts. A periodic desktop probe is distinct from continuous rendering.

An intentionally incomplete release-build stress test (--seconds 2 --test-recreate 10) correctly returned exit code 1 instead of reporting success after its first frame.

## Open G1 cases

Real GPU/driver loss has not been forced; callback plumbing and full resource reconstruction are implemented, but hardware-fault recovery is unverified. The user reported resize and minimize checks passed, and sleep/wake and lock/unlock tests succeeded on November 11, 2026. These are user-reported functional results; no exact cycle counts or resource/timing logs were supplied. Show Desktop, hotplug, changed-DPI behavior and forced driver loss remain unverified. Do not mark them passed from baseline visuals or synthetic recreation alone.

The configuration currently covers one spanned 3840x1080 desktop surface. It is not a validated independent wallpaper-per-monitor implementation. Same-PID/same-class HWND reuse is still theoretically possible between native checks; PID/class validation reduces stale-handle risk without eliminating OS races.

## Next user checks

Run normal mode with a five-minute timeout, resize and minimize/restore, then move the window between displays. Run desktop mode with the same timeout and confirm icons/focus/Show Desktop. If convenient, lock/unlock and perform one suspend/resume cycle during the desktop run; report whether the scene returns, any error, and generation/frame output. Keep hotplug and mixed scaling listed untested if not exercised. A sleep that extends beyond the five-minute deadline intentionally ends the run; use a longer deadline for that case.

Commands and the expected image are in [M1 preview](m1-preview.md). G1 remains open until the required target-configuration cases have evidence.

Primary API documentation consulted on the test date: [wgpu device-loss callback](https://docs.rs/wgpu/30.0.1/wgpu/struct.Device.html), [Windows owner PID](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid), and [Windows client bounds](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclientrect).

## User test update — November 11, 2026

The user explicitly reported successful resizing, minimizing, sleep/wake and lock/unlock checks. Preserve these as successful manual observations on the existing dual-1080p setup. Mixed-DPI/hotplug and forced GPU loss were not reported and remain open. Subsequent user instruction authorized the lifecycle commit 2ea71de.

## Bundled font and device-destruction follow-up

The fixed text now uses embedded Noto Sans with its [license/provenance record](bundled-font.md); system fonts are not scanned. Two regression tests pass: a finite recovery budget and device-test evidence ordering (a new frame without the matching actual callback, a stale callback, an unrelated reason or an old-generation frame cannot pass).

Release-build --test-device-loss runs passed in normal-window (8 seconds) and desktop (10 seconds) mode. Each recorded device.destroy on generation 1, the actual Destroyed loss callback, one recovery, and a presented frame in generation 2, with exit code 0. No Explorer or OS-driver restart was used in these runs. This exercises the actual callback/reconstruction path, but not hardware driver removal. The earlier ten real Explorer cycles and successful manual resize/minimize/lock/sleep observations remain valid evidence of their tested revision; the changed font still requires visual review.

Outstanding matrix cases are Show Desktop, hotplug and changed DPI, plus real driver-fault behavior. They remain untested; do not infer a stable Windows-wide product support claim. M1 implementation tasks can be tracked separately from this incomplete acceptance matrix.
