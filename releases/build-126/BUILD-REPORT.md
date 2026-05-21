# CinaVault Premium Build 126

Build date: 2026-05-21
Version: 1.0.0-13
Branch: codex/carry-forward-build-features

## Summary

Build 126 fixes forward-facing poster rendering in the installed app. Local poster and screenshot files now load through Tauri's asset protocol, and internal poster/trash images are filtered out of the user-facing library so artwork files do not appear as media cards.

## Corrective Changes

- Enabled Tauri v2 `assetProtocol` for local poster files used by `convertFileSrc`.
- Added the required Rust `protocol-asset` feature so the app build and Tauri config agree.
- Preserved CSP `img-src` support for `asset:`, `https:`, and `data:` poster sources.
- Added shared detection for internal CinaVault artwork cache/trash paths.
- Prevented `.cinavault-trash` and `CinaVault\generated-posters` image rows from appearing in the visible library.
- Prevented future scans from indexing internal generated poster/trash paths.
- Added durable regression coverage for asset protocol poster rendering and internal artwork row filtering.
- Updated app/version surfaces to Build 126 / 1.0.0-13.

## Verification

- `node --test tests\libraryLoadPolicy.test.mjs tests\durableFeatureSurface.test.mjs tests\pluginUiSafety.test.mjs tests\metadataResults.test.mjs tests\mediaArtwork.test.mjs tests\mediaPlaybackSafety.test.mjs` passed 26 tests.
- `npm run build` passed.
- `cargo check` passed.
- `cargo test -- --nocapture` passed 57 tests.
- `npm run tauri build` produced MSI and NSIS installers.
- Build 126 NSIS install completed with exit code 0.
- Installed executable reports ProductVersion/FileVersion `1.0.0-13`.
- Installed app launch smoke test: process responding `True`, window title showed the CinaVault Premium media server window.
- No fresh Windows Error Reporting CinaVault events were found after launch.
- Live app database after Build 126 launch: 17,763 media rows, 4,398 rows with poster paths, and 0 internal artwork/trash media rows.

## Artifacts

- `CinaVault Premium_1.0.0-13_x64_en-US.msi` - 7,634,944 bytes.
- `CinaVault Premium_1.0.0-13_x64-setup.exe` - 5,468,291 bytes.
