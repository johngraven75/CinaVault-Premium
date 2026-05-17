# CinaVault Premium RC2 Build 1

Date: 2026-05-16
Branch: beta-4
Version: `1.0.0-rc.2+1`
Sequential build number: `117`

## Release Label

- This build is labeled `RC2 Build 1`.
- The app footer now shows `CinaVault Premium RC2 Build 1`.
- The About settings panel now shows `v1.0.0 RC2 - Build 1 - Tauri v2 + React 18`.
- Tauri/package metadata now uses `1.0.0-rc.2+1`, producing RC2 installer filenames.

## Fix Included

- Fixed the Live TV Xtream Codes save flow that could show `invalid argument` when saving provider details.
- The UI now sends the Tauri command argument as `serverUrl` instead of `server_url`.
- Provider URLs pasted as full Xtream endpoints are normalized to the server base URL.
- Xtream usernames and passwords are percent-encoded for API, EPG, and live stream URLs.
- Doubled the global scrollbar width and enlarged the draggable thumb with a high-contrast accent style tied to the active theme colors.
- Added an A-Z and `#` Library title selector so users can show only media whose title starts with the selected letter.
- Added explicit mouse wheel and trackpad scrolling on the main app page while preserving nested scroll regions.

## Verification

- `node --test tests\*.test.mjs`
- `cargo test --manifest-path src-tauri\Cargo.toml`
- `npm run build`
- `npm run tauri -- build`
- In-app browser UI check verified the Live TV add-profile validation behavior.
- Playwright/Chrome payload check verified successful profile save sends `serverUrl` and does not send `server_url`.
- In-app browser UI check verified the Library A-Z selector renders in RC2 Build 1.
- Playwright/Chrome UI check verified mouse wheel scrolling moves the Library page and selecting `B` shows only `Blade Runner` and `Breaking Bad`.

## Artifacts

- `CinaVault-Premium-RC2-Build1-Installer.exe`
- `CinaVault-Premium-RC2-Build1-Installer.msi`

## SHA256

- EXE: `45EE6B9DB950E9FCE3479758A558F5A1B38E8DA39290D18AB9B2464919F8EB28`
- MSI: `FBBC101BE7EE4C74B9AA5D0D74793FBA5EA282099333ED7E247D2CAB3899B199`
