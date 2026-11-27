# Match read-only WFP evidence for the receiving host of one fixed loopback test.
function Find-SahneLoopbackDrop {
    param(
        [Parameter(Mandatory)][xml] $Events,
        [Parameter(Mandatory)][int] $ListenerPort,
        [Parameter(Mandatory)][int] $HostProcessId,
        [Parameter(Mandatory)][string] $HostImage,
        [Parameter(Mandatory)][DateTimeOffset] $StartedUtc,
        [Parameter(Mandatory)][DateTimeOffset] $FinishedUtc,
        [ValidateRange(0, 65535)][int] $SenderPort = 0
    )
    # Netsh records NT device paths. Compare the full path below the drive and
    # also require the exact live host PID, narrow run interval and listener port.
    if ($HostImage -notmatch '^[A-Za-z]:\\' -or $FinishedUtc -lt $StartedUtc) { return }
    $suffix = $HostImage.Substring(2)
    foreach ($event in $Events.SelectNodes('/netEvents/item')) {
        try {
            $header = $event.header
            $drop = $event.classifyDrop
            $fields = $event.internalFields
            $time = [DateTimeOffset]::Parse([string]$header.timeStamp, [Globalization.CultureInfo]::InvariantCulture)
            if ($time -lt $StartedUtc -or $time -gt $FinishedUtc) { continue }
            if ([string]$event.type -notin @('FWPM_NET_EVENT_TYPE_CLASSIFY_DROP', 'FWPM_NET_EVENT_TYPE_PUBLIC_CLASSIFY_DROP')) { continue }
            if ([string]$header.ipVersion -ne 'FWP_IP_VERSION_V4' -or [string]$header.ipProtocol -ne '6') { continue }
            if ([string]$header.localAddrV4 -ne '127.0.0.1' -or [string]$header.remoteAddrV4 -ne '127.0.0.1') { continue }
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
