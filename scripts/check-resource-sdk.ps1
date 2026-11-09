$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$manifest = Get-Content -LiteralPath (Join-Path $root 'sdk/wasm-resource-sdk.provenance.json') -Raw | ConvertFrom-Json
if ($manifest.formatVersion -ne 1 -or $manifest.sourceRepository -ne 'pitapa79/DIY_IME' -or
    $manifest.sourceCommit -ne '1aa3a9400eed9d8b5ba9f806497d6f97acd87430' -or
    $manifest.status -ne 'reviewed-candidate-unmerged' -or $manifest.sourcePullRequest -ne 69) {
    throw 'Invalid reviewed candidate SDK identity'
}
$seen = @{}
foreach ($entry in $manifest.files) {
    $relative = [string]$entry.path
    if ($relative -cnotmatch '^sdk/wasm-resource-sdk/(Cargo\.toml|src/[a-z_]+\.rs)$' -or $seen.ContainsKey($relative)) {
        throw 'Invalid or duplicate resource SDK path'
    }
    if ($entry.sourcePath -cne $relative.Replace('sdk/wasm-resource-sdk/', 'crates/wasm-resource-sdk/')) {
        throw 'Resource SDK source path mismatch'
    }
    $seen[$relative] = $true
    $bytes = [IO.File]::ReadAllBytes((Join-Path $root $relative))
    $header = [Text.Encoding]::ASCII.GetBytes("blob $($bytes.Length)`0")
    $hash = [Convert]::ToHexString([Security.Cryptography.SHA1]::HashData($header + $bytes)).ToLowerInvariant()
    if ($hash -cne $entry.blob -or $hash -cne $entry.sourceBlob) { throw "Resource SDK bytes changed: $relative" }
}
if ($seen.Count -ne 4) { throw 'Incomplete resource SDK receipt' }
foreach ($file in Get-ChildItem -LiteralPath (Join-Path $root 'sdk/wasm-resource-sdk') -File -Recurse) {
    $relative = [IO.Path]::GetRelativePath($root, $file.FullName).Replace('\', '/')
    if (-not $seen.ContainsKey($relative)) { throw "Untracked resource SDK source: $relative" }
}
Write-Output "Verified $($seen.Count) byte-exact resource SDK files (unmerged public PR #69 candidate)"
