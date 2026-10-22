param([string]$ExtensionFolder='E:\9. Azy Creative\TEST\glb-grabber')
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$env:AZY_CORE_DATA_DIR=Join-Path $projectRoot ('.cache\grabber-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
$browserProcess=$null
try {foreach($stage in @('initial','verify')){
 if($stage -eq 'verify'){
  $storeFile=Join-Path $env:AZY_CORE_DATA_DIR 'browser.json'
  $store=Get-Content -Raw -LiteralPath $storeFile | ConvertFrom-Json
  foreach($entry in $store.extensions.PSObject.Properties){foreach($field in @('popup','icon','action_title')){$entry.Value.PSObject.Properties.Remove($field)}}
  [IO.File]::WriteAllText($storeFile,($store | ConvertTo-Json -Depth 30),[Text.UTF8Encoding]::new($false))
 }
 $browserProcess=Start-Process -FilePath (Join-Path $projectRoot 'target\release\AzyCore.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
 if($stage -eq 'verify'){& node (Join-Path $PSScriptRoot 'grabber.cjs') $ExtensionFolder --verify}else{& node (Join-Path $PSScriptRoot 'grabber.cjs') $ExtensionFolder}
 if($LASTEXITCODE -ne 0){throw 'GLB extension test failed'}
 if(-not $browserProcess.WaitForExit(5000)){throw 'Browser did not exit'}
}} finally {if($browserProcess -and -not $browserProcess.HasExited){Stop-Process -Id $browserProcess.Id}}
