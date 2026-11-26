# M2 completion record

Reviewed: November 14, 2026. Decision: the Windows sandbox and bounded IPC **technical spike is complete for the fixed trusted fixtures on the recorded local machine**. This delivers the roadmap's initial G2 evidence. The full integrated G2 gate remains open; arbitrary content stays disabled.

## Acceptance against the planned milestone

The [roadmap](roadmap.md#5-m2--windows-sandbox-and-bounded-ipc) places production source/scene/callback integration in M3/M4. This record evaluates M2's minimal-worker feasibility work, without treating missing product APIs as tested or removing their validation requirements.

| M2 requirement | Implemented and exercised evidence | Limits |
| --- | --- | --- |
| Actual restricted worker | Fresh exact AppContainer SID, independently queried token, zero capabilities, explicit two-handle inheritance, curated environment | Standard AppContainer; production/LPAC policy and broker inventory remain open |
| Job limits | 256 MiB process commit, one active process, kill-on-close configured, CPU hard cap read back; direct child quota denial and single-worker CPU membership checked | Abrupt host-death kill/cleanup and all nested-Job environments not exercised |
| Permitted IPC | Protected host-user-only pipe DACLs, data-only inherited handle rights, bounded codec, exact session/sequence/revision/nonce validation | No source/package or scene protocol yet |
| Queue and progress bounds | One-slot result/handshake/command queues, one outstanding request, at most 128 roundtrips, cumulative response deadline, EOF in both directions | Production pipelines, callback traffic and scene ACK scheduling remain M4 work |
| Direct filesystem denial | Sixteen synthetic access/create/listing denials, host positive controls, unchanged post-stop contents/namespace | Selected staged/system reads allowed and documented; not an exhaustive system-read inventory |
| Direct registry denial | Ten synthetic rights/create/delete denials with host controls and exact owned-key cleanup | Other namespaces and independent AppContainer hive/registration deletion audit open |
| Network evidence | Explicit IPv4 outbound TCP denial to TEST-NET-1; separate receiving-side WFP AppContainer Loopback blocks captured and matched | Native loopback timeout still fails alone; WFP diagnostic is elevated/local; sender identity, IPv6/UDP/DNS/brokers and other policies not certified |
| Process/endpoint/handle denial | Host VM access denied, child creation rejected by quota, excluded handle denied or strict-policy terminated, unrelated named pipe denied, channel ACL changes/reverse I/O denied | Synthetic objects and selected APIs only |
| Memory/stack and JS | Native single and cumulative commit tests, release/recovery allocation, host peak accounting; fixed JS heap/stack/interruption/Promise-job cases | QuickJS maintenance uncertainty remains; no arbitrary scripts or host callbacks |
| Failure containment | Native stalls, malformed/truncated/replayed/overlong/flooded/open IPC, crashes, blocked host writes, mid-session stalls/replay | Every accepted attempt retains the one-second classification/Job-stop/I/O-join check; OS construction/profile-call timing is separate |
| Bounded recovery | Three-attempt run-wide budget, backoff, fresh identities/channels, stale completion protection, permanent protocol failures | Explicit fixtures, not a complete production supervisor |
| Cleanup | Explicit Job stop/wait, reader/writer joins, populated profile directory absence; registry sentinel/namespace and owned-key absence | Drop fallback and abrupt host-death/profile-hive audit remain open |
| Import boundary | No imported capability escalation, raw worker paths, sample execution or inspection-time execution | I1 overall mappings/callback semantics and actual importers remain later work |

Implementation details and source references are in [native foundation](m2-sandbox.md), [channel security](m2-ipc-security.md), [filesystem](m2-filesystem.md), [registry](m2-registry.md), [loopback diagnosis](m2-loopback.md), [incremental memory](m2-incremental-memory.md), [roundtrip IPC](m2-roundtrip.md), [JS](m2-js.md), [recovery](m2-recovery.md) and [runner](m2-validation-runner.md).

## Reproducible local evidence

- Final corpus report: ignored `target/sandbox-validation/0bc8f4624c404d7d9a262a0bcbdbf17c/summary.json`, `Status=Passed`, thirty-five cases, forty-one launches/profile cleanups and one registry cleanup. Its metadata records base commit 5b71ff1 with a dirty working tree; the validated implementation was subsequently committed in 06a253a. It is not a clean-checkout or hosted-CI claim.
- Binary SHA256: `efc2d18f2e7629b463c30317da2bf45fa403f22e0665673e404c7c0d7aa9dcb7`.
- Local platform: Windows 11 build 26200, x64, pinned Rust 1.99.1 MSVC. CPU usage observations apply to this eight-logical-processor machine.
- Twenty-one workspace Rust tests, Clippy with warnings denied, formatting and local documentation-link checks passed. Runner self-checks exercised four profile-free helper processes and nine evidence checks; loopback matcher checks passed one positive and twenty negative cases. The queue-full reader regression also passed independently.
- Existing Cargo incremental-cache hardlink/copy notices remain. Manual sandbox CI configuration exists; hosted execution and non-Windows runtime checks for this spike remain unverified.

`G2Complete=false` in runner reports remains intentional: a fixed-corpus run is not an automatic decision to enable untrusted content. M2 completion does not change the security gate, capabilities, limits or release contents. No additional user hardware/visual test is needed for this developer-only transport/worker milestone.

## Mandatory follow-up before untrusted content

These requirements remain active, rather than being declared passed by the fixed corpus:

| Stage | Remaining work |
| --- | --- |
| M3 / G3 | Bounded restricted compiler and scene data, hostile syntax/resources, finite numeric validation, tree/image/archive budgets and source diagnostics; parsing/decoding isolation before untrusted input |
| M4 / integrated G2 | Actual production source/scene transport; IDs/revisions/transactions/ACKs; invalid floats, stale/reordered updates and ACK/log floods; last accepted scene preservation |
| M4 / integrated G2 | Timers, callbacks, Promise/host-callback fairness, pause/resume and their count/time/memory/queue limits; none exists merely because fixed JS executes |
| M4 / isolation review | Final allowed-read/capability/broker policy, broader OS/network endpoint coverage, host-death lifecycle/cleanup and profile registry/hive audit; rerun failures through the real content path |
| M4 / dependency review | Reassess the adopted engine snapshot and full active dependency graph using current advisories and source review before arbitrary source is enabled; preserve unresolved risks in the gate decision |
| M5 / M6 | Product/session/performance and declared Windows/Linux target matrices; no untested platform support claims |

Known dependency findings, including QuickJS source/maintenance uncertainty and the separately assessed renderer lru/ttf-parser issues, remain in [JS dependency review](m2-js-dependencies.md) and [M1 dependency review](m1-dependencies.md). This task changes no dependency or feature and does not resolve those risks. No third-party sample has been executed by these fixtures.

## M4 integrated evidence checkpoint

Reviewed November 26, 2026. The Windows headless package regression passed 34 local cases. Its results cover the development paths below. The G2 gate stays open.

| Area | Evidence | Remaining verification |
| --- | --- | --- |
| Native intake | The restricted helper checks the manifest, compiles flat HTML/CSS and decodes supported PNGs. The host validates returned scene and pixel data. The [isolated script preview](m4-scene-transactions.md#isolated-script-package-preview) also verifies bindings before starting a separate execution worker. The isolated live mode was visually verified on the Windows desktop. | Custom font processing, broader input handling and visual coverage of other isolated script display modes remain pending. |
| Live transactions | The preview accepts bounded script updates and retains the accepted revision after callback or scene rejection. | Run the transport failure suite through the integrated path, including stale/reordered frames and independent deadline and cleanup cases. |
| Callback limits | Promise and DOM limits, staged edit limits, callback deadline, and pause/resume have local AppContainer evidence. | Broader callback and resource failure coverage, including host-death cleanup and OS isolation review. |

See [Windows AppContainer intake check](m3-manifest.md#windows-appcontainer-intake-check), [integrated script regression](m4-scene-transactions.md#integrated-script-preview-regression), and [live trusted package lifecycle](m4-timers.md#live-trusted-package-lifecycle-preview).
