$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot
$testFolder = 'smoke-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')
$env:AZY_CORE_DATA_DIR = Join-Path $projectRoot ".cache\$testFolder"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=0'
$env:TEMP = Join-Path $projectRoot '.cache\tmp'
$env:TMP = $env:TEMP
$browserProcess = Start-Process -FilePath (Join-Path $projectRoot 'AzyCore.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
try {
    & node (Join-Path $PSScriptRoot 'smoke.cjs')
    if ($LASTEXITCODE -ne 0) { throw 'Browser integration tests failed' }
    $browserProcess.WaitForExit(5000) | Out-Null
} finally {
    if (-not $browserProcess.HasExited) { Stop-Process -Id $browserProcess.Id }
    Remove-Item Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
    Remove-Item Env:\AZY_CORE_DATA_DIR
}
