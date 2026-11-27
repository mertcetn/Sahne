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

## Trusted package lifecycle probe

The Windows developer probe accepts a [trusted native flat package](m4-scene-transactions.md#running-a-trusted-local-script-package). Pass its absolute folder path.

```powershell
cargo build --release -p wallpaper-sandbox-probe --locked
target/release/wallpaper-sandbox-probe.exe --run-trusted-lifecycle-package $folder
```

The manifest's script must evaluate to an object with three functions. `pause()` and `resume()` receive no arguments. `tick(document, value)` receives the existing DOM adapter and an empty string. It must stage a nonempty valid scene batch. The callbacks can share JavaScript state across the sequence.

```javascript
(() => {
    let paused = false;
    return {
        pause() { paused = true; },
        resume() { paused = false; },
        tick(document) {
            if (paused) throw new Error("Unexpected tick during pause");
            document.getElementById("status").textContent = "Resumed";
        },
    };
})()
```

The worker sends `LifecycleReady` (empty message kind 19, sequence 3, base revision 0) after evaluating the callback object. The host waits for readiness and then sends its two scripted transitions in order. The worker acknowledges each transition only after the callback returns. The probe runs one tick, verifies terminal rollback and cleans up the worker. Package parsing runs in the host and requires trusted input. This headless command drives synthetic events. Windows session notifications and package timer registration remain open.

The local release probe acknowledged pause, resume and one edit in that order. A replacement script that threw during resume stopped before the tick. Both runs verified profile cleanup. Four AppContainer regression cases and the headless live clock also passed. Local regression report at `target/sandbox-validation/3ce42ff9c71942e6bf5c5e7431327733/`.

## Promise jobs in retained callbacks

The Windows preview executable also exposes the trusted package lifecycle sequence as a headless check.

```powershell
cargo build --release -p wallpaper-preview -p wallpaper-sandbox-probe --locked
target/release/wallpaper-preview.exe --check-trusted-script-lifecycle $folder
target/release/wallpaper-preview.exe --check-isolated-script-lifecycle $folder
```

These checks use the preview's scene bridge and profile cleanup path. They check worker readiness, synthetic pause and resume acknowledgements, one tick by default and terminal scene rollback. If a lifecycle callback fails, the check stops before accepting a tick. Both variants use the callback object described above.

The isolated variant prepares the package in a separate AppContainer and checks that its scene and bindings agree before launching the lifecycle worker. It accepts the same callback object and timer options. The headless regression exercises three ordered ticks and the rejection of an out-of-range tick count, with cleanup of both AppContainer profiles.

The November 26, 2026 regression verified that pause finished its Promise job before resume, that the tick observed the resumed state, and that a rejected resume returned a fixed error with no accepted scene update. Both cases verified worker profile cleanup. Local report: `target/script-preview-validation/5f3d8ce2a1a64f53acdcd54ab67d2fe0/summary.json`.

The preview check also accepts an optional numeric `intervalMs` field on the callback object. Supply an integer from 16 through 1000. With an interval, the optional integer `tickCount` limits the run to one through three dispatches and defaults to one. Without an interval, the host accepts one immediate tick after resume. A fractional or out-of-range timer value fails initialization.

The host starts the first timer after the synthetic resume acknowledgement. It starts each later timer after acknowledging the previous scene revision. Each tick sees edits accepted by the host on the prior tick. A rejected tick stops further dispatches. This headless check uses one bounded timer and synthetic lifecycle events. General timer registration and real Windows lifecycle delivery remain open.

The November 26, 2026 regression verified a 32 ms registration, dispatch after resume and rejection of a fractional delay before either lifecycle callback. All 24 headless cases passed. Local report: `target/script-preview-validation/26f8afc8cf444bdf91e9732e289bb6c3/summary.json`.

A subsequent run verified three ordered dispatches and accepted revisions, a second-tick failure that stopped before the third dispatch, and rejection of a four-tick request during initialization. All 27 headless cases passed. Local report: `target/script-preview-validation/6f876bdee118404aac5da8739cab3ae8/summary.json`.

### Live trusted package lifecycle preview

```powershell
target/release/wallpaper-preview.exe --trusted-script-live $folder --seconds 90
target/release/wallpaper-preview.exe --check-trusted-script-live $folder
```

The live preview requires a trusted package with the same `pause()`, `resume()` and `tick(document, value)` functions and an integer `intervalMs` from 16 through 1000. The timer runs for the preview lifetime. Use `--desktop` to attach behind Windows icons. `tickCount` is reserved for the bounded lifecycle check.

The preview uses the existing Windows session, display and power notifications and the window's minimized state. It sends pause and resume between acknowledged updates. Acknowledged pause stops new ticks. After resume, the host waits for a new interval before dispatching the next tick. If the worker fails, the preview retains the last accepted scene. Windows session messages are documented by [Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/termserv/wm-wtssession-change).

The live preview logs the first eight presented frames, accepted snapshots and lifecycle acknowledgements in each category. The exit summary reports the totals observed by the preview, and terminal worker failures still appear in the log. Snapshot and acknowledgement totals count events received by the preview from the worker bridge's latest-value mailbox.

The headless live check used a synthetic pause and resume, observed two changed scenes after resuming and verified worker profile cleanup. All 28 cases passed in the November 26, 2026 run. Local report: `target/script-preview-validation/d886cfc265924667abbc1c02b10eeded/summary.json`.

A second headless run verified a live package that staged an edit and threw during its second tick. The bridge verified that revision 1 and its accepted snapshot were preserved before tearing down the worker. The process omitted the private exception text and verified profile cleanup. All 29 cases passed. Local report: `target/script-preview-validation/5e0fe340182c4c659d49ebe446b9b3b0/summary.json`.

A subsequent run distinguished worker callback failure from host scene rejection. A Promise left pending during the second tick preserved revision 1. In another package, the second tick wrote the existing value and advanced the transaction to revision 2 without changing its snapshot. The third tick staged a color change and invalid text. The host rejected the whole batch and preserved revision 2 and the accepted snapshot. Both cases verified profile cleanup. All 31 headless cases passed. Local report: `target/script-preview-validation/35d9d7db1574402a93de15a2253dff43/summary.json`.

The deadline regression stages a write during the second live tick, then starts a finite calculation exceeding the callback's 100 ms budget on the test machine. The worker returned a callback rejection and the host verified that revision 1 remained accepted and that the worker profile was cleaned up. The 34-case run passed. Local report: `target/script-preview-validation/1165dc5f171b4f1481e3bc1c1c6bea01/summary.json`.

On November 26, 2026, the user locked and unlocked Windows, then suspended and resumed it with the live package window running from commit `58234a4`. The user saw the tick counter advance after both operations and the resume count reach two. The log recorded two pause/resume acknowledgement pairs with no accepted snapshots between the acknowledgements in either pair. The preview exited cleanly with 57 frames in one renderer generation and verified AppContainer profile cleanup. Local logs: `target/m4-live-lifecycle-visual/18b70d76aaf341d8a42d1e194f0b45b3/`.

The registered and continuous clock callbacks and the trusted package lifecycle probe use the same bounded Promise job drain. A lifecycle callback runs at most 64 jobs within its own 100 ms deadline. It must finish with an empty queue and fulfill any Promise it returns before the worker acknowledges the event. A tick follows the same rules before its DOM writes are collected. A rejection reported as unhandled fails the invocation. The trusted package's initial callback object must be available synchronously with no queued jobs.

The worker reads `pause`, `resume` and `tick` once during setup. It keeps those function references for the session. A callback can still update shared script state. Replacing properties on the returned object after setup does not change which functions the worker invokes.

The worker initializes the DOM adapter before evaluating package source. The adapter captures the built-ins it uses to build and validate bindings. A regression wrapped the package context's integer helper and verified that two ticks still applied valid color and text edits without invoking that wrapper.

A failed lifecycle callback sends a bounded rejection and terminates the worker session. Its pending jobs cannot run in a subsequent invocation. The host retains the last accepted scene and revision.

In the trusted package release test, pause updated shared state through a Promise. Resume then changed that state in its 64th job, and tick read the result before publishing its edit. A 65-job resume, an unhandled rejection and an unresolved returned Promise each stopped before the resume acknowledgement and tick. Five AppContainer regressions and both headless clock checks passed. The continuous check observed 15:28 → 15:29 UTC with pause, resume and cleanup. Local report at `target/sandbox-validation/28bbb7457be641d7b3e9ead4af73bbdf/`.

### Integrated package watchdog check

```powershell
target/release/wallpaper-preview.exe --check-isolated-script-worker-timeout $folder
```

This headless development check takes a live callback package whose first tick changes the scene. A dedicated AppContainer test worker deliberately waits in native code after the second dispatch. The preview must observe the 8-second response deadline, retain the first accepted snapshot, stop the worker and clean up both profiles. The test uses the normal isolated package preparation and live host supervision paths.

The November 26, 2026 run passed all 47 cases. The watchdog case completed in about 8.7 seconds including startup and cleanup. Local report: `target/script-preview-validation/3e78bf95dfd44d71940b83b6806ebf27/summary.json`.

For the same package shape, `--check-isolated-script-worker-exit <absolute-folder>` runs a separate test worker that exits on the second dispatch before sending a reply. The check requires one accepted snapshot, a clean end-of-stream diagnostic, and cleanup of both profiles.

The `--check-isolated-script-stale-batch <absolute-folder>` fixture sends the previously acknowledged update in response to the second dispatch. The preview must reject the obsolete transport context, keep the first snapshot and clean up both profiles.

The `--check-isolated-script-truncated-batch <absolute-folder>` fixture closes the channel partway through the second response. The preview must report a truncated batch, keep the first snapshot and clean up both profiles.

The regression runner also uses `--check-isolated-script-wrong-session`, `--check-isolated-script-skipped-sequence` and `--check-isolated-script-wrong-message` with the same live callback package shape. Each case expects one accepted snapshot, rejection of the second response without another snapshot, and cleanup of both profiles.

### Sustained live acknowledgements

Run `--check-isolated-script-ack-pressure <absolute-folder>` with a live callback package that produces changing snapshots. The headless check waits for at least 128 acknowledged revisions after synthetic resume and verifies both AppContainer profile cleanups. The runner uses a 16 ms interval and checks from JavaScript that each callback sees the value acknowledged for its predecessor. The revision count measures acknowledged transactions independently of snapshot coalescing in the preview.

The November 27, 2026 regression passed all 48 cases. The early-exit case preserved the first snapshot and detected the closed channel before the watchdog deadline. Local report: `target/script-preview-validation/0d25640fcd5240be98bf62f3a7716316/summary.json`.

## Native interval call limits

The retained registered-clock fixture accepts one native interval registration during setup and one cancellation during its final tick. The worker checks the delay before conversion to milliseconds. It must be an integer from 16 through 64, the fixed fixture's supported range. Cancellation requires the sole valid handle. Invalid and repeated calls leave a sticky failure that prevents the worker from sending the affected initialization response, lifecycle acknowledgement or scene batch. The host independently checks the interval request before scheduling it. General package timers remain outside this fixed fixture.

The native controls check the active callback phase before changing timer state. Only setup can register and only a timer tick can cancel. The cancellation phase stays active through the bounded Promise job drain for that tick. A phase violation leaves the timer registration and cancellation state intact and records a failure for the exchange.

The unit test covers valid controls, repeated operations, fractional and out-of-range delays, and invalid handles. Four AppContainer probes and the headless rollover check passed. Local report at `target/sandbox-validation/de0bebefa4484a91b7def857afb226e4/`.
