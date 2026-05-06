# CinaVault Beta 4 Build 3 Summary

Date: 2026-05-06 15:46:02 -04:00
Branch Target: beta-4
Build Sequence: Beta 4 / Build 3

## Included Work
- Fixed library rescans so enabling `Prefer embedded titles over filenames` refreshes existing items instead of leaving the old filename-based title in place.
- Preserved user library state such as watched and favorite flags while refreshing scan-owned fields like title, media type, file size, and source link.
- Added a Rust regression test to lock in the rescan title-refresh behavior for already indexed files.

## Verification
- `cargo test`: PASS
- `cargo check`: PASS (warnings only)
- `npm run build`: PASS
- `npm run tauri build`: PASS

## Artifacts
- `releases/CinaVault Premium_1.0.0_x64-setup-beta4-build3.exe`
- `releases/CinaVault Premium_1.0.0_x64_en-US-beta4-build3.msi`

## SHA256
- setup-beta4-build3.exe: DBD327EB60DAE46D8295C1D5550AECB88B22BF70F1B9E328D05B6975A6C81813
- msi-beta4-build3.msi: B2BECE99839C160072A00575E1CC67ECE7320E2F1A45F365ED8F84484EA4775D
