#Requires -Version 7.0
[CmdletBinding()]
param(
    [string[]] $Cases,
    [switch] $SkipBuild,
    [ValidateRange(1, 120)][int] $CaseTimeoutSeconds = 20
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'The sandbox validation runner requires Windows.' }
. (Join-Path $PSScriptRoot 'sandbox-process.ps1')
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$plan = [ordered]@{
    healthy=1; 'package-basic'=1; 'package-invalid'=1; 'compiler-basic'=1; 'compiler-invalid'=1
    'image-basic'=1; 'image-invalid'=1
    'hierarchy-basic'=1; 'hierarchy-invalid'=1
    'groups-basic'=1; 'groups-invalid'=1
    'network-egress'=1; memory=1; 'memory-incremental'=1; process=1; handles=1; endpoint=1
    'ipc-rights'=1; 'ipc-roundtrip'=1; 'ipc-roundtrip-replay'=1; 'ipc-roundtrip-stall'=1; 'scene-transaction'=1; 'scene-script'=1; 'scene-timer'=1
    filesystem=1; registry=1; 'ipc-write-drain'=1; 'ipc-write-stall'=1; 'cpu-busy'=1
    hang=1; oversized=1; crash=1; 'ipc-flood'=1; 'ipc-stall'=1; 'ipc-truncated'=1
    'ipc-replay'=1; 'ipc-open'=1; 'js-basic'=1; 'js-interrupt'=1; 'js-memory'=1
    'js-stack'=1; 'js-jobs'=1; 'restart-recover'=2; 'restart-exhaust'=3
    'restart-timeout'=2; 'restart-handshake'=2; 'restart-stale'=2; 'restart-protocol'=1
}
$fullSuite = -not $Cases -or $Cases.Count -eq 0
if ($fullSuite) { $Cases = @($plan.Keys) }
if (@($Cases | Select-Object -Unique).Count -ne $Cases.Count) { throw 'Duplicate cases are not allowed.' }
foreach ($case in $Cases) {
    if (-not $plan.Contains($case) -and $case -ne 'network') { throw "Unknown case: $case" }
}
$expectedLaunches = 0
foreach ($case in $Cases) { $expectedLaunches += $(if ($case -eq 'network') { 1 } else { $plan[$case] }) }
$runRoot = Join-Path $repoRoot ('target/sandbox-validation/' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $runRoot | Out-Null
Write-Host "Sandbox validation reports: $runRoot"
$rows = [Collections.Generic.List[object]]::new()
$report = [ordered]@{
    SchemaVersion = 2
    StartedUtc = [DateTime]::UtcNow.ToString('o')
    FinishedUtc = $null
    Commit = $null
    DirtyWorktree = $null
    OS = [Runtime.InteropServices.RuntimeInformation]::OSDescription
    OSArchitecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    PowerShell = $PSVersionTable.PSVersion.ToString()
    RuntimeReportedProcessors = [Environment]::ProcessorCount
    Rustc = $null
    BinarySha256 = $null
    CargoLockSha256 = $null
    RunnerSha256 = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
    ProcessGuardSha256 = (Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'sandbox-process.ps1') -Algorithm SHA256).Hash.ToLowerInvariant()
    BuildPerformed = (-not $SkipBuild)
    CaseTimeoutSeconds = $CaseTimeoutSeconds
    OutputLimitBytes = 1048576
    FullDefaultSuite = $fullSuite
    SelectedCases = $Cases
    ExpectedLaunches = $expectedLaunches
    Status = 'Failed'
    G2Complete = $false
    Failure = $null
    Results = $rows
}
Push-Location $repoRoot
try {
    $report.Commit = (git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not identify source revision' }
    $report.DirtyWorktree = -not [string]::IsNullOrWhiteSpace((git status --porcelain | Out-String))
    $report.Rustc = (rustc --version | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not identify Rust toolchain' }
    $report.CargoLockSha256 = (Get-FileHash -LiteralPath 'Cargo.lock' -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $SkipBuild) {
        cargo build --release -p wallpaper-sandbox-probe --locked
        if ($LASTEXITCODE -ne 0) { throw 'Sandbox probe build failed' }
    }
    $binary = Join-Path $repoRoot 'target/release/wallpaper-sandbox-probe.exe'
    $report.BinarySha256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
    foreach ($case in $Cases) {
        $expected = if ($case -eq 'network') { 1 } else { $plan[$case] }
        $expectedRegistry = if ($case -eq 'registry') { 1 } else { 0 }
        $processResult = Invoke-SahneGuardedProcess -FilePath $binary -ArgumentList @('--case',$case) `
            -WorkingDirectory $repoRoot -OutputDirectory (Join-Path $runRoot $case) `
            -TimeoutSeconds $CaseTimeoutSeconds -OutputLimitBytes $report.OutputLimitBytes
        $stdout = Read-SahneBoundedLog -Path $processResult.StdoutPath
        $stderr = Read-SahneBoundedLog -Path $processResult.StderrPath
        if ($env:GITHUB_STEP_SUMMARY) {
            # Treat captured diagnostics as data, never GitHub workflow commands.
            $commandToken = [guid]::NewGuid().ToString('N')
            Write-Host "::stop-commands::$commandToken"
            Write-Host "CASE $case STDOUT"
            Write-Host $stdout
            Write-Host "CASE $case STDERR"
            Write-Host $stderr
            Write-Host "::$($commandToken)::"
        }
        $evidence = Test-SahneCaseEvidence -Stdout $stdout -Stderr $stderr `
            -ExitCode $processResult.ExitCode -Completion $processResult.Completion -ExpectedLaunches $expected `
            -ExpectedRegistryCleanups $expectedRegistry
        $rows.Add([pscustomobject]@{
            Case = $case; Passed = $evidence.Passed; Completion = $processResult.Completion
            ExitCode = $processResult.ExitCode; ElapsedMs = $processResult.ElapsedMs
            ExpectedLaunches = $expected; Launches = $evidence.Launches
            ProfileCleanups = $evidence.ProfileCleanups; PassMarkers = $evidence.PassMarkers
            RegistryCleanups = $evidence.RegistryCleanups; ExpectedRegistryCleanups = $expectedRegistry
            Stdout = "$case/stdout.log"; Stderr = "$case/stderr.log"
        })
        Write-Host "$case : passed=$($evidence.Passed), exit=$($processResult.ExitCode), cleanup=$($evidence.ProfileCleanups)/$expected"
        if (-not $evidence.Passed) { throw "Case failed acceptance: $case ($($processResult.Completion))" }
    }
    $report.Status = 'Passed'
}
catch { $report.Failure = $_.Exception.Message; Write-Host "Validation failed: $($report.Failure)" }
finally {
    try {
        $report.FinishedUtc = [DateTime]::UtcNow.ToString('o')
        $json = $report | ConvertTo-Json -Depth 8
        $json | Set-Content -LiteralPath (Join-Path $runRoot 'summary.json') -Encoding utf8
        $markdown = @(
            '# M2 sandbox validation'
            ''
            "Status: $($report.Status). Full default suite: $fullSuite. G2 complete: false."
            "Revision: $($report.Commit). Dirty worktree: $($report.DirtyWorktree)."
            "Binary SHA256: $($report.BinarySha256)"
            ''
            '| Case | Passed | Exit | Completion | Launches | Profile cleanups | Registry cleanups | ms |'
            '| --- | --- | --- | --- | --- | --- | --- | --- |'
        )
        foreach ($row in $rows) {
            $markdown += "| $($row.Case) | $($row.Passed) | $($row.ExitCode) | $($row.Completion) | $($row.Launches)/$($row.ExpectedLaunches) | $($row.ProfileCleanups)/$($row.ExpectedLaunches) | $($row.RegistryCleanups)/$($row.ExpectedRegistryCleanups) | $($row.ElapsedMs) |"
        }
        $markdown += @('', 'Passing this fixed corpus does not close G2. Loopback denial and broader isolation/integration remain open.')
        if ($report.Failure) { $markdown += "Failure: $($report.Failure)" }
        $markdown | Set-Content -LiteralPath (Join-Path $runRoot 'summary.md') -Encoding utf8
        if ($env:GITHUB_STEP_SUMMARY) {
            $markdown | Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Encoding utf8
            Write-Host 'M2_SUMMARY_JSON_BEGIN'
            Write-Host $json
            Write-Host 'M2_SUMMARY_JSON_END'
        }
    }
    finally { Pop-Location }
}
if ($report.Status -ne 'Passed') { exit 1 }
