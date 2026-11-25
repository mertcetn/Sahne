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

## Content snapshots for presentation

The host keeps the transaction revision separate from the content revision. A valid batch that writes the current values still advances the transaction revision and yields an acknowledgement. It skips the scene copy because it leaves already validated content unchanged. A changed batch replaces the immutable scene only after full validation and advances its content revision. Rejection leaves both the snapshot and accepted revision intact.

A consumer can call `snapshot_after` with its last opaque key to obtain the latest immutable scene. Keys distinguish session and content revision. The API owns only the latest scene, so several unobserved updates collapse into the freshest snapshot. Consumers should retain only their current snapshot and request a fresh one on session change or renderer recreation. Rendering integration is still open.

A CPU regression checks 12:34 → 12:34 → 12:35 followed by rejection. It observes two content changes, retains the earlier immutable scene independently and rejects stale presentation after rollback. In the local three-tick AppContainer run the UTC minute stayed unchanged after the first commit, and the host observed one content update across three acknowledged revisions. The typed and scripted controls also passed. Local report: `target/sandbox-validation/0a36f0e42d894d229502f4802e775797/`.
