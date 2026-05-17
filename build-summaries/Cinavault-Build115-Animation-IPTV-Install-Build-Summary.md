# CinaVault Build 115 Summary

Date: 2026-05-17
Branch: codex/library-enrichment-normalization
App version: 1.0.0-2

## Summary

Build 115 packages the updated VS Code/worktree implementation for CinaVault Premium. It includes the new Home tab meteor shower animation, the built-in Live TV IPTV player surface, and the existing Build 115 library enrichment cleanup fixes.

## User-facing changes

- Home tab now uses a full-color meteor shower animation in the spotlight area.
- Live TV tab now opens channels inside a built-in IPTV player instead of relying only on external playback.
- Live TV player UI is styled around a cable set-top-box experience with playback controls and guide-oriented channel browsing.
- Library enrichment fixes continue to clean sidecar photo artifacts, restore poster paths, and refresh visible library results after enrichment actions.

## Verification

- npm run build: passed
- cargo check: passed
- cargo test -- --nocapture: passed, 34 tests
- node --test tests\mediaPlaybackSafety.test.mjs tests\pluginUiSafety.test.mjs: passed, 10 tests
- npm run tauri -- build: passed
- Old Program Files install removed: passed
- Per-user MSI install: passed, exit code 0
- Installed executable verified: %LOCALAPPDATA%\Programs\CinaVault Premium\cinavault-premium.exe, version 1.0.0-2

## Release artifacts

- releases/build-115/CinaVault-Premium-Build115-Installer.msi
- releases/build-115/CinaVault-Premium-Build115-Setup.exe
- releases/build-115/BUILD-REPORT.md
