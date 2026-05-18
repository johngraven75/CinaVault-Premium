# CinaVault Premium Build 118

Build timestamp: 2026-05-17 23:43:10 -04:00
Version: 1.0.0-5
Branch: codex/library-enrichment-normalization

## Purpose

Build 118 fixes a media library loading hang in the installed app. The live library database contains 17,781 media rows, and the Home tab was requesting and rendering the full library in one pass. SQLite queries were responsive, but the UI could stall while creating thousands of cards and image requests.

## Changes

- Added a paged library load policy with a 240 item page size.
- Updated the Home / Library tab to request the newest page first instead of the entire library.
- Added a Load Next 240 control so users can keep browsing deeper into the library without freezing the UI.
- Preserved explicit type filtering while still bounding each request.
- Updated the AI Diagnostics refresh path so enrichment and embedded-title actions reload only the first library page instead of the entire database.
- Added Node tests for library page request construction, page continuation, and duplicate-safe page merging.

## Verification

- `node --test tests\libraryLoadPolicy.test.mjs tests\mediaPlaybackSafety.test.mjs tests\pluginUiSafety.test.mjs` passed: 14 tests.
- `npm run build` passed.
- `cargo check` passed.
- `cargo test -- --nocapture` passed: 38 tests.
- `npm run tauri -- build` passed and produced MSI plus NSIS installers.

## Artifacts

- `CinaVault-Premium-Build118-Installer.msi` - 7,536,640 bytes.
- `CinaVault-Premium-Build118-Setup.exe` - 5,396,074 bytes.

## Install Notes

Install with the MSI as a per-user package:

```powershell
msiexec /i "CinaVault-Premium-Build118-Installer.msi" MSIINSTALLPERUSER=1 ALLUSERS=2 /qn /norestart
```
