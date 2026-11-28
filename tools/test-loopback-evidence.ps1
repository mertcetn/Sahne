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

[xml]$fixtureV6 = $fixture.OuterXml.Replace('FWP_IP_VERSION_V4','FWP_IP_VERSION_V6').
    Replace('AddrV4','AddrV6.byteArray16').Replace('127.0.0.1','::1')
if (@(Find-SahneLoopbackDrop -Events $fixtureV6 @parameters -IPv6 -SenderPort 54738).Count -ne 1) { throw 'IPv6 positive fixture rejected' }
if (@(Find-SahneLoopbackDrop -Events $fixtureV6 @parameters).Count -ne 0) { throw 'IPv6 event accepted for IPv4' }
if (@(Find-SahneLoopbackDrop -Events $fixture @parameters -IPv6).Count -ne 0) { throw 'IPv4 event accepted for IPv6' }
$fixtureV6.SelectSingleNode('/netEvents/item/header/remoteAddrV6.byteArray16').InnerText = '::2'
if (@(Find-SahneLoopbackDrop -Events $fixtureV6 @parameters -IPv6).Count -ne 0) { throw 'Non-loopback IPv6 peer accepted' }
Write-Output 'IPv6 matcher checks passed: 1 positive, 3 negative.'

[xml]$fixtureUdp = $fixture.OuterXml.Replace('<ipProtocol>6</ipProtocol>','<ipProtocol>17</ipProtocol>')
if (@(Find-SahneLoopbackDrop -Events $fixtureUdp @parameters -Udp -SenderPort 54738).Count -ne 1) { throw 'UDP positive fixture rejected' }
if (@(Find-SahneLoopbackDrop -Events $fixtureUdp @parameters).Count -ne 0) { throw 'UDP drop accepted for TCP' }
if (@(Find-SahneLoopbackDrop -Events $fixtureUdp @parameters -Udp -SenderPort 54739).Count -ne 0) { throw 'Wrong UDP sender accepted' }
Write-Output 'UDP matcher checks passed: 1 positive, 2 negative.'
[xml]$fixtureUdpV6 = $fixtureV6.OuterXml.Replace('<ipProtocol>6</ipProtocol>','<ipProtocol>17</ipProtocol>')
$fixtureUdpV6.SelectSingleNode('/netEvents/item/header/remoteAddrV6.byteArray16').InnerText = '::1'
if (@(Find-SahneLoopbackDrop -Events $fixtureUdpV6 @parameters -IPv6 -Udp -SenderPort 54738).Count -ne 1) { throw 'IPv6 UDP positive fixture rejected' }
if (@(Find-SahneLoopbackDrop -Events $fixtureUdpV6 @parameters -IPv6).Count -ne 0) { throw 'IPv6 UDP drop accepted for TCP' }
if (@(Find-SahneLoopbackDrop -Events $fixtureUdpV6 @parameters -Udp).Count -ne 0) { throw 'IPv6 UDP drop accepted for IPv4' }
Write-Output 'IPv6 UDP matcher checks passed: 1 positive, 2 negative.'

$verified = "Verified AppContainer identity and zero capabilities;`nProfile cleanup verified: test`n"
$explicit = $verified + "Explicit TCP access denial confirmed for network.`nProbe passed; test`n"
$timeout = 'Network test inconclusive or allowed: Err((TimedOut, None))'
if ((Get-SahneLoopbackAcceptance Exited 0 $explicit '' $false) -ne 'ExplicitDenialConfirmed') { throw 'Native denial rejected' }
if ((Get-SahneLoopbackAcceptance Exited 1 $verified $timeout $true) -ne 'LoopbackDropConfirmed') { throw 'Correlated block rejected' }
if ((Get-SahneLoopbackAcceptance Exited 1 $verified $timeout $false) -ne 'Inconclusive') { throw 'Uncorrelated timeout accepted' }
if ((Get-SahneLoopbackAcceptance Exited 1 $verified 'Network test inconclusive or allowed: Ok(())' $true) -ne 'Inconclusive') { throw 'Allowed connection accepted' }
if ((Get-SahneLoopbackAcceptance TimedOut 1 $verified $timeout $true) -ne 'Inconclusive') { throw 'Host timeout accepted' }
if ((Get-SahneLoopbackAcceptance Exited 1 $verified "$timeout Attempt cleanup failed:" $true) -ne 'Inconclusive') { throw 'Failed cleanup accepted' }
if ((Get-SahneLoopbackAcceptance Exited 1 'Verified AppContainer identity and zero capabilities;' $timeout $true) -ne 'Inconclusive') { throw 'Missing cleanup accepted' }
if ((Get-SahneLoopbackAcceptance Exited 0 ($explicit.Replace('for network.', 'for network-egress.')) '' $false) -ne 'Inconclusive') { throw 'Wrong native case accepted' }
if ((Get-SahneLoopbackAcceptance Exited 0 ($explicit.Replace('for network.', 'for network-loopback-v6.')) '' $false -IPv6) -ne 'ExplicitDenialConfirmed') { throw 'Native IPv6 denial rejected' }
if ((Get-SahneLoopbackAcceptance Exited 0 $explicit '' $false -IPv6) -ne 'Inconclusive') { throw 'Native IPv4 denial accepted for IPv6' }
Write-Output 'Loopback acceptance checks passed: 3 positive, 7 negative.'
$udpVerified = $verified + "UDP loopback receiver had no worker datagram.`n"
$udpSent = 'Network test inconclusive or allowed: Ok(())'
if ((Get-SahneLoopbackAcceptance Exited 1 $udpVerified $udpSent $true -Udp) -ne 'LoopbackDropConfirmed') { throw 'Correlated UDP drop rejected' }
if ((Get-SahneLoopbackAcceptance Exited 1 $udpVerified $udpSent $false -Udp) -ne 'Inconclusive') { throw 'Uncorrelated UDP drop accepted' }
if ((Get-SahneLoopbackAcceptance Exited 1 $verified $udpSent $true -Udp) -ne 'Inconclusive') { throw 'Missing UDP receiver observation accepted' }
Write-Output 'UDP acceptance checks passed: 1 positive, 2 negative.'
