# M4 UTC clock preview

The fixed `clock` example feeds host-generated clock text through validated flat-scene transactions and immutable snapshots into the existing Windows D3D12 renderer. It uses the system UTC minute and schedules its next wake near the next minute boundary. The host requests a redraw after the displayed content changes. Resize and exposure can also request a redraw.

```powershell
cargo build --release -p wallpaper-preview --locked
target/release/wallpaper-preview.exe --example clock --seconds 90
target/release/wallpaper-preview.exe --example clock --test-clock-rollover --seconds 10
```

The reference mode samples 23:59, changes to 00:00 at the next two-second scheduled tick, and leaves the displayed value at 00:00. Escape closes the preview. The reference command requires window mode, a duration of 5–30 seconds and no other test flag. Desktop mode is unavailable for this example.

The renderer checks stable scene topology, including bounds, node IDs, fonts and resources, before accepting a snapshot. It reshapes changed text in the bundled font and keeps the accepted scene in the application across renderer reconstruction. It uses the same snapshot interface tested by the separate AppContainer fixture. This preview generates its ticks and edits in the host. Worker-to-window publication remains open.

CPU tests cover the subsecond minute boundary, the deterministic rollover, unchanged content suppression and topology checks. Release help and invalid combinations can be checked without opening a window. Visual execution awaits authorization. [winit scheduling](https://docs.rs/winit/0.30.13/winit/event_loop/enum.ControlFlow.html)
