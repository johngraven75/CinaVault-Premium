# CinaVault Build 119 Cumulative Feature Carryover

Build 119 restores the recent feature work that had been split between the `beta-4` build line and the `codex/library-enrichment-normalization` build line.

## Included

- Build 115 photorealistic Home spotlight visual update with meteor animation.
- Build 115 built-in IPTV player UI.
- Build 116 startup hang prevention.
- Build 117 remote access account authentication.
- Adult metadata and poster compatibility fixes.
- Xtream profile argument and URL normalization fixes.
- RC2 library navigation refresh.
- Alphabetized library tab filtering.
- Mouse wheel and trackpad page scrolling.
- Paged library loading for large libraries.
- Sidecar artwork cleanup so poster files are reused as posters but hidden as photo rows.

## Verification

- `node --test tests\*.test.mjs`: passed, 26 tests.
- `npm run build`: passed.
- `cargo check --manifest-path src-tauri\Cargo.toml`: passed.
- `cargo test --manifest-path src-tauri\Cargo.toml -- --nocapture`: passed, 44 tests.
- `npm run tauri -- build`: passed.

## Artifacts

- `releases\build-119\CinaVault-Premium-Build119-Installer.msi`
- `releases\build-119\CinaVault-Premium-Build119-Setup.exe`
- `C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\build-119`
