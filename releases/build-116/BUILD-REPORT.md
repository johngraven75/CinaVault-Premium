# CinaVault Premium Build 116

Date: 2026-05-16
Branch: beta-4

## Fix

- Fixed the Live TV Xtream Codes profile save flow that could show a generic `invalid argument` error when entering provider details.
- Root cause: the React form passed `server_url` directly to the Tauri command, but the Tauri JavaScript bridge expects the Rust `server_url` parameter as `serverUrl`.
- Added a tested Xtream profile argument builder so the UI now sends `serverUrl`, trims profile fields, and normalizes common provider URLs.
- Provider URLs pasted as full Xtream endpoints such as `player_api.php?...` or `get.php?...` are reduced to the server base URL before saving.
- Added frontend validation for unusable schemes such as `ftp://`, producing a clear status message instead of a bridge-level invalid argument.
- Hardened Xtream sync URL generation so usernames and passwords with special characters are percent-encoded for API, EPG, and live stream URLs.

## Verification

- `node --test tests\xtreamProfile.test.mjs`
- `cargo test --manifest-path src-tauri\Cargo.toml xtream_`
- `cargo test --manifest-path src-tauri\Cargo.toml`
- `node --test tests\*.test.mjs`
- `npm run build`
- `npm run tauri -- build`
- In-app browser UI check at `http://127.0.0.1:1420/` verified the Live TV add-profile form shows a friendly invalid URL message and no Vite overlay or app console errors.
- Playwright/Chrome UI check verified successful profile save sends `add_xtream_profile` with `serverUrl: "https://provider.example.com:8443"` and does not send `server_url`.

## Artifacts

- `CinaVault-Premium-Build116-Installer.exe`
- `CinaVault-Premium-Build116-Installer.msi`

## SHA256

- EXE: `B1B3693F8B4F0CB6C347F2211C7AC4F0A6DC2458DE7598A8B23866BEF97C366A`
- MSI: `82B189E2A108DB9F83178246AF365DB45FA7822A98EF10EC8577F6848637CDCF`
