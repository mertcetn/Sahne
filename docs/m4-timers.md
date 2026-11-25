# M4 monotonic callback timers

The host timer queue stores at most 64 active entries per instance. It accepts one-shot and repeating timers with delays from 16 ms through 24 hours. Timer IDs are opaque Rust values and are never reused within an instance. Cancellation is idempotent. The owner supplies monotonic instants and can query the next wake without polling.

A paused queue contributes no timer deadline and returns no due callbacks. Deadlines continue to age while paused. On resume an overdue repeating timer can run once and is rescheduled from its actual dispatch time. Overdue one-shots remain eligible until dispatched or cancelled. Only one timer is selected per call, and dispatches are separated by at least 16 ms per instance. Equal deadlines use registration order. The host must additionally enforce callback execution and transport budgets when wiring the queue into the worker. Decreasing instants and arithmetic overflow are rejected before state changes.

Tests use controlled instants to verify 60 seconds of pause without replay, post-resume rescheduling, bounded dispatch order, capacity, cancellation, delay limits and backward time rejection. This checkpoint supplies the Rust scheduling primitive. JavaScript timer registration, pause/resume callback delivery and the live clock remain integration tasks. [Rust monotonic clock](https://doc.rust-lang.org/std/time/struct.Instant.html)

## Host-driven clock tick over IPC

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case scene-timer`. The host probe registers a 16 ms repeating timer and pauses it across at least four intervals. While paused the queue reports no wake or due callback. On resume it dispatches once and checks the next deadline before sending the clock tick. The 16 ms delay makes the probe short. It is not a clock refresh policy.

Protocol kind 8 carries a little-endian u16 minute of day from 0 through 1,439. The host samples current UTC from the wall clock after the monotonic timer dispatch. The worker verifies the tick's session, sequence and base revision, formats the minute as zero-padded HH:MM and calls the embedded callback. The host requires the clock text and accent color to match revision 1 and verifies that subsequent terminal rejection preserves it. The worker gets time data through a single bounded message. Continuous ticking, OS pause events and rendering remain open.

The local scene-timer, scene-script, scene-transaction and IPC roundtrip cases passed with four verified profile cleanups. Local report: `target/sandbox-validation/becca8d291ef45dca8eb33eb7ee2cac6/`. [Rust wall-clock API](https://doc.rust-lang.org/std/time/struct.SystemTime.html)

## Sequential clock updates in one worker session

The `scene-timer` case now runs three acknowledged clock callbacks after the pause check, using one AppContainer worker and one monotonic timer. The host sends the first tick when the timer resumes. After receiving each matching acknowledgement, the worker can send `ClockReady` for the next sequence. This empty protocol kind 9 carries the next sequence and current base revision in the envelope. The host requires exact context and waits for the timer deadline before sampling and sending the next UTC minute. Each tick and its scene transaction use the same correlation fields. The one-slot writer queue and total response deadline still apply.

The worker carries forward acknowledged clock text into the next callback. Each callback runs an isolated bounded context for the fixed script. Multiple ticks within the same minute may write identical display text, and this fixture verifies distinct commits rather than redraw frequency or a minute rollover. After revision 3, an invalid batch closes the session and the host verifies that revision 3 and its encoded scene survive intact. This is a bounded three-update IPC test. Continuous scheduling, redraw deduplication and renderer publication remain open.

The timed, scripted and typed scene cases and three roundtrip controls passed with six verified profile cleanups. Local report: `target/sandbox-validation/e572dd6dfaf148c890e873d4abad464d/`.
