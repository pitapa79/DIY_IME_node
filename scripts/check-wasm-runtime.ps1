param([string]$Cargo='cargo',[string]$RuntimeRoot,[switch]$Isolated)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if(-not $RuntimeRoot){$RuntimeRoot=Join-Path $root 'target/tools/wasmtime50'}
[IO.Directory]::CreateDirectory($RuntimeRoot)|Out-Null
$archive=Join-Path $RuntimeRoot 'wasmtime-v50.0.1-x86_64-windows-c-api.zip'
if(-not(Test-Path -LiteralPath $archive)){Invoke-WebRequest -Uri 'https://github.com/bytecodealliance/wasmtime/releases/download/v50.0.1/wasmtime-v50.0.1-x86_64-windows-c-api.zip' -OutFile $archive}
if((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne '8b461d9dc12b076d0414b1023433bbb2dc08d3b1c0f0a156853ede35f0147802'){throw 'Official runtime archive hash mismatch'}
$RuntimeDll=Join-Path $RuntimeRoot 'wasmtime-v50.0.1-x86_64-windows-c-api/lib/wasmtime.dll'
if(-not(Test-Path -LiteralPath $RuntimeDll)){Expand-Archive -LiteralPath $archive -DestinationPath $RuntimeRoot}
Push-Location $root
try{
 & ./scripts/check-runtime-probe.ps1
 & $Cargo build --locked --release --lib --target wasm32-unknown-unknown
 if($LASTEXITCODE -ne 0){throw 'Wasm build failed; install rustup target wasm32-unknown-unknown for project toolchain'}
 & $Cargo build --locked --example wasm_runtime_probe
 if($LASTEXITCODE -ne 0){throw 'Probe build failed'}
 $metadata=& $Cargo metadata --locked --no-deps --format-version 1|ConvertFrom-Json
 if($LASTEXITCODE -ne 0){throw 'Cargo metadata failed'}
 $Wasm=Join-Path $metadata.target_directory 'wasm32-unknown-unknown/release/diy_ime2_nodes.wasm'
 $Probe=Join-Path $metadata.target_directory 'debug/examples/wasm_runtime_probe.exe'
 $officialHash=(Get-FileHash -LiteralPath $Wasm -Algorithm SHA256).Hash
 $fixture=Join-Path $root 'tests/fixtures/service_guest/Cargo.toml'
 $fixtureTarget=Join-Path $metadata.target_directory 'service-guest'
 $pattern='(?m)^name = "([^"]+)"\r?\nversion = "([^"]+)"\r?\nsource = "([^"]+)"\r?\nchecksum = "([^"]+)"'
 $known=[Collections.Generic.HashSet[string]]::new()
 foreach($m in [regex]::Matches([IO.File]::ReadAllText((Join-Path $root 'Cargo.lock')),$pattern)){[void]$known.Add($m.Value.Replace([string][char]13,''))}
 foreach($m in [regex]::Matches([IO.File]::ReadAllText((Join-Path $root 'tests/fixtures/service_guest/Cargo.lock')),$pattern)){if(-not $known.Contains($m.Value.Replace([string][char]13,''))){throw 'Service fixture dependency is not covered by the existing Node lockfile'}}
 & $Cargo fmt --manifest-path $fixture -- --check
 if($LASTEXITCODE -ne 0){throw 'Service fixture fmt failed'}
 & $Cargo build --locked --manifest-path $fixture --release --target wasm32-unknown-unknown --target-dir $fixtureTarget
 if($LASTEXITCODE -ne 0){throw 'Service fixture build failed'}
 & $Cargo clippy --locked --manifest-path $fixture --target wasm32-unknown-unknown --target-dir $fixtureTarget -- -D warnings
 if($LASTEXITCODE -ne 0){throw 'Service fixture clippy failed'}
 $ServiceWasm=Join-Path $fixtureTarget 'wasm32-unknown-unknown/release/diyime2_service_guest.wasm'
 if((Get-FileHash -LiteralPath $Wasm -Algorithm SHA256).Hash -ne $officialHash){throw 'Custom-guest build overwrote the official module'}
 if($Isolated){
  & ./scripts/check-worker-probe.ps1
  & $Cargo build --locked --example appcontainer_worker_probe
  if($LASTEXITCODE -ne 0){throw 'Isolation Worker probe build failed'}
  $worker=Join-Path $metadata.target_directory 'debug/examples/appcontainer_worker_probe.exe'
  & $worker canary
  if($LASTEXITCODE -ne 0){throw 'Native isolation canary failed'}
  & $worker timeout
  if($LASTEXITCODE -ne 0){throw 'Physical termination/recovery failed'}
  $oldWorker=$env:DIYIME2_ISOLATION_PROBE
  $oldWasm=$env:DIYIME2_WASM_PROBE
  try{
   $env:DIYIME2_ISOLATION_PROBE=$worker
   $env:DIYIME2_WASM_PROBE=$Probe
   & ./scripts/check-wasm-guest.ps1 -RuntimeDll $RuntimeDll -Wasm $Wasm -Probe (Join-Path $root 'scripts/invoke-isolated-probe.ps1')
   & ./scripts/check-no-output.ps1 -RuntimeDll $RuntimeDll -Wasm $Wasm -Probe (Join-Path $root 'scripts/invoke-isolated-probe.ps1')
   & ./scripts/check-prepare.ps1 -RuntimeDll $RuntimeDll -Wasm $Wasm -Probe (Join-Path $root 'scripts/invoke-isolated-probe.ps1')
   & ./scripts/check-service-guest.ps1 -RuntimeDll $RuntimeDll -Wasm $ServiceWasm -Probe (Join-Path $root 'scripts/invoke-isolated-probe.ps1')
  }finally{
   $env:DIYIME2_ISOLATION_PROBE=$oldWorker
   $env:DIYIME2_WASM_PROBE=$oldWasm
  }
 }else{
  & ./scripts/check-wasm-guest.ps1 -RuntimeDll $RuntimeDll -Wasm $Wasm -Probe $Probe
  & ./scripts/check-no-output.ps1 -RuntimeDll $RuntimeDll -Wasm $Wasm -Probe $Probe
  & ./scripts/check-prepare.ps1 -RuntimeDll $RuntimeDll -Wasm $Wasm -Probe $Probe
  & ./scripts/check-service-guest.ps1 -RuntimeDll $RuntimeDll -Wasm $ServiceWasm -Probe $Probe
 }
}finally{Pop-Location}