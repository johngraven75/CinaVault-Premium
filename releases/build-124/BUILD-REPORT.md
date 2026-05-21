# CinaVault Premium Build 124

Build timestamp: 2026-05-20 20:14:05 -04:00
Version: 1.0.0-11
Branch: codex/carry-forward-build-features

## Purpose

Build 124 integrates the provided `porn-site-nuxt.bundle.zip` API contract as an adult metadata provider that can retrieve scene metadata and write it through CinaVault's adult metadata gather pipeline.

## Changes

- Added a shared `adult_site_provider` Rust parser for the Nuxt bundle's `/search?q=...` response shape.
- Added provider aliases for `porn_site_nuxt`, `porn-site-nuxt`, `pornhub-irene`, and `irenehub`.
- Added `Porn Site Nuxt` to the backend metadata provider list and the frontend metadata provider catalog.
- Added metadata search support for the bundle API using the configured provider value as the base URL, defaulting to `http://localhost:42069/`.
- Added adult gather support so `PornEntry` results can update database metadata and `*.cinavault.json` sidecars through the same writeback path as ThePornDB/StashDB/PhoenixAdult.
- Parsed bundled-provider fields into title, source URL overview, poster/thumbnail path, rating, genre, and a stable provider identifier.
- Added short request timeouts so the provider disables itself for the current gather run if the local Nuxt API service is not running.
- Updated app/version surfaces to Build 124 / 1.0.0-11.

## Verification

- `node --test tests\libraryLoadPolicy.test.mjs tests\durableFeatureSurface.test.mjs tests\pluginUiSafety.test.mjs tests\metadataResults.test.mjs` passed: 17 tests.
- `npm run build` passed.
- `cargo test -- --nocapture` passed: 55 tests.
- `cargo check` passed.
- `npm run tauri build` passed and produced MSI plus NSIS installers.
- Local uninstall of the previous Program Files install succeeded.
- Local install of Build 124 succeeded.
- Installed executable reports ProductVersion/FileVersion `1.0.0-11`.
- Launch smoke passed: `cinavault-premium` was responding and showed `CinaVault Premium - Media Server`.
- No fresh Windows Application Hang / WER entries for CinaVault were found immediately after launch.

## Artifacts

- `CinaVault Premium_1.0.0-11_x64_en-US.msi` - 7,565,312 bytes.
- `CinaVault Premium_1.0.0-11_x64-setup.exe` - 5,413,774 bytes.

## Provider Notes

- The bundled Nuxt frontend expects an API service at `http://localhost:42069/` with `/search?q=...` returning `content` entries shaped like `PornEntry` records.
- If a different base URL is needed, save it as the `porn_site_nuxt` provider value. CinaVault treats URL-shaped values as the provider base URL.
- If the service is unavailable during adult gather, CinaVault logs the provider error once, disables that provider for the current run, and continues with the other providers and local writeback.
