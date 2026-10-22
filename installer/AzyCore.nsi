Unicode True
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"
!include "WinVer.nsh"

!ifndef PROJECT_DIR
  !error "PROJECT_DIR must point to the project root"
!endif
Name "Azy Core"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\Azy Core"
InstallDirRegKey HKCU "Software\AzyCore" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetDatablockOptimize on
VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey "ProductName" "Azy Core Setup"
VIAddVersionKey "FileDescription" "Azy Core installer for Windows x64"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"
VIAddVersionKey "LegalCopyright" "Azy Core"

!define MUI_ICON "${APP_ICON}"
!define MUI_UNICON "${APP_ICON}"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "Instal Azy Core"
!define MUI_WELCOMEPAGE_TEXT "Wizard ini akan memasang Azy Core ${APP_VERSION} untuk akun Windows Anda.$\r$\n$\r$\nPilih folder instalasi pada halaman berikutnya. Gunakan folder yang dapat Anda tulis.$\r$\n$\r$\nMemerlukan Windows 10/11 x64 dan Microsoft Edge WebView2 Runtime."
!insertmacro MUI_PAGE_WELCOME
!define MUI_DIRECTORYPAGE_TEXT_TOP "Pilih lokasi pemasangan Azy Core. Klik Browse untuk mengganti folder. Profil browser akan disimpan di folder AzyCore.Data di dalam lokasi ini."
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\AzyCore.exe"
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!define MUI_FINISHPAGE_RUN_TEXT "Buka Azy Core"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH
!insertmacro MUI_LANGUAGE "Indonesian"

Function .onInit
  SetShellVarContext current
  SetRegView 64
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "Azy Core memerlukan Windows x64." /SD IDOK
    SetErrorLevel 1
    Quit
  ${EndIf}
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_OK|MB_ICONSTOP "Azy Core memerlukan Windows 10 atau lebih baru." /SD IDOK
    SetErrorLevel 1
    Quit
  ${EndIf}
FunctionEnd

Function .onVerifyInstDir
  ${GetRoot} "$INSTDIR" $0
  ${If} $INSTDIR == $0
    Abort
  ${EndIf}
  ${If} $INSTDIR == "$WINDIR"
    Abort
  ${EndIf}
FunctionEnd

Section "Azy Core dan shortcut Start Menu" MainSection
  SectionIn RO
  SetShellVarContext current
  SetRegView 64
  ClearErrors
  SetOutPath "$INSTDIR"
  IfErrors install_directory_error
  GetTempFileName $0 "$INSTDIR"
  IfErrors install_directory_error
  Delete "$0"
  IfFileExists "$INSTDIR\AzyCore.exe" 0 install_files
  ClearErrors
  FileOpen $0 "$INSTDIR\AzyCore.exe" a
  IfErrors install_locked
  FileClose $0
install_files:
  ClearErrors
  SetOverwrite on
  File "${PROJECT_DIR}\AzyCore.exe"
  File /oname=AzyCore.BUILD-INFO.json "${PROJECT_DIR}\BUILD-INFO.json"
  File /oname=AzyCore.README.md "${PROJECT_DIR}\README.md"
  File /oname=AzyCore.FILTER-NOTICE.md "${PROJECT_DIR}\assets\filters\NOTICE.md"
  IfErrors install_files_error
  WriteUninstaller "$INSTDIR\Uninstall-AzyCore.exe"
  IfErrors install_files_error
  CreateDirectory "$SMPROGRAMS\Azy Core"
  CreateShortcut "$SMPROGRAMS\Azy Core\Azy Core.lnk" "$INSTDIR\AzyCore.exe" "" "$INSTDIR\AzyCore.exe" 0
  CreateShortcut "$SMPROGRAMS\Azy Core\Uninstall Azy Core.lnk" "$INSTDIR\Uninstall-AzyCore.exe"
  WriteRegStr HKCU "Software\AzyCore" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AzyCore" "DisplayName" "Azy Core"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AzyCore" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AzyCore" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AzyCore" "DisplayIcon" "$INSTDIR\AzyCore.exe,0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AzyCore" "UninstallString" '$\"$INSTDIR\Uninstall-AzyCore.exe$\"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AzyCore" "QuietUninstallString" '$\"$INSTDIR\Uninstall-AzyCore.exe$\" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AzyCore" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AzyCore" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/M=AzyCore*.* /S=0K" $0 $1 $2
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AzyCore" "EstimatedSize" $0
  IfErrors install_files_error
  Goto install_done
install_directory_error:
  MessageBox MB_OK|MB_ICONSTOP "Folder ini tidak dapat ditulis. Pilih folder lain untuk akun Windows Anda." /SD IDOK
  SetErrorLevel 1
  Abort
install_locked:
  MessageBox MB_OK|MB_ICONSTOP "Tutup Azy Core dari folder tujuan, kemudian jalankan installer kembali." /SD IDOK
  SetErrorLevel 1
  Abort
install_files_error:
  MessageBox MB_OK|MB_ICONSTOP "Pemasangan tidak selesai. Periksa izin folder dan ruang penyimpanan, lalu coba kembali." /SD IDOK
  SetErrorLevel 1
  Abort
install_done:
SectionEnd

Section /o "Shortcut desktop" DesktopSection
  SetShellVarContext current
  CreateShortcut "$DESKTOP\Azy Core.lnk" "$INSTDIR\AzyCore.exe" "" "$INSTDIR\AzyCore.exe" 0
  WriteRegDWORD HKCU "Software\AzyCore" "DesktopShortcut" 1
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${MainSection} "Aplikasi browser dan shortcut pada Start Menu."
  !insertmacro MUI_DESCRIPTION_TEXT ${DesktopSection} "Tambahkan shortcut Azy Core pada desktop akun ini."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  SetShellVarContext current
  SetRegView 64
  ClearErrors
  Delete "$INSTDIR\AzyCore.exe"
  ${If} ${Errors}
    MessageBox MB_OK|MB_ICONSTOP "Tutup Azy Core sebelum menghapus aplikasi." /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
  Delete "$INSTDIR\AzyCore.BUILD-INFO.json"
  Delete "$INSTDIR\AzyCore.README.md"
  Delete "$INSTDIR\AzyCore.FILTER-NOTICE.md"
  ReadRegStr $0 HKCU "Software\AzyCore" "InstallDir"
  ${If} $0 == $INSTDIR
    Delete "$SMPROGRAMS\Azy Core\Azy Core.lnk"
    Delete "$SMPROGRAMS\Azy Core\Uninstall Azy Core.lnk"
    RMDir "$SMPROGRAMS\Azy Core"
    StrCpy $1 0
    ReadRegDWORD $1 HKCU "Software\AzyCore" "DesktopShortcut"
    ${If} $1 == 1
      Delete "$DESKTOP\Azy Core.lnk"
    ${EndIf}
    DeleteRegKey HKCU "Software\AzyCore"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\AzyCore"
  ${EndIf}
  Delete "$INSTDIR\Uninstall-AzyCore.exe"
  ; Never recursively delete the user's selected folder or browser profile.
  RMDir "$INSTDIR"
SectionEnd
