# Threat Model and Spike Validation

Status: M0 draft. No sandbox is implemented or certified.

## Boundaries

Packages, compiled scenes, JavaScript, images, fonts, and worker messages are untrusted. The host, OS kernel, and GPU driver are trusted. Assume a worker can be compromised through a native engine/parser defect. Keep GPU access in the host and parsing/decoding in restricted helpers. Host validation applies even to compiler-generated data.

Protect user files, credentials, network access, host integrity/availability, other sessions, and bounded system resources. Do not claim protection against a malicious OS/driver, administrator, or physical attacker. Engine interruption is cooperative; an external watchdog is mandatory.

## G1 harness

Record Windows build, GPU/driver/backend and display topology. Draw shape/text/image in a normal HWND, then behind icons. Count renders and wakeups separately. Test 10 Explorer restarts, DPI/resize, monitor hotplug, lock, sleep and resume. No stale native surface use, crashes, or unbounded retry loops. Unavailable hardware cases stay untested.

## G2 harness

Use trusted probe code inside the actual restricted worker to attempt unauthorized file read/write, network connection, process creation/open, and handle access. Check expected permitted executable/library/system reads separately. Prove authorized IPC succeeds and unrelated endpoints/handles are inaccessible. Test infinite JS, native stall, allocation beyond limits, Promise flood, timer flood, invalid lengths, NaN/infinity, stale IDs/revisions, reordered sequences, and ACK/log floods. The host must survive, preserve its last accepted scene, and bound retries/queues. Measure detection and termination times separately.

## G3 harness

Exercise supported and unsupported HTML/CSS syntax with source diagnostics; archive traversal, case collision, links, decompression/decoded-pixel limits; deep trees, excessive text and invalid resource IDs. Use visual references for static scene, animation, and clock; test cascade, layout, group opacity and pause/resume. Fuzz parser, archive, and IPC boundaries when implemented.

## Linux investigation backlog

GNOME background-surface integration remains unresolved for Ubuntu/Fedora targets. Plasma/Cinnamon stacking and shell lifecycle require separate target-session validation. Layer-shell availability does not establish GNOME support. Kernel namespace/seccomp/Landlock policy and packaging remain unvalidated. Freeze exact distribution/session/hardware versions before G6; see the Linux support plan. Android remains deferred.

Record each test with source revision, environment, expected/observed outcome, timing/resource data, and evidence location. Passing a limited corpus is not proof of absolute sandbox safety.
