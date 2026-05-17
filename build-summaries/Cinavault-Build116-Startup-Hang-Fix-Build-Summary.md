# CinaVault Build 116 Summary

Date: 2026-05-17
Branch: codex/library-enrichment-normalization
App version: 1.0.0-3

## Summary

Build 116 corrects the Build 115 installed-app launch hang while preserving the meteor shower animation, Live TV IPTV player UI, and library enrichment cleanup work.

## Root cause

The installed app was not loading because database startup cleanup synchronously scanned sidecar poster candidates for thousands of existing video rows with missing poster paths. On the live library this meant heavy filesystem probing, mostly against `E:`, before the UI could respond.

## Fix

- Removed startup poster backfill from database initialization.
- Kept startup cleanup limited to cheap removal of generated chapter image and sidecar artwork photo rows.
- Kept manual/tested sidecar poster backfill behavior available outside startup.
- Added regression coverage for reopening an existing database without filesystem poster backfill.

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

## Release artifacts

- releases/build-116/CinaVault-Premium-Build116-Installer.msi
- releases/build-116/CinaVault-Premium-Build116-Setup.exe
- releases/build-116/BUILD-REPORT.md
