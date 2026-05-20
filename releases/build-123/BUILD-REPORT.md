# CinaVault Premium Build 123

Build timestamp: 2026-05-20 08:06:36 -04:00
Version: 1.0.0-10
Branch: codex/carry-forward-build-features

## Purpose

Build 123 fixes provider durability, adult metadata fallback behavior, poster generation, and full-library hydration without dropping previously added surfaces between builds.

## Changes

- Restored full-library background hydration: the first page remains capped at 240 for responsiveness, but full pages now automatically load the remaining library instead of stopping at 240.
- Added virtualization for large card grids so the full loaded library can remain responsive after hydration.
- Kept the Duplicate Finder / Deleter as a permanent app tab and regression-tested the UI and backend command wiring.
- Kept Synology QuickConnect as a visible media source path in both Cloud & NAS and Media Sources, with virtual-protocol scan preservation.
- Updated ThePornDB adult metadata lookup to try the Stash-Box GraphQL endpoint and then the REST scene API.
- Made PhoenixAdult an active local adult metadata provider instead of an unsupported/pending provider.
- Added local metadata fallback responses for providers without live API integrations, so provider searches return deterministic local metadata rather than a pending message.
- Added screenshot poster generation from media files when no sidecar or embedded poster is available.
- Updated all app/version surfaces to Build 123 / 1.0.0-10.

## Verification

- `node --test tests\libraryLoadPolicy.test.mjs tests\durableFeatureSurface.test.mjs tests\pluginUiSafety.test.mjs tests\metadataResults.test.mjs` passed: 16 tests.
- `npm run build` passed.
- `cargo test -- --nocapture` passed: 53 tests.
- `cargo check` passed with no warnings.
- `npm run tauri build` passed and produced MSI plus NSIS installers.
- Local uninstall of the previous Program Files install succeeded.
- Local install of Build 123 succeeded.
- Installed executable reports ProductVersion/FileVersion `1.0.0-10`.
- Launch smoke passed: `cinavault-premium` was responding and showed `CinaVault Premium - Media Server`.
- No fresh Windows Application Hang / WER entries for CinaVault were found immediately after launch.

## Artifacts

- `CinaVault Premium_1.0.0-10_x64_en-US.msi` - 7,532,544 bytes.
- `CinaVault Premium_1.0.0-10_x64-setup.exe` - 5,396,535 bytes.

## Notes

- External providers such as ThePornDB and StashDB still require valid keys and reachable services for live remote metadata. PhoenixAdult now works as a local provider path and the app no longer reports it as unsupported.
- The full library is no longer limited to only 240 loaded rows; the 240 item size is now the page size used for safe background hydration.
