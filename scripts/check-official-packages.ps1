param([Parameter(Mandatory)][string]$Producer,[Parameter(Mandatory)][string]$RuntimeDll,[Parameter(Mandatory)][string]$Wasm,[Parameter(Mandatory)][string]$Probe,[Parameter(Mandatory)][string]$SourceCommit,[Parameter(Mandatory)][string]$Output)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if($SourceCommit -notmatch '^[a-f0-9]{40}$'){throw 'Source commit must be an explicit full lowercase Git identity'}
$module=[IO.File]::ReadAllBytes($Wasm)
$moduleHash=(Get-FileHash -LiteralPath $Wasm -Algorithm SHA256).Hash.ToLowerInvariant()
$baseline=Get-Content -LiteralPath (Join-Path $root 'docs/legacy-node-inventory.json') -Raw|ConvertFrom-Json -Depth 100
$definitions=@($baseline.nodes|Where-Object {$_.registered -and $_.classification -ne 'system'})
if($definitions.Count -ne 48){throw 'Official registered inventory differs'}
$stage=Join-Path $root ('target/official-probes/'+[guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($stage)|Out-Null
$requests=@($definitions|ForEach-Object {@{apiVersion=1;node=@{kind=$_.definition.kind;version=$_.definition.version};operation=@{method='describe'}}})
$request=Join-Path $stage 'definitions.json';[IO.File]::WriteAllText($request,(ConvertTo-Json -InputObject $requests -Depth 100 -Compress),[Text.UTF8Encoding]::new($false))
$reply=& $Probe $RuntimeDll $Wasm $request 2> (Join-Path $stage 'stderr.txt')
if($LASTEXITCODE -ne 0){throw ('Actual module proof failed: '+[IO.File]::ReadAllText((Join-Path $stage 'stderr.txt')))}
$replies=@($reply|ConvertFrom-Json -Depth 100)
if($replies.Count -ne 48){throw 'Actual definition proof count differs'}
for($i=0;$i -lt 48;$i++){
 if(-not $replies[$i].ok){throw 'Actual module refused an official definition'}
 $actual=[System.Text.Json.Nodes.JsonNode]::Parse((ConvertTo-Json $replies[$i].data -Depth 100 -Compress))
 $expected=[System.Text.Json.Nodes.JsonNode]::Parse((ConvertTo-Json $definitions[$i].definition -Depth 100 -Compress))
 if(-not [System.Text.Json.Nodes.JsonNode]::DeepEquals($actual,$expected)){throw ('Actual Wasm definition differs: '+$definitions[$i].definition.kind)}
}
if((Get-FileHash -LiteralPath $Wasm -Algorithm SHA256).Hash.ToLowerInvariant() -ne $moduleHash){throw 'Module changed after actual proof'}
& $Producer $Wasm $moduleHash $SourceCommit $Output
if($LASTEXITCODE -ne 0){throw 'Official producer failed'}
$indexPath=Join-Path $Output 'official-index.json'
$index=Get-Content -LiteralPath $indexPath -Raw|ConvertFrom-Json -Depth 100
if($index.formatVersion -ne 1 -or $index.source.repository -ne 'pitapa79/DIY_IME_node' -or $index.source.commit -ne $SourceCommit -or $index.module.sha256 -ne $moduleHash -or $index.module.bytes -ne $module.Length -or $index.module.guestApiVersion -ne 1 -or $index.packages.Count -ne 48){throw 'Official source/module inventory mismatch'}
foreach($pair in @(@('Cargo.lock','cargoLockSha256'),@('migration-provenance.json','migrationProvenanceSha256'),@('docs/legacy-node-inventory.json','inventorySha256'))){if((Get-FileHash -LiteralPath (Join-Path $root $pair[0]) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $index.source.($pair[1])){throw 'Source receipt hash mismatch'}}
$names=[Collections.Generic.HashSet[string]]::new();$total=0L;$dictionary=0
foreach($package in $index.packages){
 if($package.file -notmatch '^[a-z0-9][a-z0-9._-]*\.diynode$' -or -not $names.Add($package.file)){throw 'Unsafe or duplicate package file'}
 $path=Join-Path $Output $package.file;$bytes=[IO.File]::ReadAllBytes($path)
 if($bytes.Length -ne $package.bytes -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $package.sha256){throw 'Package digest/size mismatch'}
 $total+=$bytes.Length
 if($bytes.Length -lt 24 -or [Text.Encoding]::ASCII.GetString($bytes,0,8) -ne 'DIY2NODE' -or [BitConverter]::ToUInt32($bytes,8) -ne 1){throw 'Package magic/version mismatch'}
 $manifestLength=[BitConverter]::ToUInt32($bytes,12);$moduleLength=[BitConverter]::ToUInt64($bytes,16)
 if($manifestLength -gt 1048576 -or $moduleLength -ne $module.Length -or 24L+$manifestLength+$moduleLength -ne $bytes.Length){throw 'Package bounded framing mismatch'}
 $manifest=[Text.Encoding]::UTF8.GetString($bytes,24,$manifestLength)|ConvertFrom-Json -Depth 100
 $selected=@($definitions|Where-Object {$_.definition.kind -eq $package.kind})
 if($selected.Count -ne 1 -or $manifest.packageId -ne ('official.'+$package.kind) -or $manifest.packageId -ne $package.packageId -or $manifest.runtime -ne 'wasm32' -or $manifest.guestApiVersion -ne 1 -or $manifest.moduleSha256 -ne $moduleHash -or $manifest.version -ne ('0.1.0+g'+$SourceCommit) -or $manifest.node.version -ne $package.nodeVersion -or $selected[0].classification -ne $package.classification -or $selected[0].status -ne $package.status){throw 'Package metadata/classification mismatch'}
 if(-not [System.Text.Json.Nodes.JsonNode]::DeepEquals([System.Text.Json.Nodes.JsonNode]::Parse((ConvertTo-Json $manifest.node -Depth 100 -Compress)),[System.Text.Json.Nodes.JsonNode]::Parse((ConvertTo-Json $selected[0].definition -Depth 100 -Compress)))){throw 'Packaged definition differs from canonical inventory'}
 $stream=[IO.MemoryStream]::new($bytes,24+$manifestLength,[int]$moduleLength,$false)
 try{$hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)).ToLowerInvariant()}finally{$stream.Dispose()}
 if($hash -ne $moduleHash){throw 'Embedded module differs'}
 if($package.kind -eq 'text.dictionary-exact'){$dictionary++;if($manifest.capabilities.Count -ne 1 -or $manifest.capabilities[0].id -ne 'dictionary.read' -or $manifest.capabilities[0].binding -ne 'dictionary'){throw 'Dictionary capability mismatch'}}elseif($manifest.capabilities.Count -ne 0){throw 'Unexpected authority request'}
}
if($dictionary -ne 1 -or $total -ne $index.packageBytes -or $total -gt 536870912){throw 'Official distribution aggregate mismatch'}
if(@(Get-ChildItem -LiteralPath $Output -File).Count -ne 49){throw 'Unexpected output files'}
# A second publication must fail without changing even the index.
$before=(Get-FileHash -LiteralPath $indexPath -Algorithm SHA256).Hash
& $Producer $Wasm $moduleHash $SourceCommit $Output 2> (Join-Path $stage 'collision.txt')
if($LASTEXITCODE -eq 0 -or (Get-FileHash -LiteralPath $indexPath -Algorithm SHA256).Hash -ne $before){throw 'Existing output was overwritten'}
$global:LASTEXITCODE=0
Write-Output ('Official packages: 48 exact actual-Wasm definitions, framing/digests/module/classification/capabilities/source proof and no-overwrite passed; '+$total+' package bytes')