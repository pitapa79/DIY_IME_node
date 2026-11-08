param([Parameter(Mandatory)][string]$RuntimeDll,[Parameter(Mandatory)][string]$Wasm,[Parameter(Mandatory)][string]$Request)
$ErrorActionPreference='Stop'
# Test-only adapter. Both executable identities are selected by the owning CI script.
if(-not $IsWindows -or -not $env:DIYIME2_ISOLATION_PROBE -or -not $env:DIYIME2_WASM_PROBE){throw 'Isolation probe paths unavailable; refusing launch'}
$output=& $env:DIYIME2_ISOLATION_PROBE wasm $env:DIYIME2_WASM_PROBE $RuntimeDll $Wasm $Request
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
$reply=$output|ConvertFrom-Json -Depth 100
if($reply.kind -ne 'appcontainer-wasm' -or $reply.physicalExit -ne $true -or $reply.profileDeleted -ne $true){throw 'Parent did not confirm Worker completion/cleanup'}
ConvertTo-Json -InputObject $reply.result -Depth 100 -Compress
$global:LASTEXITCODE=0