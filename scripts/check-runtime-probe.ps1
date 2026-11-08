$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$origin=Get-Content -LiteralPath (Join-Path $root 'dev/runtime-probe-origin.json') -Raw|ConvertFrom-Json
foreach($entry in $origin.files){
 $path=[IO.Path]::GetFullPath((Join-Path $root $entry.path));if(-not $path.StartsWith($root+[IO.Path]::DirectorySeparatorChar)){throw 'Invalid probe path'}
 $b=[IO.File]::ReadAllBytes($path);$h=[Text.Encoding]::ASCII.GetBytes("blob $($b.Length)`0");$hash=[Convert]::ToHexString([Security.Cryptography.SHA1]::HashData($h+$b)).ToLowerInvariant()
 if($hash -ne $entry.blob){throw "Pinned probe changed: $($entry.path)"}
}
Write-Output 'Runtime probe matches pinned Core source'