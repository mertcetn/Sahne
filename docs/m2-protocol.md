# M2 Control-Frame Foundation

Date: November 11, 2026. Status: implemented codec, used by the [trusted Windows sandbox experiment](m2-sandbox.md) and its [fixed JS fixtures](m2-js.md). The codec itself supplies no OS isolation, authentication or JavaScript execution. M1 extended-display tests remain open. M2 may proceed independently under the roadmap.

## Implementation

wallpaper-protocol is a dependency-free, safe Rust library over std::io::Read/Write. The library starts no subprocess and opens no network connection. This is a transport-neutral control subset used in the AppContainer IPC spike; scene transactions remain planned.

| Field | Encoding | Validation |
| --- | --- | --- |
| length | u32 little endian | Payload length excluding these 4 bytes; 27..65536, checked before payload allocation/read |
| protocolVersion | u16 little endian | Exactly 1 |
| sessionId | u64 little endian | Nonzero; must also match host-selected session |
| sequence | u64 little endian | Nonzero; host must enforce the expected value per direction |
| baseRevision | u64 little endian | Must match host-expected scene revision when context is checked |
| kind | u8 | Only the five control types below |
| body | Kind-specific bytes | Exact consumption; extra bytes inside a frame are rejected |

Session/sequence/revision header plus tag occupies 27 bytes. There is no native memory layout serialization, pointer field, pathname field or deserialization of arbitrary objects. Integer conversion is explicit; slicing and size arithmetic are checked.

| Tag | Control message | Body |
| --- | --- | --- |
| 1 | Hello | apiVersion u16, exactly 1 |
| 2 | Ping | nonce u64 |
| 3 | Pong | nonce u64 |
| 4 | Reject | code u16; UTF-8 byte length u16; message bytes, at most 256 |
| 5 | ProbeResult | denied u32; test-only report, not a security authority |

Hello is intended worker-to-host; Ping host-to-worker; Pong and ProbeResult worker-to-host. ProbeResult bits 0/1/2 indicate sentinel read/write and host VM-open denial; bit 4 indicates the trusted worker's AppContainer self-check; bit 5 indicates successful completion of the trusted Job memory probe; bit 6 indicates child-process quota denial; bit 7 indicates a returned invalid-handle result. The host requires exact masks: 0x17 normally, 0x37 for memory, 0x57 for process, or 0x97 for a normally returning handles case, and independently checks the process token. Network denial is investigated separately and is not encoded in this mask. Other values are structurally valid for the codec but not accepted as final results by the probe. Reject can report a bounded failure. Direction, handshake order, nonce matching and rejection policy remain host/worker responsibilities; the codec validates structure, not these application rules. Reject codes have no assigned product semantics yet. Diagnostic text is untrusted and must be escaped before log/display use.

Only the handles fixture uses bit 8 as an armed-stage flag: after Hello(1) and Pong(2), it sends ProbeResult(3) with exactly 0x117 and flushes before querying the excluded handle. A returning API call must produce a final ProbeResult(4) with 0x97 and worker exit 0. Clean EOF after the verified armed stage is merely a termination candidate: the host must also have independently queried enabled strict-handle policy before resume and observe exactly 0xc0000008. Missing/incorrect arming, partial frames, Reject, timeout, wrong context or any other exit fails. This is trusted test telemetry, not production worker authority. No wire tag or codec version changed.

The five fixed JS cases use bit 9 after the selected trusted fixture has verified its expected result. Their exact final mask is 0x217, with the normal nonce/envelope/EOF/exit checks. This is not a privilege grant or independent attestation from an untrusted worker. No new wire tag is added for source loading or general execution.

The [restart experiment](m2-recovery.md) creates a new channel/session for every attempt, resetting per-direction wire sequence to 1 only within that new session. No prior message queue is reused. A correctly formed Pong with the current nonce but an earlier real session ID must fail context validation and stop recovery permanently. Missing frames are now typed host errors so a failed process exit after EOF can be distinguished from malformed/context/trailing-data failures; this does not change the wire format.

