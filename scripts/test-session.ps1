$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot
$env:AZY_CORE_DATA_DIR = Join-Path $projectRoot ('.cache\session-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force -Path $env:AZY_CORE_DATA_DIR | Out-Null
@{ session=@('http://127.0.0.1:3139/first','http://127.0.0.1:3139/second','azy://newtab');bookmarks=@(@{title='Saved bookmark';url='https://example.com';time=1});settings=@{restore=$true} } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $env:AZY_CORE_DATA_DIR 'browser.json') -Encoding UTF8
# Windows PowerShell writes a BOM; the application JSON input intentionally uses UTF-8 without it.
$sessionFile = Join-Path $env:AZY_CORE_DATA_DIR 'browser.json'
$json = Get-Content -LiteralPath $sessionFile -Raw
[IO.File]::WriteAllText($sessionFile,$json,[Text.UTF8Encoding]::new($false))
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=0'
$env:TEMP = Join-Path $projectRoot '.cache\tmp'
$env:TMP = $env:TEMP
$browserProcess = Start-Process -FilePath (Join-Path $projectRoot 'AzyCore.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
try {
    & node (Join-Path $PSScriptRoot 'session.cjs')
    if ($LASTEXITCODE -ne 0) { throw 'Deferred session verification failed' }
    $browserProcess.WaitForExit(5000) | Out-Null
} finally {
    if (-not $browserProcess.HasExited) { Stop-Process -Id $browserProcess.Id }
}
