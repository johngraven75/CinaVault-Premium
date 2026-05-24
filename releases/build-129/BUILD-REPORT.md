# CinaVault Premium Build 129

Version: 1.0.0-17
Date: 2026-05-23
Branch: codex/build-129-duplicate-bulk-stability

## Fixes

- Fixed duplicate-tab bulk actions that could hang the app into an opaque white screen.
- Changed Select All to a lightweight selection mode instead of storing every duplicate id in React state.
- Changed Remove Selected, Delete Selected Files, and Delete All to process duplicate rows in 50-row UI batches.
- Added progress text during bulk duplicate cleanup so the UI remains active while work is running.
- Added a backend 100-row limit per `remove_duplicates` command call to prevent oversized Tauri invoke payloads.
- Added duplicate-cache cleanup for stale singleton duplicate groups before the duplicate tab returns results.
- Kept the Build 128 foreign-key fix: duplicate child rows are deleted before the parent media row.

## Verification

- `node --test tests\durableFeatureSurface.test.mjs tests\pluginUiSafety.test.mjs`: 19 passed.
- `node --test tests\*.mjs`: 44 passed.
- `cargo test duplicates::tests:: -- --nocapture`: 2 passed.
- `cargo fmt -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `npm run build`: passed.
- `cargo test -- --nocapture`: 59 passed.
- `npm run tauri build`: passed.
- Local NSIS install: exit code 0.
- Installed app smoke test:
  - Executable: `C:\Program Files\CinaVault Premium\cinavault-premium.exe`
  - ProductVersion: 1.0.0-17
  - FileVersion: 1.0.0-17
  - Responding: True
  - Main window title: `CinaVault Premium — Media Server`
- Live DB duplicate cache checked after install:
  - Duplicate groups: 0
  - Duplicate items: 0
  - Backup created: `C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\build-129\cinavault-before-duplicate-cache-cleanup.db`

## Artifacts

- NSIS installer: `CinaVault Premium_1.0.0-17_x64-setup.exe`
  - Size: 5,493,930 bytes
  - SHA256: `197563E24569EF61EA1AD96BAF28714C5A36A40774BB047C9CF9330AA7EE4FE7`
- MSI installer: `CinaVault Premium_1.0.0-17_x64_en-US.msi`
  - Size: 7,671,808 bytes
  - SHA256: `D2F7134364CB487ECA4601CFCC291DC330F22AF484FB7EFD752A261D2E1E1EC3`
