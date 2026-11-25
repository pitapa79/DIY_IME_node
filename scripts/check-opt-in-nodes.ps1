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

$symbolNotation = 'experimental.text-symbol-notation'
$officialSymbolNotation = Invoke-Batch $OfficialWasm @((Request $symbolNotation @{method='describe'})) 'official-symbol-notation'
if ($officialSymbolNotation[0].ok -or $officialSymbolNotation[0].error.code -ne 'unknown-kind') { throw 'symbol notation leaked into official Wasm' }
$symbolRequests = @(
 (Request $symbolNotation @{method='execute'; config=@{preset='japanese'}; inputs=@{in=(TextPacket '("日本語")')}})
 (Request $symbolNotation @{method='execute'; config=@{preset='ascii'}; inputs=@{in=(TextPacket '（「日本語」）')}})
)
$symbolReplies = Invoke-Batch $Wasm $symbolRequests 'symbol-notation'
if (-not $symbolReplies[0].ok -or $symbolReplies[0].data.out.packet.root.text -cne '（「日本語」）') { throw 'Wasm Japanese symbol notation failed' }
if (-not $symbolReplies[1].ok -or $symbolReplies[1].data.out.packet.root.text -cne '("日本語")') { throw 'Wasm ASCII symbol notation failed' }

$kanjiVariants = 'experimental.text-kanji-variants'
$officialKanjiVariants = Invoke-Batch $OfficialWasm @((Request $kanjiVariants @{method='describe'})) 'official-kanji-variants'
if ($officialKanjiVariants[0].ok -or $officialKanjiVariants[0].error.code -ne 'unknown-kind') { throw 'kanji variants leaked into official Wasm' }
$kanjiVariantRequests = @((Request $kanjiVariants @{method='execute'; config=@{}; inputs=@{in=(TextPacket '崎')}}))
$kanjiVariantReplies = Invoke-Batch $Wasm $kanjiVariantRequests 'kanji-variants'
$kanjiVariantRoot = $kanjiVariantReplies[0].data.out.packet.root
if (-not $kanjiVariantReplies[0].ok -or $kanjiVariantRoot.type -ne 'choice' -or ($kanjiVariantRoot.options.value.text -join ',') -cne '崎,﨑,嵜') { throw 'Wasm kanji variants candidate order failed' }

$kanjiForm = 'experimental.text-kanji-form'
$officialKanjiForm = Invoke-Batch $OfficialWasm @((Request $kanjiForm @{method='describe'})) 'official-kanji-form'
if ($officialKanjiForm[0].ok -or $officialKanjiForm[0].error.code -ne 'unknown-kind') { throw 'kanji form leaked into official Wasm' }
$kanjiFormRequests = @(
 (Request $kanjiForm @{method='execute'; config=@{direction='to-shinjitai'}; inputs=@{in=(TextPacket '國學')}})
 (Request $kanjiForm @{method='execute'; config=@{direction='to-kyujitai'}; inputs=@{in=(TextPacket '国学')}})
)
$kanjiFormReplies = Invoke-Batch $Wasm $kanjiFormRequests 'kanji-form'
if (-not $kanjiFormReplies[0].ok -or $kanjiFormReplies[0].data.out.packet.root.text -cne '国学') { throw 'Wasm shinjitai conversion failed' }
if (-not $kanjiFormReplies[1].ok -or $kanjiFormReplies[1].data.out.packet.root.text -cne '國學') { throw 'Wasm kyujitai conversion failed' }

