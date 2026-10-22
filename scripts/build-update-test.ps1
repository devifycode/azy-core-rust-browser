# Build an older debug client with the real updater; debug-only local feed support
# is deliberately absent from production release builds.
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$manifest=Join-Path $projectRoot 'Cargo.toml';$lock=Join-Path $projectRoot 'Cargo.lock'
$original=[IO.File]::ReadAllText($manifest);$originalLock=[IO.File]::ReadAllText($lock)
$current=[version]([regex]::Match($original,'(?m)^version = "([^"]+)"').Groups[1].Value)
if($current.Build -lt 1){throw 'Choose an older test version before testing a release with patch version zero'}
$older='{0}.{1}.{2}' -f $current.Major,$current.Minor,($current.Build-1)
try{
 [IO.File]::WriteAllText($manifest,($original -replace '(?m)^version = "[^"]+"',('version = "'+$older+'"')),[Text.UTF8Encoding]::new($false))
 & (Join-Path $PSScriptRoot 'build.ps1') -Action build
}finally{
 [IO.File]::WriteAllText($manifest,$original,[Text.UTF8Encoding]::new($false))
 [IO.File]::WriteAllText($lock,$originalLock,[Text.UTF8Encoding]::new($false))
}
