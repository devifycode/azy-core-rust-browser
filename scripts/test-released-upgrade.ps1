param([string]$FromVersion='0.1.5')
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$env:AZY_UPGRADE_TEST_ROOT=Join-Path $projectRoot ('.cache\released-upgrade-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:AZY_UPGRADE_FROM=$FromVersion
$env:AZY_UPGRADE_TO=(Get-Content (Join-Path $projectRoot 'BUILD-INFO.json') -Raw | ConvertFrom-Json).version
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
try{
 & node (Join-Path $PSScriptRoot 'released-upgrade.cjs')
 if($LASTEXITCODE -ne 0){throw 'Published-release upgrade failed'}
}finally{
 $resolved=[IO.Path]::GetFullPath($env:AZY_UPGRADE_TEST_ROOT)+'\'
 if(-not $resolved.StartsWith(([IO.Path]::GetFullPath((Join-Path $projectRoot '.cache'))+'\'),[StringComparison]::OrdinalIgnoreCase)){throw 'Cleanup escaped test cache'}
 Get-Process AzyCore,update-helper -ErrorAction SilentlyContinue | Where-Object {$_.Path -and $_.Path.StartsWith($resolved,[StringComparison]::OrdinalIgnoreCase)} | Stop-Process -ErrorAction SilentlyContinue
}
