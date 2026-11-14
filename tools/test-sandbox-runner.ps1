#Requires -Version 7.0
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'Runner tests require Windows.' }
. (Join-Path $PSScriptRoot 'sandbox-process.ps1')
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$testRoot = Join-Path $repoRoot ('target/sandbox-runner-tests/space path ' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
$powerShell = Join-Path $PSHOME 'pwsh.exe'
$fixtures = @(
    @{ Name='exit-zero'; Code="[Console]::WriteLine('fixture'); exit 0"; Completion='Exited'; Exit=0; Timeout=10; Limit=1048576 }
    @{ Name='exit-seven'; Code="[Console]::Error.WriteLine('expected failure'); exit 7"; Completion='Exited'; Exit=7; Timeout=10; Limit=1048576 }
    @{ Name='timeout'; Code='Start-Sleep -Seconds 30'; Completion='TimedOut'; Exit=$null; Timeout=1; Limit=1048576 }
    @{ Name='output-limit'; Code="[Console]::Write(('x' * 4096)); exit 0"; Completion='OutputLimit'; Exit=$null; Timeout=10; Limit=1024 }
)
foreach ($fixture in $fixtures) {
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($fixture.Code))
    $result = Invoke-SahneGuardedProcess -FilePath $powerShell `
        -ArgumentList @('-NoProfile','-NonInteractive','-EncodedCommand',$encoded) `
        -WorkingDirectory $repoRoot -OutputDirectory (Join-Path $testRoot $fixture.Name) `
        -TimeoutSeconds $fixture.Timeout -OutputLimitBytes $fixture.Limit
    if ($result.Completion -ne $fixture.Completion -or
        ($null -ne $fixture.Exit -and $result.ExitCode -ne $fixture.Exit)) {
        throw "Process guard failed: $($fixture.Name): $($result | ConvertTo-Json -Compress)"
    }
    Write-Host "Process guard passed: $($fixture.Name)"
}
$bounded = Read-SahneBoundedLog -Path (Join-Path $testRoot 'output-limit/stdout.log') -MaxBytes 100
if ($bounded.Length -ne 100) { throw 'Log reader did not honor its memory bound' }
$good = "Verified AppContainer identity`nProfile cleanup verified: fixture`nProbe passed; fixture`n"
$valid = Test-SahneCaseEvidence -Stdout $good -Stderr '' -ExitCode 0 -Completion Exited -ExpectedLaunches 1
if (-not $valid.Passed) { throw 'Complete evidence was rejected' }
$bad = @(
    @{ Out=$good; Err=''; Exit=7; Completion='Exited' }
    @{ Out=$good; Err=''; Exit=0; Completion='TimedOut' }
    @{ Out=$good; Err='cleanup failed'; Exit=0; Completion='Exited' }
    @{ Out=$good.Replace("Profile cleanup verified: fixture`n",''); Err=''; Exit=0; Completion='Exited' }
    @{ Out=($good + "Probe passed; duplicate`n"); Err=''; Exit=0; Completion='Exited' }
    @{ Out=($good + "Verified AppContainer identity`n"); Err=''; Exit=0; Completion='Exited' }
)
foreach ($fixture in $bad) {
    $check = Test-SahneCaseEvidence -Stdout $fixture.Out -Stderr $fixture.Err `
        -ExitCode $fixture.Exit -Completion $fixture.Completion -ExpectedLaunches 1
    if ($check.Passed) { throw 'Invalid or incomplete evidence was accepted' }
}
$registryGood = $good + "Registry fixture cleanup verified: fixture`n"
$registryPass = Test-SahneCaseEvidence -Stdout $registryGood -Stderr '' -ExitCode 0 `
    -Completion Exited -ExpectedLaunches 1 -ExpectedRegistryCleanups 1
$registryMissing = Test-SahneCaseEvidence -Stdout $good -Stderr '' -ExitCode 0 `
    -Completion Exited -ExpectedLaunches 1 -ExpectedRegistryCleanups 1
if (-not $registryPass.Passed -or $registryMissing.Passed) { throw 'Registry cleanup evidence check failed' }
Write-Host "Runner checks passed: four real helper processes, bounded log reading and nine evidence checks. Artifacts: $testRoot"
