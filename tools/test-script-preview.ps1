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
    @{ Name = 'promise-at-limit'; Changed = 'true'; Script = '(document) => { document.getElementById("status").textContent = "Updated"; for (let i = 0; i < 64; i++) Promise.resolve().then(() => {}); }' }
    @{ Name = 'promise-rejection'; Script = '(document) => { document.getElementById("status").textContent = "Updated"; return Promise.reject(new Error("private-fixture-error")); }' }
    @{ Name = 'dom-over-limit-caught'; Script = '(document) => { document.getElementById("status").textContent = "Updated"; try { for (let i = 0; i < 255; i++) document.getElementById("status"); } catch (error) { if (!(error instanceof RangeError)) throw error; } }' }
    @{ Name = 'dom-at-limit'; Changed = 'true'; Script = '(document) => { document.getElementById("status").textContent = "Updated"; for (let i = 0; i < 254; i++) document.getElementById("status"); }' }
    @{ Name = 'scene-edits-at-limit'; Changed = 'true'; Targets = 32; Script = '(document) => { for (let i = 0; i < 32; i++) { const node = document.getElementById("status" + i); const style = node.style; node.textContent = "Staged"; node.textContent = "Updated"; style.color = [1, 2, 3, 255]; style.color = [4, 5, 6, 255]; } }' }
    @{ Name = 'initial-scene-edits-over-limit'; Targets = 33; BeforeLaunch = 'edit count outside bounds'; Script = '() => {}' }
    @{ Name = 'missing-script' }
    @{ Name = 'lifecycle-promise'; Lifecycle = $true; Changed = 'true'; Script = '(() => { let phase = 0; return { pause() { return Promise.resolve().then(() => { phase = 1; }); }, resume() { if (phase !== 1) throw new Error("private-fixture-error"); phase = 2; }, tick(document) { if (phase !== 2) throw new Error("private-fixture-error"); document.getElementById("status").textContent = "Resumed"; } }; })()' }
    @{ Name = 'lifecycle-resume-rejected'; Lifecycle = $true; Script = '({ pause() {}, resume() { return Promise.reject(new Error("private-fixture-error")); }, tick(document) { document.getElementById("status").textContent = "Unexpected"; } })' }
    @{ Name = 'lifecycle-scheduled'; Lifecycle = $true; Ticks = 1; Changed = 'true'; Script = '(() => { let resumed = false; return { intervalMs: 32, pause() {}, resume() { resumed = true; }, tick(document) { if (!resumed) throw new Error("private-fixture-error"); document.getElementById("status").textContent = "Timer ran"; } }; })()' }
    @{ Name = 'lifecycle-repeated'; Lifecycle = $true; Ticks = 3; Changed = 'true'; Script = '(() => { let calls = 0; return { intervalMs: 16, tickCount: 3, pause() {}, resume() {}, tick(document) { const element = document.getElementById("status"); if (calls > 0 && element.textContent !== "Tick " + calls) throw new Error("private-fixture-error"); element.textContent = "Tick " + (++calls); } }; })()' }
    @{ Name = 'lifecycle-repeated-rejected'; Lifecycle = $true; Script = '(() => { let calls = 0; return { intervalMs: 16, tickCount: 3, pause() {}, resume() {}, tick(document) { document.getElementById("status").textContent = "Tick " + (++calls); if (calls === 2) throw new Error("private-fixture-error"); } }; })()'; RejectTick = $true }
    @{ Name = 'lifecycle-stable-callbacks'; Lifecycle = $true; Ticks = 2; Changed = 'true'; Script = '(() => { let reads = 0, calls = 0; const callbacks = { intervalMs: 32, tickCount: 2, pause() { callbacks.resume = () => { throw new Error("private-fixture-error"); }; }, resume() {}, get tick() { if (++reads !== 1) throw new Error("private-fixture-error"); return (document) => { document.getElementById("status").textContent = "Tick " + (++calls); }; } }; return callbacks; })()' }
    @{ Name = 'lifecycle-stable-dom-helpers'; Lifecycle = $true; Ticks = 2; Changed = 'true'; Script = '(() => { let calls = 0, helperCalls = 0; const original = Number.isInteger; Number.isInteger = (value) => { helperCalls++; return original(value); }; return { intervalMs: 32, tickCount: 2, pause() {}, resume() {}, tick(document) { const node = document.getElementById("status"); node.style.color = [1, 2, 3, 255]; if (helperCalls !== 0) throw new Error("private-fixture-error"); node.textContent = "Tick " + (++calls); } }; })()' }
    @{ Name = 'lifecycle-invalid-delay'; Lifecycle = $true; InvalidDelay = $true; Script = '({ intervalMs: 32.5, pause() {}, resume() {}, tick() {} })' }
    @{ Name = 'lifecycle-invalid-count'; Lifecycle = $true; InvalidDelay = $true; Script = '({ intervalMs: 16, tickCount: 4, pause() {}, resume() {}, tick() {} })' }
    @{ Name = 'lifecycle-live'; Live = $true; Script = '(() => { let phase = 0, calls = 0; return { intervalMs: 32, pause() { phase = 1; }, resume() { if (phase !== 1) throw new Error("private-fixture-error"); phase = 2; }, tick(document) { if (phase !== 2) throw new Error("private-fixture-error"); document.getElementById("status").textContent = "Tick " + (++calls); } }; })()' }
    @{ Name = 'lifecycle-live-rejected'; Live = $true; Preserved = '1 preserved after worker callback rejection'; Script = '(() => { let calls = 0; return { intervalMs: 32, pause() {}, resume() {}, tick(document) { document.getElementById("status").textContent = "Tick " + (++calls); if (calls === 2) throw new Error("private-fixture-error"); } }; })()' }
    @{ Name = 'lifecycle-live-unresolved'; Live = $true; Preserved = '1 preserved after worker callback rejection'; Script = '(() => { let calls = 0; return { intervalMs: 32, pause() {}, resume() {}, tick(document) { document.getElementById("status").textContent = "Tick " + (++calls); if (calls === 2) return new Promise(() => {}); } }; })()' }
    # The second callback stages a write before a finite workload that exceeds its deadline.
    @{ Name = 'lifecycle-live-deadline'; Live = $true; Preserved = '1 preserved after worker callback rejection'; Script = '(() => { let calls = 0; return { intervalMs: 32, pause() {}, resume() {}, tick(document) { const node = document.getElementById("status"); node.textContent = "Tick " + (++calls); if (calls === 2) { let sum = 0; for (let i = 0; i < 1000000000; i++) sum += i; node.textContent = String(sum); } } }; })()' }
    @{ Name = 'lifecycle-live-invalid-edit'; Live = $true; Preserved = '2 preserved after host scene rejection'; Script = '(() => { let calls = 0; return { intervalMs: 32, pause() {}, resume() {}, tick(document) { const element = document.getElementById("status"); element.textContent = "Accepted"; if (++calls === 3) { element.style.color = [1, 2, 3, 255]; element.textContent = String.fromCharCode(0); } } }; })()' }
    @{ Name = 'intake-valid'; Intake = $true; Script = '() => { throw new Error("private-fixture-error"); }'; HtmlBytes = 128 * 1024 }
    @{ Name = 'intake-invalid-manifest'; Intake = $true; Manifest = '{'; Reject = 'validation' }
    @{ Name = 'intake-missing-script'; Intake = $true; Reject = 'validation' }
    @{ Name = 'intake-invalid-html'; Intake = $true; Script = '() => {}'; Html = '<unknown></unknown>'; Reject = 'compilation' }
    @{ Name = 'intake-invalid-css'; Intake = $true; Script = '() => {}'; Html = '<span style="position:fixed">Ready</span>'; Reject = 'compilation' }
    @{ Name = 'intake-image'; Intake = $true; Image = 'valid' }
    @{ Name = 'intake-invalid-image'; Intake = $true; Image = 'invalid'; Reject = 'compilation' }
    @{ Name = 'intake-html-over-limit'; Intake = $true; Script = '() => {}'; HtmlBytes = (1024 * 1024) + 1; Reject = 'compilation' }
    @{ Name = 'intake-html-at-limit'; Intake = $true; Script = '() => {}'; HtmlBytes = 1024 * 1024 }
    @{ Name = 'isolated-script-changed'; Isolated = $true; Script = '(document) => { document.getElementById("status").textContent = "Updated"; }' }
    @{ Name = 'isolated-script-source-at-limit'; Isolated = $true; ScriptBytes = 32 * 1024; Script = '(document) => { document.getElementById("status").textContent = "Updated"; }' }
    @{ Name = 'isolated-script-source-over-limit'; Isolated = $true; ScriptBytes = (32 * 1024) + 1; Script = '(document) => { document.getElementById("status").textContent = "Updated"; }'; Reject = 'script preparation' }
    @{ Name = 'isolated-script-runtime-rejected'; Isolated = $true; Script = '() => { throw new Error("private-fixture-error"); }'; RuntimeReject = $true }
    @{ Name = 'isolated-script-too-many-bindings'; Isolated = $true; Targets = 33; Script = '() => {}'; Reject = 'script preparation' }
    @{ Name = 'isolated-script-missing-source'; Isolated = $true; Reject = 'validation' }
    @{ Name = 'isolated-script-live'; Isolated = $true; Live = $true; Script = '(() => { let phase = 0, calls = 0; return { intervalMs: 32, pause() { phase = 1; }, resume() { if (phase !== 1) throw new Error("private-fixture-error"); phase = 2; }, tick(document) { if (phase !== 2) throw new Error("private-fixture-error"); document.getElementById("status").textContent = "Tick " + (++calls); } }; })()' }
    @{ Name = 'isolated-script-live-rejected'; Isolated = $true; Live = $true; RuntimeReject = $true; Script = '(() => { let calls = 0; return { intervalMs: 32, pause() {}, resume() {}, tick(document) { document.getElementById("status").textContent = "Tick " + (++calls); if (calls === 2) throw new Error("private-fixture-error"); } }; })()' }
)
$manifest = '{"formatVersion":1,"apiVersion":1,"id":"probe.integration","name":"Script preview validation","entry":"index.html","script":"update.js","permissions":[],"preferredFps":30}'
Write-Host "Script preview validation reports in $run"
$results = foreach ($case in $cases) {
    $inputFolder = Join-Path $packages $case.Name
    New-Item -ItemType Directory -Path $inputFolder -Force | Out-Null
    $manifestSource = if ($case.ContainsKey('Manifest')) { $case.Manifest } else { $manifest }
    [IO.File]::WriteAllText((Join-Path $inputFolder 'manifest.json'), $manifestSource)
    [IO.File]::WriteAllText((Join-Path $inputFolder 'index.html'), '<span id="status" style="width:400px;height:24px">Ready</span>')
    if ($case.ContainsKey('Html')) {
        [IO.File]::WriteAllText((Join-Path $inputFolder 'index.html'), $case.Html)
    }
    if ($case.ContainsKey('Targets')) {
        $elements = for ($index = 0; $index -lt $case.Targets; $index++) {
            '<span id="status{0}" style="width:400px;height:24px">Ready</span>' -f $index
        }
        [IO.File]::WriteAllText((Join-Path $inputFolder 'index.html'), ($elements -join "`n"))
    }
    if ($case.ContainsKey('Image')) {
        $example = Join-Path $repo 'examples/static-image'
        foreach ($file in @('manifest.json', 'index.html', 'style.css', 'checker.png')) {
            Copy-Item -LiteralPath (Join-Path $example $file) -Destination $inputFolder
        }
        if ($case.Image -eq 'invalid') {
            [IO.File]::WriteAllBytes((Join-Path $inputFolder 'checker.png'), [byte[]]@(0, 1, 2))
        }
    }
    if ($case.ContainsKey('Script')) {
        [IO.File]::WriteAllText((Join-Path $inputFolder 'update.js'), $case.Script)
    }
    if ($case.ContainsKey('ScriptBytes')) {
        $scriptPath = Join-Path $inputFolder 'update.js'
        $padding = $case.ScriptBytes - (Get-Item -LiteralPath $scriptPath).Length
        [IO.File]::AppendAllText($scriptPath, ' ' * $padding)
        if ((Get-Item -LiteralPath $scriptPath).Length -ne $case.ScriptBytes) {
            throw "Incorrect script fixture size for $($case.Name)"
        }
    }
    if ($case.ContainsKey('HtmlBytes')) {
        $htmlPath = Join-Path $inputFolder 'index.html'
        $padding = $case.HtmlBytes - (Get-Item -LiteralPath $htmlPath).Length
        [IO.File]::AppendAllText($htmlPath, ' ' * $padding)
        if ((Get-Item -LiteralPath $htmlPath).Length -ne $case.HtmlBytes) {
            throw "Incorrect HTML fixture size for $($case.Name)"
        }
    }

    $mode = if ($case.ContainsKey('Intake')) { '--check-package-intake' }
        elseif ($case.ContainsKey('Isolated') -and $case.ContainsKey('Live')) { '--check-isolated-script-live' }
        elseif ($case.ContainsKey('Isolated')) { '--check-isolated-script-preview' }
        elseif ($case.ContainsKey('Lifecycle')) { '--check-trusted-script-lifecycle' }
        elseif ($case.ContainsKey('Live')) { '--check-trusted-script-live' }
        else { '--check-trusted-script-preview' }
    $result = Invoke-SahneGuardedProcess -FilePath $preview -ArgumentList @($mode) `
        -PackageFolder $inputFolder -WorkingDirectory $repo -OutputDirectory (Join-Path $run "logs/$($case.Name)")
    $stdout = Read-SahneBoundedLog -Path $result.StdoutPath
    $stderr = Read-SahneBoundedLog -Path $result.StderrPath
    $cleanupCount = [regex]::Matches($stdout, '(?m)^Package worker profile cleanup verified\.\r?$').Count
    $accepted = '(?m)^Live package transaction verified; scene changed=(true|false); worker cleaned up\.\r?$'
    $passed = $result.Completion -eq 'Exited' -and "$stdout$stderr" -notmatch 'private-fixture-error'
    if ($case.ContainsKey('Intake')) {
        $cleanupCount = [regex]::Matches($stdout, '(?m)^Package helper profile cleanup verified\.\r?$').Count
        $validated = $stdout -match '(?m)^Package validated and compiled in AppContainer\.\r?$'
        $passed = $passed -and $cleanupCount -eq 1
        if ($case.ContainsKey('Reject')) {
            $passed = $passed -and $result.ExitCode -ne 0 -and -not $validated -and
                $stderr.Contains("Package rejected by AppContainer $($case.Reject)")
        }
        else {
            $imageCount = if ($case.ContainsKey('Image')) { 1 } else { 0 }
            $outputVerified = $stdout -match "(?m)^Host independently validated flat output\. Nodes: [1-9][0-9]*\. Images: $imageCount\.\r?$"
            $passed = $passed -and $result.ExitCode -eq 0 -and $validated -and $outputVerified -and
                [string]::IsNullOrWhiteSpace($stderr)
        }
    }
    elseif ($case.ContainsKey('Isolated')) {
        $helperCount = [regex]::Matches($stdout, '(?m)^Package helper profile cleanup verified\.\r?$').Count
        $passed = $passed -and $helperCount -eq 1
        $successPattern = if ($case.ContainsKey('Live')) { '(?m)^Live package pause, resume and two updates verified\.\r?$' }
            else { $accepted.Replace('(true|false)', 'true') }
        if ($case.ContainsKey('Reject')) {
            $passed = $passed -and $result.ExitCode -ne 0 -and $cleanupCount -eq 0 -and
                $stdout -notmatch $accepted -and $stderr.Contains("Package rejected by AppContainer $($case.Reject)")
        }
        elseif ($case.ContainsKey('RuntimeReject')) {
            $passed = $passed -and $result.ExitCode -ne 0 -and $cleanupCount -eq 1 -and
                $stdout -notmatch $successPattern -and $stderr.Contains('Package scene batch rejected')
            if ($case.ContainsKey('Live')) {
                $passed = $passed -and $stdout.Contains('Live package revision 1 preserved after worker callback rejection.')
            }
        }
        else {
            $passed = $passed -and $result.ExitCode -eq 0 -and $cleanupCount -eq 1 -and
                $stdout -match $successPattern -and [string]::IsNullOrWhiteSpace($stderr)
        }
    }
    elseif ($case.ContainsKey('Live')) {
        $verified = $stdout.Contains('Live package pause, resume and two updates verified.')
        $passed = $passed -and $cleanupCount -eq 1
        if ($case.ContainsKey('Preserved')) {
            $passed = $passed -and $result.ExitCode -ne 0 -and -not $verified -and
                $stdout.Contains("Live package revision $($case.Preserved).") -and
                $stderr.Contains('Package scene batch rejected')
        }
        else {
            $passed = $passed -and $result.ExitCode -eq 0 -and $verified -and [string]::IsNullOrWhiteSpace($stderr)
        }
    }
    elseif ($case.ContainsKey('Lifecycle')) {
        $pause = $stdout.IndexOf('Package lifecycle acknowledged: pause.')
        $resume = $stdout.IndexOf('Package lifecycle acknowledged: resume.')
        $dispatches = [regex]::Matches($stdout, 'Host dispatched scheduled package tick\.')
        $acks = [regex]::Matches($stdout, 'Host acknowledged scheduled package revision ([0-9]+)\.')
        $passed = $passed -and $cleanupCount -eq 1
        if ($case.ContainsKey('Changed')) {
            $ticks = if ($case.ContainsKey('Ticks')) { $case.Ticks } else { 0 }
            $tickOrder = $dispatches.Count -eq $ticks -and $acks.Count -eq $ticks
            $previous = $resume
            for ($i = 0; $tickOrder -and $i -lt $ticks; $i++) {
                $tickOrder = $dispatches[$i].Index -gt $previous -and $acks[$i].Index -gt $dispatches[$i].Index -and
                    [int]$acks[$i].Groups[1].Value -eq ($i + 1)
                $previous = $acks[$i].Index
            }
            $passed = $passed -and $pause -ge 0 -and $resume -gt $pause -and $tickOrder -and $result.ExitCode -eq 0 -and
                $stdout -match ($accepted.Replace('(true|false)', $case.Changed)) -and [string]::IsNullOrWhiteSpace($stderr)
        }
        elseif ($case.ContainsKey('InvalidDelay')) {
            $passed = $passed -and $pause -eq -1 -and $resume -eq -1 -and $dispatches.Count -eq 0 -and $result.ExitCode -ne 0 -and
                $stdout -notmatch $accepted -and $stderr.Contains('Package lifecycle initialization rejected')
        }
        elseif ($case.ContainsKey('RejectTick')) {
            $passed = $passed -and $pause -ge 0 -and $resume -gt $pause -and $dispatches.Count -eq 2 -and $acks.Count -eq 1 -and
                $acks[0].Groups[1].Value -eq '1' -and $acks[0].Index -gt $dispatches[0].Index -and
                $dispatches[1].Index -gt $acks[0].Index -and $result.ExitCode -ne 0 -and
                $stdout -notmatch $accepted -and $stderr.Contains('Package scene batch rejected')
        }
        else {
            $passed = $passed -and $pause -ge 0 -and $resume -eq -1 -and $dispatches.Count -eq 0 -and $result.ExitCode -ne 0 -and
                $stdout -notmatch $accepted -and $stderr.Contains('Package resume callback rejected')
        }
    }
    elseif ($case.ContainsKey('Changed')) {
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
        if ($case.ContainsKey('BeforeLaunch')) {
            $expectedError = $case.BeforeLaunch
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
