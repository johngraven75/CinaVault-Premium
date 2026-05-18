# CinaVault Premium Build 115

Date: 2026-05-17
Branch: codex/library-enrichment-normalization
App version: 1.0.0-2

## Included installers

- CinaVault-Premium-Build115-Installer.msi
- CinaVault-Premium-Build115-Setup.exe

## Changes

- Added the full-color meteor shower animation to the Home tab spotlight area.
- Added the built-in IPTV player experience for the Live TV tab with a cable set-top-box style playback surface and guide-oriented layout.
- Kept the Build 115 library enrichment fixes for poster cleanup, sidecar artwork filtering, embedded-title fallback, and post-enrichment library refresh.
- Kept the media display safety fixes that hide generated chapter images and sidecar artwork rows from the visible library.
- Preserved the app build metadata as Build 115 Premium Edition.

## Verification

- npm run build: passed
- cargo check: passed
- cargo test -- --nocapture: passed, 34 tests
- node --test tests\mediaPlaybackSafety.test.mjs tests\pluginUiSafety.test.mjs: passed, 10 tests
- npm run tauri -- build: passed
- Old Program Files install removed: passed
- Per-user MSI install: passed, exit code 0
- Installed executable verified: %LOCALAPPDATA%\Programs\CinaVault Premium\cinavault-premium.exe, version 1.0.0-2

## Build output

- MSI source: src-tauri\target\release\bundle\msi\CinaVault Premium_1.0.0-2_x64_en-US.msi
- NSIS source: src-tauri\target\release\bundle\nsis\CinaVault Premium_1.0.0-2_x64-setup.exe
- Install log copy: C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\build-115\Build115-msi-install.log
