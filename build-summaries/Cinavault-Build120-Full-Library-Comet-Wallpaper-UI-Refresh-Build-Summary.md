# Cinavault Build 120 - Full Library, Comet Wallpaper, UI Refresh

Build 120 packages version `1.0.0-7` as the cumulative carry-forward build.

## Included Fixes

- Restored the full recent feature line so Build 115, 116, 117, 118, and 119 work carries into this build.
- Fixed the library behavior where only the newest 240 items were loaded. The app now opens quickly on the first page and keeps loading the full library in the background.
- Changed the Home top graphic into a real-looking animated comet shower canvas and made it visible even when no media item is selected.
- Refreshed the forward-facing UI appearance with higher contrast, sharper engineered controls, stronger input/button focus transitions, styled selects, checkboxes, tabs, alphabet controls, cards, panels, and navigation.
- Confirmed Remote Access account/authentication/access-key work is present.

## Verification

- Frontend policy tests: 12 passed.
- Production frontend build: passed.
- Rust check: passed.
- Rust tests: 44 passed.
- Tauri Windows build: passed.
- Local visual screenshot check: passed for Home comet hero and refreshed controls.
- Local MSI uninstall/install: passed.
- Installed app smoke launch: passed, responding from `C:\Users\johng\AppData\Local\Programs\CinaVault Premium\cinavault-premium.exe`.

## Installer Copies

- Repo: `releases/build-120`
- Local: `C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\build-120`

## SHA256

- MSI: `A9F0D03B410B9E9D8034723477559700E0957C8EC97A0EC4EE8932010C52C4A2`
- Setup EXE: `587579B4275BDE8D6B83E38C2D1A39B78BABF3A47C33DA88830872790B9DE3BC`
