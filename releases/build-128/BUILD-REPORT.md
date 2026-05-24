# CinaVault Premium Build 128

Version: 1.0.0-16
Date: 2026-05-23
Branch: codex/build-128-duplicate-removal-fix

## Fixes

- Fixed duplicate removal failing with `FOREIGN KEY constraint failed`.
- Reworked duplicate deletion so `duplicate_items` child rows are removed before the referenced `media_items` parent row.
- Added a bulk `remove_duplicates` command for safer multi-row cleanup.
- Restored duplicate-tool bulk controls:
  - Select All
  - Clear
  - Remove Selected Rows
  - Delete Selected Files
  - Delete All
- Bulk selection keeps the first item in each duplicate group as the reference copy and selects only removable duplicates.
- Ran a full code debug pass and cleaned strict Rust formatting/Clippy issues across the backend so `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.

## Verification

- `node --test tests\durableFeatureSurface.test.mjs tests\pluginUiSafety.test.mjs`: 19 passed.
- `node --test tests\*.mjs`: 44 passed.
- `cargo test removing_duplicate_deletes_child_rows_before_media_row -- --nocapture`: passed.
- `cargo fmt -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo check`: passed.
- `npm run build`: passed.
- `cargo test -- --nocapture`: 58 passed.
- `npm run tauri build`: passed.
- Local NSIS install: exit code 0.
- Installed app smoke test:
  - Executable: `C:\Program Files\CinaVault Premium\cinavault-premium.exe`
  - ProductVersion: 1.0.0-16
  - FileVersion: 1.0.0-16
  - Responding: True
  - Main window title: `CinaVault Premium — Media Server`

## Artifacts

- NSIS installer: `CinaVault Premium_1.0.0-16_x64-setup.exe`
  - Size: 5,490,970 bytes
  - SHA256: `349953D2BFCA683DCACFDE550E412F05A6D3BC2C47B27E7D6A6814FAAE0FF9BE`
- MSI installer: `CinaVault Premium_1.0.0-16_x64_en-US.msi`
  - Size: 7,667,712 bytes
  - SHA256: `AF77F975F5D3D7F53361C398FD87E2F8D78EB4ADF42715CCEE37EFC372FCC8DC`
