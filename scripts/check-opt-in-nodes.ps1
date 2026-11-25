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
$compare = 'experimental.text-compare'
$prefix = 'experimental.text-dictionary-prefix'
$describe = @(
 (Request $unicode @{method='describe'})
 (Request $compare @{method='describe'})
 (Request $prefix @{method='describe'})
)
$official = Invoke-Batch $OfficialWasm $describe 'official'
foreach ($reply in $official) {
 if ($reply.ok -or $reply.error.code -ne 'unknown-kind') { throw 'An opt-in node leaked into official Wasm' }
}

$decomposed = 'か' + [char]0x3099
$dictionary = [Text.Encoding]::UTF8.GetBytes("reading`tsurface`tweight`nかな`t仮名`t7`nかに`t蟹`t5`n")
$resources = @{"dictionary.tsv"=@($dictionary)}
$prefixConfig = @{path="dictionary.tsv"; limit=1}
$probes = @(
 (Request $unicode @{method='describe'})
 (Request $unicode @{method='execute'; config=@{form='nfc'}; inputs=@{in=(TextPacket $decomposed)}})
 (Request $unicode @{method='execute'; config=@{form='nfd'}; inputs=@{in=(TextPacket ('が' * 4096))}})
 (Request $compare @{method='execute'; config=@{mode='prefix'}; inputs=@{a=(TextPacket '日本語'); b=(TextPacket '日本')}})
 (Request $compare @{method='execute'; config=@{mode='exact'}; inputs=@{a=(TextPacket 'が'); b=(TextPacket $decomposed)}})
 (Request $prefix @{method='execute'; config=$prefixConfig; inputs=@{in=(TextPacket 'か')}; resources=$resources})
 (Request $prefix @{method='execute'; config=$prefixConfig; inputs=@{in=(TextPacket 'か')}})
 (Request $prefix @{method='execute'; config=$prefixConfig; inputs=@{in=(TextPacket 'わ')}; resources=$resources})
 (Request $prefix @{method='prepare'; config=$prefixConfig; resources=@{'dictionary.tsv'=@(255)}})
)
$replies = Invoke-Batch $Wasm $probes 'opt-in'
if (-not $replies[0].ok -or $replies[0].data.kind -ne $unicode) { throw 'Opt-in Wasm did not describe Unicode normalization' }
if (-not $replies[1].ok -or $replies[1].data.out.packet.root.text -cne 'が') { throw 'Wasm NFC failed' }
if (-not $replies[2].ok -or $replies[2].data.out.packet.root.text -cne ($decomposed * 4096)) { throw 'Wasm NFD expansion failed at the runtime input limit' }
if (-not $replies[3].ok -or $replies[3].data.matched.packet.root.text -cne '日本語' -or $replies[3].data.unmatched.state -ne 'noOutput') { throw 'Wasm compare matched routing failed' }
if (-not $replies[4].ok -or $replies[4].data.unmatched.packet.root.text -cne 'が' -or $replies[4].data.matched.state -ne 'noOutput') { throw 'Wasm compare unmatched routing failed' }
$options = $replies[5].data.out.packet.root.options
if (-not $replies[5].ok -or $options.Count -ne 1 -or $options[0].value.text -cne '仮名') { throw 'Wasm dictionary prefix or candidate limit failed' }
if ($replies[6].ok -or $replies[6].error.code -ne 'dictionary-path') { throw 'Wasm dictionary prefix accepted missing snapshot' }
if (-not $replies[7].ok -or $replies[7].data.out.packet.root.type -ne 'choice' -or $replies[7].data.out.packet.root.options.Count -ne 0) { throw 'Wasm dictionary prefix missing match failed' }
if ($replies[8].ok -or $replies[8].error.code -ne 'dictionary-encoding') { throw 'Wasm dictionary prefix accepted invalid snapshot' }
Write-Output 'Opt-in Wasm: normalization, text comparison, dictionary prefix and official-module exclusion passed'

