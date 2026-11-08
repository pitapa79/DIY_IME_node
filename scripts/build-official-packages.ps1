param([string]$Cargo='cargo',[string]$RuntimeRoot,[Parameter(Mandatory)][string]$SourceCommit,[string]$Output,[switch]$Isolated)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if($SourceCommit -notmatch '^[a-f0-9]{40}$'){throw 'Provide the full source commit selected by the owning build pipeline'}
$oldTarget=$env:CARGO_TARGET_DIR;$oldFlags=$env:RUSTFLAGS;$oldEncoded=$env:CARGO_ENCODED_RUSTFLAGS
Push-Location $root
try {
 $metadata=& $Cargo metadata --locked --no-deps --format-version 1|ConvertFrom-Json
 if($LASTEXITCODE -ne 0){throw 'Cargo metadata unavailable'}
 $build=Join-Path $metadata.target_directory 'official-build'
 if(-not $Output){$Output=Join-Path $metadata.target_directory ('official-nodes-'+$SourceCommit)}
 $Output=[IO.Path]::GetFullPath($Output)
 if(Test-Path -LiteralPath $Output){throw 'Output already exists; choose a fresh output directory'}
 $cargoRegistry=if($env:CARGO_HOME){Join-Path $env:CARGO_HOME 'registry'}else{Join-Path $env:USERPROFILE '.cargo/registry'}
 $cargoRegistry=[IO.Path]::GetFullPath($cargoRegistry)
 # Encoded flags preserve paths containing spaces, and no native path is recorded in the receipt.
 $env:CARGO_ENCODED_RUSTFLAGS=@("--remap-path-prefix=$root=source","--remap-path-prefix=$cargoRegistry=registry") -join [char]31
 $env:RUSTFLAGS=$null;$env:CARGO_TARGET_DIR=$build
 $compiler=@(& rustc -Vv)
 if($LASTEXITCODE -ne 0 -or $compiler[0] -notmatch '^rustc 1\.90\.0 '){throw 'Official build requires the pinned Rust 1.90.0 toolchain'}
 & ./scripts/check-migration.ps1
 # This rebuilds and proves the exact remapped official module and keeps custom ABI features separate.
 & ./scripts/check-wasm-runtime.ps1 -Cargo $Cargo -RuntimeRoot $RuntimeRoot -Isolated:$Isolated
 if($LASTEXITCODE -ne 0){throw 'Exact official module proof failed'}
 & $Cargo build --locked --release --example official_packages
 if($LASTEXITCODE -ne 0){throw 'Official producer build failed'}
 $module=Join-Path $build 'wasm32-unknown-unknown/release/diy_ime2_nodes.wasm'
 $producer=Join-Path $build 'release/examples/official_packages.exe'
 $probe=Join-Path $build 'debug/examples/wasm_runtime_probe.exe'
 if(-not $RuntimeRoot){$RuntimeRoot=Join-Path $root 'target/tools/wasmtime50'}
 $dll=Join-Path $RuntimeRoot 'wasmtime-v50.0.1-x86_64-windows-c-api/lib/wasmtime.dll'
 & ./scripts/check-official-packages.ps1 -Producer $producer -RuntimeDll $dll -Wasm $module -Probe $probe -SourceCommit $SourceCommit -Output $Output
 if($LASTEXITCODE -ne 0){throw 'Official package verification failed'}
 $bytes=[IO.File]::ReadAllBytes($module)
 $strings=[Text.Encoding]::UTF8.GetString($bytes).ToLowerInvariant()
 foreach($privatePath in @($root,$cargoRegistry)){
  if($strings.Contains($privatePath.ToLowerInvariant()) -or $strings.Contains($privatePath.Replace('\','/').ToLowerInvariant())){throw 'Official module contains an unremapped build path'}
 }
 $receipt=@{formatVersion=1;sourceRepository='pitapa79/DIY_IME_node';sourceCommit=$SourceCommit;rustc=$compiler;target='wasm32-unknown-unknown';profile='release';defaultFeatures=$true;pathRemapping=@('workspace=source','cargo-registry=registry');moduleSha256=(Get-FileHash -LiteralPath $module -Algorithm SHA256).Hash.ToLowerInvariant();indexSha256=(Get-FileHash -LiteralPath (Join-Path $Output 'official-index.json') -Algorithm SHA256).Hash.ToLowerInvariant();producerSha256=(Get-FileHash -LiteralPath $producer -Algorithm SHA256).Hash.ToLowerInvariant();actualDefinitionProof=48;isolatedProof=[bool]$Isolated}
 $receiptPath=$Output+'.build-receipt.json'
 $encoded=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json $receipt -Depth 10))
 $file=[IO.File]::Open($receiptPath,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$file.Write($encoded);$file.Flush($true)}finally{$file.Dispose()}
 Write-Output ('Official release data and build receipt verified: '+$Output)
}finally{$env:CARGO_TARGET_DIR=$oldTarget;$env:RUSTFLAGS=$oldFlags;$env:CARGO_ENCODED_RUSTFLAGS=$oldEncoded;Pop-Location}