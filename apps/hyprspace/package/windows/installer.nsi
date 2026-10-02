; The HyprSpace installer: per user, no admin, into %LOCALAPPDATA%\HyprSpace.
;
; It wears the Tauri app's identity on purpose (docs/adr/0010-installers.md): the same
; product name, publisher, registry keys, install folder and shortcut, so it installs over the
; Tauri app in place and Apps keeps one HyprSpace entry. The Tauri app's updater runs it as
;   HyprSpace_<version>_x64-setup.exe /P /R /UPDATE /ARGS
; (tauri-plugin-updater 2.10, passive mode) and so does the GPUI app's own updater. Flags:
;   /S       silent, NSIS's own                /P       passive: a progress bar, no questions
;   /R       start the app when done           /UPDATE  an updater runs us: wait for the app to
;   /ARGS .. arguments for that start                   exit, keep the user's shortcuts as they are
;   /NS      no shortcuts                      /D=dir   install somewhere else (must come last)
;
; Built by scripts/package-windows.ps1, which passes VERSION, BINARY, ICON and OUTFILE. A test
; build can also pass PRODUCTNAME, MANUFACTURER and BUNDLEID so it never touches a real install.

Unicode true
ManifestDPIAware true
ManifestDPIAwareness PerMonitorV2
SetCompressor /SOLID lzma
RequestExecutionLevel user

!include MUI2.nsh
!include FileFunc.nsh
!include LogicLib.nsh
!include x64.nsh
!include "Win\COM.nsh"
!include "Win\Propkey.nsh"

!macro REQUIRE name
  !ifndef ${name}
    !error "pass /D${name}= (scripts/package-windows.ps1 does)"
  !endif
!macroend
!insertmacro REQUIRE VERSION
!insertmacro REQUIRE BINARY
!insertmacro REQUIRE ICON
!insertmacro REQUIRE OUTFILE
!ifndef PRODUCTNAME
  !define PRODUCTNAME "HyprSpace"
!endif
; the Tauri bundler took the publisher from the bundle id, com.hyprspace.app
!ifndef MANUFACTURER
  !define MANUFACTURER "hyprspace"
!endif
!ifndef BUNDLEID
  !define BUNDLEID "com.hyprspace.app"
!endif

!define MAINBINARY "hyprspace.exe"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}"
!define MANUKEY "Software\${MANUFACTURER}"
!define MANUPRODUCTKEY "${MANUKEY}\${PRODUCTNAME}"
!define TASKBAR "$APPDATA\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar"

!include "utils.nsh"

Name "${PRODUCTNAME}"
OutFile "${OUTFILE}"
; .onInit picks the real folder; NSIS only appends the product name to what the folder page picks
!define PLACEHOLDER "placeholder\${PRODUCTNAME}"
InstallDir "${PLACEHOLDER}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${PRODUCTNAME}"
VIAddVersionKey "FileDescription" "${PRODUCTNAME}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" ""

Var PassiveMode
Var UpdateMode
Var NoShortcutMode
; the binary the last install left, from the registry: hyprspace-tauri.exe for the Tauri app
Var OldBinary

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"

!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_FUNCTION RunApp
!define MUI_FINISHPAGE_SHOWREADME
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Create a desktop shortcut"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION DesktopShortcut
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_FINISH