$replace = 'experimental.text-regex-replace'
$officialReplace = Invoke-Batch $OfficialWasm @((Request $replace @{method='describe'})) 'official-regex-replace'
if ($officialReplace[0].ok -or $officialReplace[0].error.code -ne 'unknown-kind') { throw 'Regex replacement leaked into official Wasm' }
$replaceProbes = @(
 (Request $replace @{method='execute'; config=@{pattern='([0-9]+)月'; replacement='[${1}]'; mode='all'}; inputs=@{in=(TextPacket '1月と12月')}})
 (Request $replace @{method='validate'; config=@{pattern='('; replacement=''; mode='all'}})
)
$replaceReplies = Invoke-Batch $Wasm $replaceProbes 'regex-replace'
if (-not $replaceReplies[0].ok -or $replaceReplies[0].data.out.packet.root.text -cne '[1]と[12]') { throw 'Wasm regex capture replacement failed' }
if ($replaceReplies[1].ok -or $replaceReplies[1].error.code -ne 'invalid-pattern') { throw 'Wasm regex accepted an invalid pattern' }
Write-Output 'Opt-in Wasm: regex replacement and official-module exclusion passed'

$search = 'experimental.text-regex-search'
$officialCheck = Invoke-Batch $OfficialWasm @((Request $search @{method='describe'})) 'official-text-regex-search'
if ($officialCheck[0].ok -or $officialCheck[0].error.code -ne 'unknown-kind') { throw 'regex search leaked into official Wasm' }
$searchConfig = @{pattern='[0-9]'; limit=1}
$searchProbes = @(
 (Request $search @{method='execute'; config=$searchConfig; inputs=@{in=(TextPacket '😀1あ2')}})
 (Request $search @{method='execute'; config=$searchConfig; inputs=@{in=(TextPacket '数字なし')}})
)
$searchReplies = Invoke-Batch $Wasm $searchProbes 'regex-search'
$searchRoot = $searchReplies[0].data.out.packet.root
$searchItem = $searchRoot.items[0]
if (-not $searchReplies[0].ok -or $searchRoot.type -ne 'sequence' -or $searchRoot.items.Count -ne 1 -or $searchItem.text -cne '1' -or ($searchItem.meta.sourceSpan -join ',') -ne '1,2') { throw 'Wasm regex search did not return the expected Unicode span and limit' }
if (-not $searchReplies[1].ok -or $searchReplies[1].data.out.packet.root.type -ne 'sequence' -or $searchReplies[1].data.out.packet.root.items.Count -ne 0) { throw 'Wasm regex search did not return an empty sequence' }
Write-Output 'Opt-in Wasm: regex search and official-module exclusion passed'

$mapping = 'experimental.text-mapping'
$officialCheck = Invoke-Batch $OfficialWasm @((Request $mapping @{method='describe'})) 'official-text-mapping'
if ($officialCheck[0].ok -or $officialCheck[0].error.code -ne 'unknown-kind') { throw 'text mapping leaked into official Wasm' }
$mappingEntries = @(@{from='か';to='短'}, @{from='かな';to='仮名'}, @{from='仮名';to='再置換'})
$mappingProbes = @(
 (Request $mapping @{method='execute'; config=@{entries=$mappingEntries; unknown='preserve'}; inputs=@{in=(TextPacket 'かな!')}})
 (Request $mapping @{method='execute'; config=@{entries=$mappingEntries; unknown='error'}; inputs=@{in=(TextPacket 'かな!')}})
)
$mappingReplies = Invoke-Batch $Wasm $mappingProbes 'mapping'
if (-not $mappingReplies[0].ok -or $mappingReplies[0].data.out.packet.root.text -cne '仮名!') { throw 'Wasm mapping did not use the longest nonrecursive replacement' }
if ($mappingReplies[1].ok -or $mappingReplies[1].error.code -ne 'unmapped-input') { throw 'Wasm mapping did not reject unmapped input' }
Write-Output 'Opt-in Wasm: text mapping and official-module exclusion passed'

$kana = 'experimental.text-kana-width'
$officialCheck = Invoke-Batch $OfficialWasm @((Request $kana @{method='describe'})) 'official-text-kana-width'
if ($officialCheck[0].ok -or $officialCheck[0].error.code -ne 'unknown-kind') { throw 'kana width leaked into official Wasm' }
$kanaProbes = @(
 (Request $kana @{method='execute'; config=@{mode='fullwidth'}; inputs=@{in=(TextPacket 'ｶﾞﾊﾟ･Aひ')}})
 (Request $kana @{method='execute'; config=@{mode='halfwidth'}; inputs=@{in=(TextPacket 'ガパ・Aひ')}})
)
$kanaReplies = Invoke-Batch $Wasm $kanaProbes 'kana-width'
if (-not $kanaReplies[0].ok -or $kanaReplies[0].data.out.packet.root.text -cne 'ガパ・Aひ') { throw 'Wasm kana conversion to fullwidth failed' }
if (-not $kanaReplies[1].ok -or $kanaReplies[1].data.out.packet.root.text -cne 'ｶﾞﾊﾟ･Aひ') { throw 'Wasm kana conversion to halfwidth failed' }
Write-Output 'Opt-in Wasm: kana width and official-module exclusion passed'

