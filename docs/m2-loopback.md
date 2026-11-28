# M2 loopback diagnosis

Updated November 27, 2026. The controlled IPv4 TCP loopback check has acceptance evidence on the local Windows 11 build 26200 machine. Overall G2 remains incomplete.

## Acceptance rule

The controlled fixture requires a successful host listener control, a verified AppContainer with zero capabilities and successful profile cleanup. Either a direct native access-denied result or a native timeout with a WFP AppContainer Loopback block correlated to the verified worker's TCP source port satisfies the loopback check. A timeout alone, receiving-side evidence without sender correlation, or an allowed connection is inconclusive or fails. This decision covers the recorded IPv4 TCP loopback fixture only.

## Finding

The fixed `network` fixture previously failed with `TimedOut` and no raw OS error. A timeout alone cannot distinguish isolation from a broken listener. The host now connects to its exact listener, accepts and verifies that control connection, and closes both control sockets before launching the restricted worker. It prints the listener port, host PID and staged worker image for diagnosis. A failed control prevents launch.

Read-only Windows Filtering Platform (WFP) queries captured two incoming loopback drops during the worker's connection attempt. Both named `AppContainer Loopback` as their filter origin and identified the receiving host, its exact listener port and an App Isolation sublayer blocking filter. This explains why the sender timed out rather than receiving an immediate access-denied error. The receiving event belongs to the host, with a null package SID; treating it as a worker-token event would be incorrect.

No network capability, firewall rule, loopback exemption, event-buffering setting or global capture was changed. No dependency, native binding feature or IPC message was added. Arbitrary content remains disabled.

## Repeat the diagnostic

Build from the repository root in ordinary PowerShell:

```powershell
cargo build --release -p wallpaper-sandbox-probe --locked
./tools/test-loopback-evidence.ps1
```

Then run in an elevated PowerShell 7 window from the repository root:

```powershell
./tools/diagnose-loopback.ps1
```

The script refuses non-administrator execution before launching anything. It runs only the fixed loopback fixture with the existing bounded process guard, then reads existing WFP events filtered to IPv4 TCP, both loopback addresses, the ephemeral listener port and the last thirty seconds. Separate local-port and remote-port queries cover both directions. It does not start tracing or enable event buffering. Platforms without suitable buffered records remain inconclusive; never enable network access to make the diagnostic pass.

Results are retained in a fresh ignored `target/loopback-diagnostics/<id>/` directory: bounded probe logs, filtered event XML, query logs and `summary.json`. XML loading prohibits DTDs/external resolution and limits each file/document to 1 MiB. The summary records the binary SHA256, run interval, port, native exit status and matched drop timestamps/filter IDs. Reports contain local paths and process/network metadata; review before sharing.

`LoopbackDropConfirmed` requires the native fixture's specific timeout rejection, completed host process, verified AppContainer identity with zero capabilities, profile cleanup and WFP evidence correlated to the worker. The WFP matcher requires:

- A classify-drop event during this run, IPv4 TCP, both addresses exactly 127.0.0.1 and the exact listener port.
- Incoming loopback direction, the exact host PID, and the full receiving image path below its drive letter. Netsh supplies an NT device path, so the drive/device mapping itself is not independently verified.
- `AppContainer Loopback` filter origin and the event's exact nonzero filter ID in an App Isolation sublayer entry with `FWP_ACTION_BLOCK`.

The native `--case network` retains its original exit status for a timeout. The elevated diagnostic reports `LoopbackAccepted=true` for `ExplicitDenialConfirmed` or `LoopbackDropConfirmed`. It skips WFP queries when the probe reports a direct denial. The ordinary default runner excludes the separately evaluated loopback fixture.

The WFP event identifies the receiving side. The diagnostic also samples the Windows TCP table during the fixture and retains only a matching loopback connection attributed to the staged worker by executable path and parent PID. The report sets `SenderCorrelated` only when there is one verified sender observation and its source port agrees with a matching WFP drop. A missing or ambiguous observation leaves the receiving-side result available. The receiving event itself still does not carry the worker's package SID. This evidence covers the controlled IPv4 TCP fixture on the recorded Windows configuration. Production validation and G2 remain open. [Windows TCP table and PID display](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/netstat)

## Local evidence

The completed diagnostic report `ffa8b285f54446d4ba4da169a215d615` recorded `LoopbackDropConfirmed`, listener port 61108, native exit 1 and two matching drops at 17:21:49.563Z and 17:21:50.573Z on November 14. Filter ID 73064 was the receiving App Isolation block; these numeric IDs and ports are run-specific. Profile cleanup succeeded. The built binary SHA256 was `7da61fa6def750e919ae8ba6a02f9b7aed85fd4907e7a0560eb44165f06fbb95`.

