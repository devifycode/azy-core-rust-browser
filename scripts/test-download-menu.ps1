$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$env:AZY_CORE_DATA_DIR=Join-Path $projectRoot ('.cache\download-menu-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
$files=Join-Path $env:AZY_CORE_DATA_DIR 'files'
New-Item -ItemType Directory -Force -Path $files,$env:TEMP | Out-Null
$downloads=1..8 | ForEach-Object { @{id=$_;url="http://127.0.0.1:3164/file/$_";path=(Join-Path $files $(if($_ -eq 8){'<img src=x onerror=window.unsafe=1>.txt'}else{"seed-$_.txt"}));status=$(if($_ -eq 7){'failed'}elseif($_ -eq 6){'interrupted'}else{'complete'});time=$_} }
$store=@{settings=@{download_ask=$false;download_directory=$files};downloads=@($downloads)}
[IO.File]::WriteAllText((Join-Path $env:AZY_CORE_DATA_DIR 'browser.json'),($store|ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
$p=$null
try{foreach($stage in @('initial','verify')){
 $p=Start-Process -FilePath (Join-Path $projectRoot 'target\release\AzyCore.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
 $env:AZY_CORE_TEST_PID=[string]$p.Id
 & node (Join-Path $PSScriptRoot 'download-menu.cjs') $stage
 if($LASTEXITCODE -ne 0){throw 'Download menu tests failed'}
 if(-not $p.WaitForExit(5000)){throw 'Browser did not exit'}
}}finally{if($p -and -not $p.HasExited){Stop-Process -Id $p.Id}}
