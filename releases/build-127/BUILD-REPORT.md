# CinaVault Premium Build 127

Build date: 2026-05-23
Version: 1.0.0-15
Branch: codex/build-127-rc3-carryforward
Source baseline: Build 126 / preprod-build-1 plus selected RC3 Build 1 feature carry-forward

## Summary

Build 127 keeps the Build 126 forward-facing UI, poster rendering, large-library paging, and installed-app behavior while carrying forward RC3 Build 1 restorations for media-library alphabet tabs, metadata provider API-key controls, WD MyCloud sources, built-in VPN controls, and built-in antivirus controls.

## Corrective Changes

- Preserved Build 126 appearance, skin, forward-facing library UI, poster asset protocol support, and library paging.
- Kept the alphabetical media-library selector visible and covered by tests.
- Restored visible metadata-provider API-key save/test controls in the Plugins & Metadata tab.
- Normalized old RC3/provider IDs such as `theporndb`, `fanarttv`, `rotten_tomatoes`, `myanimelist`, and `epg_guide` to the backend provider keys.
- Enabled all adult metadata providers by default and forced older persisted adult-provider settings back on at restore time.
- Added WD MyCloud as a first-class media source type with visible source-entry UI.
- Carried forward built-in WireGuard VPN controls with status, config generation, connect, and disconnect command surfaces.
- Carried forward built-in Microsoft Defender antivirus controls with status, scan, and signature-update command surfaces.
- Updated app/version surfaces to Build 127 / 1.0.0-15.
- Cleaned the GitHub upload scope so installer binaries are release assets only, not tracked source files.

## Verification

- `node --test tests\libraryAlphabetFilter.test.mjs tests\durableFeatureSurface.test.mjs tests\pluginUiSafety.test.mjs tests\metadataResults.test.mjs tests\mediaArtwork.test.mjs tests\mediaPlaybackSafety.test.mjs tests\libraryLoadPolicy.test.mjs tests\metadataTaskProgress.test.mjs tests\pageWheelScroll.test.mjs tests\xtreamProfile.test.mjs` passed 44 tests.
- `npm run build` passed.
- `cargo check` passed.
- `cargo test -- --nocapture` passed 57 tests.
- `npm run tauri build` produced MSI and NSIS installers.
- Build 127 NSIS install completed with exit code 0.
- Installed executable reports ProductVersion/FileVersion `1.0.0-15`.
- Installed app launch smoke test: process responding `True`, window title `CinaVault Premium - Media Server`.
- No fresh Windows Error Reporting CinaVault events were found after launch.
- Live Build 127 database was updated so canonical metadata keys exist and adult metadata providers are enabled.
- GitHub release check confirmed no existing `build-127` release before upload.

## Artifacts

- `CinaVault Premium_1.0.0-15_x64-setup.exe` - 5,495,995 bytes - SHA256 `CE47E156B0A69CF3F48D16B87AD09093585F1D58B56422B1D87FFFA45B0AC6A5`.
- `CinaVault Premium_1.0.0-15_x64_en-US.msi` - 7,675,904 bytes - SHA256 `9696A1622D41F266D6D89A88D010AF38A1CF758CD1548A7DFCD061C9F99C5277`.
