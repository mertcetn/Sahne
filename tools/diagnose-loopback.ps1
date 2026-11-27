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
$watch = Start-ThreadJob -ArgumentList $PSScriptRoot, $binary, $repo, $output -ScriptBlock {
    param($tools, $probe, $workingDirectory, $reportDirectory)
    $ErrorActionPreference = 'Stop'
    . (Join-Path $tools 'sandbox-process.ps1')
    Invoke-SahneGuardedProcess -FilePath $probe -ArgumentList @('--case', 'network') `
        -WorkingDirectory $workingDirectory -OutputDirectory $reportDirectory
}
$senders = @{}
try {
    $stdoutPath = Join-Path $output 'stdout.log'
    $netstat = Join-Path $env:SystemRoot 'System32/netstat.exe'
    while ($watch.State -in @('NotStarted', 'Running')) {
        if ($senders.Count -lt 8 -and (Test-Path -LiteralPath $stdoutPath)) {
            $partial = Read-SahneBoundedLog -Path $stdoutPath
            $portLine = [regex]::Match($partial, '(?m)^Loopback host control passed: 127\.0\.0\.1:(\d+)\r?$')
            $pidLine = [regex]::Match($partial, '(?m)^Loopback host process: (\d+)\r?$')
            $imageLine = [regex]::Match($partial, '(?m)^Loopback worker image: ([^\r\n]+)\r?$')
            if ($portLine.Success -and $pidLine.Success -and $imageLine.Success) {
                $listenPort = [int]$portLine.Groups[1].Value
                $probeHostId = [int]$pidLine.Groups[1].Value
                $workerImage = [IO.Path]::GetFullPath($imageLine.Groups[1].Value)
                foreach ($line in (& $netstat -ano -p tcp)) {
                    if ($line -notmatch "^\s*TCP\s+127\.0\.0\.1:(?<sender>\d+)\s+127\.0\.0\.1:$listenPort\s+SYN_SENT\s+(?<owner>\d+)\s*$") { continue }
                    $sourcePort = [int]$Matches.sender
                    $owner = [int]$Matches.owner
                    $key = "$owner/$sourcePort"
                    if ($senders.ContainsKey($key) -or $senders.Count -ge 8) { continue }
                    try {
                        $worker = Get-CimInstance Win32_Process -Filter "ProcessId = $owner" -OperationTimeoutSec 1
                        if ($null -ne $worker -and $worker.ParentProcessId -eq $probeHostId -and $worker.ExecutablePath -ieq $workerImage) {
                            $senders[$key] = [pscustomobject]@{ WorkerPid = $owner; SenderPort = $sourcePort; ListenerPort = $listenPort }
                        }
                    }
                    catch { continue }
                }
            }
        }
        Start-Sleep -Milliseconds 40
    }
    $run = Receive-Job -Job $watch -Wait
}
finally { Remove-Job -Job $watch -Force }
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
$senderDrops = @()
$uniqueSender = if ($senders.Count -eq 1) { @($senders.Values)[0] } else { $null }
$nativeStatus = Get-SahneLoopbackAcceptance -Completion $run.Completion -ExitCode $run.ExitCode -Stdout $stdout -Stderr $stderr
$directions = if ($nativeStatus -ne 'ExplicitDenialConfirmed') { @('localport', 'remoteport') }
foreach ($direction in $directions) {
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
        if ($null -ne $uniqueSender -and $uniqueSender.ListenerPort -eq $port) {
            $senderDrops += @(Find-SahneLoopbackDrop -Events $events -ListenerPort $port `
                -HostProcessId ([int]$hostIds[0].Groups[1].Value) -HostImage $binary `
                -StartedUtc $started -FinishedUtc $finished -SenderPort $uniqueSender.SenderPort)
        }
    }
    finally { $reader.Dispose() }
}
$senderCorrelated = $senderDrops.Count -gt 0
$status = Get-SahneLoopbackAcceptance -Completion $run.Completion -ExitCode $run.ExitCode `
    -Stdout $stdout -Stderr $stderr -SenderCorrelated $senderCorrelated
[ordered]@{
    Status = $status
    StartedUtc = $started.ToString('O')
    FinishedUtc = [DateTime]::UtcNow.ToString('O')
    BinarySha256 = $binaryHash
    ListenerPort = $port
    ProbeExitCode = $run.ExitCode
    Completion = $run.Completion
    LoopbackAccepted = $status -ne 'Inconclusive'
    G2Complete = $false
    MatchingDrops = $matches
    SenderObservations = @($senders.Values)
    SenderCorrelated = $senderCorrelated
    SenderMatchingDrops = $senderDrops
    Note = 'Controlled IPv4 TCP loopback accepts explicit native denial or a native timeout with a WFP block correlated to the verified worker source port. Overall G2 is assessed separately.'
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'summary.json')
Write-Output "Loopback diagnostics retained at $output"
if ($status -eq 'Inconclusive') { throw 'Loopback acceptance requires explicit denial or a WFP block correlated to the test worker.' }
Write-Output "$status. Controlled IPv4 TCP loopback accepted."
