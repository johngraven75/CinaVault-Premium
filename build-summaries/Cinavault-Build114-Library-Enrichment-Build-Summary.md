# Cinavault Build 114 Summary

Date: 2026-05-13

## What Changed

- Added reusable library enrichment for video metadata.
- Added optional filename normalization through the AI tab.
- Added balanced rename-confidence rules and tests.
- Added safe rename persistence and metadata writeback helpers.
- Shared adult-source classification between enrichment and adult metadata gather.
- Added structured enrichment summaries in the AI Activity Log.
- Added tracked `src/data` plugin catalog files required for clean frontend builds.

## Verification

- `cargo test -- --nocapture`: 30 passed.
- `cargo check`: passed.
- `node --test tests\pluginUiSafety.test.mjs tests\mediaPlaybackSafety.test.mjs`: 9 passed.
- `npm run build`: passed.
- `npm run tauri build`: passed.
- NSIS installer `/S /currentuser`: exit code 0.
- Installed version: `1.0.0-1`.

## Artifacts

- `releases/build-114/CinaVault-Premium-Build114-Installer.exe`
- `releases/build-114/CinaVault-Premium-Build114-Installer.msi`
- Desktop build copy: `C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\Cinavault-Standalone-Installer-v114.exe`

## SHA256

- `824BED8670D8C38D5E5827BEC1C3C9E83EAE0BAB5C976459AE10B56D06EB9F31`  `CinaVault-Premium-Build114-Installer.exe`
- `47A47BF9D33D0EF8E41EC0E6C57416336F23E92CDADD1CFE6B2A9308ABFDBF6C`  `CinaVault-Premium-Build114-Installer.msi`
