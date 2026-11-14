# Read-only WFP diagnostics for the fixed loopback fixture; never grants network access.
#Requires -Version 7.0
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw 'This diagnostic requires Windows.' }
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'WFP diagnostics require an elevated PowerShell. This script does not elevate itself or change firewall settings.'
}
$repo = Split-Path -Parent $PSScriptRoot
$binary = Join-Path $repo 'target/release/wallpaper-sandbox-probe.exe'
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    throw 'Build the current probe first: cargo build --release -p wallpaper-sandbox-probe --locked'
}
. (Join-Path $PSScriptRoot 'sandbox-process.ps1')
. (Join-Path $PSScriptRoot 'loopback-evidence.ps1')
$output = Join-Path $repo ("target/loopback-diagnostics/" + [guid]::NewGuid().ToString('N'))
$started = [DateTime]::UtcNow
$binaryHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash
$run = Invoke-SahneGuardedProcess -FilePath $binary -ArgumentList @('--case', 'network') `
    -WorkingDirectory $repo -OutputDirectory $output
$stdout = Read-SahneBoundedLog -Path $run.StdoutPath
$stderr = Read-SahneBoundedLog -Path $run.StderrPath
$finished = [DateTimeOffset]::UtcNow
$ports = [regex]::Matches($stdout, '(?m)^Loopback host control passed: 127\.0\.0\.1:(\d+)\r?$')
$hostIds = [regex]::Matches($stdout, '(?m)^Loopback host process: (\d+)\r?$')
if ($run.Completion -ne 'Exited' -or $ports.Count -ne 1 -or $hostIds.Count -ne 1) {
    throw "Fixture did not complete with exactly one host positive control. Inspect $output"
}
$port = [int]$ports[0].Groups[1].Value
if ($port -lt 1 -or $port -gt 65535) { throw 'Invalid fixture listener port.' }
# Do not start a system-wide capture or enable event buffering. Existing buffered
# events may be unavailable; missing events never prove either denial or access.
$netsh = Join-Path $env:SystemRoot 'System32/netsh.exe'
$matches = @()
foreach ($direction in @('localport', 'remoteport')) {
    $eventPath = Join-Path $output "$direction.xml"
    $queryOutput = & $netsh wfp show netevents "file=$eventPath" protocol=6 `
        localaddr=127.0.0.1 remoteaddr=127.0.0.1 "$direction=$port" timewindow=30 2>&1
    $queryExit = $LASTEXITCODE
    $queryOutput | Set-Content -LiteralPath (Join-Path $output "$direction-query.log")
    if ($queryExit -ne 0) { throw "WFP query failed ($queryExit). Inspect $output" }
    if ((Get-Item -LiteralPath $eventPath).Length -gt 1048576) { throw 'WFP evidence exceeds 1 MiB limit.' }
    $settings = [Xml.XmlReaderSettings]::new()
    $settings.DtdProcessing = [Xml.DtdProcessing]::Prohibit
    $settings.XmlResolver = $null
    $settings.MaxCharactersInDocument = 1048576
    $reader = [Xml.XmlReader]::Create($eventPath, $settings)
    try {
        $events = [Xml.XmlDocument]::new()
        $events.XmlResolver = $null
        $events.Load($reader)
        $matches += @(Find-SahneLoopbackDrop -Events $events -ListenerPort $port `
            -HostProcessId ([int]$hostIds[0].Groups[1].Value) -HostImage $binary `
            -StartedUtc $started -FinishedUtc $finished)
    }
    finally { $reader.Dispose() }
}
$status = 'Inconclusive'
# The native case deliberately remains strict. Only supplement its specific
# timeout rejection with independently matched AppContainer blocking evidence.
if ($run.ExitCode -eq 1 -and $stderr.Contains('Network test inconclusive or allowed: Err((TimedOut, None))') -and
    -not $stderr.Contains('Attempt cleanup failed:') -and
    ([regex]::Matches($stdout, '(?m)^Profile cleanup verified:')).Count -eq 1 -and
    $stdout.Contains('Verified AppContainer identity and zero capabilities;') -and $matches.Count -gt 0) {
    $status = 'LoopbackDropConfirmed'
}
[ordered]@{
    Status = $status
    StartedUtc = $started.ToString('O')
    FinishedUtc = [DateTime]::UtcNow.ToString('O')
    BinarySha256 = $binaryHash
    ListenerPort = $port
    ProbeExitCode = $run.ExitCode
    Completion = $run.Completion
    G2Complete = $false
    MatchingDrops = $matches
    Note = 'Receiving-host AppContainer Loopback block, matched by run interval, host PID, image path suffix and listener port. This is supplemental local fixture evidence; the native timeout alone still fails.'
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'summary.json')
Write-Output "Loopback diagnostics retained at $output"
if ($status -ne 'LoopbackDropConfirmed') { throw 'No matching AppContainer loopback block proved the timeout; evidence is inconclusive.' }
Write-Output 'LoopbackDropConfirmed: matching receiving-side AppContainer block; no network permission changed.'
