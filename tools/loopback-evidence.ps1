# Match read-only WFP evidence for the receiving host of one fixed loopback test.
function Find-SahneLoopbackDrop {
    param(
        [Parameter(Mandatory)][xml] $Events,
        [Parameter(Mandatory)][int] $ListenerPort,
        [Parameter(Mandatory)][int] $HostProcessId,
        [Parameter(Mandatory)][string] $HostImage,
        [Parameter(Mandatory)][DateTimeOffset] $StartedUtc,
        [Parameter(Mandatory)][DateTimeOffset] $FinishedUtc,
        [ValidateRange(0, 65535)][int] $SenderPort = 0,
        [switch] $IPv6,
        [switch] $Udp
    )
    # Netsh records NT device paths. Compare the full path below the drive and
    # also require the exact live host PID, narrow run interval and listener port.
    if ($HostImage -notmatch '^[A-Za-z]:\\' -or $FinishedUtc -lt $StartedUtc) { return }
    $suffix = $HostImage.Substring(2)
    if ($IPv6 -and $Udp) { return }
    $protocol = if ($Udp) { '17' } else { '6' }
    $version, $address, $localField, $remoteField = if ($IPv6) {
        'FWP_IP_VERSION_V6', '::1', 'localAddrV6.byteArray16', 'remoteAddrV6.byteArray16'
    } else {
        'FWP_IP_VERSION_V4', '127.0.0.1', 'localAddrV4', 'remoteAddrV4'
    }
    foreach ($event in $Events.SelectNodes('/netEvents/item')) {
        try {
            $header = $event.header
            $drop = $event.classifyDrop
            $fields = $event.internalFields
            $time = [DateTimeOffset]::Parse([string]$header.timeStamp, [Globalization.CultureInfo]::InvariantCulture)
            if ($time -lt $StartedUtc -or $time -gt $FinishedUtc) { continue }
            if ([string]$event.type -notin @('FWPM_NET_EVENT_TYPE_CLASSIFY_DROP', 'FWPM_NET_EVENT_TYPE_PUBLIC_CLASSIFY_DROP')) { continue }
            if ([string]$header.ipVersion -ne $version -or [string]$header.ipProtocol -ne $protocol) { continue }
            if ($header.SelectSingleNode($localField).InnerText -ne $address -or
                $header.SelectSingleNode($remoteField).InnerText -ne $address) { continue }
            if ([int]$header.localPort -ne $ListenerPort -or [int]$header.remotePort -lt 1 -or [int]$header.remotePort -gt 65535) { continue }
            if ($SenderPort -ne 0 -and [int]$header.remotePort -ne $SenderPort) { continue }
            if ([string]$drop.isLoopback -ne 'true' -or [string]$drop.msFwpDirection -ne 'MS_FWP_DIRECTION_IN') { continue }
            if ([string]$fields.filterOrigin -ne 'AppContainer Loopback' -or [int]$fields.processId -ne $HostProcessId) { continue }
            $image = [Text.Encoding]::Unicode.GetString([Convert]::FromHexString([string]$header.appId.data)).TrimEnd([char]0)
            if (-not $image.StartsWith('\device\', [StringComparison]::OrdinalIgnoreCase) -or
                -not $image.EndsWith($suffix, [StringComparison]::OrdinalIgnoreCase)) { continue }
            $filterId = [UInt64]$drop.filterId
            if ($filterId -eq 0) { continue }
            $blocks = @($fields.terminatingFiltersInfo.item | Where-Object {
                [UInt64]$_.filterId -eq $filterId -and
                [string]$_.subLayer -eq 'FWPP_SUBLAYER_INTERNAL_FIREWALL_APP_ISOLATION' -and
                [string]$_.actionType -eq 'FWP_ACTION_BLOCK'
            })
            if ($blocks.Count -ne 1) { continue }
            [pscustomobject]@{ TimestampUtc = $time.ToString('O'); FilterId = $filterId; RemotePort = [int]$header.remotePort }
        }
        catch { continue } # Missing, malformed or newer schema fields never satisfy acceptance.
    }
}

function Get-SahneLoopbackAcceptance {
    param([string] $Completion, [int] $ExitCode, [string] $Stdout, [string] $Stderr, [bool] $SenderCorrelated, [switch] $IPv6, [switch] $Udp)
    if ($Completion -ne 'Exited' -or $Stderr.Contains('Attempt cleanup failed:') -or
        ([regex]::Matches($Stdout, '(?m)^Profile cleanup verified:')).Count -ne 1 -or
        -not $Stdout.Contains('Verified AppContainer identity and zero capabilities;')) {
        return 'Inconclusive'
    }
    if ($Udp) {
        if ($IPv6 -or $Stdout -notmatch '(?m)^UDP loopback receiver had no worker datagram\.\r?$') { return 'Inconclusive' }
        if ($ExitCode -eq 0 -and $Stdout -match '(?m)^Explicit UDP access denial confirmed; host received no worker datagram\.\r?$' -and
            $Stdout -match '(?m)^Probe passed;') { return 'ExplicitDenialConfirmed' }
        if ($ExitCode -eq 1 -and $SenderCorrelated -and
            $Stderr.Contains('Network test inconclusive or allowed: Ok(())')) { return 'LoopbackDropConfirmed' }
        return 'Inconclusive'
    }
    $case = if ($IPv6) { 'network-loopback-v6' } else { 'network' }
    if ($ExitCode -eq 0 -and $Stdout -match "(?m)^Explicit TCP access denial confirmed for $case\.\r?$" -and
        $Stdout -match '(?m)^Probe passed;') {
        return 'ExplicitDenialConfirmed'
    }
    if ($ExitCode -eq 1 -and $SenderCorrelated -and
        $Stderr.Contains('Network test inconclusive or allowed: Err((TimedOut, None))')) {
        return 'LoopbackDropConfirmed'
    }
    return 'Inconclusive'
}
