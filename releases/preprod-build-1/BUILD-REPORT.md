# CinaVault Premium Preprod Build 1

Version: 1.0.0-14
Date: 2026-05-21

## Fix

- Fixed user-facing poster rendering in the installed Library UI.
- Root cause: local poster files existed in the live database, but the Tauri Content Security Policy did not allow the asset host used by `convertFileSrc`.
- Updated the CSP to allow both `http://asset.localhost` and `https://asset.localhost` for image loading.
- Added a durability test so future builds must keep the Tauri asset protocol host allowed.

## Verification

- Live database check: 17,763 media rows and 4,398 poster rows.
- Sampled current first-page poster paths existed on disk.
- `node --test tests\libraryLoadPolicy.test.mjs tests\durableFeatureSurface.test.mjs tests\pluginUiSafety.test.mjs tests\metadataResults.test.mjs tests\mediaArtwork.test.mjs tests\mediaPlaybackSafety.test.mjs`: 26 passed.
- `npm run build`: passed.
- `cargo check`: passed.
- `cargo test -- --nocapture`: 57 passed.
- `npm run tauri build`: produced MSI and NSIS installers.
- Installed NSIS build locally.
- Launched installed app from `C:\Program Files\CinaVault Premium\cinavault-premium.exe`.
- User-facing UI poster verification: passed. Screenshot pixel analysis found 8 poster-like rendered card regions in the actual Library window.

## Build Label

Preprod Build 1

## Artifacts

- `CinaVault Premium_1.0.0-14_x64-setup.exe`
- `CinaVault Premium_1.0.0-14_x64_en-US.msi`
- `BUILD-REPORT.md`