$paragraph = 'experimental.paragraph-split'
$officialParagraph = Invoke-Batch $OfficialWasm @((Request $paragraph @{method='describe'})) 'official-paragraph-split'
if ($officialParagraph[0].ok -or $officialParagraph[0].error.code -ne 'unknown-kind') { throw 'paragraph split leaked into official Wasm' }
$paragraphRequests = @(
 (Request $paragraph @{method='execute'; config=@{}; inputs=@{in=(TextPacket "あ`r`n`r`n😀")}})
 (Request $paragraph @{method='execute'; config=@{}; inputs=@{in=@{state='noOutput'}}})
)
$paragraphReplies = Invoke-Batch $Wasm $paragraphRequests 'paragraph-split'
$paragraphRoot = $paragraphReplies[0].data.out.packet.root
if (-not $paragraphReplies[0].ok -or $paragraphRoot.type -ne 'sequence' -or $paragraphRoot.items.Count -ne 2 -or $paragraphRoot.items[0].text -cne "あ`r`n`r`n" -or $paragraphRoot.items[1].text -cne '😀') { throw 'Wasm paragraph split failed' }
if (-not $paragraphReplies[1].ok -or $paragraphReplies[1].data.out.state -ne 'noOutput') { throw 'Wasm paragraph split did not propagate NoOutput' }
$quantity = 'experimental.text-quantity-format'
$officialQuantity = Invoke-Batch $OfficialWasm @((Request $quantity @{method='describe'})) 'official-quantity-format'
if ($officialQuantity[0].ok -or $officialQuantity[0].error.code -ne 'unknown-kind') { throw 'quantity format leaked into official Wasm' }
$quantityRequests = @((Request $quantity @{method='execute'; config=@{}; inputs=@{in=(TextPacket '6冊')}}))
$quantityReplies = Invoke-Batch $Wasm $quantityRequests 'quantity-format'
if (-not $quantityReplies[0].ok -or $quantityReplies[0].data.out.packet.root.text -cne 'ろくさつ') { throw 'Wasm quantity format mishandled six books' }
$language = 'experimental.text-language-detect'
$officialLanguage = Invoke-Batch $OfficialWasm @((Request $language @{method='describe'})) 'official-language-detect'
if ($officialLanguage[0].ok -or $officialLanguage[0].error.code -ne 'unknown-kind') { throw 'language detect leaked into official Wasm' }
$languageRequests = @(
 (Request $language @{method='execute'; config=@{}; inputs=@{in=(TextPacket 'helloПривет')}})
 (Request $language @{method='execute'; config=@{}; inputs=@{in=(TextPacket '・')}})
 (Request $language @{method='execute'; config=@{}; inputs=@{in=(TextPacket 'かな')}})
)
$languageReplies = Invoke-Batch $Wasm $languageRequests 'language-detect'
if (-not $languageReplies[0].ok -or $languageReplies[0].data.out.packet.root.text -cne 'und') { throw 'Wasm language detect misclassified mixed scripts' }
if (-not $languageReplies[1].ok -or $languageReplies[1].data.out.packet.root.text -cne 'und') { throw 'Wasm language detect classified punctuation as kana' }
if (-not $languageReplies[2].ok -or $languageReplies[2].data.out.packet.root.text -cne 'ja') { throw 'Wasm language detect missed kana' }
$randomSelect = 'experimental.choice-random-select'
$officialRandomSelect = Invoke-Batch $OfficialWasm @((Request $randomSelect @{method='describe'})) 'official-random-select'
if ($officialRandomSelect[0].ok -or $officialRandomSelect[0].error.code -ne 'unknown-kind') { throw 'random select leaked into official Wasm' }
$choiceInput = @{state='data'; packet=@{version=1; root=@{type='choice'; options=@(@{value=@{type='text'; text='あ'}}, @{value=@{type='text'; text='い'}}, @{value=@{type='text'; text='う'}})}}}
$emptyChoice = @{state='data'; packet=@{version=1; root=@{type='choice'; options=@()}}}
$randomRequests = @(
 (Request $randomSelect @{method='execute'; config=@{seed=0}; inputs=@{in=$choiceInput}})
 (Request $randomSelect @{method='execute'; config=@{seed=0}; inputs=@{in=$emptyChoice}})
)
$randomReplies = Invoke-Batch $Wasm $randomRequests 'random-select'
if (-not $randomReplies[0].ok -or $randomReplies[0].data.out.packet.root.text -cne 'う') { throw 'Wasm random select returned the wrong candidate' }
if (-not $randomReplies[1].ok -or $randomReplies[1].data.out.state -ne 'noOutput') { throw 'Wasm random select did not handle an empty choice' }
$constraints = 'experimental.packet-validate-constraints'
$officialConstraints = Invoke-Batch $OfficialWasm @((Request $constraints @{method='describe'})) 'official-constraints'
if ($officialConstraints[0].ok -or $officialConstraints[0].error.code -ne 'unknown-kind') { throw 'constraints node leaked into official Wasm' }
$constraintRequests = @(
 (Request $constraints @{method='execute'; config=@{maxCandidates=$null; maxTextLength=[long]4294967296; allowedRoots=@()}; inputs=@{in=(TextPacket 'a')}})
 (Request $constraints @{method='execute'; config=@{maxCandidates=$null; maxTextLength=1; allowedRoots=@()}; inputs=@{in=(TextPacket 'あ😀')}})
)
$constraintReplies = Invoke-Batch $Wasm $constraintRequests 'constraints'
if (-not $constraintReplies[0].ok -or $constraintReplies[0].data.valid.packet.root.text -cne 'a') { throw 'Wasm constraints wrapped a large limit' }
if (-not $constraintReplies[1].ok -or $constraintReplies[1].data.invalid.packet.root.text -cne 'あ😀') { throw 'Wasm constraints did not count Unicode characters' }
$scoreFormat = 'experimental.score-format'
$officialScoreFormat = Invoke-Batch $OfficialWasm @((Request $scoreFormat @{method='describe'})) 'official-score-format'
if ($officialScoreFormat[0].ok -or $officialScoreFormat[0].error.code -ne 'unknown-kind') { throw 'score format leaked into official Wasm' }
$scoreInput = @{state='data'; packet=@{version=1; root=@{type='choice'; options=@(@{value=@{type='text'; text='large'}; score=1.0e308})}}}
$scoreRequests = @((Request $scoreFormat @{method='execute'; config=@{decimals=2; percent=$true}; inputs=@{in=$scoreInput}}))
$scoreReplies = Invoke-Batch $Wasm $scoreRequests 'score-format'
$formattedScore = $scoreReplies[0].data.out.packet.root.items[0].text
if (-not $scoreReplies[0].ok -or $formattedScore.Length -lt 310 -or -not $formattedScore.EndsWith('00.00%')) { throw 'Wasm percent formatting overflowed' }
$flowBranch = 'experimental.flow-branch'
$officialFlowBranch = Invoke-Batch $OfficialWasm @((Request $flowBranch @{method='describe'})) 'official-flow-branch'
if ($officialFlowBranch[0].ok -or $officialFlowBranch[0].error.code -ne 'unknown-kind') { throw 'flow branch leaked into official Wasm' }
$flowBranchRequests = @(
 (Request $flowBranch @{method='execute'; config=@{}; inputs=@{data=(TextPacket 'payload'); selector=(TextPacket 'b')}})
 (Request $flowBranch @{method='execute'; config=@{}; inputs=@{data=(TextPacket 'payload'); selector=(TextPacket 'c')}})
 (Request $flowBranch @{method='execute'; config=@{}; inputs=@{data=@{state='noOutput'}; selector=(TextPacket 'a')}})
)
$flowBranchReplies = Invoke-Batch $Wasm $flowBranchRequests 'flow-branch'
if (-not $flowBranchReplies[0].ok -or $flowBranchReplies[0].data.b.packet.root.text -cne 'payload' -or $flowBranchReplies[0].data.a.state -ne 'noOutput') { throw 'Wasm flow branch routing failed' }
if ($flowBranchReplies[1].ok -or $flowBranchReplies[1].error.code -ne 'invalid-selector') { throw 'Wasm flow branch accepted an invalid selector' }
if (-not $flowBranchReplies[2].ok -or $flowBranchReplies[2].data.a.state -ne 'noOutput' -or $flowBranchReplies[2].data.b.state -ne 'noOutput') { throw 'Wasm flow branch did not propagate NoOutput' }
$testData = 'experimental.packet-test-data'
$officialTestData = Invoke-Batch $OfficialWasm @((Request $testData @{method='describe'})) 'official-packet-test-data'
if ($officialTestData[0].ok -or $officialTestData[0].error.code -ne 'unknown-kind') { throw 'packet test data leaked into official Wasm' }
$testDataRequests = @(
 (Request $testData @{method='execute'; config=@{shape='text'; size=5; seed=0}; inputs=@{trigger=(TextPacket 'go')}})
 (Request $testData @{method='execute'; config=@{shape='choice'; size=2; seed=0}; inputs=@{trigger=(TextPacket 'go')}})
 (Request $testData @{method='execute'; config=@{shape='text'; size=5; seed=0}; inputs=@{trigger=@{state='noOutput'}}})
)
$testDataReplies = Invoke-Batch $Wasm $testDataRequests 'packet-test-data'
if (-not $testDataReplies[0].ok -or $testDataReplies[0].data.out.packet.root.text -cne 'つけあとう') { throw 'Wasm packet test data text generation failed' }
$testChoice = $testDataReplies[1].data.out.packet.root
if (-not $testDataReplies[1].ok -or $testChoice.type -ne 'choice' -or $testChoice.options.Count -ne 2 -or $testChoice.options[0].value.text -cne 'つけあ' -or $testChoice.options[0].score -ne 0.444) { throw 'Wasm packet test data choice generation failed' }
if (-not $testDataReplies[2].ok -or $testDataReplies[2].data.out.state -ne 'noOutput') { throw 'Wasm packet test data did not propagate NoOutput' }
$additionalKinds = @('experimental.text-hiragana-to-romaji', 'experimental.text-number-notation', 'experimental.text-date-format', 'experimental.text-time-format', 'experimental.text-unit-notation', 'experimental.text-length', 'experimental.text-script-classify', 'experimental.text-script-split', 'experimental.text-number-unit-extract')
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

