# CinaVault Premium Beta 3 Build Summary

- Build label: Beta 3
- Built on: 2026-05-05 20:48:12 -04:00
- Branch: main
- Scope: adult metadata loop fix, hidden ffmpeg windows, single-run lock, media click auto-play fix

## Verification

1. 
pm run build - PASS
2. 
pm run tauri build - PASS
3. Live executable smoke launch - PASS

## Artifacts

- NSIS Installer: src-tauri/target/release/bundle/nsis/CinaVault Premium_1.0.0_x64-setup.exe
- MSI Installer: src-tauri/target/release/bundle/msi/CinaVault Premium_1.0.0_x64_en-US.msi

## Desktop Beta 3 Copies

- C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\CinaVault Premium_1.0.0_x64-setup-Beta3.exe
- C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\CinaVault Premium_1.0.0_x64_en-US-Beta3.msi

## Notes

- Adult metadata gather no longer spawns visible terminal windows per ffmpeg/ffprobe call on Windows.
- Adult metadata gather now prevents overlapping concurrent runs.
- Clicking a media item in Library now opens playback immediately.
