param([string]$NsisPath)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
$stage=Join-Path $projectRoot '.cache\installer-build'
New-Item -ItemType Directory -Force -Path $stage,$env:TEMP | Out-Null
$info=Get-Content -LiteralPath (Join-Path $projectRoot 'BUILD-INFO.json') -Raw | ConvertFrom-Json
$executable=Join-Path $projectRoot 'AzyCore.exe'
if((Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash -ne $info.sha256){throw 'AzyCore.exe does not match BUILD-INFO.json'}
if(-not $NsisPath){$NsisPath=Join-Path $projectRoot '.cache\nsis\nsis-3.13\makensis.exe'}
if(-not (Test-Path -LiteralPath $NsisPath)){throw 'NSIS compiler not found. Supply -NsisPath or extract NSIS 3.13 portable into .cache/nsis.'}
Add-Type -AssemblyName System.Drawing
$icon=[Drawing.Icon]::ExtractAssociatedIcon($executable)
$iconPath=Join-Path $stage 'azy-core.ico'
$stream=[IO.File]::Create($iconPath)
try{$icon.Save($stream)}finally{$stream.Dispose();$icon.Dispose()}
$output=Join-Path $projectRoot ('AzyCore-Setup-'+$info.version+'-x64.exe')
& $NsisPath '/V2' "/DPROJECT_DIR=$projectRoot" "/DAPP_VERSION=$($info.version)" "/DAPP_ICON=$iconPath" "/DOUTPUT_FILE=$output" (Join-Path $projectRoot 'installer\AzyCore.nsi')
if($LASTEXITCODE -ne 0){throw "NSIS failed ($LASTEXITCODE)"}
$result=[ordered]@{version=$info.version;installer=[IO.Path]::GetFileName($output);bytes=(Get-Item -LiteralPath $output).Length;sha256=(Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash;applicationSha256=$info.sha256;compiler='NSIS 3.13';scope='Current user; custom writable install directory';profile='AzyCore.Data inside selected directory; preserved on uninstall'}
[IO.File]::WriteAllText((Join-Path $stage 'installer-info.json'),($result|ConvertTo-Json)+[Environment]::NewLine,[Text.UTF8Encoding]::new($false))
$result|ConvertTo-Json
