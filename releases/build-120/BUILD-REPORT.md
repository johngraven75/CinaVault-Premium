# CinaVault Premium Build 120 Report

Build: 120  
Version: 1.0.0-7  
Date: 2026-05-18  
Branch: codex/carry-forward-build-features

## Summary

Build 120 is the cumulative carry-forward build after Build 119. It keeps the recovered Build 115-119 feature work and adds the final fixes requested in this session:

- Library opens quickly with the first 240 items, then automatically continues loading the full library in the background instead of stopping at 240.
- Home page top graphic now renders a full-color, real-looking live comet shower canvas and is visible even when no media item is selected.
- Forward-facing UI appearance was refreshed without removing or renaming inputs, buttons, labels, tabs, or feature controls.
- Default theme was upgraded to a higher-contrast engineered palette.
- Remote Access account and access-key work from Build 117 is included and verified in source and tests.

## Carried Forward Features

- Build 115 library enrichment and metadata normalization.
- Build 116 startup stability fix.
- Build 117 Remote Access account authentication and access-key flows.
- Build 118 large-library paging and AppHang prevention.
- Build 118 beta-4 alphabet tabs and wheel/trackpad scrolling.
- Build 119 cumulative merge of beta-4 and normalization work.
- Adult metadata, poster fallback, embedded-title fallback, sidecar-artwork hiding, Xtream IPTV credential URL handling, and plugin/catalog behavior from the recent build line.

## Installer Assets

- `CinaVault-Premium-Build120-Installer.msi`
  - SHA256: `A9F0D03B410B9E9D8034723477559700E0957C8EC97A0EC4EE8932010C52C4A2`
- `CinaVault-Premium-Build120-Setup.exe`
  - SHA256: `587579B4275BDE8D6B83E38C2D1A39B78BABF3A47C33DA88830872790B9DE3BC`

Repo folder:

- `releases/build-120`

Local build folder:

- `C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\build-120`

## Verification

- `node --test tests\libraryLoadPolicy.test.mjs tests\libraryAlphabetFilter.test.mjs tests\pageWheelScroll.test.mjs`: 12 passed.
- `npm run build`: passed.
- `cargo check`: passed.
- `cargo test`: 44 passed.
- `npm run tauri -- build`: passed and produced MSI plus NSIS setup EXE.
- Visual verification: local Chromium screenshot confirmed the comet hero renders at the top of Home with the refreshed high-contrast UI controls intact.
- Remote Access verification: source contains `RemoteAccessTab`, Tauri command registration, remote user/session/access-key DB methods, and Rust tests for password/access-key authentication plus disabled-user rejection.
- Local uninstall/install:
  - Previous install: `CinaVault Premium 1.0.0.6`, product code `{C4F7F446-EEF5-49FE-AD14-E1C15F346F90}`.
  - New install: `CinaVault Premium 1.0.0.7`, product code `{DDD42F1E-9BA2-4FD7-9C44-1DE484D53A28}`.
  - MSI install log status: `Installation success or error status: 0`.
- Installed executable smoke test:
  - Path: `C:\Users\johng\AppData\Local\Programs\CinaVault Premium\cinavault-premium.exe`
  - File/Product version: `1.0.0-7`
  - Launch result: process responding, window title `CinaVault Premium - Media Server`.

## Repository Safety Notes

- This build is intended to be the permanent cumulative baseline for future builds.
- After commit, the final Build 120 commit should be pushed to both `codex/carry-forward-build-features` and `beta-4` so future work starts with all carried-forward changes.
- No old release branches should be deleted without a separate explicit cleanup decision.
