# M1 Windows Preview

Status: implementation in progress; G1 is not complete. This is a trusted built-in rendering spike, not package execution or stable wallpaper support.

## Run and stop

From the repository root:

```powershell
cargo run -p wallpaper-preview -- --seconds 15
cargo run -p wallpaper-preview -- --desktop --seconds 15
```

Use the normal window first. Expected scene: dark background, Sahne text, a green rectangle, and an orange/teal checkerboard image uploaded as an RGBA texture. No external image decoding, package, JavaScript, or user font loading is exposed. Text uses installed system fonts for this spike; a bundled fixed font is still needed for portable visual references. Shape/image positions scale with the surface; text uses the reported DPI scale.

Close the normal window or press Escape. Desktop mode may not receive keyboard focus; always use --seconds for the first trials. Ctrl+C in the launching terminal is also available. Duration is 1..3600 seconds; a run with no presented frame fails. The application does not change the stored Windows wallpaper selection, install autostart, or restart Explorer.

Desktop mode requests WorkerW and creates a child window rather than reparenting an already configured GPU surface. It uses the Explorer client area, not a verified per-monitor policy. Ordinary-window fallback is disabled. Failed connections retry at two-second intervals up to ten failures per run. A two-second parent-liveness check is separate from rendering: the static renderer does not continuously draw. Explorer recovery code exists but is not yet validated through actual restarts.

Minimized/zero-size windows do not configure invalid surfaces. Resize, DPI, and visibility events request redraw. Surface loss/outdated recovery is limited to three retries; timeout/occlusion waits for another platform event. Device-loss recovery beyond safe error exit remains pending. Hardware loss and shell HWND reuse remain lifecycle risks to test in G1.

## Recorded checks — November 11, 2026

Environment: Windows 11 Pro build 26200; RTX 3050 Laptop GPU, driver 32.0.16.1062; DX12; observed scale 1.0. Normal window ran for five seconds with one presented frame and exit code 0. Desktop mode ran for eight seconds with one presented frame and exit code 0 outside the agent sandbox. Sandbox-contained Explorer messaging timed out; retry outside that environment succeeded. These counters prove runtime submission, not visual correctness or GPU-energy efficiency.

Formatting, Clippy and test-target compilation passed. There are no behavioral unit tests yet. Release workspace build passed. The release preview ran for three seconds with one presented frame and exit code 0; duration zero was correctly rejected. Do not infer hosted CI success for uncommitted M1 changes. Prior M0 release remains a scaffold.

## User validation required before G1 completion

1. Run the normal-window command and confirm text, shape and checkerboard visibility. Resize, minimize/restore and move between different-DPI displays if available. Confirm clean exit and frame counts stop growing while unchanged.
2. Run desktop mode with a time limit. Confirm it is behind icons/windows, preserves clicks/focus, and disappears cleanly at timeout. Verify Show Desktop behavior and Windows background restoration visually.
3. When convenient, test Explorer restart/reconnection ten times, lock/unlock, suspend/resume, monitor hotplug and display scaling. Record each configuration and outcome; do not mark unavailable cases passed. Explorer restarts were deliberately not automated during the initial interactive session.
4. Record active display topology and adapter selection. Multi-monitor, mixed-DPI, device loss and ten Explorer restart acceptance cases are still open. G1 remains pending until this evidence is available.

Review [M1 dependency assessment](m1-dependencies.md) before adding any untrusted input or expanding the prototype.
