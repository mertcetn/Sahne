# M4 host scene transactions

This checkpoint implements the host-side state machine for a validated flat scene. The host starts at revision 0 and expects sequence 1 in a nonzero session. It chooses the writable node IDs from the validated scene. The worker-facing adapter will bind script-visible elements to those IDs in a later step.

| Edit | Target |
| --- | --- |
| Text | Existing writable text node |
| RGBA color | Existing writable rectangle or text node |

An accepted batch contains 1–64 edits, at most 16 KiB per text edit and at most 48 KiB of text across the batch. Two edits to the same property on the same node in one batch are rejected. A text and color edit on the same text node can be accepted together. Existing validation still bounds text across the entire scene and rejects disallowed control characters. Nodes, geometry, fonts, resources and paint order are outside this edit allowlist.

The host checks session, sequence and base revision before inspecting edits. It validates an isolated candidate scene before replacing its current state. An accepted batch advances the revision once and returns the session, acknowledged sequence and new revision. Rejection keeps the exact previously accepted scene and revision and closes the session. A transport failure after commit also closes the session while preserving the committed scene. The next transport must use a fresh session and explicitly synchronize host state before sending updates.

The host API returns typed acknowledgements and rejection reasons. See the wire fixture below for delivery evidence. Worker-side element lookup and renderer support remain open. CPU regressions cover sequential commits, replay and context rejection, invalid targets/properties, budget checks, atomic rollback and preservation after transport close. [Checked revision arithmetic](https://doc.rust-lang.org/std/primitive.u64.html#method.checked_add)

## Wire format and Windows fixture

The bounded control codec adds message kinds 6 and 7 to protocol version 1. Existing tags retain their encodings. The tested transport starts a scene sequence at 1 after its separate probe handshake.

- Kind 6 is a worker-to-host batch. A little-endian u16 count precedes 1–64 edits. Each edit starts with a u8 tag and a little-endian u32 node ID. Tag 1 adds a little-endian u16 byte length followed by UTF-8 text. Tag 2 adds four RGBA bytes. The codec applies the 16 KiB per-text and 48 KiB batch limits before copying text. Malformed frames are transport failures.
- Kind 7 is the host acknowledgement. It echoes the request envelope and carries a little-endian u64 revision equal to the request's base revision plus one. The worker must correlate the reply to the outstanding session, sequence and base revision.
- A decoded invalid batch or wrong-direction body closes the scene session and returns the existing Reject format with code 100 and a fixed diagnostic. Codec and I/O failures require the channel owner to close the session and stop the worker. An acknowledgement delivery failure leaves the committed scene intact.

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case scene-transaction`. This fixed Windows fixture uses the existing zero-capability AppContainer, inherited pipes, Job limits, response deadline, and one-slot writer queue. The worker receives a real acknowledgement for its first update and a terminal rejection for a later batch containing a valid color change followed by invalid text. The host compares the retained scene bytes and revision, and the worker observes EOF after rejection. This path accepts fixed typed edits. Script-visible element lookup and renderer publication remain separate M4 work.

Local evidence: `target/sandbox-validation/15db3a27c2ad42a7884b33e3044ea8e3/`. The scene fixture and the healthy, roundtrip, replay and stall controls passed. The first run stopped at the launcher's case allowlist, which was updated for the fixed fixture.

## Fixed JavaScript clock update

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case scene-script` to exercise a fixed JavaScript update over the same IPC path. The AppContainer worker runs an embedded script in QuickJS with a 16 MiB heap, 256 KiB stack and a 100 ms interrupt deadline. It provides a fixture-scoped `document.getElementById` binding for `clock`. Unknown IDs return null. The text setter stages a string and the getter sees it immediately within the callback. The callback uses the fixed sample time `12:34`. Rust maps its result to the fixture's host-selected node, checks the UTF-8 byte limit and sends the text edit. The worker waits for the matching acknowledgement before continuing.

The fixture's document is local to the embedded script. It covers one lookup and one staged edit. A reusable DOM adapter, timer scheduling, real clock delivery, creator scripts and visual updates are subsequent work. This path shares the existing runtime limits and checks for pending jobs before sending the edit.

Local report: `target/sandbox-validation/304a91be87ee4c1199959c1a65480ea3/`. The JavaScript scene fixture, typed scene control and five existing JS limit/API controls passed with seven verified profile cleanups. [Pinned QuickJS Rust API](https://docs.rs/rquickjs/0.14.0/rquickjs/struct.Runtime.html)

## Multiple element bindings

The fixed script uses the shared worker-side adapter for an author-ID table supplied by Rust. The adapter returns stable objects within one invocation and null for absent names. A text element exposes staged `textContent`, and both text and rectangle elements expose `style.color` as a four-byte RGBA array. Multiple writes to one property emit only the final value. Color arrays are copied at the setter and getter boundaries. Rust maps local indices to the selected scene nodes, checks the edit count and UTF-8 budgets and emits typed edits. Source and bindings remain embedded in this probe.

The `scene-script` case now records the text of `clock` and the RGBA color of `accent` in revision 1. The host verifies both fields and checks preservation after terminal rejection. The fixture verifies lookup identity, missing IDs, duplicate-write coalescing and color copy semantics. The actual input remains a deterministic `12:34` sample. A real clock tick, lifecycle callbacks and general script loading remain future work.

The scripted and typed scene cases plus the basic JavaScript and job-limit controls passed with four verified cleanups. Local report: `target/sandbox-validation/8263cc41a63d48b68c2aac016ca4370e/`. All 155 workspace tests and Clippy passed.

## Compiler-derived fixture bindings

The `scene-script` case compiles embedded HTML and CSS declarations to a validated flat scene. The host and worker independently resolve `clock` and `accent` from the compiler's author-ID table. A binding requires exactly one node of the expected kind. A background node precedes both targets in the fixture. The host grants transaction access only to the two resolved nodes.

The worker maps staged text and RGBA edits to those generated node IDs. The host verifies the committed values and the matching acknowledgement. An invalid follow-up batch still closes the session and leaves the accepted scene and revision intact. This path uses trusted embedded source. Binding delivery for creator packages and support for author elements that compile to multiple nodes remain open.

Release checks for `scene-script`, `scene-transaction` and `scene-timer` passed with three verified profile cleanups. The live rollover bridge passed its headless regression check. Local report at `target/sandbox-validation/6ebfa1ff6c54495ba2bdf908121362e7/`.

## Foreground bindings for spans with backgrounds

The scripted fixture now gives both author spans a background and text. The resolver selects the unique node of the requested content kind inside the author ID's node list. A missing or ambiguous match fails fixture setup. The clock text and the UTC label's foreground RGBA target their respective text nodes. The host allowlist contains those two nodes. The terminal test pairs a permitted foreground edit with an unauthorized background edit and checks that rejection preserves the committed scene and revision. [CSS foreground color](https://www.w3.org/TR/css-color-4/#the-color-property)

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case scene-script` to exercise the mapping and rejection in AppContainer. The fixture covers spans with independent foreground and background nodes. Creator-package binding delivery remains open.

The scripted, typed and timer probes passed with three verified profile cleanups. The live clock bridge also passed its headless rollover check. Local report at `target/sandbox-validation/4c66c42d831847319c0573e200e96502/`.

## Host-delivered clock bindings

The `scene-script` case now compiles the validated embedded package in the host. After Pong and before the first script callback, the host sends a `ClockBindings` frame with the session, sequence 2 and base revision 0. Message kind 10 carries two little-endian u32 node IDs for `clock` and `accent`. Both IDs must be distinct and nonzero. The worker checks the envelope and uses the IDs for the callback's staged edits. Existing scene batches and acknowledgements keep their sequence and revision rules.

The host independently checks each transaction against its writable targets. The terminal test still verifies that an unauthorized background edit rolls back the entire batch. The fixed binding frame applies to the embedded clock example. General author-ID tables and creator-package script loading remain open.

The scripted, typed, timer and roundtrip probes passed with four verified profile cleanups. The live clock bridge also passed its headless rollover check. Local report at `target/sandbox-validation/082f9fb1e4154ef58d8b378f78debf92/`.

### Named binding table

Message kind 11 (`SceneBindings`) replaces the development-only kind 10. Build host and worker from the same checkout. The new frame carries a u8 entry count from 1 to 64. Each entry contains a u8 name length, an ASCII author ID of up to 64 bytes, and a little-endian u32 node ID. Names begin with a letter or underscore and may then include letters, digits, underscores and hyphens. The decoder rejects duplicate names, duplicate or zero node IDs, malformed payloads and nonzero base revisions.

The worker still requires exactly `clock` and `accent` for this embedded fixture. Future callbacks can use the same wire format with other names. Creator script admission and per-property bindings remain open.

The four scripted, typed, timer and roundtrip AppContainer probes passed, as did the headless live rollover check. Local report at `target/sandbox-validation/5e08e0db3d4442ed84577747a6910b23/`.

### Manifest-selected fixture script

The `scene-script` probe now declares a JavaScript entry in its embedded wallpaper package. The host resolves it from the validated snapshot using the compiler's retained script reference. Message kind 12 (`SceneProgram`) sends the binding table and source together. A little-endian u16 byte count and UTF-8 source of up to 32 KiB follow the binding table. The codec rejects zero-length, oversized, invalid UTF-8 and NUL-containing source.

The worker evaluates the source as a callback expression and runs it with the existing DOM adapter. Evaluation and the callback share the existing 100 ms QuickJS deadline, 16 MiB heap, 256 KiB stack and AppContainer isolation. The host applies the same transaction validation, acknowledgement and rollback tests. This command uses an embedded package and continues to require its two clock bindings. Loading creator-supplied packages and their general DOM state remains open.

The scripted, typed and timer probes, three JavaScript controls and the headless live clock check passed. Local AppContainer report at `target/sandbox-validation/6fd0d656a93741899e47418014b6cb79/`.

### Trusted script package check

On Windows, build the developer CLI and pass the absolute path to a trusted local package folder.

```powershell
cargo build --release -p wallpaper-cli --locked
target/release/wallpaper-cli.exe --check-trusted-script-package $folder
```

The command collects the package, compiles a flat scene and validates its image assets. It resolves the declared JavaScript entry and checks the candidate bindings and source with the same initialization encoder used by the sandbox probe. A named span with text and a background contributes its text node. A named element with a single rectangle contributes that rectangle. Image nodes and unnamed elements contribute no binding. The command prints the binding count and source size. It uses the existing workspace protocol crate and adds no third-party dependency.

Run this check only on packages you trust. It parses content in the CLI process and writes the encoded frame to a sink. The command performs no JavaScript evaluation or rendering. A successful check confirms the frame can be encoded. Runtime behavior and creator-package execution require separate validation.

The local release check rejected a missing script and a script over 32 KiB. It accepted a bounded script containing a top-level throw, confirming that it did not evaluate the source. The embedded `scene-script` AppContainer probe passed with the shared target resolver. Local report at `target/sandbox-validation/1b45d80cd3f147669c65e37b65337e09/`.

### Running a trusted local clock package

```powershell
cargo build --release -p wallpaper-sandbox-probe --locked
target/release/wallpaper-sandbox-probe.exe --run-trusted-clock-package $folder
```

Use a trusted local Windows package folder and its absolute path. The native manifest must declare an HTML entry and a JavaScript callback source. The local command requires the named spans `clock` and `accent`. Their initial text and foreground colors come from the package. Either span may have a background. Their order is flexible, and the scene can include decorative elements. Only `clock` and `accent` are exposed to the callback. One possible HTML entry is shown below.

```html
<div style="width:100px;height:100px;background-color:#121212"></div>
<span id="clock" style="width:100px;height:24px;background-color:#222222">initial</span>
<span id="accent" style="width:100px;height:24px;background-color:#333333">UTC</span>
```

The callback receives the fixture document and the sample time `12:34`. It must write that value into `clock.textContent` and set `accent.style.color` to `[36, 179, 144, 255]`.

The host collects and compiles the package with the existing workspace package collector. It sends the source and bindings over the bounded initialization channel. The worker evaluates the callback inside AppContainer, and the host verifies the accepted scene and rollback after its deliberate invalid update. This command is headless. Host-side package collection and parsing still require trusted inputs. General script callbacks, timers and untrusted package intake remain open.

The local release check accepted the folder callback and rejected a replacement callback that threw an exception. Both runs verified AppContainer profile cleanup. The embedded script, timer and restart recovery regressions passed. Local report at `target/sandbox-validation/d96fdf0062e2493588056ad42eb4549d/`.

The layout-independent local case uses the same invalid-text rollback control as the typed probe. The embedded `scene-script` case continues to check an unauthorized background mutation.

The release check passed with the accent span first, a named decorative rectangle next, and the clock span last. The callback confirmed the decorative ID was unavailable. The local worker cleaned up successfully, and the embedded scripted, typed and timer probes passed. Local regression report at `target/sandbox-validation/fa29398f74ba4dd59f2b9a99c017975c/`.

### Initial DOM values from the scene

Message kind 13 extends the scene program with initial text and RGBA records after the source, encoded with the existing bounded edit format. A target must appear in the binding table, and each target and property pair may appear once. These records initialize the worker's callback values at revision zero. The clock worker requires text and foreground RGBA for both spans. The developer CLI's kind 12 check still validates the source and named binding table.

The package callback reads the actual initial text and foreground color for each span. The host verifies that the clock's foreground and accent's label are preserved across the expected edit. A color-only change publishes a snapshot even when the clock text already matches the sample.

A trusted local release test started with `12:34`, a custom `Zone` label and two nonwhite foregrounds. The callback read all four source values correctly. The host acknowledged the color-only change and verified rollback and profile cleanup. The embedded script, typed and timer cases and the headless live clock passed. Local report at `target/sandbox-validation/daf1109b36964455a03b7854d83c3518/`.

### Multiple initialized node types

The worker creates adapter objects from the received names and per-node properties. Each binding requires an initial RGBA value. A text record enables `textContent`. A rectangle returns null for `textContent` and accepts only RGBA writes. The worker retains the values from an edit only after the host acknowledges its transaction.

The embedded `scene-script` case binds the `panel` rectangle in addition to the two existing spans. The script reads the rectangle's initial color, writes a new color, and stages both clock edits in the same batch. The host verifies all three results and then verifies rollback on the unauthorized background mutation. The trusted local clock command continues to grant only its two documented bindings.

The release script, typed scene, timer and basic JavaScript probes passed. The local package and headless live rollover also passed. Local report at `target/sandbox-validation/4e3f5bd95be34a4f84778eff8317b1b3c0/`.

### Running a trusted local script package

```powershell
cargo build --release -p wallpaper-sandbox-probe --locked
target/release/wallpaper-sandbox-probe.exe --run-trusted-script-package $folder
```

Use the absolute path to a trusted native package folder on Windows. The manifest must name an HTML entry and a JavaScript source file. The flat compiler uses a 1920×1080 viewport. The callback can use any admitted author ID for named text and rectangle nodes. It receives `document` as its first argument and an empty string as its second argument. For an HTML text element named `status`, a minimal script is:

```javascript
(document) => {
    document.getElementById("status").textContent = "Updated";
}
```

Each run requires a nonempty batch of valid text or RGBA writes. The source is limited to 32 KiB, and the initialization frame accepts at most 64 property records. Text nodes use two records and rectangles use one. The probe reports the number of acknowledged edits, verifies rollback with a deliberately invalid follow-up and cleans up the worker. The command runs headlessly. Package parsing still runs in the host for trusted inputs. Timer registration, continuous execution and rendering of local scripted packages remain open.

A local release run acknowledged two edits to `status` and `tile` and preserved the accepted scene after rejection. A separate run rejected an attempt to assign rectangle text. Profile cleanup passed for both runs. The trusted clock package, embedded script, typed scene, timer and headless live clock regressions passed. Local AppContainer regression report at `target/sandbox-validation/ef779655429d4bb7bb3014433b7db5a6/`.

### Callback deadlines and diagnostics

Scene script setup, update and lifecycle callbacks use the 100 ms monotonic execution budget. After successful evaluation, the worker checks the time again before publishing staged edits or acknowledging a lifecycle transition. The independent host response deadline remains in force.

During a script failure, the worker returns rejection code 101 with a fixed diagnostic and the in-flight session, sequence and base revision. The host validates this envelope and reports which phase failed without printing worker-supplied diagnostic text or script exception details. A failed callback publishes no scene batch, and a failed lifecycle callback receives no acknowledgement. The host stops the worker and cleans up its profile.

Release checks exercised a throwing scene callback and a throwing resume callback. Both reported the failed phase, omitted the exception text and verified profile cleanup. The resume failure stopped before any scene transaction. The scripted scene, registered timer, interrupt and Promise job probes and headless rollover regression passed. Local regression report at `target/sandbox-validation/9d41ca529bf4467fb037cf10f14854cc/`.

### Bounded Promise jobs

The one-shot scene callback can use Promise microtasks. The worker runs at most 64 queued jobs within the same 100 ms deadline as the synchronous callback. It collects the DOM edits only after the queue is empty. If the script returns a Promise, it must be fulfilled by then. A rejection reported as unhandled by QuickJS also fails the callback. These failures use the bounded diagnostic path and send no partial scene batch.

For example, a trusted script run by `--run-trusted-script-package` can return `Promise.resolve().then(() => { document.getElementById("status").textContent = "Updated"; })` from its callback. The [retained callbacks](m4-timers.md#promise-jobs-in-retained-callbacks) use the same bounds. [QuickJS pending job API](https://docs.rs/rquickjs/0.14.0/rquickjs/struct.Runtime.html#method.execute_pending_job)

The release check acknowledged synchronous and Promise writes in one batch and accepted a write in the 64th queued job. A 65-job queue, an unhandled rejection and an unresolved returned Promise each failed before the host acknowledged any edit. Profile cleanup passed for each run. Four AppContainer regressions and headless rollover also passed. Local report at `target/sandbox-validation/483dafb7748640e6ae42586a1ee70e75/`.

### DOM operation budget

A scene callback can perform up to 256 DOM operations across its synchronous body and Promise jobs. A `getElementById` call counts once, including a lookup that finds no element. Reading or writing `textContent`, reading `style`, and reading or writing its `color` property each count once. The adapter still bounds the batch at 64 distinct property writes. Exceeding either limit fails the entire callback even if the script catches the exception. When the adapter collects the edits, objects obtained from that callback expire. A retained script can obtain fresh bindings on its next tick.

The local release test accepted a write on operation 256 inside a Promise job. Operation 257 failed the invocation even when caught and the host acknowledged no edits. Both runs verified profile cleanup. Four AppContainer regression cases and headless rollover passed. Local report at `target/sandbox-validation/f29755f813d54a4fb370ec26606bfc85/`.

### Retained local script scene

Run the trusted script or lifecycle probe on a local flat text-and-rectangle package. After the host verifies its accepted scene, terminal rollback and worker cleanup, it prints the location of `scene-snapshot.bin` in the run folder. Use the absolute run folder as `$runFolder`.

```powershell
cargo build --release -p wallpaper-sandbox-probe -p wallpaper-preview --locked
target/release/wallpaper-preview.exe --check-scene-worker-snapshot $runFolder
target/release/wallpaper-preview.exe --scene-worker-snapshot $runFolder --seconds 20
```

The check loads the validated flat scene without opening a window. The window command displays the retained scene until Escape or the optional deadline. Both commands independently decode the bounded binary file and require a scene with zero image resources. The artifact contains scene data and provides no origin authentication. Use a trusted run folder. Window rendering uses the existing flat text and rectangle renderer. A live package-to-renderer connection remains open.

The release check exported a two-node scene after the host acknowledged text and rectangle color changes and the worker profile was cleaned up. The headless loader accepted the 1920×1080 export, rejected trailing bytes and rejected a conflicting duration flag. The exported text was `Worker update accepted`. Local visual test input at `target/sandbox-runs/9344-dd9302e011b7eace/`. Three AppContainer regression cases and headless rollover passed. Local report at `target/sandbox-validation/ff8ac44acf654f9c99f2654c28a00acf/`.

The Windows D3D12 visual check of `ee40396` displayed `Worker update accepted` on the updated blue panel in a 960×600 client. The preview recorded one presented frame in one GPU generation. Escape closed the window. Local log at `target/m4-retained-scene-visual/2778d3cdf1f44ff9a952aad33cc27b7c/stderr.log`.

### Direct trusted package preview

Build the preview and sibling probe and pass an absolute native package folder. Use a trusted text-and-rectangle package with a manifest-selected callback.

```powershell
cargo build --release -p wallpaper-preview -p wallpaper-sandbox-probe --locked
target/release/wallpaper-preview.exe --check-trusted-script-preview $folder
target/release/wallpaper-preview.exe --trusted-script-preview $folder --seconds 20
```

The headless check performs the same AppContainer exchange without starting D3D12. Window mode presents the compiled initial scene, then starts the worker. The worker runs one callback. The host validates and acknowledges its batch and publishes only changed content. A subsequent deliberate invalid batch tests rollback. The preview continues to display its last accepted scene after worker exit. The input is compiled in the host. This mode supports text and rectangles.

Release checks verified changed content, an acknowledged identical write and rejection of an invalid first batch. The original headless clock and three AppContainer regression cases passed. The local visual fixture is at `target/m4-retained-scene-package/f7a0e767866745b789a6e91df306833a/`. Local regression report at `target/sandbox-validation/bc9c435b92734305ac1c1b8820771c49/`.

On November 26, 2026, the D3D12 visual check of `52cd70a` recorded the initial presentation, worker snapshot acceptance and the updated presentation in that order. After worker completion, the 960×600 window still showed `Worker update accepted` on the updated blue panel. Escape closed the preview with two frames in one GPU generation. The initial presentation is recorded in the log. The visual observation covers the retained updated frame. Local log at `target/m4-live-package-visual/1ef39f0367064ff6a49d1381522c2b6f/stderr.log`.

#### Integrated script preview regression

Run the Windows headless regression with PowerShell 7 from the repository root.

```powershell
pwsh -NoProfile -File tools/test-script-preview.ps1
```

The runner builds the release binaries and generates local fixtures. It checks changed and unchanged transactions, a Promise-based update, rejection of invalid text, a redacted script exception, the Promise job limit and a missing script. It checks worker profile cleanup for every case that launches an AppContainer. Each run writes a JSON summary and bounded process logs under `target/script-preview-validation/`. Use `-SkipBuild` to check already-built release binaries. This regression covers trusted local input and runs without a renderer.

#### Direct package preview on the desktop

```powershell
target/release/wallpaper-preview.exe --desktop --trusted-script-preview $folder --seconds 30
```

This development command uses the existing Explorer desktop attachment. It displays the trusted text-and-rectangle package behind desktop icons and runs its one-shot callback after the initial frame. The preview keeps the latest accepted scene if the desktop surface is recreated. The callback runs once per preview session. The headless check and file-based retained scene remain separate commands.

On November 26, 2026, the user confirmed that the blue panel and `Worker update accepted` text appeared behind Windows desktop icons with `3d81fd4`. The 90-second run used a 3840×1080 desktop surface across two 1920×1080 displays. The log recorded the initial frame, the worker snapshot, the second frame and worker completion in one GPU generation. The preview stopped at its configured deadline. Local log at `target/m4-script-desktop-visual/8dc5185953ff4fd396e3943a87cb1619/stderr.log`.

## Content snapshots for presentation

The host keeps the transaction revision separate from the content revision. A valid batch that writes the current values still advances the transaction revision and yields an acknowledgement. It skips the scene copy because it leaves already validated content unchanged. A changed batch replaces the immutable scene only after full validation and advances its content revision. Rejection leaves both the snapshot and accepted revision intact.

A consumer can call `snapshot_after` with its last opaque key to obtain the latest immutable scene. Keys distinguish session and content revision. The API owns only the latest scene, so several unobserved updates collapse into the freshest snapshot. Consumers should retain only their current snapshot and request a fresh one on session change or renderer recreation. Rendering integration is still open.

A CPU regression checks 12:34 → 12:34 → 12:35 followed by rejection. It observes two content changes, retains the earlier immutable scene independently and rejects stale presentation after rollback. In the local three-tick AppContainer run the UTC minute stayed unchanged after the first commit, and the host observed one content update across three acknowledged revisions. The typed and scripted controls also passed. Local report: `target/sandbox-validation/0a36f0e42d894d229502f4802e775797/`.
