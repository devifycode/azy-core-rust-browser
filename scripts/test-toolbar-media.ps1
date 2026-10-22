$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$env:AZY_CORE_DATA_DIR=Join-Path $projectRoot ('.cache\toolbar-media-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
$files=Join-Path $env:AZY_CORE_DATA_DIR 'Downloads';New-Item -ItemType Directory -Force -Path $files | Out-Null
$store=@{settings=@{download_ask=$false;download_directory=$files;dark=$true};downloads=@(@{id=1;url='https://example.com/legacy';path=(Join-Path $files 'legacy.txt');status='complete';time=1})}
[IO.File]::WriteAllText((Join-Path $env:AZY_CORE_DATA_DIR 'browser.json'),($store|ConvertTo-Json -Depth 7),[Text.UTF8Encoding]::new($false))
$p=$null
try{foreach($stage in @('initial','restart','seen')){
 $p=Start-Process -FilePath (Join-Path $projectRoot 'target\release\AzyCore.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
 $env:AZY_CORE_TEST_PID=[string]$p.Id
 & node (Join-Path $PSScriptRoot 'toolbar-media.cjs') $stage
 if($LASTEXITCODE -ne 0){throw 'Toolbar/media integration failed'}
 if(-not $p.WaitForExit(5000)){throw 'Test browser did not close'}
 Start-Sleep -Milliseconds 1000
}}finally{if($p -and -not $p.HasExited){Stop-Process -Id $p.Id}}
