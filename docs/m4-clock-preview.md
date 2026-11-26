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

The local AppContainer test accepted three clock revisions, rejected the terminal batch and preserved its last snapshot. The headless preview agreed with the retained 19:57 UTC value. It rejected short input, trailing bytes, hour 24 and minute 60. The worker is finished before the snapshot is opened. This is a retained snapshot handoff. The retained value was verified in a window as recorded below. Continuous worker publication remains open.

### Retained value visual result

On November 25, 2026, source `b2a39c8` displayed the worker run's retained 19:57 UTC value in the 960×600 D3D12 client. The observed digits and UTC label matched the five-byte artifact and the headless check. Escape exited 0. Logs recorded one presentation in one generation. The [worker snapshot reference](images/m4-worker-clock-reference.jpg) is a 962×632 capture including window chrome. Local log: `target/m4-worker-clock-visual/6d083b252f7e4d71a6f07eb254733108/stderr.log`.

## Live AppContainer window bridge

Build both development binaries from the same checkout in a trusted directory. The preview stages the sibling `wallpaper-sandbox-probe.exe` through the existing zero-capability AppContainer launcher. This opt-in fixture requires both binaries.

```powershell
cargo build --release -p wallpaper-preview -p wallpaper-sandbox-probe --locked
target/release/wallpaper-preview.exe --check-clock-worker-live
target/release/wallpaper-preview.exe --example clock --clock-worker-live --seconds 15
```

This fixture sends three UTC samples, 23:59, 00:00 and a repeated 00:00, through inherited pipes. These are deterministic test values. The host verifies the exact fixture edit allowlist, session, sequence and base revision. It acknowledges each accepted batch. The latest changed snapshot replaces the previous mailbox value. The window consumes that immutable snapshot and requests redraw only for new content. The worker sends a final invalid batch to test terminal rejection. The preview retains the last snapshot after worker exit or failure.

The supervisor owns the Job, response deadline and cancellation. The channel thread owns synchronous pipe I/O, and the window reads the single-slot mailbox when it receives a worker event. There is one outstanding tick at a time. Fixture completion requires a clean worker exit, joined I/O thread and profile cleanup. The preview keeps a fixed 00:00 fallback until a snapshot arrives. The live window command requires 8–30 seconds and rejects desktop mode and other test flags. The headless command requires two content changes and retained 00:00 after rejection, so it fails even when the window path would keep its last value.

The headless release test passed over an actual AppContainer connection. All 163 workspace tests and Clippy passed. Local window validation passed. This fixture uses a finite sample sequence and the development probe.

### Live bridge visual result

On November 25, 2026, source `1f5fd4b` ran on the local RTX 3050 Laptop D3D12 configuration with a 960×600 client at DPI scale 1. The preview first displayed its fallback frame, then presented the worker's [23:59](images/m4-live-clock-2359.jpg) and [00:00](images/m4-live-clock-retained.jpg) snapshots. The UTC label changed to the worker's accent color. The final capture was taken after the host logged worker completion following terminal rejection. It still showed 00:00 UTC. The duplicate tick produced no additional presentation. Escape exited 0 after three frames in one GPU generation. Local log: `target/m4-live-clock-visual/33f83c57f61b4416af4e0c255a1e62d2/stderr.log`. These results cover the deterministic fixture and its intentional terminal rejection.

## Continuous UTC worker

Build the two development binaries as above. The headless check waits for two different clock values from one AppContainer session and verifies shutdown and profile cleanup. It can take slightly over a minute. Window mode runs until Escape or until the optional duration elapses.

```powershell
target/release/wallpaper-preview.exe --check-clock-worker-utc
target/release/wallpaper-preview.exe --example clock --clock-worker-utc --seconds 90
```

The host samples UTC on startup and schedules each next sample with a monotonic wait derived from the next minute boundary. On wake it reads the current time, so missed minutes are skipped. The worker computes the clock text in its fixed, bounded script callback and waits for acknowledgement before accepting another tick. The supervisor allows eight seconds for startup and for each scheduled update. The window receives an event when content changes or the worker stops. Closing the preview stops the Job and cleans up the AppContainer profile. If the worker fails, the window keeps its last accepted snapshot. [Rust interruptible wait](https://doc.rust-lang.org/std/thread/fn.park_timeout.html) · [winit event proxy](https://docs.rs/winit/0.30.13/winit/event_loop/struct.EventLoopProxy.html)

On November 25, 2026, the headless release check observed 20:09 → 20:10 UTC in one AppContainer session and verified cleanup after cancellation. The deterministic rollover and the `scene-timer`, `scene-script` and `ipc-roundtrip` probes also passed.

The continuous mode uses a fixed development script. Product worker packaging and desktop attachment remain open.

### Continuous UTC visual result

On November 26, 2026, source `c673dab` displayed 07:39 and then 07:40 UTC in the same 960×600 D3D12 window on an NVIDIA GeForce RTX 3050 Laptop GPU at DPI scale 1. The UTC label retained the worker's accent color across the minute boundary. The log recorded the fallback presentation and one presentation per accepted minute, for three frames in one GPU generation. The preview closed at the configured deadline, and neither the preview nor its worker remained running. Local log at `target/m4-utc-visual/af7933fc4b824b6cb8d0deecff8bf129/stderr.log`.

### Continuous UTC pause and resume

The continuous Windows window mode checks whether the window is minimized on resize notifications. It forwards the latest pause state to the worker and logs acknowledged transitions. The worker uses the same QuickJS context for lifecycle and clock callbacks. The host correlates each transition using a separate monotonic sequence at the current scene revision. Once pause is acknowledged the host parks without scheduling ticks or a response deadline. Resume starts a bounded acknowledgement exchange and triggers a fresh UTC sample. The scene retains its last accepted value while paused. Unchanged content still produces no new snapshot. [winit minimized-state API](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html#method.is_minimized)

The headless UTC command exercises the shared bridge, including a pause longer than the eight-second IPC response deadline, resume and subsequent minute publication. On November 26, 2026, the release check observed 11:17 → 11:18 UTC, both lifecycle acknowledgements and worker cleanup. Three AppContainer controls and the headless rollover fixture passed. Local regression report at `target/sandbox-validation/20549facfa6e41cf9387c42c40775ede/`.

During an idle pause, an unexpected worker exit is detected when the next exchange starts. Closing the preview still cleans up the profile. The Windows minimize and restore path needs visual verification. This development path covers the fixed clock callback. Windows session lock and sleep delivery and integration with local package scripts remain open.

For the Windows visual check, run `wallpaper-preview.exe --example clock --clock-worker-utc --seconds 120`. After the first accepted time appears, minimize the preview and leave it minimized across a UTC minute boundary. Restore it and confirm that the window shows the current UTC minute. The log should show a pause acknowledgement, a resume acknowledgement and the refreshed time in that order.
