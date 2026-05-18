# CinaVault Premium Build 119

Date: 2026-05-18
Branch: codex/carry-forward-build-features
App version: 1.0.0-6

## Purpose

Build 119 restores the feature changes that were split across the recent `beta-4` and `codex/library-enrichment-normalization` build lines so new work carries forward prior build features instead of dropping them.

## Carried-forward changes

- Kept the Build 115 photorealistic Home spotlight visual update with the meteor animation.
- Kept the Build 115 built-in IPTV player surface and Live TV playback UI.
- Kept the Build 116 launch/startup fix that avoids expensive poster backfill during app initialization.
- Kept the Build 117 remote access account authentication and security controls.
- Kept the beta-4 adult metadata/poster compatibility fixes.
- Kept the Xtream profile argument and URL normalization fixes.
- Kept the RC2 library navigation refresh.
- Kept the Build 118 alphabetized library tab filter.
- Kept the Build 118 mouse wheel and trackpad page scrolling behavior.
- Kept the Build 118 paged library loading fix for large libraries.
- Kept sidecar artwork cleanup and media safety behavior so poster files do not show as library photo media.

## Verification

- `node --test tests\*.test.mjs`: passed, 26 tests.
- `npm run build`: passed.
- `cargo check --manifest-path src-tauri\Cargo.toml`: passed.
- `cargo test --manifest-path src-tauri\Cargo.toml -- --nocapture`: passed, 44 tests.
- `npm run tauri -- build`: passed and produced MSI plus NSIS installers.
- Version scan for old active RC2/Build 118 metadata: passed; active app metadata is Build 119 / 1.0.0-6.

## Build output

- MSI source: `src-tauri\target\release\bundle\msi\CinaVault Premium_1.0.0-6_x64_en-US.msi`
- NSIS source: `src-tauri\target\release\bundle\nsis\CinaVault Premium_1.0.0-6_x64-setup.exe`
- Repo MSI copy: `releases\build-119\CinaVault-Premium-Build119-Installer.msi`
- Repo NSIS copy: `releases\build-119\CinaVault-Premium-Build119-Setup.exe`
- Desktop build copy: `C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\build-119`

## SHA256

- `B8132E0A2FECB5BD5D336A300824B61CFABC65A895FE7D351BF42BA62130BD62`  `CinaVault-Premium-Build119-Installer.msi`
- `94B3B7315A5F7647C64429C46668F7E06C10D727D38561557B3676FDEDEB67AB`  `CinaVault-Premium-Build119-Setup.exe`
