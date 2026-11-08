param([Parameter(Mandatory)][string]$RuntimeDll,[Parameter(Mandatory)][string]$Wasm,[Parameter(Mandatory)][string]$Probe)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$stage=Join-Path $root ('target/prepare-probes/'+[guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($stage)|Out-Null
function Request([string]$kind,[object]$operation){return @{apiVersion=1;node=@{kind=$kind;version=1};operation=$operation}}
$requests=[Collections.Generic.List[object]]::new()
$dictionary=[Text.Encoding]::UTF8.GetBytes("reading`t"+'surface'+"`t"+'weight'+"`n"+'か'+"`t"+'蚊'+"`t7`n")
$resources=@{'dictionary.tsv'=@($dictionary)}
$requests.Add((Request 'text.dictionary-exact' @{method='prepare';config=@{path='dictionary.tsv'};resources=$resources}))
$requests.Add((Request 'text.dictionary-exact' @{method='prepare';config=@{path='dictionary.tsv'};resources=@{}}))
$requests.Add((Request 'text.dictionary-exact' @{method='prepare';config=@{path='dictionary.tsv'};resources=@{'dictionary.tsv'=@(255)}}))
$requests.Add((Request 'text.dictionary-exact' @{method='prepare';config=@{path='dictionary.tsv'};resources=@{'dictionary.tsv'=@([Text.Encoding]::UTF8.GetBytes('private-prepare-canary'))}}))
$requests.Add((Request 'text.romaji-to-hiragana' @{method='prepare';config=@{unknown='preserve';overrides=@()};resources=@{}}))
$requests.Add((Request 'text.romaji-to-hiragana' @{method='describe';trusted='private-prepare-canary'}))
$requests.Add((Request 'text.dictionary-exact' @{method='prepare';config=@{path='dictionary.tsv'};resources=$resources;inputs=@{}}))
$requests.Add((Request 'text.dictionary-exact' @{method='execute';config=@{path='dictionary.tsv'};inputs=@{in=@{state='data';packet=@{version=1;root=@{type='text';text='か'}}}}}))
$requests.Add((Request 'text.dictionary-exact' @{method='execute';config=@{path='dictionary.tsv'};resources=$resources;inputs=@{in=@{state='data';packet=@{version=1;root=@{type='text';text='か'}}}}}))
$file=Join-Path $stage 'batch.json'
[IO.File]::WriteAllText($file,($requests.ToArray()|ConvertTo-Json -Depth 100 -Compress),[Text.UTF8Encoding]::new($false))
$output=& $Probe $RuntimeDll $Wasm $file 2> (Join-Path $stage 'stderr.txt')
if($LASTEXITCODE -ne 0){throw 'Isolated prepare probe failed'}
$replies=@($output|ConvertFrom-Json -Depth 100)
if($replies.Count -ne 9){throw 'Prepare reply count mismatch'}
if(-not $replies[0].ok -or -not $replies[0].data.valid -or -not $replies[0].data.prepared){throw 'Dictionary prepare failed'}
$expected=@{1='dictionary-path';2='dictionary-encoding';3='dictionary-format';5='guest-json';6='guest-json';7='dictionary-path'}
foreach($index in $expected.Keys){if($replies[$index].error.code -ne $expected[$index]){throw "Unexpected prepare refusal at $index"}}
if(-not $replies[4].ok -or -not $replies[4].data.valid -or $replies[4].data.prepared){throw 'Pure preparation changed'}
if(-not $replies[8].ok -or $replies[8].data.out.packet.root.options[0].value.text -ne '蚊'){throw 'Dictionary execution did not recover'}
if(($replies|ConvertTo-Json -Depth 100 -Compress).Contains('private-prepare-canary')){throw 'Probe error echoed input'}
Write-Output 'Guest prepare: valid dictionary/pure snapshots, missing/encoding/format/unknown-field refusal, no retained resource authority and execution recovery passed'