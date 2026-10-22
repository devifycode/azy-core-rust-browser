param([ValidateSet('build','check','test','fmt','clippy')][string]$Action = 'build', [switch]$Release, [switch]$SkipPublish)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot
$env:CARGO_HOME = Join-Path $projectRoot '.cache\cargo'
$env:RUSTUP_HOME = Join-Path $projectRoot '.cache\rustup'
$env:CARGO_TARGET_DIR = Join-Path $projectRoot 'target'
$env:RUSTFLAGS = '-C target-feature=+crt-static'
$env:TEMP = Join-Path $projectRoot '.cache\tmp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
$compilerBin = 'C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.41.34120\bin\Hostx64\x64'
$resourceBin = Join-Path $projectRoot '.cache\sdk-tools\c\bin\10.0.26100.0\x64'
$crtDirectory = Get-ChildItem (Join-Path $projectRoot '.cache\msvc-crt\Contents\VC\Tools\MSVC') -Directory | Select-Object -First 1
$env:LIB = "$($crtDirectory.FullName)\lib\x64;$projectRoot\.cache\sdk-x64\c\um\x64;$projectRoot\.cache\sdk-x64\c\ucrt\x64"
$env:PATH = "$env:CARGO_HOME\bin;$compilerBin;$resourceBin;$env:PATH"
$env:RC = Join-Path $resourceBin 'rc.exe'
$env:INCLUDE = "$($crtDirectory.FullName)\include;$projectRoot\.cache\sdk-tools\c\Include\10.0.26100.0\ucrt;$projectRoot\.cache\sdk-tools\c\Include\10.0.26100.0\um;$projectRoot\.cache\sdk-tools\c\Include\10.0.26100.0\shared"
$cargoArguments = @($Action, '--offline')
if ($Action -eq 'fmt') { $cargoArguments = @('fmt', '--all') }
if ($Release -and $Action -in @('build','check','test','clippy')) { $cargoArguments += '--release' }
& "$env:CARGO_HOME\bin\cargo.exe" @cargoArguments
if ($LASTEXITCODE -ne 0) { throw "cargo $Action failed ($LASTEXITCODE)" }
if ($Action -eq 'build' -and $Release -and -not $SkipPublish) {
    Copy-Item -LiteralPath (Join-Path $projectRoot 'target\release\AzyCore.exe') -Destination (Join-Path $projectRoot 'AzyCore.exe') -Force
    Write-Host "Ready: $projectRoot\AzyCore.exe"
}
