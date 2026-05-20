# CinaVault Premium Build 122 - Provider Validation and Adult Metadata

Version: 1.0.0-9
Branch: codex/carry-forward-build-features
Build tag: build-122

## Changes

- Added ThePornDB REST metadata lookup to Adult Metadata Gather using the provider API scene search path.
- Adult Metadata Gather now tries ThePornDB before StashDB, then falls back to TMDb and OMDb.
- Provider failures are reported through `provider_errors`; a failing provider is disabled for the rest of that gather run to avoid thousands of repeated failing requests.
- PhoenixAdult and IAFD are reported as unsupported adult providers instead of being counted as working gather providers.
- Check Providers now performs live validation for TMDb, OMDb, StashDB, and ThePornDB without exposing saved API keys.
- OMDb invalid-key responses are no longer treated as valid just because the HTTP status is 200.
- The AI Diagnostics tab refreshes the loaded library page after Adult Metadata Gather so newly written poster paths appear in the user-facing UI.

## Live Provider Check

- StashDB: HTTP 200, usable with the saved key.
- TMDb / TheMovieDB Images: HTTP 200, usable with the saved key.
- ThePornDB: HTTP 401 with the saved key, so the app now reports it as invalid instead of silently treating it as working.
- PhoenixAdult: no live integration in the app; now reported as unsupported.

## Verification

- `npm run build`: passed.
- `node --test tests\*.test.mjs`: 31 tests passed.
- `cargo check`: passed.
- `cargo test -- --nocapture`: 49 tests passed.
- `npm run tauri build`: passed and produced MSI plus NSIS setup packages.

## Artifacts

- `CinaVault Premium_1.0.0-9_x64_en-US.msi`
- `CinaVault Premium_1.0.0-9_x64-setup.exe`
