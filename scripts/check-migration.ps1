$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$manifest = Get-Content -LiteralPath (Join-Path $root 'migration-provenance.json') -Raw | ConvertFrom-Json
if ($manifest.sourceRepository -ne 'pitapa79/DIY_IME2' -or $manifest.sourceCommit -notmatch '^[a-f0-9]{40}$') { throw 'Invalid origin identity' }
$seen = @{}
foreach ($entry in $manifest.files) {
 $path = [IO.Path]::GetFullPath((Join-Path $root $entry.path))
 if (-not $path.StartsWith($root + [IO.Path]::DirectorySeparatorChar) -or $seen.ContainsKey($entry.path)) { throw 'Invalid or duplicate provenance path' }
 $seen[$entry.path] = $true
 $bytes = [IO.File]::ReadAllBytes($path)
 $header = [Text.Encoding]::ASCII.GetBytes("blob $($bytes.Length)`0")
 $hash = [Convert]::ToHexString([Security.Cryptography.SHA1]::HashData($header + $bytes)).ToLowerInvariant()
 if ($hash -ne $entry.blob) { throw "Imported file changed: $($entry.path)" }
 if ($entry.change -eq 'copied' -and $hash -ne $entry.sourceBlob) { throw "Unchanged origin mismatch: $($entry.path)" }
 if ($entry.path.StartsWith('sdk/') -and $hash -ne $entry.sourceBlob) { throw "SDK must match pinned origin: $($entry.path)" }
 if ($entry.path.StartsWith('tests/contracts/') -and $entry.path.EndsWith('.json') -and $hash -ne $entry.sourceBlob) { throw "Formal fixture changed: $($entry.path)" }
}
foreach ($file in Get-ChildItem -LiteralPath (Join-Path $root 'sdk/runtime') -File -Recurse) {
 $relative = [IO.Path]::GetRelativePath($root, $file.FullName).Replace('\', '/')
 if (-not $seen.ContainsKey($relative)) { throw "Untracked SDK source: $relative" }
}
Write-Output "Verified $($seen.Count) imported files and immutable SDK/contract fixtures"