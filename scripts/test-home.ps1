$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot
$env:AZY_CORE_DATA_DIR=Join-Path $projectRoot ('.cache\home-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
$browserProcess=$null
try{
 foreach($stage in @('initial','verify')){
  $browserProcess=Start-Process -FilePath (Join-Path $projectRoot 'target\release\AzyCore.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
  if($stage -eq 'verify'){& node (Join-Path $PSScriptRoot 'home-customize.cjs') --verify}else{& node (Join-Path $PSScriptRoot 'home-customize.cjs')}
  if($LASTEXITCODE -ne 0){throw 'Home customization tests failed'}
  if(-not $browserProcess.WaitForExit(5000)){throw 'Test browser did not exit'}
 }
}finally{
 if($browserProcess -and -not $browserProcess.HasExited){Stop-Process -Id $browserProcess.Id}
 Remove-Item Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
 Remove-Item Env:\AZY_CORE_DATA_DIR
}
