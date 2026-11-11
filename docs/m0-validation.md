# M0 Validation Record

Date: November 11, 2026. Source: d8614a88b955a319790c8e4042a2f6afbd035637.

## Environment

Windows 11 Pro 10.0.26200, x64; Rust/Cargo 1.99.1; MSVC Build Tools (Visual Studio 18); Windows SDK 10.0.26100.0. GPUs reported by Windows: Intel Iris Xe (driver 32.0.101.7088) and NVIDIA GeForce RTX 3050 Laptop GPU (driver 32.0.16.1062).

Monitor inventory reports Generic PnP and Default Monitor entries at 1920x1080. Inventory entries do not establish two active physical displays. Active topology, scaling, refresh rate, renderer-selected adapter/backend and driver behavior must be recorded in G1. M0 performs no rendering certification.

## Clean source reproduction

Exported committed HEAD with git archive into an isolated ignored directory under target/, extracted it without existing build outputs, and ran the pinned toolchain. Formatting, Clippy with warnings denied, test-target compilation and release build passed. The exported CLI printed wallpaper-cli 0.1.0-dev successfully. There are zero behavioral tests at this stage. This proves clean source-tree reproduction on this machine, not a clean OS installation or independent dependency download.

The first attempt used the sandbox temporary directory, where Cargo output creation was denied. Retrying under the workspace's ignored target directory succeeded without changing source or toolchain.

## Hosted checks

[Main CI run](https://github.com/mertcetn/Sahne/actions/runs/42984652986) passed checkout, pinned toolchain activation, formatting, Clippy, test-target compilation, release build, and CLI version smoke check on windows-2025. The release job was correctly skipped for main.

The user explicitly authorized publishing this commit as a development prerelease. Release publication and downloaded artifact verification are being recorded below after the workflow completes. No stable wallpaper-product support is implied.

## Release validation

[Release workflow](https://github.com/mertcetn/Sahne/actions/runs/42984774527) passed both check and publication jobs. [Published development prerelease](https://github.com/mertcetn/Sahne/releases/tag/dev-d8614a88b955a319790c8e4042a2f6afbd035637-run-42984774527-1) contains the Windows x64 ZIP and SHA256SUMS.txt.

Downloaded the published assets, verified SHA256 300a071d7f3df6af0d234b6347ae053908897fe1277b1a3ee7e6b62f1970e76c, extracted docs/ and wallpaper-cli.exe, and successfully ran --version and --help. This checks the hosted artifact on the development machine, not a clean OS installation. Windows product installation/lifecycle certification remains M5 work.

GitHub reported that checkout v4.2.2 declares deprecated Node 20 and was forced onto Node 24 by the runner. Both checkouts succeeded. Track migration to an audited Node-24-native checkout revision as maintenance before the next workflow update; do not describe this notice as a failed check or ignore it as proof of future compatibility.

## M0 exit

M0 foundation acceptance is met: clean source reproduction, actual hosted checks, automatic release publication, artifact checksum/execution checks, initial environment inventory, draft contracts and reviewable security probes are recorded. Rendering and sandbox behavior remain deliberately unimplemented M1/M2 work. Future engine/renderer dependencies require fresh security reviews when adopted. User review and commit of this evidence update remain pending.
