# M2 loopback diagnosis

Date: November 14, 2026. Status: receiving-side AppContainer block observed on the local Windows 11 build 26200 machine. G2 remains incomplete.

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

`LoopbackDropConfirmed` requires the native fixture's specific timeout rejection, completed host process, verified AppContainer/zero-capability marker, profile cleanup, and independently matching WFP evidence. The matcher requires:

- A classify-drop event during this run, IPv4 TCP, both addresses exactly 127.0.0.1 and the exact listener port.
- Incoming loopback direction, the exact host PID, and the full receiving image path below its drive letter. Netsh supplies an NT device path, so the drive/device mapping itself is not independently verified.
- `AppContainer Loopback` filter origin and the event's exact nonzero filter ID in an App Isolation sublayer entry with `FWP_ACTION_BLOCK`.

Missing, malformed, stale or unrelated records do not pass. The native `--case network` still exits nonzero for a timeout; the ordinary runner still excludes this investigative case and does not reinterpret it as a passing native result. The elevated diagnostic supplies separate OS evidence. It currently expects this machine's timeout behavior; an immediate native access denial should be reviewed through the ordinary runner instead.

This is receiving-side evidence for the controlled local fixture. It does not independently identify the sender's ephemeral socket or match a worker package SID in that receiving record. Another process deliberately targeting the same short-lived port during this interval has not been modeled. It does not certify every address family, protocol, broker, Windows build or firewall configuration. The elevated host run is recorded separately from ordinary non-elevated corpus runs. Production validation and G2 remain open.

## Local evidence

The completed diagnostic report `ffa8b285f54446d4ba4da169a215d615` recorded `LoopbackDropConfirmed`, listener port 61108, native exit 1 and two matching drops at 17:21:49.563Z and 17:21:50.573Z on November 14. Filter ID 73064 was the receiving App Isolation block; these numeric IDs and ports are run-specific. Profile cleanup succeeded. The built binary SHA256 was `7da61fa6def750e919ae8ba6a02f9b7aed85fd4907e7a0560eb44165f06fbb95`.

Pure matcher checks passed one valid event and twenty rejection cases, including wrong time, PID, image, port, direction, protocol, filter association, origin and block action, plus missing fields/events. The manual sandbox CI workflow runs these privilege-free matcher checks; elevated diagnostics have only been exercised locally.

The default native corpus also passed all thirty-one cases, thirty-seven profile cleanups and one registry cleanup in report `a3376f1e5bb741f79764ef4f44abcccc`, with `G2Complete=false`. All twenty workspace Rust tests, Clippy with warnings denied, formatting, PowerShell syntax, runner self-checks and local documentation links passed. The existing incremental-cache hardlink/copy notices remain. No hosted workflow or other Windows build was tested.

The elevated diagnostic was repeated on November 27, 2026 with the current local release probe. It returned `LoopbackDropConfirmed` and matched two receiving-side AppContainer Loopback drops for the fixture listener on port 65217. Both events identified filter 73076. The native probe returned its strict timeout failure and profile cleanup passed. The diagnostic read existing filtered records without changing policy. Local report: `target/loopback-diagnostics/67515ba0713d491da1486529bfa817df/summary.json`. This rerun leaves the native acceptance rule and G2 decision in place.

## Primary sources reviewed

- [Microsoft: troubleshooting UWP connectivity with WFP](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/windows-firewall/troubleshooting-uwp-firewall), consulted November 14, 2026: loopback isolation and correlation of classify-drop events. Its exemption instructions are not applied here.
- [Project Zero: Understanding Network Access in Windows AppContainers](https://projectzero.google/2021/08/understanding-network-access-windows-app.html), consulted November 14, 2026: incoming drops can silently discard traffic while outgoing blocks can return an error. The local WFP records, rather than this general explanation alone, establish the observed cause.
- Local Windows `netsh wfp show netevents help`, consulted November 14, 2026: protocol/address/port/time filters. Read-only option queries and event enumeration require administrator elevation on this machine.
