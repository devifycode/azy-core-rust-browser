$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$env:AZY_CORE_DATA_DIR=Join-Path $projectRoot ('.cache\instance-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
& node (Join-Path $PSScriptRoot 'local-pdf.cjs') prepare
if($LASTEXITCODE -ne 0){throw 'PDF fixture generation failed'}
& node (Join-Path $PSScriptRoot 'instance.cjs')
if($LASTEXITCODE -ne 0){throw 'Instance handoff tests failed'}
