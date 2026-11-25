# M4 UTC clock preview

The fixed `clock` example feeds host-generated clock text through validated flat-scene transactions and immutable snapshots into the existing Windows D3D12 renderer. It uses the system UTC minute and schedules its next wake near the next minute boundary. The host requests a redraw after the displayed content changes. Resize and exposure can also request a redraw.

```powershell
cargo build --release -p wallpaper-preview --locked
target/release/wallpaper-preview.exe --example clock --seconds 90
target/release/wallpaper-preview.exe --example clock --test-clock-rollover --seconds 10
```

The reference mode samples 23:59, changes to 00:00 at the next two-second scheduled tick, and leaves the displayed value at 00:00. Escape closes the preview. The reference command requires window mode, a duration of 5–30 seconds and no other test flag. Desktop mode is unavailable for this example.

The renderer checks stable scene topology, including bounds, node IDs, fonts and resources, before accepting a snapshot. It reshapes changed text in the bundled font and keeps the accepted scene in the application across renderer reconstruction. It uses the same snapshot interface tested by the separate AppContainer fixture. This preview generates its ticks and edits in the host. Worker-to-window publication remains open.

CPU tests cover the subsecond minute boundary, the deterministic rollover, unchanged content suppression and topology checks. Release help and invalid combinations can be checked without opening a window. Local visual and lifecycle results are recorded below. [winit scheduling](https://docs.rs/winit/0.30.13/winit/event_loop/enum.ControlFlow.html)

## Local Windows verification

On November 25, 2026, source `b8be040681f84bbad60f69413c57dd9c97fc7305` ran on an NVIDIA GeForce RTX 3050 Laptop GPU with D3D12, driver 32.0.16.1062 and DPI scale 1. The release binary SHA-256 was `C77A1974ED1AA4591524641D8BEADA63BB7C990FBAA0E04CC1852D2BC6F7DB3A`.

The rollover window showed [23:59](images/m4-clock-2359.jpg) and then [00:00](images/m4-clock-0000.jpg). Both captures are original 962×632 window images, including chrome. The 00:00 value and UTC label remained visible during maximize to a 1920×1009 client and restoration to 960×600. Escape exited 0. The guarded run recorded four presentations in one generation, with the last two following resize events. Local log: `target/m4-clock-visual/804cbdabc36c4b33b932b62492e9c8f3/stderr.log`.

Two eight-second runs in system-clock mode also passed. Controlled recreation presented three frames over two generations. Requested application-device loss observed the callback and a restored frame, with two presentations over two generations. Both runs exited zero. This checks the local host-driven clock path. The AppContainer-to-renderer path is separate. Local report: `target/m4-clock-lifecycle/7c642b5d7ce7436588be38cc2f869fac/summary.json`.

## Retained AppContainer clock snapshot

The `scene-timer` probe records the accepted clock value after terminal rejection, normal worker exit and verified profile cleanup. The host writes `clock-snapshot.bin` in the probe run folder reported on stdout. The artifact carries exactly five ASCII bytes in UTC HH:MM form. It is a local developer fixture. The file format does not authenticate its origin.

```powershell
cargo build --release -p wallpaper-preview -p wallpaper-sandbox-probe --locked
target/release/wallpaper-sandbox-probe.exe --case scene-timer
```

Use the absolute run folder printed by the probe as `$runFolder`:

```powershell
target/release/wallpaper-preview.exe --check-clock-worker-snapshot $runFolder
target/release/wallpaper-preview.exe --example clock --clock-worker-snapshot $runFolder --seconds 15
```

The loader opens the fixed file name relative to a validated local folder handle, rejects links and reads five bytes plus an EOF check. It accepts only valid UTC hour and minute digits. The preview stages the value in its own validated scene, then holds the snapshot without scheduling clock ticks. The headless check uses this same staging path. The normal host-driven preview remains available independently.

The local AppContainer test accepted three clock revisions, rejected the terminal batch and preserved its last snapshot. The headless preview agreed with the retained 19:57 UTC value. It rejected short input, trailing bytes, hour 24 and minute 60. The worker is finished before the snapshot is opened. This is a retained snapshot handoff. Visual verification of this mode and continuous worker publication remain open.
