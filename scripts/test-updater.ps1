$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$testRoot=Join-Path $projectRoot ('.cache\updater-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:AZY_UPDATE_TEST_ROOT=$testRoot
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
try{
 & node (Join-Path $PSScriptRoot 'updater.cjs')
 if($LASTEXITCODE -ne 0){throw 'Updater integration tests failed'}
}finally{
 # Stop only test-owned browser/helper processes. Never touch an installed browser.
 $resolved=[IO.Path]::GetFullPath($testRoot)+'\'
 if(-not $resolved.StartsWith(([IO.Path]::GetFullPath((Join-Path $projectRoot '.cache'))+'\'),[StringComparison]::OrdinalIgnoreCase)){throw 'Test cleanup escaped project cache'}
 Get-Process AzyCore,update-helper -ErrorAction SilentlyContinue | Where-Object {$_.Path -and $_.Path.StartsWith($resolved,[StringComparison]::OrdinalIgnoreCase)} | Stop-Process -ErrorAction SilentlyContinue
}
