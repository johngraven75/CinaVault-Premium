# CinaVault Premium Build 125

Build date: 2026-05-21
Version: 1.0.0-12
Branch: codex/carry-forward-build-features

## Summary

Build 125 adds an individual metadata check action to every media card and carries forward the adult metadata provider work from Build 124. The per-card action calls a new one-item backend command so a single media item can refresh metadata, poster artwork, sidecar metadata, and adult provider classification without running a full-library gather.

## Corrective Changes

- Added a card-level metadata check icon button with isolated click handling so the button does not open/select the card.
- Added `check_media_item_metadata` Tauri command for one-item metadata refresh.
- The one-item command uses local posters, generated screenshot posters, embedded titles, ThePornDB, StashDB, Porn Site Nuxt, PhoenixAdult local fallback, TMDb, and OMDb when applicable.
- Added PhoenixAdult manifest/config support from the supplied Jellyfin/Emby manifest and HTML config, including GUID `8f97371f-8617-463c-9859-a33072182494` and plugin configuration ID `dc40637f-6ebd-4a34-b8a1-8799629120cf`.
- Added ThePornDB provider configuration defaults from the supplied C# config, including token, filepath/hash matching, tag/studio/collection/image options, and actor metadata options.
- Preserved durable regression tests for Synology QuickConnect, Duplicate Finder, PhoenixAdult, ThePornDB, Porn Site Nuxt, and the new per-card metadata action.
- Updated app/version surfaces to Build 125 / 1.0.0-12.

## Verification

- `node --test tests\libraryLoadPolicy.test.mjs tests\durableFeatureSurface.test.mjs tests\pluginUiSafety.test.mjs tests\metadataResults.test.mjs` passed 19 tests.
- `npm run build` passed.
- `cargo check` passed.
- `cargo test -- --nocapture` passed 55 tests.
- `npm run tauri build` produced MSI and NSIS installers.
- Previous local install was stopped and uninstalled with exit code 0.
- Build 125 NSIS install completed with exit code 0.
- Installed executable reports ProductVersion/FileVersion `1.0.0-12`.
- Installed app launch smoke test: process responding `True`, window title showed the CinaVault Premium media server window.
- No fresh Windows Error Reporting CinaVault events were found after launch.
- Live app database check: 17,764 media rows, 2,374 rows with poster paths, 11,064 adult/movie/video candidates.

## Artifacts

- `CinaVault Premium_1.0.0-12_x64_en-US.msi` - 7,606,272 bytes.
- `CinaVault Premium_1.0.0-12_x64-setup.exe` - 5,445,145 bytes.
