# CinaVault Premium Build 117

Date: 2026-05-17
Branch: codex/library-enrichment-normalization
App version: 1.0.0-4

## Included installers

- CinaVault-Premium-Build117-Installer.msi
- CinaVault-Premium-Build117-Setup.exe

## Remote access authentication

- Added remote access user accounts stored in the CinaVault app database.
- Added email/password authentication for remote users.
- Added alternate `cvra_` access keys for remote access without entering the account password.
- Added salted SHA-256 storage for passwords, access keys, and issued session tokens so plaintext secrets are not stored.
- Added 12-hour remote sessions for successful password or access-key authentication.
- Added remote user enable/disable and access-key rotation.
- Added Remote Access tab UI for account setup, password access checks, access-key checks, key copy, rotation, and account status.

## Verification

- npm run build: passed
- cargo check: passed clean
- cargo test -- --nocapture: passed, 38 tests
- node --test tests\mediaPlaybackSafety.test.mjs tests\pluginUiSafety.test.mjs: passed, 10 tests
- npm run tauri -- build: passed
- Build 116 MSI uninstall: passed, exit code 0
- Build 117 per-user MSI install: passed, exit code 0
- Installed executable verified: %LOCALAPPDATA%\Programs\CinaVault Premium\cinavault-premium.exe, version 1.0.0-4
- Launch check: passed, process remained responsive after 12 seconds

## Build output

- MSI source: src-tauri\target\release\bundle\msi\CinaVault Premium_1.0.0-4_x64_en-US.msi
- NSIS source: src-tauri\target\release\bundle\nsis\CinaVault Premium_1.0.0-4_x64-setup.exe
- Install log copy: C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\build-117\Build117-msi-install.log
