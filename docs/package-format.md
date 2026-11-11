# Package and IPC Contract

Status: M0 draft; schemas and concrete validators are pending implementation.

## Package v1

A source package has manifest.json, an HTML entry, optional CSS/JS sources, and local assets. Required manifest fields are formatVersion, apiVersion, id, name, entry, permissions, and preferredFps; script is optional. Both versions start at 1; permissions is empty. Unknown fields and unsupported versions are rejected in the prototype. Accept JavaScript source only. Compiled output contains initial scene, styles/animation, source JS, asset index, and content hashes; the host validates it independently.

Reject traversal, absolute/drive-relative/UNC/alternate-stream paths, links/reparse points, duplicate/case-colliding entries, and external URLs. Validate expanded totals before allocation and decoded dimensions before GPU upload. Retain validated bytes/handles to avoid reopen races. Hashes do not authenticate a publisher.

## IPC v1

Use length-prefixed typed messages. Envelope fields: protocolVersion, sessionId, sequence, baseRevision, operations. Start with SetText and SetStyle using host-issued node/resource IDs and generation checks. No native addresses, paths, or GPU instructions. Validate complete batches atomically. ACK returns accepted revision; rejection terminates the session. Allow one outstanding mutation batch initially. Bound ACK/error/log channels as well as commands.

## Provisional spike limits

These are proposed test-policy values, not established safe/product defaults: 256 MiB worker process memory; 32 MiB JS heap; 50 ms callback cooperative budget; watchdog termination within 1 second after violation detection; 64 KiB IPC frame; 128 operations/batch; 60 batches/second; 256 timers; 1000 microtask jobs/turn; 4096 nodes; 16 KiB UTF-8 text/node; 30 FPS default and 60 FPS cap.

Import proposal: 1024 entries, 64 MiB expanded package, 16 MiB source file, 4096 pixels per image dimension, and 128 MiB aggregate decoded assets. Enforce all applicable limits together. M2/G3 must test boundary values and determine adjustments; changes require documented rationale, not disabled checks. Define binary encoding, overflow-safe arithmetic, exact schema, and timeout monitoring before parsing untrusted IPC.