!define MUI_PAGE_CUSTOMFUNCTION_PRE un.SkipIfPassive
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; Waits until $INSTDIR\$R1 no longer runs. A running .exe can't be opened for writing, which is
; the test. An updater quits right after starting us, so in update mode it gets 10 seconds to go.
; After that, or straight away otherwise, the copy running from this folder is asked to close
; (its window gets WM_CLOSE, so it ends its sessions the normal way) and killed 5 seconds later
; if it hasn't. Only that copy: the match is on its full path, never the process name, so another
; install of the app is left alone.
!macro WAITFOR un
Function ${un}WaitFor
  StrCpy $R2 0
  StrCpy $R4 0
  loop:
    IfFileExists "$INSTDIR\$R1" 0 done
    ClearErrors
    FileOpen $R3 "$INSTDIR\$R1" a
    IfErrors 0 free
    IntOp $R2 $R2 + 1
    ${If} $UpdateMode = 1
    ${AndIf} $R2 <= 50
      Sleep 200
      Goto loop
    ${EndIf}
    ${If} $R4 = 0
      StrCpy $R4 1
      ${IfNot} ${Silent}
      ${AndIf} $PassiveMode <> 1
        MessageBox MB_OKCANCEL|MB_ICONINFORMATION "${PRODUCTNAME} is running. Close it and continue?" IDOK close
        Abort "${PRODUCTNAME} is still running."
      ${EndIf}
      close:
      DetailPrint "Closing ${PRODUCTNAME}"
      ; the installer is a 32-bit process, and a 32-bit PowerShell can't read a 64-bit process's
      ; path, so reach the 64-bit one through Sysnative
      StrCpy $R5 "powershell.exe"
      ${If} ${RunningX64}
        StrCpy $R5 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
      ${EndIf}
      nsExec::Exec `"$R5" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$p = @(Get-Process | Where-Object { $$_.Path -eq '$INSTDIR\$R1' }); $$p | ForEach-Object { [void]$$_.CloseMainWindow() }; $$p | ForEach-Object { if (-not $$_.WaitForExit(5000)) { $$_.Kill() } }"`
      Pop $R3
      StrCpy $R2 0
      Goto loop
    ${EndIf}
    ${If} $R2 <= 25
      Sleep 200
      Goto loop
    ${EndIf}
    Abort "Couldn't close ${PRODUCTNAME}. Close it and run the installer again."
  free:
    FileClose $R3
  done:
FunctionEnd
!macroend
!insertmacro WAITFOR ""
!insertmacro WAITFOR "un."

Function .onInit
  ${GetOptions} $CMDLINE "/P" $R0
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
  ${EndIf}
  ${GetOptions} $CMDLINE "/UPDATE" $R0
  ${IfNot} ${Errors}
    StrCpy $UpdateMode 1
  ${EndIf}
  ${GetOptions} $CMDLINE "/NS" $R0
  ${IfNot} ${Errors}
    StrCpy $NoShortcutMode 1
  ${EndIf}

  SetShellVarContext current
  ${If} ${RunningX64}
    SetRegView 64
  ${EndIf}

  ; /D= wins; otherwise wherever the last install went (the Tauri app's, on an update from it)
  ${If} $INSTDIR == "${PLACEHOLDER}"
    StrCpy $INSTDIR "$LOCALAPPDATA\${PRODUCTNAME}"
    ReadRegStr $R0 HKCU "${MANUPRODUCTKEY}" ""
    ${If} $R0 != ""
      StrCpy $INSTDIR $R0
    ${EndIf}
  ${EndIf}
FunctionEnd

Section Install
  SetOutPath $INSTDIR

  ReadRegStr $OldBinary HKCU "${UNINSTKEY}" "MainBinaryName"
  StrCpy $R1 "${MAINBINARY}"
  Call WaitFor
  ${If} $OldBinary != ""
  ${AndIf} $OldBinary != "${MAINBINARY}"
    StrCpy $R1 $OldBinary
    Call WaitFor
  ${EndIf}

  File "/oname=${MAINBINARY}" "${BINARY}"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; the Tauri app's binary is ours now
  ${If} $OldBinary != ""
  ${AndIf} $OldBinary != "${MAINBINARY}"
    Delete /REBOOTOK "$INSTDIR\$OldBinary"
  ${EndIf}

  ; the same values the Tauri app's installer wrote, so Apps shows one entry that uninstalls us
  WriteRegStr HKCU "${MANUPRODUCTKEY}" "" $INSTDIR
  WriteRegStr HKCU "${UNINSTKEY}" "MainBinaryName" "${MAINBINARY}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${PRODUCTNAME}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$\"$INSTDIR\${MAINBINARY}$\""
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "${MANUFACTURER}"
  WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" "$\"$INSTDIR$\""
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K /G=0" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINSTKEY}" "EstimatedSize" $0

  Call Shortcuts

  ${If} $PassiveMode = 1
    SetAutoClose true
  ${EndIf}
SectionEnd

; Shortcuts the old binary owned (the Start menu one, a desktop one, a taskbar pin) now open the
; new one. A first install adds a Start menu shortcut, and a desktop one when nobody is asked;
; an update leaves the user's choice of shortcuts alone.
Function Shortcuts
  ${If} $OldBinary != ""
  ${AndIf} $OldBinary != "${MAINBINARY}"
    !insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\$OldBinary"
    Pop $0
    ${If} $0 = 1
      !insertmacro SetShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARY}"
    ${EndIf}
    !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\$OldBinary"
    Pop $0
    ${If} $0 = 1
      !insertmacro SetShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARY}"
    ${EndIf}
    !insertmacro IsShortcutTarget "${TASKBAR}\${PRODUCTNAME}.lnk" "$INSTDIR\$OldBinary"
    Pop $0
    ${If} $0 = 1
      !insertmacro SetShortcutTarget "${TASKBAR}\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARY}"
    ${EndIf}
  ${EndIf}

  ${If} $UpdateMode = 1
  ${OrIf} $NoShortcutMode = 1
    Return
  ${EndIf}
  CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARY}"
  !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  ${If} $PassiveMode = 1
  ${OrIf} ${Silent}
    Call DesktopShortcut
  ${EndIf}
