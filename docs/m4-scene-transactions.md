# M4 host scene transactions

This checkpoint implements the host-side state machine for a validated flat scene. The host starts at revision 0 and expects sequence 1 in a nonzero session. It chooses the writable node IDs from the validated scene. The worker-facing adapter will bind script-visible elements to those IDs in a later step.

| Edit | Target |
| --- | --- |
| Text | Existing writable text node |
| RGBA color | Existing writable rectangle or text node |

An accepted batch contains 1–64 edits, at most 16 KiB per text edit and at most 48 KiB of text across the batch. Two edits to the same property on the same node in one batch are rejected. A text and color edit on the same text node can be accepted together. Existing validation still bounds text across the entire scene and rejects disallowed control characters. Nodes, geometry, fonts, resources and paint order are outside this edit allowlist.

The host checks session, sequence and base revision before inspecting edits. It validates an isolated candidate scene before replacing its current state. An accepted batch advances the revision once and returns the session, acknowledged sequence and new revision. Rejection keeps the exact previously accepted scene and revision and closes the session. A transport failure after commit also closes the session while preserving the committed scene. The next transport must use a fresh session and explicitly synchronize host state before sending updates.

The current API returns typed acknowledgements and rejection reasons. Wire encoding, worker-side element lookup, actual acknowledgement delivery, renderer support, and integrated AppContainer testing are subsequent work. CPU regressions cover sequential commits, replay and context rejection, invalid targets/properties, budget checks, atomic rollback and preservation after transport close. [Checked revision arithmetic](https://doc.rust-lang.org/std/primitive.u64.html#method.checked_add)