$reverse = 'experimental.text-dictionary-reverse'
$officialCheck = Invoke-Batch $OfficialWasm @((Request $reverse @{method='describe'})) 'official-text-dictionary-reverse'
if ($officialCheck[0].ok -or $officialCheck[0].error.code -ne 'unknown-kind') { throw 'dictionary reverse leaked into official Wasm' }
$reverseDictionary = [Text.Encoding]::UTF8.GetBytes("reading`tsurface`tweight`nよみ`t表記`t2`nかな`t表記`t9`n")
$reverseResources = @{'dictionary.tsv'=@($reverseDictionary)}
$reverseConfig = @{path='dictionary.tsv'; limit=1}
$reverseProbes = @(
 (Request $reverse @{method='execute'; config=$reverseConfig; inputs=@{in=(TextPacket '表記')}; resources=$reverseResources})
 (Request $reverse @{method='execute'; config=$reverseConfig; inputs=@{in=(TextPacket '表記')}})
)
$reverseReplies = Invoke-Batch $Wasm $reverseProbes 'dictionary-reverse'
$reverseRoot = $reverseReplies[0].data.out.packet.root
if (-not $reverseReplies[0].ok -or $reverseRoot.type -ne 'choice' -or $reverseRoot.options.Count -ne 1 -or $reverseRoot.options[0].value.text -cne 'よみ' -or $reverseRoot.options[0].score -ne 2) { throw 'Wasm dictionary reverse did not honor dictionary order and candidate limit' }
if ($reverseReplies[1].ok -or $reverseReplies[1].error.code -ne 'dictionary-path') { throw 'Wasm dictionary reverse accepted a missing snapshot' }
Write-Output 'Opt-in Wasm: dictionary reverse and official-module exclusion passed'

$additionalKinds = @('experimental.text-hiragana-to-romaji', 'experimental.text-number-notation', 'experimental.text-date-format', 'experimental.text-time-format', 'experimental.text-unit-notation', 'experimental.text-length')
$additionalRequests = @($additionalKinds | ForEach-Object { Request $_ @{method='describe'} })
$additionalOfficial = Invoke-Batch $OfficialWasm $additionalRequests 'additional-official'
$additionalOptIn = Invoke-Batch $Wasm $additionalRequests 'additional-opt-in'
for ($index = 0; $index -lt $additionalKinds.Count; $index++) {
 $kind = $additionalKinds[$index]
 if ($additionalOfficial[$index].ok -or $additionalOfficial[$index].error.code -ne 'unknown-kind') { throw "Opt-in kind leaked into official Wasm: $kind" }
 if (-not $additionalOptIn[$index].ok -or $additionalOptIn[$index].data.kind -cne $kind -or $additionalOptIn[$index].data.version -ne 1) { throw "Could not describe opt-in Wasm kind: $kind" }
}
$validationRequests = @(for ($index = 0; $index -lt $additionalKinds.Count; $index++) {
 Request $additionalKinds[$index] @{method='validate'; config=$additionalOptIn[$index].data.defaultConfig}
})
$validationReplies = Invoke-Batch $Wasm $validationRequests 'additional-default-configs'
for ($index = 0; $index -lt $additionalKinds.Count; $index++) {
 if (-not $validationReplies[$index].ok -or -not $validationReplies[$index].data.valid) { throw "Default Wasm configuration rejected: $($additionalKinds[$index])" }
}
Write-Output 'Opt-in Wasm: additional definitions, defaults and official-module exclusion passed'

$lengthInput = 'か' + [char]0x3099
$lengthRequests = @(
 (Request 'experimental.text-length' @{method='execute'; config=@{unit='grapheme'}; inputs=@{in=(TextPacket $lengthInput)}})
 (Request 'experimental.text-length' @{method='execute'; config=@{unit='scalar'}; inputs=@{in=(TextPacket $lengthInput)}})
)
$lengthReplies = Invoke-Batch $Wasm $lengthRequests 'text-length'
if (-not $lengthReplies[0].ok -or $lengthReplies[0].data.out.packet.root.text -cne '1') { throw 'Wasm grapheme count failed' }
if (-not $lengthReplies[1].ok -or $lengthReplies[1].data.out.packet.root.text -cne '2') { throw 'Wasm scalar count failed' }
