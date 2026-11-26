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

## Script timer registration probe

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case js-timers` on Windows. The embedded JavaScript uses scoped `setTimeout`, `setInterval`, `clearTimeout` and `clearInterval` helpers. Native callbacks bridge registration and cancellation to the existing Rust timer queue and map script handles to queue IDs. The fixture rejects a delay below 16 ms, cancels a timeout before dispatch, runs another timeout once, and cancels a repeating timer from its first callback after resume.

The restricted worker supplies controlled monotonic instants in this test. Callback execution stays under the existing QuickJS and AppContainer limits. The local package API still has no timer globals. Host-owned IPC registration, live deadlines and JavaScript lifecycle event delivery remain open. [QuickJS function bindings](https://docs.rs/rquickjs/0.14.0/rquickjs/struct.Function.html)

The release `js-timers`, `js-basic`, `js-interrupt`, `js-jobs` and `scene-timer` probes passed with five verified profile cleanups. Local report at `target/sandbox-validation/f450c1931d0a43ed82d948f3562332dd/`.

## Worker interval registration over IPC

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case scene-timer-registration`. In this fixture an embedded JavaScript setup requests a 32 ms interval inside AppContainer. The worker sends message kind 14 with one little-endian u32 delay in milliseconds after Pong. The host checks the session, sequence 3, base revision 0 and a fixture-specific upper limit of 64 ms. The codec accepts the scheduler's 16 ms to 24 hour range.

The host starts the monotonic schedule from the received delay, pauses it for four intervals, verifies dispatch remains suppressed and resumes with one immediately due callback. Three clock ticks and scene acknowledgements follow, with one tick outstanding at a time. The host checks the schedule after resume and verifies terminal rollback.

The registered timer, existing timer, JavaScript timer and scripted scene probes passed with four verified profile cleanups. The headless live clock regression also passed. Local report at `target/sandbox-validation/17f4dca9fc414204a83d4affc68c1202/`.

## Worker interval cancellation over IPC

The `scene-timer-registration` probe also verifies cancellation. After the third scene acknowledgement the worker sends kind 15 (`CancelInterval`) with sequence 4 and base revision 3. The host requires that exact envelope in the cancellation phase, removes the sole timer and tests the queue at the former deadline. It returns kind 16 (`IntervalCancelled`) with the same envelope. Both messages have empty payloads. The worker waits for this reply before sending the terminal invalid batch. The host checks that the accepted revision and scene survive the rejection.

The initial fixture issued cancellation from Rust after a fixed number of callbacks. The retained callback below now initiates that request from JavaScript.

The registered timer probe logged cancellation and no dispatch at the former deadline. The existing timer, JavaScript timer and scripted scene probes also passed, with four profile cleanups across those cases. The headless live clock check passed. Local report at `target/sandbox-validation/a5779b26f75b4f3bbe1d6201401c1b2a/`.

## Retained callback and script-driven cancellation

The `scene-timer-registration` fixture now stores its registered callback in one QuickJS context for the three acknowledged ticks. Each invocation gets a fresh 100 ms interrupt deadline and uses the same bounded runtime. The DOM values are initialized from acknowledged bindings for that invocation. The callback closes over a private counter and calls its scoped `clearInterval` on the third invocation. The worker checks the cancellation flag before accepting the sequence as complete and sends the existing cancellation request after the corresponding scene acknowledgement.

This is a single-handle development fixture with an embedded script. General user timer handles remain open. [QuickJS context globals](https://docs.rs/rquickjs/0.14.0/rquickjs/struct.Ctx.html#method.globals)

The retained interval, original interval, JavaScript timer, scripted scene and interrupt probes passed with five profile cleanups. The headless live rollover check also passed. Local report at `target/sandbox-validation/09af0234661c443586d9acc035ebe91f/`.

## Pause and resume callbacks over IPC

The `scene-timer-registration` fixture delivers pause and resume to the retained QuickJS context. Protocol kinds 17 and 18 carry the lifecycle event and acknowledgement. Their single state byte is 1 for pause or 0 for resume. The fixture uses sequence 4 for pause and 5 for resume, both at base revision zero, and verifies the full envelope and state for each reply.

The host pauses its queue before notifying the worker. The worker calls the matching JavaScript function under the existing per-callback deadline and acknowledges completion. After the resume acknowledgement, the host dispatches the overdue interval once and schedules its next deadline from that dispatch. The embedded JavaScript checks that pause precedes resume and that both occur before any timer call. Lifecycle callbacks in this fixture stage no DOM edits. Connecting operating system lifecycle events and exposing the callbacks to local package scripts remain open.

The release probe logged both JavaScript acknowledgements and verified three subsequent clock revisions, cancellation and profile cleanup. Four AppContainer controls and the headless live rollover also passed. Local report at `target/sandbox-validation/52db4c027ea4473ea91eb01632cbe1f2/`.
