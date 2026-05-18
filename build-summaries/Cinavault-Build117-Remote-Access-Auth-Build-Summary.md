# CinaVault Build 117 Summary

Date: 2026-05-17
Branch: codex/library-enrichment-normalization
App version: 1.0.0-4

## Summary

Build 117 adds remote access authentication for CinaVault users. Remote users can be created with email/password credentials and can also use a generated `cvra_` access key as an alternate authentication method.

## Changes

- Added `remote_access_users` and `remote_access_sessions` database tables.
- Added backend commands for creating remote users, password authentication, access-key authentication, key rotation, account enable/disable, user listing, and remote security status.
- Added salted hash storage for passwords, access keys, and session tokens.
- Added Remote Access tab controls for remote account setup, login checks, key login checks, key copy, key rotation, and account status.
- Preserved Build 116 startup-hang fix and Build 115 UI/library changes.

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

## Release artifacts

- releases/build-117/CinaVault-Premium-Build117-Installer.msi
- releases/build-117/CinaVault-Premium-Build117-Setup.exe
- releases/build-117/BUILD-REPORT.md
