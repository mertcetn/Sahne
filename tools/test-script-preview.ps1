#Requires -Version 7.0
param([switch] $SkipBuild)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'Script preview validation requires Windows' }
. (Join-Path $PSScriptRoot 'sandbox-process.ps1')

$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$run = Join-Path $repo "target/script-preview-validation/$([guid]::NewGuid().ToString('N'))"
$packages = Join-Path $run 'packages'
$preview = Join-Path $repo 'target/release/wallpaper-preview.exe'
$worker = Join-Path $repo 'target/release/wallpaper-sandbox-probe.exe'

if (-not $SkipBuild) {
    Push-Location $repo
    try {
        cargo build --release -p wallpaper-preview -p wallpaper-sandbox-probe --locked
        if ($LASTEXITCODE -ne 0) { throw 'Script preview release build failed' }
    }
    finally { Pop-Location }
}
foreach ($binary in @($preview, $worker)) {
    if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw "Missing binary $binary" }
}

$cases = @(
    @{ Name = 'changed'; Changed = 'true'; Script = '(document) => { document.getElementById("status").textContent = "Updated"; }' }
    @{ Name = 'unchanged'; Changed = 'false'; Script = '(document) => { document.getElementById("status").textContent = "Ready"; }' }
    @{ Name = 'promise-edit'; Changed = 'true'; Script = '(document) => Promise.resolve().then(() => { document.getElementById("status").textContent = "Updated"; })' }
    @{ Name = 'invalid-edit'; Script = '(document) => { document.getElementById("status").textContent = String.fromCharCode(0); }' }
    @{ Name = 'script-error'; Script = '() => { throw new Error("private-fixture-error"); }' }
    @{ Name = 'promise-budget'; Script = '(document) => { document.getElementById("status").textContent = "Updated"; for (let i = 0; i < 65; i++) Promise.resolve().then(() => {}); }' }
    @{ Name = 'missing-script' }
)
$manifest = '{"formatVersion":1,"apiVersion":1,"id":"probe.integration","name":"Script preview validation","entry":"index.html","script":"update.js","permissions":[],"preferredFps":30}'
Write-Host "Script preview validation reports in $run"
$results = foreach ($case in $cases) {
    $inputFolder = Join-Path $packages $case.Name
    New-Item -ItemType Directory -Path $inputFolder -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $inputFolder 'manifest.json'), $manifest)
    [IO.File]::WriteAllText((Join-Path $inputFolder 'index.html'), '<span id="status" style="width:400px;height:24px">Ready</span>')
    if ($case.ContainsKey('Script')) {
        [IO.File]::WriteAllText((Join-Path $inputFolder 'update.js'), $case.Script)
    }

    $result = Invoke-SahneGuardedProcess -FilePath $preview -ArgumentList @('--check-trusted-script-preview') `
        -PackageFolder $inputFolder -WorkingDirectory $repo -OutputDirectory (Join-Path $run "logs/$($case.Name)")
    $stdout = Read-SahneBoundedLog -Path $result.StdoutPath
    $stderr = Read-SahneBoundedLog -Path $result.StderrPath
    $cleanupCount = [regex]::Matches($stdout, '(?m)^Package worker profile cleanup verified\.\r?$').Count
    $accepted = '(?m)^Live package transaction verified; scene changed=(true|false); worker cleaned up\.\r?$'
    $passed = $result.Completion -eq 'Exited' -and "$stdout$stderr" -notmatch 'private-fixture-error'
    if ($case.ContainsKey('Changed')) {
        $passed = $passed -and $result.ExitCode -eq 0 -and $cleanupCount -eq 1 -and
            $stdout -match ($accepted.Replace('(true|false)', $case.Changed)) -and [string]::IsNullOrWhiteSpace($stderr)
    }
    else {
        $expectedError = 'Package scene batch rejected'
        $expectedCleanup = 1
        if (-not $case.ContainsKey('Script')) {
            $expectedError = 'MissingSource'
            $expectedCleanup = 0
        }
        $passed = $passed -and $result.ExitCode -ne 0 -and $cleanupCount -eq $expectedCleanup -and
            $stdout -notmatch $accepted -and $stderr.Contains($expectedError)
    }
    Write-Host "$($case.Name) passed=$passed ($($result.Completion), exit $($result.ExitCode))"
    [pscustomobject]@{
        Case = $case.Name
        Passed = $passed
        CleanupCount = $cleanupCount
        Process = $result
    }
}
$results | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $run 'summary.json')
if ($results.Passed -contains $false) { throw 'Script preview validation failed. See the per-case logs.' }
