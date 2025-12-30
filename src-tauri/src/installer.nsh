!macro NSIS_HOOK_POSTINSTALL
  Exec '"$INSTDIR\${MAIN_BINARY_NAME}.exe"'
!macroend
