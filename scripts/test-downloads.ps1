$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$env:AZY_CORE_DATA_DIR=Join-Path $projectRoot ('.cache\downloads-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
New-Item -ItemType Directory -Force -Path $env:AZY_CORE_DATA_DIR,$env:TEMP | Out-Null
$p=$null
try{foreach($stage in @('initial','verify')){
 $p=Start-Process -FilePath (Join-Path $projectRoot 'target\release\AzyCore.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
 $env:AZY_CORE_TEST_PID=[string]$p.Id
 & node (Join-Path $PSScriptRoot 'downloads.cjs') $stage
 if($LASTEXITCODE -ne 0){throw 'Download integration failed'}
 if(-not $p.WaitForExit(5000)){throw 'Browser did not exit'}
}}finally{if($p -and -not $p.HasExited){Stop-Process -Id $p.Id}}
