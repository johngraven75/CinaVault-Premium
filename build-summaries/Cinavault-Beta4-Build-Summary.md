# CinaVault Beta 4 Build Summary

Date: 2026-05-05 21:13:30 -04:00
Branch: main

## Included Work
- Fixed adult metadata gather loop by preventing duplicate concurrent runs and disabling repeat trigger clicks while processing.
- Fixed chapter image generation terminal-window loop on Windows by launching ffmpeg/ffprobe without console windows.
- Fixed click-to-play behavior from library media cards/rows.
- Improved player launch reliability with default-player fallback and Windows shell fallback.
- Added library option: prefer embedded titles over filenames.
- Added additional library behavior toggles in Sources tab.
- Rebranded user-facing competitor platform labels to neutral acronyms (MS-A/MS-B/MS-C) in UI text.

## Verification Results
- `npm run build`: PASS
- `cargo check`: PASS (warnings only)
- `npm run tauri build`: PASS

## Build Artifacts
- NSIS installer: `releases/CinaVault Premium_1.0.0_x64-setup-beta4.exe`
- MSI installer: `releases/CinaVault Premium_1.0.0_x64_en-US-beta4.msi`

## SHA256
- CinaVault Premium_1.0.0_x64-setup-beta4.exe: A1F3399E60594771B4B773BF4D8494C89DB0D99EEC6E3D9845320C6E2A7621D7
- CinaVault Premium_1.0.0_x64_en-US-beta4.msi: 48E131FD53D5FFF3A0184A3B5FA09C4FE338743057BF611C388F6F58F70F1422
