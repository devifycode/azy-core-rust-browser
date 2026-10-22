$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$env:AZY_CORE_DATA_DIR=Join-Path $projectRoot ('.cache\browser-update-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force -Path $env:AZY_CORE_DATA_DIR | Out-Null
$history=@(@{title='Portal <img src=x onerror=window.unsafe=1>';url='http://127.0.0.1:3155/portal-one';time=3},@{title='Portal duplicate';url='http://127.0.0.1:3155/portal-one';time=2},@{title='Second portal';url='http://127.0.0.1:3155/portal-two';time=1})
[IO.File]::WriteAllText((Join-Path $env:AZY_CORE_DATA_DIR 'browser.json'),(@{history=$history}|ConvertTo-Json -Depth 10),[Text.UTF8Encoding]::new($false))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
$browserProcess=$null
try{foreach($stage in @('initial','verify')){
 $browserProcess=Start-Process -FilePath (Join-Path $projectRoot 'target\release\AzyCore.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
 $env:AZY_CORE_TEST_PID=[string]$browserProcess.Id
 if($stage -eq 'verify'){& node (Join-Path $PSScriptRoot 'browser-update.cjs') --verify}else{& node (Join-Path $PSScriptRoot 'browser-update.cjs')}
 if($LASTEXITCODE -ne 0){throw 'Browser update integration failed'}
 if(-not $browserProcess.WaitForExit(5000)){throw 'Browser did not exit'}
}}finally{if($browserProcess -and -not $browserProcess.HasExited){Stop-Process -Id $browserProcess.Id}}
