# Shared host-process guard for the Windows validation runner, not a worker API.
function Invoke-SahneGuardedProcess {
    param(
        [Parameter(Mandatory)][string] $FilePath,
        [Parameter(Mandatory)][string[]] $ArgumentList,
        [Parameter(Mandatory)][string] $WorkingDirectory,
        [Parameter(Mandatory)][string] $OutputDirectory,
        [ValidateRange(1, 120)][int] $TimeoutSeconds = 20,
        [ValidateRange(1024, 1048576)][int] $OutputLimitBytes = 1048576
    )
    # Start-Process joins ArgumentList into a command line. This caller uses only fixed
    # case names or base64 self-test payloads; reject tokens requiring shell quoting.
    foreach ($argument in $ArgumentList) {
        if ($argument -notmatch '^[A-Za-z0-9+/=._-]+$') { throw 'Unsupported process argument token' }
    }
    New-Item -ItemType Directory -Path $OutputDirectory -ErrorAction Stop | Out-Null
    $stdout = Join-Path $OutputDirectory 'stdout.log'
    $stderr = Join-Path $OutputDirectory 'stderr.log'
    $process = $null
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $completion = 'Exited'
    try {
        $process = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList `
            -WorkingDirectory $WorkingDirectory -WindowStyle Hidden -PassThru `
            -RedirectStandardOutput $stdout -RedirectStandardError $stderr -ErrorAction Stop
        while (-not $process.WaitForExit(100)) {
            $size = (Get-Item -LiteralPath $stdout).Length + (Get-Item -LiteralPath $stderr).Length
            if ($size -gt $OutputLimitBytes) { $completion = 'OutputLimit'; break }
            if ($watch.Elapsed.TotalSeconds -ge $TimeoutSeconds) { $completion = 'TimedOut'; break }
        }
        if ($completion -eq 'Exited' -and $watch.Elapsed.TotalSeconds -ge $TimeoutSeconds) {
            $completion = 'TimedOut'
        }
        if (-not $process.HasExited) {
            $process.Kill()
            if (-not $process.WaitForExit(5000)) { throw 'Host process did not exit after forced stop' }
        }
        $process.Refresh()
        $exitCode = $process.ExitCode
        if ($null -eq $exitCode) { throw 'Host exit code unavailable' }
        $size = (Get-Item -LiteralPath $stdout).Length + (Get-Item -LiteralPath $stderr).Length
        if ($size -gt $OutputLimitBytes) { $completion = 'OutputLimit' }
        [pscustomobject]@{
            Completion = $completion
            ExitCode = $exitCode
            ElapsedMs = [math]::Round($watch.Elapsed.TotalMilliseconds, 1)
            StdoutPath = $stdout
            StderrPath = $stderr
        }
    }
    finally {
        if ($null -ne $process) {
            try {
                if (-not $process.HasExited) {
                    $process.Kill()
                    if (-not $process.WaitForExit(5000)) { throw 'Host cleanup failed' }
                }
            }
            finally { $process.Dispose() }
        }
    }
}

function Read-SahneBoundedLog {
    param([string] $Path, [ValidateRange(1, 1048576)][int] $MaxBytes = 1048576)
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    try {
        $buffer = [byte[]]::new([int][math]::Min($stream.Length, $MaxBytes))
        $used = 0
        while ($used -lt $buffer.Length) {
            $count = $stream.Read($buffer, $used, $buffer.Length - $used)
            if ($count -eq 0) { break }
            $used += $count
        }
        [Text.Encoding]::UTF8.GetString($buffer, 0, $used)
    }
    finally { $stream.Dispose() }
}

function Test-SahneCaseEvidence {
    param([string] $Stdout, [string] $Stderr, [int] $ExitCode,
        [string] $Completion, [int] $ExpectedLaunches)
    $launches = [regex]::Matches($Stdout, '(?m)^Verified AppContainer identity').Count
    $cleanups = [regex]::Matches($Stdout, '(?m)^Profile cleanup verified:').Count
    $passes = [regex]::Matches($Stdout, '(?m)^Probe passed[;:]').Count
    [pscustomobject]@{
        Passed = ($Completion -eq 'Exited' -and $ExitCode -eq 0 -and
            [string]::IsNullOrWhiteSpace($Stderr) -and $passes -eq 1 -and
            $launches -eq $ExpectedLaunches -and $cleanups -eq $ExpectedLaunches)
        Launches = $launches
        ProfileCleanups = $cleanups
        PassMarkers = $passes
    }
}
