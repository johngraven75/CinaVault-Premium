# CinaVault Premium Build 114

Date: 2026-05-13

## Corrective Changes

- Added the reusable backend library enrichment pipeline for all video libraries.
- Added source-aware adult classification shared by enrichment and the adult metadata gather flow.
- Added provider routing and merge support for TMDb, OMDb, and StashDB metadata.
- Added metadata writeback helpers that preserve user state fields such as watched and favorite.
- Added balanced rename-confidence rules so filename normalization only runs when local title evidence and embedded/provider ID support agree.
- Added safe same-folder rename handling with missing-file, invalid-target, unchanged-target, and collision reporting.
- Added AI tab actions for `Enrich Library Metadata` and `Enrich + Normalize Filenames`.
- Added structured enrichment result summaries for the AI Activity Log.
- Included the previously local `src/data` plugin catalog source files so clean checkouts can build.

## Verification

- `cargo test -- --nocapture` passed: 30 tests.
- `cargo check` passed.
- `node --test tests\pluginUiSafety.test.mjs tests\mediaPlaybackSafety.test.mjs` passed: 9 tests.
- `npm run build` passed.
- `npm run tauri build` passed and produced MSI plus NSIS installers.
- NSIS installer installed successfully with `/S /currentuser`.
- Installed app verified at `C:\Users\johng\AppData\Local\Programs\CinaVault Premium\cinavault-premium.exe`.
- Installed app file version verified as `1.0.0-1`.

## Artifacts

- `CinaVault-Premium-Build114-Installer.exe`
- `CinaVault-Premium-Build114-Installer.msi`

## SHA256

- `824BED8670D8C38D5E5827BEC1C3C9E83EAE0BAB5C976459AE10B56D06EB9F31`  `CinaVault-Premium-Build114-Installer.exe`
- `47A47BF9D33D0EF8E41EC0E6C57416336F23E92CDADD1CFE6B2A9308ABFDBF6C`  `CinaVault-Premium-Build114-Installer.msi`

## Install Notes

- The MSI silent install attempt did not create a log or return, so the hung `msiexec` process was stopped before falling back to NSIS.
- The NSIS current-user installer completed with exit code 0.
