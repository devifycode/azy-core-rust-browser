$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot
$env:AZY_CORE_DATA_DIR=Join-Path $projectRoot ('.cache\favicon-test-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
$browserProcess=Start-Process -FilePath (Join-Path $projectRoot 'target\release\AzyCore.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
try{& node (Join-Path $PSScriptRoot 'favicon.cjs');if($LASTEXITCODE -ne 0){throw 'Favicon tests failed'};$browserProcess.WaitForExit(5000) | Out-Null}
finally{if(-not $browserProcess.HasExited){Stop-Process -Id $browserProcess.Id};Remove-Item Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS;Remove-Item Env:\AZY_CORE_DATA_DIR}
