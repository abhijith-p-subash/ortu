!macro NSIS_HOOK_POSTINSTALL
  CreateShortCut "$DESKTOP\Ortu.lnk" "$INSTDIR\${MAIN_BINARY_NAME}.exe"
  CreateShortCut "$SMPROGRAMS\Ortu.lnk" "$INSTDIR\${MAIN_BINARY_NAME}.exe"
!macroend
