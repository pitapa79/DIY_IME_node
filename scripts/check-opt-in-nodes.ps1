param(
 [Parameter(Mandatory)][string]$RuntimeDll,
 [Parameter(Mandatory)][string]$OfficialWasm,
 [Parameter(Mandatory)][string]$Wasm,
 [Parameter(Mandatory)][string]$Probe
)
$ErrorActionPreference = 'Stop'

function Request([string]$kind, [hashtable]$operation) {
 return @{apiVersion=1; node=@{kind=$kind; version=1}; operation=$operation}
}

function TextPacket([string]$value) {
 return @{state='data'; packet=@{version=1; root=@{type='text'; text=$value}}}
}

function Invoke-Batch([string]$module, [object[]]$requests, [string]$name) {
 $file = Join-Path $stage ($name + '.json')
 [IO.File]::WriteAllText($file, (ConvertTo-Json -InputObject $requests -Depth 30 -Compress), [Text.UTF8Encoding]::new($false))
 $output = & $Probe $RuntimeDll $module $file 2> (Join-Path $stage ($name + '-stderr.txt'))
 if ($LASTEXITCODE -ne 0) { throw "Wasm probe failed for $name" }
 $parsed = ConvertFrom-Json -InputObject ($output -join [Environment]::NewLine) -Depth 30 -NoEnumerate
 if ($parsed.Count -ne $requests.Count) { throw "Unexpected Wasm reply count for $name" }
 return ,$parsed
}

$stage = Join-Path $PSScriptRoot ('../target/opt-in-probes/' + [guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($stage) | Out-Null

$unicode = 'experimental.text-unicode-normalize'
$describe = @((Request $unicode @{method='describe'}))
$official = Invoke-Batch $OfficialWasm $describe 'official'
if ($official[0].ok -or $official[0].error.code -ne 'unknown-kind') { throw 'Unicode normalization leaked into official Wasm' }

$decomposed = 'か' + [char]0x3099
$probes = @(
 (Request $unicode @{method='describe'})
 (Request $unicode @{method='execute'; config=@{form='nfc'}; inputs=@{in=(TextPacket $decomposed)}})
 (Request $unicode @{method='execute'; config=@{form='nfd'}; inputs=@{in=(TextPacket ('が' * 4096))}})
)
$replies = Invoke-Batch $Wasm $probes 'opt-in'
if (-not $replies[0].ok -or $replies[0].data.kind -ne $unicode) { throw 'Opt-in Wasm did not describe Unicode normalization' }
if (-not $replies[1].ok -or $replies[1].data.out.packet.root.text -cne 'が') { throw 'Wasm NFC failed' }
if (-not $replies[2].ok -or $replies[2].data.out.packet.root.text -cne ($decomposed * 4096)) { throw 'Wasm NFD expansion failed at the runtime input limit' }
Write-Output 'Opt-in Wasm: Unicode normalization and official-module exclusion passed'
