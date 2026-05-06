# CinaVault Premium Beta 2 Build Summary

- Build label: Beta 2
- Built on: 2026-05-05 20:02:46 -04:00
- Branch: main
- Commit scope: Remote Access tab, AI enhancements, adult metadata gather action, Windows glass UI skin refresh, runtime config fix

## Verification

1. 
pm run build - PASS
2. AI endpoint smoke check (/v1/chat/completions) - PASS
3. 
pm run tauri build - PASS
4. Live app smoke launch (cinavault-premium.exe, 8s run) - PASS

## Artifacts

- NSIS Installer: src-tauri/target/release/bundle/nsis/CinaVault Premium_1.0.0_x64-setup.exe
- MSI Installer: src-tauri/target/release/bundle/msi/CinaVault Premium_1.0.0_x64_en-US.msi
- App Binary: src-tauri/target/release/cinavault-premium.exe

## Notes

- Hugging Face token is configured and valid.
- Default AI model was updated to katanemo/Arch-Router-1.5B:hf-inference for guaranteed provider compatibility in this environment.
- Adult metadata gather action includes poster + chapter-image pipeline and provider-aware reporting.
