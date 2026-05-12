# CinaVault Premium Build 113

Date: 2026-05-12

## Corrective Changes

- Restored the notification bell interaction. Clicking the bell now opens the latest status messages instead of doing nothing.
- Repaired the plugin catalog so external MS-A/MS-B/MS-C plugins remain available until the user downloads and enables them. Built-in CinaVault-native plugins remain active.
- Wired plugin install, uninstall, enable/disable, run, and JSON configuration actions through the Tauri backend.
- Restored live `Enrich Metadata` and `Normalize Filenames` actions in AI Diagnostics through the new `run_library_enrichment` backend command.
- Added safe filename normalization with collision checks and metadata-only fallback for low-confidence items.
- Changed the default scanner behavior to prefer embedded titles over filenames.
- Added first-scan embedded poster extraction to the scanner. Embedded poster images are cached under the user's app data folder and saved as the media poster path on initial import.

## Verification

- `node --test tests\pluginUiSafety.test.mjs tests\mediaPlaybackSafety.test.mjs` passed: 9 tests.
- `cargo test -- --nocapture` passed: 22 tests.
- `npm run build` passed.
- `npm run tauri -- build` passed and produced MSI plus NSIS installers.
- Browser QA at `http://127.0.0.1:1420` verified the notification panel, plugin catalog state, plugin settings drawer, plugin download/enable state change, and restored enrichment buttons.

## Artifacts

- `CinaVault-Premium-Build113-Installer.exe`
- `CinaVault-Premium-Build113-Installer.msi`

## SHA256

- `09C7195DDC173431B95B9C8497B893AFDCBC42A2B84B6EE6C735F3366D91B3CD`  `CinaVault-Premium-Build113-Installer.exe`
- `2F3FF14CC8C1A79C8C23AE938713F81A0021EB5C6CE244F4B3BB954357C32A44`  `CinaVault-Premium-Build113-Installer.msi`
