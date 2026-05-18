# CinaVault Premium Build 116

Date: 2026-05-17
Branch: codex/library-enrichment-normalization
App version: 1.0.0-3

## Included installers

- CinaVault-Premium-Build116-Installer.msi
- CinaVault-Premium-Build116-Setup.exe

## Corrective fix

- Fixed the installed app hanging on launch after Build 115.
- Root cause: startup database initialization was probing sidecar poster candidates for thousands of existing media rows on the live library before the UI could become responsive.
- Fix: startup cleanup now only removes generated chapter/sidecar artwork photo rows and does not perform filesystem poster backfill on launch.
- Added a regression test proving reopening an existing database does not backfill video posters from the filesystem during startup.

## Included Build 115 changes

- Home tab meteor shower animation.
- Built-in Live TV IPTV player UI.
- Library enrichment cleanup for poster handling, sidecar artwork filtering, embedded-title fallback, and post-enrichment refresh.

## Verification

- npm run build: passed
- cargo check: passed clean
- cargo test -- --nocapture: passed, 36 tests
- node --test tests\mediaPlaybackSafety.test.mjs tests\pluginUiSafety.test.mjs: passed, 10 tests
- npm run tauri -- build: passed
- Broken Build 115 MSI uninstall: passed, exit code 0
- Build 116 per-user MSI install: passed, exit code 0
- Installed executable verified: %LOCALAPPDATA%\Programs\CinaVault Premium\cinavault-premium.exe, version 1.0.0-3
- Launch check: passed, process remained responsive after 12 seconds

## Build output

- MSI source: src-tauri\target\release\bundle\msi\CinaVault Premium_1.0.0-3_x64_en-US.msi
- NSIS source: src-tauri\target\release\bundle\nsis\CinaVault Premium_1.0.0-3_x64-setup.exe
- Install log copy: C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\build-116\Build116-msi-install.log
