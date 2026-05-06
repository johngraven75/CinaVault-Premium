# CinaVault Beta 4 Build 2 Summary

Date: 2026-05-06 15:21:21 -04:00
Branch Target: beta-4
Build Sequence: Beta 4 / Build 2

## Included Work
- Stopped generated `*_chapters/chapter_*.jpg` files from being indexed into the main library during scans.
- Cleaned chapter-image library rows out of the active app database so the home library returns to real media items.
- Kept chapter images scoped to media cards and prevented them from being treated as playable library items.
- Hardened adult metadata gather to skip missing files and non-video assets instead of stalling on dead paths or generated images.
- Replaced the Windows `cmd /c start` fallback with a quieter Explorer fallback for system-default playback launches.
- Hardened plugin and metadata-provider UI state restoration/search so incomplete saved data does not blank the app.

## Verification
- `node --experimental-strip-types --test tests/mediaPlaybackSafety.test.mjs tests/pluginUiSafety.test.mjs`: PASS
- `cargo test`: PASS
- `cargo check`: PASS (warnings only)
- `npm run build`: PASS
- `npm run tauri build`: PASS

## Artifacts
- `releases/CinaVault Premium_1.0.0_x64-setup-beta4-build2.exe`
- `releases/CinaVault Premium_1.0.0_x64_en-US-beta4-build2.msi`

## SHA256
- setup-beta4-build2.exe: B37D41E12CA660417F62DD03D77085052BBCFD1827FCAE6CAB2860A297430E0B
- msi-beta4-build2.msi: D1982EF79EB2BB299245DFFA2CF68569CFB15CE77CEA0715173EAB154137FCC8
