# M2 Reproducible Validation Runner

Date: November 12, 2026. Status: local Windows runner after 4e23cd9. Hosted execution remains unverified; passing this fixed corpus does not close G2.

## Run from a checkout

Windows and PowerShell 7 are required. From the repository root:

```powershell
pwsh -NoProfile -File tools/test-sandbox-runner.ps1
pwsh -NoProfile -File tools/test-sandbox.ps1
```

The first command tests the host-process guard with four original helper processes: exit zero, exit seven, forced timeout and excessive output. It also tests bounded log reads and nine evidence-acceptance cases, including required registry cleanup. These helpers do not create AppContainer profiles. No Pester module or new package installation is required.

The second command builds wallpaper-sandbox-probe in release mode with --locked and runs the thirty-one expected-success native/JS/recovery scenarios sequentially. The default plan expects thirty-seven worker launches/profile cleanups and one registry cleanup. It stops at the first failed case and exits 1. Build, metadata, launch and evidence failures also produce failed reports where report storage remains available. The ordinary successful exit is zero.

For a selected diagnostic case:

```powershell
pwsh -NoProfile -File tools/test-sandbox.ps1 -SkipBuild -Cases cpu-busy
pwsh -NoProfile -File tools/test-sandbox.ps1 -SkipBuild -Cases network
```

SkipBuild reuses the existing executable and records BuildPerformed=false; it is a diagnostic convenience, not proof that the current checkout produced that executable. Selected runs have FullDefaultSuite=false. Duplicate or unknown case names are rejected before launch. Network is an explicit investigative selector and is excluded from the default expected-success plan. The current inconclusive loopback result fails acceptance and returns 1; the runner does not reinterpret timeout as access denial.

## Evidence and reports

Each invocation creates a unique target/sandbox-validation/<id>/ directory, with summary.json, summary.md and separate case stdout/stderr logs. Existing native staging artifacts remain under target/sandbox-runs/. Both locations are ignored by Git. The JSON records source commit, dirty-worktree status, Rust version, OS/architecture, runtime-reported processor count, Cargo.lock, executable and runner/guard source SHA256, build selection, limits, selected cases and each result.

A passing case requires all of these independently:

- Normal host-process completion and exact exit zero.
- Empty stderr, so a logged cleanup failure cannot silently pass.
- Exactly one final Probe passed marker.
- The expected number of launch confirmations and explicit profile cleanup confirmations for that selector, including every recovery attempt.
- Schema 2 requires exactly one registry cleanup confirmation for registry, and zero for other selectors; missing or unexpected cleanup markers fail.

The native executable remains responsible for token/ACL, wire, timing, CPU, filesystem and other case-specific assertions. Counting its transcript is a regression wrapper for trusted fixed fixtures, not independent attestation from untrusted worker output. Status=Passed means the selected corpus passed; G2Complete remains false until the wider acceptance gate is explicitly reviewed. Subsets do not claim full default coverage.

## Outer host-process guard

Each executable runs hidden, with separate redirected output files. The runner waits in 100 ms polls and applies a default twenty-second outer deadline, configurable with CaseTimeoutSeconds in the range 1..120. This does not change the native worker's existing three-second deadlines or one-second classification/stop/join acceptance. Native process creation/build/OS calls are not newly claimed hard real-time operations.

Combined stdout/stderr above 1 MiB fails the run, including when a fast process exits before the polling loop sees the limit. The polling bound can be exceeded on disk between observations; it is not a hard filesystem quota. Subsequent log reads copy at most 1 MiB per stream, even for oversized files, avoiding unbounded report-memory growth.

On outer timeout/output violation, the runner kills only the process object it started and waits up to five seconds for its exit. Closing the host's kill-on-close Job should terminate its restricted worker, but forced host death does not execute Rust Drop/profile cleanup. Such a run is failed, stops the batch and must not claim profile deletion; no sweep of existing profiles or manual recursive deletion workaround is added. The guard's timeout self-test uses a profile-free helper to avoid deliberately leaving profile storage behind.

## Manual GitHub workflow

The Sandbox validation (experimental) workflow is defined in .github/workflows/sandbox-validation.yml. After it is present on the repository's default branch, it can be started through Actions -> Run workflow. It uses windows-2025, read-only repository permissions and the same immutable checkout commit already used by the existing build workflow, with credential persistence disabled. It runs runner self-tests, Rust workspace tests and the default native corpus. Its job timeout is thirty minutes; it has no push/release trigger.

The runner writes its Markdown report to GITHUB_STEP_SUMMARY and prints JSON between M2_SUMMARY_JSON_BEGIN/END markers in the job log. Bounded per-case native diagnostics are also logged, with workflow-command processing disabled around captured output. Local JSON/log files are temporary on a hosted runner; there is no uploaded artifact or added artifact-upload action. Review the job summary and download workflow logs for retained hosted evidence.

No new action version, Rust dependency, worker API, permission or sandbox limit is introduced. The workflow reuses the existing checkout pin rather than changing the project's build/release dependency baseline. Its previously documented runtime-maintenance warning remains unresolved; hosted compatibility is not claimed here. The new workflow was not pushed or dispatched in this task. Existing main/release automation is unchanged.

## Initial local validation — November 12, 2026

The final default invocation produced `target/sandbox-validation/ad20d4e02d8945f18e578cbed29608be/`: Status=Passed, FullDefaultSuite=true, BuildPerformed=true, thirty case results, thirty-six launches/cleanups and G2Complete=false. JSON counts and current runner/guard source SHA256 were independently checked. Earlier default runs also passed; they are not combined to inflate this invocation's counts.

The final guard self-test passed four real profile-free helper processes, bounded-log reading and seven evidence checks, with artifacts under `target/sandbox-runner-tests/space path 0004d01aa1384cf398888ffbdf8d2e74/`. The real investigative network run `6c26e0ab34ef4b2fbffa238e2663de79` produced Failed/exit 1 for its inconclusive loopback result, with one verified profile cleanup. A local GitHub-format smoke test checked Markdown, JSON framing and disabled command processing for one healthy case; this did not contact or execute GitHub Actions.

PowerShell parser checks, local documentation links and diff checks passed. Workflow syntax/settings were reviewed locally; no hosted workflow or alternate platform was executed. Rust/native implementation and Cargo dependencies are unchanged in this continuation. No push or release was performed; the new runner/workflow/docs remain uncommitted. No visual judgment or physical input is required for the local checks.

## Registry continuation — November 14, 2026

The [registry matrix](m2-registry.md) expands the current default plan to thirty-one cases/thirty-seven launches. Report SchemaVersion is now 2 and includes RegistryCleanups/ExpectedRegistryCleanups in JSON and a corresponding Markdown column. The schema-2 default run `667432ed41e04eddb59d0f8e52b61fd8` passed with one verified registry cleanup and G2Complete=false. Historical schema-1 reports above retain their original thirty-case scope. No hosted workflow run or new release is inferred.

Primary documentation consulted November 12, 2026:

- [PowerShell process redirection, argument handling and hidden windows](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.management/start-process)
- [Bounded .NET process waits and handle disposal](https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.process.waitforexit)
- [GitHub workflow syntax, manual triggers, permissions and job timeouts](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax)
