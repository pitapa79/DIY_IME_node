param([Parameter(Mandatory)][string]$RuntimeDll,[Parameter(Mandatory)][string]$Wasm,[Parameter(Mandatory)][string]$Probe)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$stage=Join-Path $root ('target/service-probes/'+[guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($stage)|Out-Null
function New-Request([object]$operation){return @{apiVersion=1;node=@{kind='fixture.service';version=1};operation=$operation}}
function Text-Value(){return @{state='data';packet=@{version=1;root=@{type='text';text='private-service-input🐈';meta=@{reading='かな'}}}}}
function Execute([string]$mode){return (New-Request @{method='execute';config=@{mode=$mode};inputs=@{in=(Text-Value)};mode='compose'})}
function Resume([object]$continuation,[string]$mode='network'){return (New-Request @{method='resume';config=@{mode=$mode};continuation=$continuation;response=@{status=201;body=@(0,255,65)}})}
$state=@{text='private-service-input🐈';mode='compose';resource=$null;hop=1}
$resources=@{'dictionary.tsv'=@([Text.Encoding]::UTF8.GetBytes('private-service-snapshot'))}
$cases=[Collections.Generic.List[object]]::new()
$cases.Add((New-Request @{method='describe'}))
$cases.Add((New-Request @{method='validate';config=@{mode='network'}}))
$cases.Add((New-Request @{method='prepare';config=@{mode='network';resource='dictionary.tsv'};resources=$resources}))
$cases.Add((Execute 'network'))
$cases.Add((Resume $state))
$cases.Add((Execute 'pure'))
$no=Execute 'network';$no.operation.inputs.in=@{state='noOutput'};$cases.Add($no)
$missing=Execute 'network';$missing.operation.config.resource='dictionary.tsv';$cases.Add($missing)
$read=Execute 'network';$read.operation.config.resource='dictionary.tsv';$read.operation.resources=$resources;$cases.Add($read)
$resumed=Resume $state;$resumed.operation.config.resource='dictionary.tsv';$resumed.operation.resources=$resources;$cases.Add($resumed)
$bad=Resume $state;$bad.operation.inputs=@{in='private-forged-field'};$cases.Add($bad)
$bad=Resume $state;$bad.operation.response.headers=@{Authorization='private-forged-field'};$cases.Add($bad)
$bad=Resume $state;$bad.operation.Remove('continuation');$cases.Add($bad)
$cases.Add((Resume $null))
$bad=Execute 'network';$bad.apiVersion=2;$cases.Add($bad)
$cases.Add((Execute 'bad-binding'))
$cases.Add((Execute 'bad-output'))
$cases.Add((Resume $state 'twice'))
$cases.Add((Resume @{hop=2;text='private-service-input🐈'} 'twice'))
$bad=Execute 'network';$bad.operation.inputs.in.packet.root=@{type='choice';options=@()};$cases.Add($bad)
$bad=New-Request @{method='describe'};$bad.node.kind='input.text';$cases.Add($bad)
$bad=Resume $state;$bad.operation.response.body=@(256);$cases.Add($bad)
$bad=Execute 'network';$bad.permissions='private-forged-field';$cases.Add($bad)
$request=Join-Path $stage 'batch.json';[IO.File]::WriteAllText($request,(ConvertTo-Json -InputObject @($cases.ToArray()) -Depth 100 -Compress),[Text.UTF8Encoding]::new($false))
$output=& $Probe $RuntimeDll $Wasm $request 2> (Join-Path $stage 'stderr.txt')
if($LASTEXITCODE -ne 0){throw ('Service guest probe failed: '+[IO.File]::ReadAllText((Join-Path $stage 'stderr.txt')))}
$replies=@($output|ConvertFrom-Json -Depth 100)
if($replies.Count -ne $cases.Count){throw 'Service reply count mismatch'}
if(-not $replies[0].ok -or $replies[0].data.kind -ne 'fixture.service'){throw 'Service identity mismatch'}
if(-not $replies[1].data.valid -or -not $replies[2].data.prepared){throw 'Service validate/prepare mismatch'}
if(-not $replies[3].ok -or $replies[3].call.binding -ne 'api' -or [Text.Encoding]::UTF8.GetString([byte[]]$replies[3].call.body) -ne 'private-service-input🐈' -or $replies[3].call.continuation.mode -ne 'compose' -or $replies[3].PSObject.Properties.Name -contains 'data'){throw 'Service call envelope mismatch'}
$final=$replies[4].data.out.packet.root.text|ConvertFrom-Json -Depth 100
if($final.status -ne 201 -or ($final.body -join ',') -ne '0,255,65' -or $null -ne $final.mode -or $final.continuation.text -ne 'private-service-input🐈'){throw 'Service resume projection mismatch'}
if($replies[5].data.out.packet.root.meta.reading -ne 'かな' -or $replies[5].data.out.packet.root.text -ne 'private-service-input🐈'){throw 'Service pure packet mismatch'}
if($replies[6].data.out.state -ne 'noOutput' -or $replies[6].PSObject.Properties.Name -contains 'call'){throw 'Service required NoOutput did not skip'}
if($replies[8].call.continuation.resource -ne 'private-service-snapshot'){throw 'Service resource scope mismatch'}
$final=$replies[9].data.out.packet.root.text|ConvertFrom-Json -Depth 100;if($final.resource -ne 'private-service-snapshot'){throw 'Resume resources missing'}
if(-not $replies[13].ok -or -not $replies[18].ok -or $replies[17].call.binding -ne 'api' -or ($replies[17].call.body -join ',') -ne '0,255,65' -or $replies[17].call.continuation.hop -ne 2){throw 'Explicit null state or multihop mismatch'}
$errors=@{7='resource-unavailable';10='guest-json';11='guest-json';12='guest-json';14='guest-version';15='guest-network';16='guest-output';19='invalid-root';20='unknown-kind';21='guest-json';22='guest-json'}
foreach($entry in $errors.GetEnumerator()){$reply=$replies[$entry.Key];if($reply.ok -or $reply.error.code -ne $entry.Value -or (ConvertTo-Json $reply -Depth 100 -Compress).Contains('private-forged-field')){throw ('Wrong service refusal at '+$entry.Key+': '+(ConvertTo-Json $reply -Depth 100 -Compress))}}
Write-Output ("Service guest: "+$cases.Count+" actual ABI/call/resume/snapshot/NoOutput/strict-refusal cases passed")