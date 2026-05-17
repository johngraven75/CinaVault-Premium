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

## Verification

- `node --test tests\*.test.mjs`
- `cargo test --manifest-path src-tauri\Cargo.toml`
- `npm run build`
- `npm run tauri -- build`
- In-app browser UI check verified the Live TV add-profile validation behavior.
- Playwright/Chrome payload check verified successful profile save sends `serverUrl` and does not send `server_url`.

## Artifacts

- `CinaVault-Premium-RC2-Build1-Installer.exe`
- `CinaVault-Premium-RC2-Build1-Installer.msi`

## SHA256

- EXE: `1B0CE30B8BB650AF7B80C3236259E22118A056BE8EC89C275AE3F81BFF69BF6C`
- MSI: `5B00D0AB1C17C62AA727E3F76F9F2255FBD78B10AA8172D8636F2F4758CCC21F`
