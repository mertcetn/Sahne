# Package and IPC Contract

Status: native package and full production scene schemas remain drafts. An [experimental M3 flat-scene codec and validator](m3-scene.md) is implemented for fixed data; it accepts no package/source/asset path. The M2 control-frame subset is implemented; see [Control-frame foundation](m2-protocol.md).

## Package v1

A source package has manifest.json, an HTML entry, optional CSS/JS sources, and local assets. Required manifest fields are formatVersion, apiVersion, id, name, entry, permissions, and preferredFps; script is optional. Both versions start at 1; permissions is empty. Unknown fields and unsupported versions are rejected in the prototype. Accept JavaScript source only. Compiled output contains initial scene, styles/animation, source JS, asset index, and content hashes; the host validates it independently.

Reject traversal, absolute/drive-relative/UNC/alternate-stream paths, links/reparse points, duplicate/case-colliding entries, and external URLs. Validate expanded totals before allocation and decoded dimensions before GPU upload. Retain validated bytes/handles to avoid reopen races. Hashes do not authenticate a publisher.

## IPC v1

Use length-prefixed typed messages. Envelope fields: protocolVersion, sessionId, sequence, baseRevision, operations. Start with SetText and SetStyle using host-issued node/resource IDs and generation checks. No native addresses, paths, or GPU instructions. Validate complete batches atomically. ACK returns accepted revision; rejection terminates the session. Allow one outstanding mutation batch initially. Bound ACK/error/log channels as well as commands.

## Provisional spike limits

These are proposed test-policy values, not established safe/product defaults: 256 MiB worker process memory; 32 MiB JS heap; 50 ms callback cooperative budget; watchdog termination within 1 second after violation detection; 64 KiB IPC frame; 128 operations/batch; 60 batches/second; 256 timers; 1000 microtask jobs/turn; 4096 nodes; 16 KiB UTF-8 text/node; 30 FPS default and 60 FPS cap.

The current fixed M2 JS fixtures use a narrower/different test scope: 16 MiB engine heap, 256 KiB stack, a cumulative 100 ms fixture deadline (not an implemented per-callback allowance), and 64 explicit Promise jobs. Product callbacks/timers and arbitrary source remain disabled; the proposed values above are not a claim that those interfaces exist. See [Actual M2 budgets](m2-js.md). Imported content cannot select between profiles or raise either policy.

Import proposal: 1024 entries, 64 MiB expanded package, 16 MiB source file, 4096 pixels per image dimension, and 128 MiB aggregate decoded assets. Enforce all applicable limits together. M2/G3 must test boundary values and determine adjustments; changes require documented rationale, not disabled checks. Define binary encoding, overflow-safe arithmetic, exact schema, and timeout monitoring before parsing untrusted IPC.

## Planned imported-source frontend

Wallpaper Engine inputs are not native packages and are not currently accepted. After M4, a separate importer may translate the approved local web subset into this format. I1's [capability/inspection boundary](import-adapter-boundary.md) proposes a separate sidecar report; native provenance/adapter fields, mappings and exact encodings remain M3 work. Do not accept arbitrary manifest extensions in the meantime. Native format versions, resource IDs, host validation and limits apply unchanged. Rejection/unknown or explicitly selected lossy conversion are valid outcomes. See [Import plan](wallpaper-engine-import-plan.md).

## Implemented M2 control subset

The safe dependency-free wallpaper-protocol crate implements bounded length-prefixed Hello/Ping/Pong/Reject messages and expected-envelope comparison. See [Exact wire encoding, limits and tests](m2-protocol.md). Scene operations and ACKs described above remain planned; unknown tags are rejected until their schemas exist. The codec is not a sandbox or authenticated IPC channel, and adds no script/package execution path.
