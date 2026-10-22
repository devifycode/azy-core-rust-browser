$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$env:AZY_CORE_DATA_DIR=Join-Path $projectRoot ('.cache\local-pdf-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
& node (Join-Path $PSScriptRoot 'local-pdf.cjs') prepare
if($LASTEXITCODE -ne 0){throw 'PDF fixture generation failed'}
$p=$null
try{foreach($stage in @('initial','restore','argument')){
 $launch=@{FilePath=(Join-Path $projectRoot 'target\release\AzyCore.exe');WorkingDirectory=$projectRoot;WindowStyle='Hidden';PassThru=$true}
 if($stage -eq 'argument'){$launch.ArgumentList='"'+(Join-Path $env:AZY_CORE_DATA_DIR 'Laporan #1 ü.pdf')+'"'}
 $p=Start-Process @launch
 $env:AZY_CORE_TEST_PID=[string]$p.Id
 & node (Join-Path $PSScriptRoot 'local-pdf.cjs') $stage
 if($LASTEXITCODE -ne 0){throw 'Local PDF tests failed'}
 if(-not $p.WaitForExit(5000)){throw 'Browser did not exit'}
}}finally{if($p -and -not $p.HasExited){Stop-Process -Id $p.Id}}
