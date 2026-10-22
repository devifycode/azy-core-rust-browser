# Build an older debug client with the real updater; debug-only local feed support
# is deliberately absent from production release builds.
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$manifest=Join-Path $projectRoot 'Cargo.toml';$lock=Join-Path $projectRoot 'Cargo.lock'
$original=[IO.File]::ReadAllText($manifest);$originalLock=[IO.File]::ReadAllText($lock)
try{
 [IO.File]::WriteAllText($manifest,($original -replace 'version = "0.1.5"','version = "0.1.4"'),[Text.UTF8Encoding]::new($false))
 & (Join-Path $PSScriptRoot 'build.ps1') -Action build
}finally{
 [IO.File]::WriteAllText($manifest,$original,[Text.UTF8Encoding]::new($false))
 [IO.File]::WriteAllText($lock,$originalLock,[Text.UTF8Encoding]::new($false))
}