Pure matcher checks passed one valid event and twenty rejection cases, including wrong time, PID, image, port, direction, protocol, filter association, origin and block action, plus missing fields/events. The manual sandbox CI workflow runs these privilege-free matcher checks; elevated diagnostics have only been exercised locally.

The default native corpus also passed all thirty-one cases, thirty-seven profile cleanups and one registry cleanup in report `a3376f1e5bb741f79764ef4f44abcccc`, with `G2Complete=false`. All twenty workspace Rust tests, Clippy with warnings denied, formatting, PowerShell syntax, runner self-checks and local documentation links passed. The existing incremental-cache hardlink/copy notices remain. No hosted workflow or other Windows build was tested.

The elevated diagnostic was repeated on November 27, 2026 with the current local release probe. It returned `LoopbackDropConfirmed` and matched two receiving-side AppContainer Loopback drops for the fixture listener on port 65217. Both events identified filter 73076. The native probe returned its strict timeout failure and profile cleanup passed. The diagnostic read existing filtered records without changing policy. Local report: `target/loopback-diagnostics/67515ba0713d491da1486529bfa817df/summary.json`. This rerun leaves the native acceptance rule and G2 decision in place.

A separate elevated run on November 27 returned `SenderCorrelated=true`. Windows reported one sender observation for worker PID 7760 on source port 63362, connected toward listener port 63360. Both matched receiving-side WFP drops reported remote port 63362 and filter 73076. The matcher suite passed two positive and twenty-one negative cases. Local report: `target/loopback-diagnostics/85bb6cf829aa4ac5b74d12ff9b13587b/summary.json`.

## IPv6 TCP fixture

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case network-loopback-v6` to test TCP on `[::1]`. A host connection to the same listener is accepted and drained before the zero-capability AppContainer worker tries to connect. The worker sends no application payload and performs no DNS lookup. The native fixture accepts an explicit OS access denial and treats a timeout as inconclusive. From an elevated PowerShell, run `pwsh -NoProfile -File tools/diagnose-loopback.ps1 -IPv6` to check for receiving-side AppContainer blocks on the exact IPv6 loopback address, listener port and host process. A timeout is accepted by the diagnostic only if a matching block names the source port independently observed for the verified worker. [Microsoft WFP loopback diagnostics](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/windows-firewall/troubleshooting-uwp-firewall)

On November 28, 2026, the host control passed on `[::1]:55719`. The worker's connection attempt timed out and the AppContainer profile cleanup passed. The strict fixture exited with an inconclusive result. Local report: `target/ipv6-loopback-validation/b16ccabd065f4c85b358809cf3130bb8/result.json`. Formatting, Clippy, 184 Rust tests and all 50 default Windows sandbox cases passed with 56 profile cleanups.

The elevated IPv6 diagnostic then reported `LoopbackDropConfirmed` and `SenderCorrelated=true`. The IPv4 regression produced the same result with the default option. These are local TCP loopback results. IPv6 egress, UDP and DNS remain open. Local reports: `target/loopback-diagnostics/f11eaa9cc4ea4abf93c456b705bf612c/summary.json` (IPv6) and `target/loopback-diagnostics/90e2887248d245faaa702c0234170db4/summary.json` (IPv4).

## UDP loopback fixtures

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case network-loopback-udp`. The host sends and verifies one fixed control datagram on an ephemeral IPv4 loopback port. The restricted worker attempts one fixed datagram to that port. The strict fixture requires an explicit access denial from the worker's socket operation and checks that the host received no worker datagram. Receipt fails the test. A send that returns success with no observed receipt stays inconclusive because the send result does not prove delivery. [Winsock sendto](https://learn.microsoft.com/windows/win32/api/winsock/nf-winsock-sendto)

From an elevated PowerShell, run `pwsh -NoProfile -File tools/diagnose-loopback.ps1 -Udp`. The diagnostic accepts the inconclusive send only when the host receiver reports no worker datagram and a WFP receiving-side AppContainer Loopback block matches the exact host process, listener port, IPv4 addresses, UDP protocol and the source port independently observed for the verified worker. The worker holds its sender socket open for one second after a successful send so that the diagnostic can observe the port. No firewall rule or network exemption changes.

On November 28, 2026, the UDP diagnostic confirmed a receiving-side app isolation drop with `SenderCorrelated=true` and verified profile cleanup. The native check reported a successful send and no received datagram, so the explicit-denial check alone remained inconclusive. Both TCP diagnostics also passed after the UDP change. The UDP result covers the controlled IPv4 loopback datagram on this machine. UDP egress and IPv6 UDP remain open. Local report: `target/loopback-diagnostics/609c67fcb3d3400aa53490618bcbe187/summary.json`.

