param([string]$Cargo = 'cargo')
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $root
try {
    & ./scripts/check-resource-sdk.ps1
    $metadata = & $Cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
    $target = $metadata.target_directory
    $output = Join-Path $target 'dictionary-artifact'
    [IO.Directory]::CreateDirectory($output) | Out-Null
    & $Cargo build --locked --release --target wasm32-unknown-unknown -p diy-ime2-nodes --lib
    if ($LASTEXITCODE -ne 0) { throw 'Default Wasm control build failed' }
    $default = Join-Path $target 'wasm32-unknown-unknown/release/diy_ime2_nodes.wasm'
    $before = (Get-FileHash -LiteralPath $default -Algorithm SHA256).Hash
    # Separate Cargo targets prevent feature unification from rewriting default output.
    foreach ($role in @('producer', 'consumer')) {
        $roleTarget = Join-Path $target "dictionary-artifact-$role"
        $arguments = @('build', '--locked', '--release', '--target', 'wasm32-unknown-unknown',
            '-p', 'diyime2-dictionary-artifact-guest', '--target-dir', $roleTarget)
        if ($role -eq 'producer') { $arguments += @('--features', 'producer') }
        & $Cargo @arguments
        if ($LASTEXITCODE -ne 0) { throw "Dictionary $role build failed" }
        Copy-Item -LiteralPath (Join-Path $roleTarget 'wasm32-unknown-unknown/release/diyime2_dictionary_artifact_guest.wasm') -Destination (Join-Path $output "$role.wasm") -Force
        & $Cargo clippy --locked --no-deps --target wasm32-unknown-unknown -p diyime2-dictionary-artifact-guest --target-dir $roleTarget $(if ($role -eq 'producer') { '--features'; 'producer' }) -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw "Dictionary $role Wasm clippy failed" }
    }
    if ((Get-FileHash -LiteralPath $default -Algorithm SHA256).Hash -ne $before) { throw 'Companion build changed default module' }
    $receipt = [ordered]@{
        formatVersion = 1
        sdkSourceCommit = '1aa3a9400eed9d8b5ba9f806497d6f97acd87430'
        sdkStatus = 'reviewed-candidate-unmerged'
        nodeKind = 'text.dictionary-exact'
        nodeVersion = 1
        artifactFormat = 'dictionary-exact-index'
        artifactVersion = 1
        defaultModuleSHA256 = $before.ToLowerInvariant()
        producerSHA256 = (Get-FileHash -LiteralPath (Join-Path $output 'producer.wasm') -Algorithm SHA256).Hash.ToLowerInvariant()
        consumerSHA256 = (Get-FileHash -LiteralPath (Join-Path $output 'consumer.wasm') -Algorithm SHA256).Hash.ToLowerInvariant()
        productRouteChanged = $false
    }
    [IO.File]::WriteAllText((Join-Path $output 'build-receipt.json'), ($receipt | ConvertTo-Json) + "`n", [Text.UTF8Encoding]::new($false))
    Write-Output "Built separate dictionary modules: $output"
} finally { Pop-Location }
