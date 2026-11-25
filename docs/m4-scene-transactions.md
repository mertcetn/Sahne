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