FunctionEnd

Function DesktopShortcut
  CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARY}"
  !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
FunctionEnd

Function RunApp
  Exec '"$INSTDIR\${MAINBINARY}"'
FunctionEnd

; /R: start the app again once an updater's install is done
Function .onInstSuccess
  ${If} $PassiveMode = 1
  ${OrIf} ${Silent}
    ${GetOptions} $CMDLINE "/R" $R0
    ${IfNot} ${Errors}
      ${GetOptions} $CMDLINE "/ARGS" $R0
      Exec '"$INSTDIR\${MAINBINARY}" $R0'
    ${EndIf}
  ${EndIf}
FunctionEnd

Function SkipIfPassive
  ${IfThen} $PassiveMode = 1 ${|} Abort ${|}
FunctionEnd

Function un.SkipIfPassive
  ${IfThen} $PassiveMode = 1 ${|} Abort ${|}
FunctionEnd

Function un.onInit
  ${GetOptions} $CMDLINE "/P" $R0
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
  ${EndIf}
  SetShellVarContext current
  ${If} ${RunningX64}
    SetRegView 64
  ${EndIf}
FunctionEnd

; Takes the app away and leaves the user's data (~/.hyprspace) where it is.
Section Uninstall
  StrCpy $R1 "${MAINBINARY}"
  Call un.WaitFor

  Delete "$INSTDIR\${MAINBINARY}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  ; only shortcuts that open this install
  !insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARY}"
  Pop $0
  ${If} $0 = 1
    !insertmacro UnpinShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    Delete "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  ${EndIf}
  !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARY}"
  Pop $0
  ${If} $0 = 1
    !insertmacro UnpinShortcut "$DESKTOP\${PRODUCTNAME}.lnk"
    Delete "$DESKTOP\${PRODUCTNAME}.lnk"
  ${EndIf}
  !insertmacro IsShortcutTarget "${TASKBAR}\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARY}"
  Pop $0
  ${If} $0 = 1
    !insertmacro UnpinShortcut "${TASKBAR}\${PRODUCTNAME}.lnk"
    Delete "${TASKBAR}\${PRODUCTNAME}.lnk"
  ${EndIf}

  DeleteRegKey HKCU "${MANUPRODUCTKEY}"
  DeleteRegKey /ifempty HKCU "${MANUKEY}"
  ; last, so its absence means the uninstall finished (scripts/check-windows-install.ps1 waits on it)
  DeleteRegKey HKCU "${UNINSTKEY}"

  ${If} $PassiveMode = 1
    SetAutoClose true
  ${EndIf}
SectionEnd
