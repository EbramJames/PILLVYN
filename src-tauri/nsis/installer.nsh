; PILLVYN NSIS Installer & Uninstaller Hooks
; Ensures clean process shutdown on upgrade/uninstall and clean metadata.

!macro NSIS_HOOK_PREINSTALL
  ; Gracefully close any running PILLVYN instance before replacing binaries
  nsExec::ExecToLog 'taskkill /IM "PILLVYN.exe" /T'
  Sleep 450
  nsExec::ExecToLog 'taskkill /F /IM "PILLVYN.exe" /T'
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Ensure clean AppUserModelID registry hint for Start Menu & notifications
  WriteRegStr HKCU "Software\Classes\AppUserModelId\com.pillvyn.desktop" "DisplayName" "PILLVYN"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Stop PILLVYN cleanly prior to uninstalling files
  nsExec::ExecToLog 'taskkill /IM "PILLVYN.exe" /T'
  Sleep 450
  nsExec::ExecToLog 'taskkill /F /IM "PILLVYN.exe" /T'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Clean up autostart & AppUserModelId registration
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "PILLVYN"
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\com.pillvyn.desktop"
!macroend
