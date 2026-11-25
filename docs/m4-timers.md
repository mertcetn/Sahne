# M4 monotonic callback timers

The host timer queue stores at most 64 active entries per instance. It accepts one-shot and repeating timers with delays from 16 ms through 24 hours. Timer IDs are opaque Rust values and are never reused within an instance. Cancellation is idempotent. The owner supplies monotonic instants and can query the next wake without polling.

A paused queue contributes no timer deadline and returns no due callbacks. Deadlines continue to age while paused. On resume an overdue repeating timer can run once and is rescheduled from its actual dispatch time. Overdue one-shots remain eligible until dispatched or cancelled. Only one timer is selected per call, and dispatches are separated by at least 16 ms per instance. Equal deadlines use registration order. The host must additionally enforce callback execution and transport budgets when wiring the queue into the worker. Decreasing instants and arithmetic overflow are rejected before state changes.

Tests use controlled instants to verify 60 seconds of pause without replay, post-resume rescheduling, bounded dispatch order, capacity, cancellation, delay limits and backward time rejection. This checkpoint supplies the Rust scheduling primitive. JavaScript timer registration, pause/resume callback delivery and the live clock remain integration tasks. [Rust monotonic clock](https://doc.rust-lang.org/std/time/struct.Instant.html)

## Host-driven clock tick over IPC

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case scene-timer`. The host probe registers a 16 ms repeating timer and pauses it across at least four intervals. While paused the queue reports no wake or due callback. On resume it dispatches once and checks the next deadline before sending the clock tick. The 16 ms delay makes the probe short. It is not a clock refresh policy.

Protocol kind 8 carries a little-endian u16 minute of day from 0 through 1,439. The host samples current UTC from the wall clock after the monotonic timer dispatch. The worker verifies the tick's session, sequence and base revision, formats the minute as zero-padded HH:MM and calls the embedded callback. The host requires the clock text and accent color to match revision 1 and verifies that subsequent terminal rejection preserves it. The worker gets time data through a single bounded message. Continuous ticking, OS pause events and rendering remain open.

The local scene-timer, scene-script, scene-transaction and IPC roundtrip cases passed with four verified profile cleanups. Local report: `target/sandbox-validation/becca8d291ef45dca8eb33eb7ee2cac6/`. [Rust wall-clock API](https://doc.rust-lang.org/std/time/struct.SystemTime.html)
