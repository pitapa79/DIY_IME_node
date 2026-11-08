param([string]$Cargo='cargo',[string]$Node='node',[string]$RuntimeDll,[string]$Probe,[string]$Worker)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$probeCommit='5046a6d90f5e9520314d4e27ee26a43cdbead136'
$archiveSHA='c63585bac0bd873384cc3f00cf16fb10d1d4ca50a939b051641e5b94576b4d14'
Push-Location $root
try {
    $metadata=& $Cargo metadata --locked --no-deps --format-version 1|ConvertFrom-Json
    if($LASTEXITCODE -ne 0){throw 'Dictionary integration metadata failed'}
    $target=$metadata.target_directory
    if(-not $RuntimeDll){$RuntimeDll=Join-Path $root 'target/tools/wasmtime50/wasmtime-v50.0.1-x86_64-windows-c-api/lib/wasmtime.dll'}
    if((-not $Probe) -ne (-not $Worker)){throw 'Supply both Probe and Worker, or neither'}
    if(-not $Probe){
        $tools=Join-Path $target 'tools'
        [IO.Directory]::CreateDirectory($tools)|Out-Null
        $archive=Join-Path $tools "public-probe-$probeCommit.zip"
        if(-not(Test-Path -LiteralPath $archive)){Invoke-WebRequest -Uri "https://codeload.github.com/pitapa79/DIY_IME/zip/$probeCommit" -OutFile $archive}
        if((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $archiveSHA){throw 'Public probe archive hash mismatch'}
        $sourceDirectory=Join-Path $tools 'public-probe-source'
        Expand-Archive -LiteralPath $archive -DestinationPath $sourceDirectory -Force
        $manifest=Join-Path $sourceDirectory "DIY_IME-$probeCommit/Cargo.toml"
        $probeTarget=Join-Path $target 'public-probe-native'
        & $Cargo build --locked --manifest-path $manifest --target-dir $probeTarget --features resource-integration-probe --example resource_integration_probe --bin diy-ime2-wasm-worker
        if($LASTEXITCODE -ne 0){throw 'Pinned public broker probe build failed'}
        $Probe=Join-Path $probeTarget 'debug/examples/resource_integration_probe.exe'
        $Worker=Join-Path $probeTarget 'debug/diy-ime2-wasm-worker.exe'
    }
    $output=Join-Path $target 'dictionary-integration'
    & $Node (Join-Path $root 'scripts/check-dictionary-artifact.mjs') $Probe $Worker $RuntimeDll (Join-Path $target 'dictionary-artifact/producer.wasm') (Join-Path $target 'dictionary-artifact/consumer.wasm') $output
    if($LASTEXITCODE -ne 0){throw 'Actual dictionary resource integration failed'}
    [IO.File]::WriteAllText((Join-Path $output 'probe-source.json'),(@{sourceRepository='pitapa79/DIY_IME';sourceCommit=$probeCommit;archiveSHA256=$archiveSHA;status='ci-verified-candidate';sourcePullRequest=79;externalProbeOverride=[bool]$PSBoundParameters.ContainsKey('Probe')}|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
} finally {Pop-Location}
