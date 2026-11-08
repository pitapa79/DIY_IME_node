param([Parameter(Mandatory)][string]$RuntimeDll,[Parameter(Mandatory)][string]$Wasm,[Parameter(Mandatory)][string]$Probe)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$stage=Join-Path $root ('target/no-output-probes/'+[guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($stage)|Out-Null
$requests=[Collections.Generic.List[object]]::new()
function New-Request([object]$value){return @{apiVersion=1;node=@{kind='text.romaji-to-hiragana';version=1};operation=@{method='execute';config=@{overrides=@();unknown='preserve'};inputs=@{in=$value};mode='compose'}}}
foreach($field in @('trusted','permissions','packet','extra')){
 $value=@{state='noOutput'};$value[$field]='private-no-output-canary';$requests.Add((New-Request $value))
}
$requests.Add((New-Request @{state='noOutput'}))
$requests.Add((New-Request @{state='data';packet=@{version=1;root=@{type='text';text='kya'}}}))
$file=Join-Path $stage 'batch.json'
[IO.File]::WriteAllText($file,($requests.ToArray()|ConvertTo-Json -Depth 100 -Compress),[Text.UTF8Encoding]::new($false))
$output=& $Probe $RuntimeDll $Wasm $file 2> (Join-Path $stage 'stderr.txt')
if($LASTEXITCODE -ne 0){throw ('NoOutput probe failed: '+[IO.File]::ReadAllText((Join-Path $stage 'stderr.txt')))}
$replies=@($output|ConvertFrom-Json -Depth 100)
if($replies.Count -ne 6){throw 'NoOutput reply count mismatch'}
for($i=0;$i -lt 4;$i++){
 if($replies[$i].error.code -ne 'guest-json' -or ($replies[$i]|ConvertTo-Json -Depth 100 -Compress).Contains('private-no-output-canary')){throw 'Unknown NoOutput field accepted/echoed'}
}
if(-not $replies[4].ok -or $replies[4].data.out.state -ne 'noOutput' -or $replies[4].data.pending.state -ne 'noOutput'){throw 'Normal NoOutput changed'}
if(-not $replies[5].ok -or $replies[5].data.out.packet.root.text -ne 'きゃ'){throw 'Normal conversion did not recover'}
Write-Output 'Strict NoOutput: 4 unknown-field refusals, normal NoOutput, normal conversion recovery passed'