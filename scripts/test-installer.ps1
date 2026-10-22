$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
$info=Get-Content -LiteralPath (Join-Path $projectRoot 'BUILD-INFO.json') -Raw | ConvertFrom-Json
$installer=Join-Path $projectRoot ('AzyCore-Setup-'+$info.version+'-x64.exe')
$testRoot=Join-Path $projectRoot ('.cache\installer-test-'+[DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss'))
$installDir=Join-Path $testRoot 'Folder Pilihan Pengguna ü\Azy Core'
$env:TEMP=Join-Path $projectRoot '.cache\tmp';$env:TMP=$env:TEMP
New-Item -ItemType Directory -Force -Path $testRoot,$env:TEMP | Out-Null
$registration='HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\AzyCore'
$appKey='HKCU:\Software\AzyCore'
$menu=Join-Path ([Environment]::GetFolderPath('Programs')) 'Azy Core'
$desktop=Join-Path ([Environment]::GetFolderPath('Desktop')) 'Azy Core.lnk'
if((Test-Path $registration) -or (Test-Path $appKey) -or (Test-Path -LiteralPath $menu)){throw 'An existing Azy Core installation or Start Menu folder was found; refusing to modify it during testing'}
$desktopHash=if(Test-Path -LiteralPath $desktop){(Get-FileHash -LiteralPath $desktop).Hash}else{$null}
$checks=[Collections.Generic.List[string]]::new()
$browserProcess=$null
$oldData=$env:AZY_CORE_DATA_DIR;$oldDebug=$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
function Check([bool]$ok,[string]$description){if(-not $ok){throw $description};$checks.Add($description);Write-Host "PASS $description"}
function Run-Installer {
 $p=Start-Process -FilePath $installer -ArgumentList @('/S',('/D='+$installDir)) -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
 if(-not $p.WaitForExit(30000)){throw 'Installer timeout'}
 if($p.ExitCode -ne 0){throw "Installer returned $($p.ExitCode)"}
}
function Run-Uninstaller {
 $path=Join-Path $installDir 'Uninstall-AzyCore.exe'
 $p=Start-Process -FilePath $path -ArgumentList @('/S',('_?='+$installDir)) -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
 if(-not $p.WaitForExit(30000)){throw 'Uninstaller timeout'}
 if($p.ExitCode -ne 0){throw "Uninstaller returned $($p.ExitCode)"}
}
try {
 Run-Installer
 Check ((Test-Path -LiteralPath (Join-Path $installDir 'AzyCore.exe'))) 'Installer honors custom folder with spaces and Unicode'
 Check (((Get-FileHash -LiteralPath (Join-Path $installDir 'AzyCore.exe')).Hash -eq $info.sha256)) 'Installed executable matches the tested browser SHA-256'
 Check ((Test-Path -LiteralPath (Join-Path $installDir 'AzyCore.FILTER-NOTICE.md')) -and (Test-Path -LiteralPath (Join-Path $installDir 'AzyCore.BUILD-INFO.json'))) 'Installer includes attribution and build metadata'
 $reg=Get-ItemProperty $registration
 Check (($reg.InstallLocation -eq $installDir) -and ($reg.DisplayVersion -eq $info.version) -and (Test-Path -LiteralPath (Join-Path $installDir 'Uninstall-AzyCore.exe'))) 'Per-user uninstall registration points to the chosen folder'
 $shell=New-Object -ComObject WScript.Shell
 $shortcut=$shell.CreateShortcut((Join-Path $menu 'Azy Core.lnk'))
 Check (($shortcut.TargetPath -eq (Join-Path $installDir 'AzyCore.exe'))) 'Start Menu shortcut targets the installed executable'
 $desktopUnchanged=if($desktopHash){(Get-FileHash -LiteralPath $desktop).Hash -eq $desktopHash}else{-not (Test-Path -LiteralPath $desktop)}
 Check $desktopUnchanged 'Unselected desktop option preserves any pre-existing desktop shortcut'
 $profile=Join-Path $installDir 'AzyCore.Data'
 Remove-Item Env:\AZY_CORE_DATA_DIR -ErrorAction SilentlyContinue
 $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=0'
 $browserProcess=Start-Process -FilePath (Join-Path $installDir 'AzyCore.exe') -WorkingDirectory $installDir -WindowStyle Hidden -PassThru
 $env:AZY_CORE_DATA_DIR=$profile
 & node (Join-Path $PSScriptRoot 'installer-smoke.cjs')
 if($LASTEXITCODE -ne 0){throw 'Installed browser smoke test failed'}
 if(-not $browserProcess.WaitForExit(5000)){throw 'Installed browser did not exit'}
 Check (Test-Path -LiteralPath (Join-Path $profile 'browser.json')) 'Installed browser launches and creates its profile inside the selected directory'
 New-Item -ItemType Directory -Force -Path $profile | Out-Null
 [IO.File]::WriteAllText((Join-Path $profile 'keep-profile.txt'),'profile must survive upgrade and uninstall')
 [IO.File]::WriteAllText((Join-Path $installDir 'unrelated-user-file.txt'),'unrelated file must survive')
 Run-Installer
 Check (([IO.File]::ReadAllText((Join-Path $profile 'keep-profile.txt'))) -eq 'profile must survive upgrade and uninstall') 'Reinstall preserves existing browser profile'
 Run-Uninstaller
 Check (-not (Test-Path -LiteralPath (Join-Path $installDir 'AzyCore.exe'))) 'Uninstaller removes the application executable'
 Check ((Test-Path -LiteralPath (Join-Path $profile 'keep-profile.txt')) -and (Test-Path -LiteralPath (Join-Path $installDir 'unrelated-user-file.txt'))) 'Uninstaller preserves browser profile and unrelated files'
 Check ((-not (Test-Path $registration)) -and (-not (Test-Path $appKey)) -and (-not (Test-Path -LiteralPath $menu))) 'Uninstaller removes its registry entries and Start Menu shortcuts'
 $desktopUnchanged=if($desktopHash){(Get-FileHash -LiteralPath $desktop).Hash -eq $desktopHash}else{-not (Test-Path -LiteralPath $desktop)}
 Check $desktopUnchanged 'Uninstaller leaves pre-existing desktop shortcut untouched'
 $result=[ordered]@{passed=$checks.Count;checks=@($checks);installerSha256=(Get-FileHash -LiteralPath $installer).Hash;testDirectory=$testRoot}
 [IO.File]::WriteAllText((Join-Path $testRoot 'results.json'),($result|ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
} finally {
 if($browserProcess -and -not $browserProcess.HasExited){Stop-Process -Id $browserProcess.Id}
 $env:AZY_CORE_DATA_DIR=$oldData;$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=$oldDebug
 if((Test-Path -LiteralPath (Join-Path $installDir 'Uninstall-AzyCore.exe')) -and (Test-Path $appKey) -and ((Get-ItemProperty $appKey).InstallDir -eq $installDir)){Run-Uninstaller}
}