The IPv6 variant uses `--case network-loopback-udp-v6` and the same controls on `[::1]`. Run `pwsh -NoProfile -File tools/diagnose-loopback.ps1 -Udp -IPv6` from an elevated PowerShell. On November 28, the diagnostic matched the IPv6 UDP receiving-side AppContainer block to the verified worker's source port. Both profiles for the two separate IPv4 and IPv6 UDP invocations were cleaned up. Local reports: `target/loopback-diagnostics/b09893ad80da443d9e1537022a4e2933/summary.json` (IPv6) and `target/loopback-diagnostics/c34ab2a1a97e4047a5bde0eb57370428/summary.json` (IPv4). UDP egress remains open.

## IPv6 TCP egress fixture

Run `cargo run --release -p wallpaper-sandbox-probe --locked -- --case network-egress-v6`. The zero-capability AppContainer attempts one TCP connection to `[2001:db8::1]:9`, using a documentation-only address. The worker performs no DNS lookup and sends no application payload. Only an explicit access denial satisfies the test. A timeout, routing error or successful connection fails the case. [RFC 3849](https://www.rfc-editor.org/info/rfc3849/)

On November 28, 2026, Windows returned an explicit access denial and the probe verified profile cleanup. This covers the fixed IPv6 TCP egress attempt on the tested Windows configuration. Local report: `target/ipv6-egress-validation/3798b3780fd6434a85a55592a4929552/result.json`.

## UDP egress fixtures

Run the release probe with `--case network-egress-udp` for `192.0.2.1:9` or `--case network-egress-udp-v6` for `[2001:db8::1]:9`. Each fixed fixture binds an ephemeral UDP socket and attempts to send one empty datagram. The strict probe requires an explicit access denial from the send operation. A socket bind error, successful send or routing error does not pass. [Winsock sendto](https://learn.microsoft.com/windows/win32/api/winsock/nf-winsock-sendto) · [RFC 3849](https://www.rfc-editor.org/info/rfc3849/)

On November 28, 2026, both sends returned success and both AppContainer profiles were cleaned up. The strict probes remained inconclusive. A read-only WFP query found two IPv4 outbound classify drops from the UWP Default Outbound Block Rule. The destination, UDP protocol, blocking filter, staged worker image and PID from each ownership record matched. The corresponding IPv6 query returned no events. The IPv4 observation covers only the tested documentation endpoint on this Windows configuration. IPv6 UDP egress remains unverified. Local reports: `target/udp-egress-validation/624b8128e5624aeaac0078109155539d/`, including `ipv4-worker-drops.json`.

A follow-up checked the OS route and tried a connected IPv6 UDP socket as a diagnostic. Windows selected a preferred IPv6 source and an on-link default route. The connection setup and empty send returned success, profile cleanup passed, and the immediate destination-filtered WFP query returned zero events. A successful UDP call does not confirm delivery. The connected-socket variant added no acceptance evidence, so the repository retains the original fixture. Local report: `target/udp-egress-validation/40510146cd4748e1812593e43c5fde65/`. [Winsock connect](https://learn.microsoft.com/windows/win32/api/winsock2/nf-winsock2-connect)

A separate controlled test used an IPv6 link-local UDP listener in Ubuntu WSL on the host's virtual network. The listener echoed the host's control datagram and received no worker datagram during the four-second observation. WFP recorded an IPv6 UDP outbound classify drop at that listener port under the UWP Default Outbound Block Rule. The blocking filter, staged worker image and worker PID from the ownership record matched. The probe verified profile cleanup. This confirms IPv6 UDP blocking to a reachable non-loopback peer on this Windows configuration. The original documentation-address attempt remains inconclusive and no public IPv6 destination was tested. The test-only address was removed from the source and the standard probe rebuilt. Local proof: `target/ipv6-peer-validation/16a2fbeb6d5e4333a9b87f8bbf23c395/verified-drop.json`. [WSL networking](https://learn.microsoft.com/windows/wsl/networking)

## Primary sources reviewed

- [Microsoft: troubleshooting UWP connectivity with WFP](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/windows-firewall/troubleshooting-uwp-firewall), consulted November 14, 2026: loopback isolation and correlation of classify-drop events. Its exemption instructions are not applied here.
- [Project Zero: Understanding Network Access in Windows AppContainers](https://projectzero.google/2021/08/understanding-network-access-windows-app.html), consulted November 14, 2026: incoming drops can silently discard traffic while outgoing blocks can return an error. The local WFP records, rather than this general explanation alone, establish the observed cause.
- Local Windows `netsh wfp show netevents help`, consulted November 14, 2026: protocol/address/port/time filters. Read-only option queries and event enumeration require administrator elevation on this machine.