$extractRequests = @((Request 'experimental.text-number-unit-extract' @{method='execute'; config=@{limit=64}; inputs=@{in=(TextPacket '😀-2.5 kg')}}))
$extractReplies = Invoke-Batch $Wasm $extractRequests 'number-unit-extract'
$extractRoot = $extractReplies[0].data.out.packet.root
if (-not $extractReplies[0].ok -or $extractRoot.type -ne 'sequence' -or $extractRoot.items.Count -ne 1 -or $extractRoot.items[0].text -cne '-2.5 kg' -or ($extractRoot.items[0].meta.sourceSpan -join ',') -ne '1,8') { throw 'Wasm number-unit extraction or Unicode span failed' }

$splitRequests = @((Request 'experimental.text-script-split' @{method='execute'; config=@{}; inputs=@{in=(TextPacket 'カ・ナ')}}))
$splitReplies = Invoke-Batch $Wasm $splitRequests 'script-split'
$splitRoot = $splitReplies[0].data.out.packet.root
if (-not $splitReplies[0].ok -or $splitRoot.type -ne 'sequence' -or $splitRoot.items.Count -ne 3 -or $splitRoot.items[1].text -cne '・') { throw 'Wasm script split did not separate punctuation' }

$classifyRequests = @(
 (Request 'experimental.text-script-classify' @{method='execute'; config=@{}; inputs=@{in=(TextPacket 'a・')}})
 (Request 'experimental.text-script-classify' @{method='execute'; config=@{}; inputs=@{in=(TextPacket 'かな漢字')}})
)
$classifyReplies = Invoke-Batch $Wasm $classifyRequests 'script-classify'
if (-not $classifyReplies[0].ok -or $classifyReplies[0].data.out.packet.root.text -cne 'latin') { throw 'Wasm script classification treated punctuation as kana' }
if (-not $classifyReplies[1].ok -or $classifyReplies[1].data.out.packet.root.text -cne 'mixed') { throw 'Wasm script classification did not detect mixed scripts' }

