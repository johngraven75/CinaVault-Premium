# CinaVault Build 121 - Poster Availability and Forward UI Artwork

Build: 121  
Version: 1.0.0-8  
Branch: codex/carry-forward-build-features  
Date: 2026-05-19  

## Purpose

Build 121 fixes poster artwork that was available in metadata, sidecar files, or existing database rows but was not reliably appearing on the user-facing library surfaces.

## Corrective Changes

- Added a shared backend poster resolver for media files.
- Checks `*.cinavault.json` sidecars for poster, cover, thumbnail, image, and backdrop fields.
- Checks same-stem artwork such as `Movie.jpg`, explicit suffixes like `Movie-poster.webp`, and generic folder artwork such as `poster.jpg`, `cover.jpg`, and `folder.jpg`.
- Writes missing `poster_path` values through scan, library enrichment, and AI adult metadata refresh paths instead of performing expensive filesystem checks during normal library page loads.
- Prevents matching sidecar artwork from being indexed as forward-facing photo media when it belongs to a nearby video.
- Reused the same poster resolver in scanner, library enrichment, and the AI adult metadata gather path.
- Added frontend artwork selection so poster/backdrop fields fall back to each other on poster cards, banner cards, disc cards, and the selected-media detail panel.
- Restored manual library paging so the UI opens with the first 240 rows and waits for the user to click `Load Next 240` instead of automatically pulling the full large library into React state.
- Kept the sibling-video artwork check out of database startup cleanup so large photo libraries do not trigger expensive filesystem checks during app launch.

## Live Library Audit

Read-only audit of the installed app database at:

`C:\Users\johng\AppData\Roaming\com.cinavault.premium\cinavault.db`

Results before installing Build 121:

- Video/adult/movie rows checked: 11,064
- Rows already holding a DB poster path: 2,374
- Blank poster rows: 8,690
- Blank rows with discoverable local sidecar poster candidates: 0
- Stored local poster paths missing on disk: 0
- First 1,200 newest library rows had 0 stored poster paths.

That means Build 121 fixes display for rows where poster artwork is actually available or discoverable, while rows with no poster path and no local sidecar still require provider metadata refresh to create poster paths.

## Verification

- `npm run build` passed.
- `node --test tests\*.test.mjs` passed: 29 tests.
- `cargo check` passed.
- `cargo test -- --nocapture` passed: 47 tests.
- `npm run tauri build` passed.
- Installed-app smoke testing caught nonresponsive launches during the release loop; Build 121 was rebuilt after restoring manual paging and after removing expensive sibling-video filesystem checks from startup cleanup.
- Final local reinstall passed: `%LOCALAPPDATA%\Programs\CinaVault Premium\cinavault-premium.exe` reports `1.0.0-8`, `uninstall.exe` exists, and the installed app was responding after launch.

## Installer Outputs

- `src-tauri\target\release\bundle\msi\CinaVault Premium_1.0.0-8_x64_en-US.msi`
- `src-tauri\target\release\bundle\nsis\CinaVault Premium_1.0.0-8_x64-setup.exe`
