# Pure matcher checks; no administrator privileges, processes or network access.
#Requires -Version 7.0
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'loopback-evidence.ps1')
$image = 'C:\fixture\wallpaper-sandbox-probe.exe'
$encodedImage = [Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes('\device\harddiskvolume3\fixture\wallpaper-sandbox-probe.exe' + [char]0))
[xml]$fixture = @"
<netEvents><item>
<header><timeStamp>2026-11-14T17:19:26.759Z</timeStamp>
<ipVersion>FWP_IP_VERSION_V4</ipVersion><ipProtocol>6</ipProtocol>
<localAddrV4>127.0.0.1</localAddrV4><remoteAddrV4>127.0.0.1</remoteAddrV4>
<localPort>54736</localPort><remotePort>54738</remotePort><appId><data>$encodedImage</data></appId></header>
<type>FWPM_NET_EVENT_TYPE_PUBLIC_CLASSIFY_DROP</type>
<classifyDrop><filterId>73064</filterId><msFwpDirection>MS_FWP_DIRECTION_IN</msFwpDirection><isLoopback>true</isLoopback></classifyDrop>
<internalFields><filterOrigin>AppContainer Loopback</filterOrigin><processId>23624</processId>
<terminatingFiltersInfo><item><filterId>73064</filterId><subLayer>FWPP_SUBLAYER_INTERNAL_FIREWALL_APP_ISOLATION</subLayer><actionType>FWP_ACTION_BLOCK</actionType></item></terminatingFiltersInfo>
</internalFields></item></netEvents>
"@
$parameters = @{
    ListenerPort = 54736; HostProcessId = 23624; HostImage = $image
    StartedUtc = [DateTimeOffset]'2026-11-14T17:19:26Z'
    FinishedUtc = [DateTimeOffset]'2026-11-14T17:19:29Z'
}
if (@(Find-SahneLoopbackDrop -Events $fixture @parameters).Count -ne 1) { throw 'Positive fixture rejected' }
if (@(Find-SahneLoopbackDrop -Events $fixture @parameters -SenderPort 54738).Count -ne 1) { throw 'Matching sender port rejected' }
if (@(Find-SahneLoopbackDrop -Events $fixture @parameters -SenderPort 54739).Count -ne 0) { throw 'Mismatched sender port accepted' }
$mutations = @(
    @('header/timeStamp', '2026-11-14T17:19:25Z'),
    @('header/timeStamp', '2026-11-14T17:19:30Z'),
    @('header/ipVersion', 'FWP_IP_VERSION_V6'),
    @('header/ipProtocol', '17'),
    @('header/localAddrV4', '127.0.0.2'),
    @('header/remoteAddrV4', '192.0.2.1'),
    @('header/localPort', '54737'),
    @('header/remotePort', '0'),
    @('header/appId/data', 'bad-hex'),
    @('header/appId/data', [Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes('\device\harddiskvolume3\other.exe'))),
    @('type', 'FWPM_NET_EVENT_TYPE_CLASSIFY_ALLOW'),
    @('classifyDrop/isLoopback', 'false'),
    @('classifyDrop/msFwpDirection', 'MS_FWP_DIRECTION_OUT'),
    @('classifyDrop/filterId', '73065'),
    @('internalFields/filterOrigin', 'Unrelated firewall rule'),
    @('internalFields/processId', '23625'),
    @('internalFields/terminatingFiltersInfo/item/subLayer', 'FWPP_SUBLAYER_INTERNAL_FIREWALL_WF'),
    @('internalFields/terminatingFiltersInfo/item/actionType', 'FWP_ACTION_PERMIT')
)
foreach ($mutation in $mutations) {
    $copy = $fixture.CloneNode($true)
    $copy.SelectSingleNode('/netEvents/item/' + $mutation[0]).InnerText = $mutation[1]
    if (@(Find-SahneLoopbackDrop -Events $copy @parameters).Count -ne 0) { throw "False positive: $($mutation[0])" }
}
$missing = $fixture.CloneNode($true)
$node = $missing.SelectSingleNode('/netEvents/item/internalFields/processId')
$node.ParentNode.RemoveChild($node) | Out-Null
if (@(Find-SahneLoopbackDrop -Events $missing @parameters).Count -ne 0) { throw 'Missing PID accepted' }
[xml]$empty = '<netEvents />'
if (@(Find-SahneLoopbackDrop -Events $empty @parameters).Count -ne 0) { throw 'Missing events accepted' }
Write-Output "Loopback matcher checks passed: 2 positive, $($mutations.Count + 3) negative."
