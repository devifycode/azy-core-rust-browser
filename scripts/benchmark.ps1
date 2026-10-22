param([ValidateSet('before','after')][string]$Label = 'after')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot
$env:AZY_CORE_DATA_DIR = Join-Path $projectRoot ('.cache\benchmark-' + $Label + '-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=0'
$env:TEMP = Join-Path $projectRoot '.cache\tmp'
$env:TMP = $env:TEMP
$env:AZY_BENCH_LABEL = $Label
$binary = if ($Label -eq 'before') { Join-Path $projectRoot '.cache\bench-before\AzyCore.exe' } else { Join-Path $projectRoot 'AzyCore.exe' }
$browserProcess = Start-Process -FilePath $binary -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
$env:AZY_BENCH_PID = [string]$browserProcess.Id
try {
    & node (Join-Path $PSScriptRoot 'performance.cjs')
    if ($LASTEXITCODE -ne 0) { throw 'Performance verification failed' }
    $browserProcess.WaitForExit(5000) | Out-Null
} finally {
    if (-not $browserProcess.HasExited) { Stop-Process -Id $browserProcess.Id }
}