$lengthInput = 'か' + [char]0x3099
$lengthRequests = @(
 (Request 'experimental.text-length' @{method='execute'; config=@{unit='grapheme'}; inputs=@{in=(TextPacket $lengthInput)}})
 (Request 'experimental.text-length' @{method='execute'; config=@{unit='scalar'}; inputs=@{in=(TextPacket $lengthInput)}})
)
$lengthReplies = Invoke-Batch $Wasm $lengthRequests 'text-length'
if (-not $lengthReplies[0].ok -or $lengthReplies[0].data.out.packet.root.text -cne '1') { throw 'Wasm grapheme count failed' }
if (-not $lengthReplies[1].ok -or $lengthReplies[1].data.out.packet.root.text -cne '2') { throw 'Wasm scalar count failed' }
$unitRequests = @(
 (Request 'experimental.text-unit-notation' @{method='execute'; config=@{}; inputs=@{in=(TextPacket '10 kmと3mg')}})
 (Request 'experimental.text-unit-notation' @{method='execute'; config=@{}; inputs=@{in=(TextPacket '5Kg')}})
)
$unitReplies = Invoke-Batch $Wasm $unitRequests 'unit-notation'
if (-not $unitReplies[0].ok -or $unitReplies[0].data.out.packet.root.text -cne '10キロメートルと3ミリグラム') { throw 'Wasm unit notation conversion failed' }
if (-not $unitReplies[1].ok -or $unitReplies[1].data.out.packet.root.text -cne '5Kg') { throw 'Wasm unit notation accepted wrong case' }
$timeRequests = @(
 (Request 'experimental.text-time-format' @{method='execute'; config=@{from='h24'; to='japanese'}; inputs=@{in=(TextPacket '00:05:07')}})
 (Request 'experimental.text-time-format' @{method='execute'; config=@{from='h24'; to='h12'}; inputs=@{in=(TextPacket '24:00')}})
)
$timeReplies = Invoke-Batch $Wasm $timeRequests 'time-format'
if (-not $timeReplies[0].ok -or $timeReplies[0].data.out.packet.root.text -cne '午前12時05分07秒') { throw 'Wasm time format failed at midnight' }
if ($timeReplies[1].ok -or $timeReplies[1].error.code -ne 'invalid-time') { throw 'Wasm time format accepted 24:00 in the twelve hour clock' }
$romajiRequests = @(
 (Request 'experimental.text-hiragana-to-romaji' @{method='execute'; config=@{system='hepburn'}; inputs=@{in=(TextPacket 'まっちゃとっあ')}})
)
$romajiReplies = Invoke-Batch $Wasm $romajiRequests 'hiragana-to-romaji'
if (-not $romajiReplies[0].ok -or $romajiReplies[0].data.out.packet.root.text -cne "matchato'a") { throw 'Wasm hiragana-to-romaji conversion failed' }