The endpoint fixture sets test bit 10 only after four access-denied results against the unrelated host pipe; its exact final mask is 0x417. The host also checks its own positive connection and post-owner namespace cleanup. No new wire tag or endpoint request message is introduced; test telemetry does not grant endpoint access.

The ipc-rights fixture sets bit 11 only after both inherited streams reject DACL reads/changes, ACL/pipe-instance rights upgrades through duplication and wrong-direction data I/O with access denied. Its exact final mask is 0x817, with the usual nonce/context/EOF/exit checks. Every launch also verifies actual host-user-only protected pipe DACLs and uses data-only handles. See [Channel policy and evidence](m2-ipc-security.md). No wire tag/version or worker privilege is added.

The filesystem fixture sets bit 12 only after sixteen direct file/directory access denials and the selected permitted-read controls pass. Its exact mask is 0x1017, with normal protocol/EOF/exit and independent post-stop fixture checks. See [Filesystem matrix](m2-filesystem.md). It adds no production filesystem request or wire tag/version.

check_context compares the expected session, sequence and revision without advancing any state. The host must perform body and ownership validation before committing accepted state or advancing its sequence/revision. Session randomness, peer identity, ACLs and handle ownership are not supplied by a binary header.

## I/O and failure policy

read_frame returns clean EOF only before any length-prefix byte. Partial prefixes/bodies fail. Fragmented and Interrupted reads are supported. write_frame validates/encodes a full message before writing; any I/O failure can still leave a partial stream, so the future channel must close rather than blindly retrying the frame. The codec does not flush automatically.

A synchronous reader can block on a stalled peer. The upcoming sandbox host must enforce external deadlines, bounded queues/rates and connection counts, process memory/CPU limits and termination. A 64 KiB frame cap is not complete denial-of-service protection. Unknown protocol/kind/versions and malformed input should end the future session without a weak fallback.

The trusted one-shot sandbox probe now requires EOF after the final report and rejects the first unsolicited trailing byte without draining further frames. Its fixed host deadline applies to incomplete bodies and peers that keep the stream open. Flood, truncation and replay are exercised through the actual AppContainer channel in [IPC fault validation](m2-ipc-validation.md). This host policy does not change the codec's clean-EOF semantics or introduce persistent scene messages.

Scene SetText/SetStyle transactions, ACKs, resource ownership, node-generation checks and atomic application remain planned M4 integration. They are not implemented by these control messages and must get explicit tags/schema validation before being accepted.

## Verification

Run:

```powershell
cargo test -p wallpaper-protocol --locked
```

Tests cover documented golden wire bytes, concatenated-frame boundaries/EOF, every truncation of a Ping, rejection of oversized lengths before body reads, versions/tags/zero IDs/trailing bytes, UTF-8 byte budgets and invalid UTF-8, mismatched/replayed envelope context, and fragmented/interrupted I/O. These are boundary regressions, not a fuzzing campaign or sandbox certification. Future G2 must repeat malformed-input cases through the actual restricted-process channel.

## Next M2 work

The trusted AppContainer experiment supplies actual pipe transfer, token checks, configured Job limits and a host deadline. A reviewed pinned JS engine now runs fixed trusted fixtures. Complete broader network/handle/exhaustion/channel-pressure tests, scheduling/source transport and the production AppContainer/LPAC policy before untrusted execution. I1 import adapter contracts remain pending; no Wallpaper Engine source has been run.

Current primary references consulted November 11, 2026: [Read/read_exact](https://doc.rust-lang.org/std/io/trait.Read.html), [Write/write_all](https://doc.rust-lang.org/std/io/trait.Write.html), [UTF-8 validation](https://doc.rust-lang.org/std/str/fn.from_utf8.html).

## Local validation result

The original eight-test codec foundation passed local checks and was committed after user approval in 5a245f8. The sandbox continuation adds a ninth exact-body/truncation/trailing-byte regression for ProbeResult and includes it in concatenated-frame coverage. See [Sandbox evidence](m2-sandbox.md) for actual-process checks and their limits. No hosted run, channel flood test or fuzz campaign is inferred. New sandbox changes await user testing/approval before commit.
