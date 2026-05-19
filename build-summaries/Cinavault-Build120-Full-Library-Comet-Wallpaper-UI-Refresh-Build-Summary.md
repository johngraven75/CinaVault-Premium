# Cinavault Build 120 - Full Library, Comet Wallpaper, UI Refresh

Build 120 packages version `1.0.0-7` as the cumulative carry-forward build.

## Included Fixes

- Restored the full recent feature line so Build 115, 116, 117, 118, and 119 work carries into this build.
- Fixed the library behavior where only the newest 240 items were loaded. The app now opens quickly on the first page and keeps loading the full library in the background.
- Changed the Home top graphic into a real-looking animated comet shower canvas and made it visible even when no media item is selected.
- Refreshed the forward-facing UI appearance with higher contrast, sharper engineered controls, stronger input/button focus transitions, styled selects, checkboxes, tabs, alphabet controls, cards, panels, and navigation.
- Confirmed Remote Access account/authentication/access-key work is present.
- Updated repository dependencies for the GitHub moderate-risk alerts: Vite/esbuild and Tauri were moved to patched lines before final packaging.

## Verification

- Frontend policy tests: 12 passed.
- npm audit: 0 vulnerabilities.
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

- MSI: `FC1B4ACD84DF40D00407B31DD13A073D6BB80B9C6E55B4C7FCE9A7C7BF7AFBE8`
- Setup EXE: `12A490092C0DAFD05F9055C24ADBE6DA8D928D0F50DD9AE69DAFBD9E1B99DCD8`
