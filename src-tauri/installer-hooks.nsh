; Omit Tauri's optional taskbar-unpin helper, which contains adapted CC BY-SA
; source. Normal shortcut deletion still runs; users can unpin manually.
; The pinned Tauri template loads this hook after utils.nsh, before expansion.
!ifmacrondef UnpinShortcut
  !error "Tauri installer layout changed; review the optional unpin helper"
!endif
!macroundef UnpinShortcut
!macro UnpinShortcut shortcut
!macroend

; Optional Open-with registration. Never assign the .mid/.midi default handler.
!macro NSIS_HOOK_POSTINSTALL
  IfSilent keyloom_assoc_done
  MessageBox MB_YESNO|MB_DEFBUTTON2 "Add Keyloom to Windows Open with for MIDI files? Your current default application will stay unchanged." IDNO keyloom_assoc_done
  WriteRegStr HKCU "Software\Classes\Applications\keyloom.exe" "FriendlyAppName" "Keyloom"
  WriteRegStr HKCU "Software\Classes\Applications\keyloom.exe\SupportedTypes" ".mid" ""
  WriteRegStr HKCU "Software\Classes\Applications\keyloom.exe\SupportedTypes" ".midi" ""
  WriteRegStr HKCU "Software\Classes\Applications\keyloom.exe\shell\open\command" "" '$\"$INSTDIR\keyloom.exe$\" $\"%1$\"'
  WriteRegStr HKCU "Software\Classes\.mid\OpenWithList\keyloom.exe" "" ""
  WriteRegStr HKCU "Software\Classes\.midi\OpenWithList\keyloom.exe" "" ""
  keyloom_assoc_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegKey HKCU "Software\Classes\Applications\keyloom.exe"
  DeleteRegKey HKCU "Software\Classes\.mid\OpenWithList\keyloom.exe"
  DeleteRegKey HKCU "Software\Classes\.midi\OpenWithList\keyloom.exe"
!macroend
