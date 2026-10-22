$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$env:AZY_CORE_DATA_DIR=Join-Path $projectRoot ('.cache\tabs-split-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
$browserProcess=$null
try{foreach($stage in @('initial','verify')){
 $browserProcess=Start-Process -FilePath (Join-Path $projectRoot 'target\release\AzyCore.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
 $env:AZY_CORE_TEST_PID=[string]$browserProcess.Id
 & node (Join-Path $PSScriptRoot 'tabs-split.cjs') $stage
 if($LASTEXITCODE -ne 0){throw 'Tab drag/split integration failed'}
 if(-not $browserProcess.WaitForExit(5000)){throw 'Browser did not exit'}
}}finally{if($browserProcess -and -not $browserProcess.HasExited){Stop-Process -Id $browserProcess.Id}}