$numberRequests = @(
 (Request 'experimental.text-number-notation' @{method='execute'; config=@{mode='arabic'}; inputs=@{in=(TextPacket '一万二千三百四十五')}})
 (Request 'experimental.text-number-notation' @{method='execute'; config=@{mode='arabic'}; inputs=@{in=(TextPacket '十百')}})
)
$numberReplies = Invoke-Batch $Wasm $numberRequests 'number-notation'
if (-not $numberReplies[0].ok -or $numberReplies[0].data.out.packet.root.text -cne '12345') { throw 'Wasm number notation conversion failed' }
if ($numberReplies[1].ok -or $numberReplies[1].error.code -ne 'invalid-number') { throw 'Wasm number notation accepted out-of-order units' }

$dateRequests = @(
 (Request 'experimental.text-date-format' @{method='execute'; config=@{from='iso'; to='japanese'}; inputs=@{in=(TextPacket '0001-01-01')}})
 (Request 'experimental.text-date-format' @{method='execute'; config=@{from='iso'; to='japanese'}; inputs=@{in=(TextPacket '1900-02-29')}})
)
$dateReplies = Invoke-Batch $Wasm $dateRequests 'date-format'
if (-not $dateReplies[0].ok -or $dateReplies[0].data.out.packet.root.text -cne '0001年1月1日') { throw 'Wasm date format did not preserve the four digit year' }
if ($dateReplies[1].ok -or $dateReplies[1].error.code -ne 'invalid-date') { throw 'Wasm date format accepted an invalid leap day' }
