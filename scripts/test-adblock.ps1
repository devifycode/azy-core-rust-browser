$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot
$env:AZY_CORE_DATA_DIR = Join-Path $projectRoot ('.cache\adblock-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=0 --no-proxy-server --host-resolver-rules="MAP ads.doubleclick.net 127.0.0.1, MAP 00px.net 127.0.0.1"'
$env:TEMP = Join-Path $projectRoot '.cache\tmp'
$env:TMP = $env:TEMP
$browserProcess = Start-Process -FilePath (Join-Path $projectRoot 'AzyCore.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
try {
    & node (Join-Path $PSScriptRoot 'adblock.cjs')
    if ($LASTEXITCODE -ne 0) { throw 'Adblock verification failed' }
    $browserProcess.WaitForExit(5000) | Out-Null
} finally {
    if (-not $browserProcess.HasExited) { Stop-Process -Id $browserProcess.Id }
}
