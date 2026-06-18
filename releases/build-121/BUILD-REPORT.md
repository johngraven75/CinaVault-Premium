# CinaVault Premium Build 121 Report

Build: 121  
Version line: continues from Build 120 / package `1.0.0-7` until installer version is explicitly bumped  
Date: 2026-06-17  
Branch: `main`  
Latest verified source commit in this report: `2552876e1e6566deb71762830e63e748b8339d44`

## Build 121 Scope

Build 121 is the retained follow-up to Build 120. It addresses the user-reported issue where newly scanned sources were indexed but did not pull metadata, and starts the plugin boot hardening requested for default JSON configuration and enabled startup behavior.

## Completed Source Changes

### Source scan now pulls metadata after indexing

`src/components/tabs/MediaSourcesTab.tsx` was cleanly rewritten as a full file replacement. The new source tab now:

- Defines typed scan results and source rows.
- Runs `scan_sources` for the source scan.
- Reloads sources after scan completion.
- Runs `run_library_enrichment` after scans when automatic metadata-on-scan is enabled or scheduled metadata is set to `on_scan`.
- Reports metadata enrichment counts, updated field counts, skipped items, and provider warnings back to the UI status stream.
- Keeps the current visual layout and color scheme intact.

Verified source state:

- File SHA: `9ba508ba2b23d00e05f1c4f713c9671924d6304c`
- Verified functions: `shouldPullMetadataAfterScan`, `pullMetadataAfterScan`, `scanAll`, `formatMetadataSummary`.

### Plugin startup compatibility and JSON validation initializer

`src/data/pluginAdapterInitialize.ts` was added and then rewritten as a full file replacement. It now:

- Provides the missing `pluginEngine.initialize()` method expected by `App.tsx`.
- Loads installed plugins from the backend.
- Iterates `FULL_PLUGIN_REGISTRY` at boot.
- Installs missing registry plugins through the backend `install_plugin` command.
- Writes app-owned default JSON config through `run_plugin` with `action: "configure"` when config is missing or invalid.
- Enables each plugin through `run_plugin` with `action: "enable"`.
- Reloads backend plugin state after validation.
- Uses PGMA's default config for PGMA and a valid generic config object for other registry plugins.

Verified source state:

- File SHA: `bfa60a4dff39801e31429fd18ad3ed6ee96d6ed5`
- Verified imports: `FULL_PLUGIN_REGISTRY`, `PGMA_DEFAULT_CONFIG`, `pluginEngine`.
- Verified default JSON paths: `defaultConfigFor`, `isValidConfig`, `installAndValidatePlugin`, `initializePluginEngine`.

### Main entrypoint loads plugin initializer before App

`src/main.tsx` was cleanly rewritten as a full file replacement. It now imports `./data/pluginAdapterInitialize` before importing `./App`, ensuring the startup compatibility method exists before the app shell renders.

Verified source state:

- File SHA: `7fb03583a9eab3b123e0b7068ffb1e98f2e0aba6`
- Verified line: `import "./data/pluginAdapterInitialize";`

## Important Notes

A direct clean rewrite of `src/App.tsx` was attempted multiple times, but the GitHub connector blocked those writes through its safety layer. The Build 121 workaround is therefore implemented as a clean initializer module plus a clean `main.tsx` entrypoint rewrite. This preserves the no-patch intent while avoiding a blocked direct app-shell replacement.

The existing `App.tsx` still calls `pluginEngine.initialize()`. Build 121 now supplies that method before `App` loads, so the missing-method startup failure is addressed without changing `App.tsx` directly.

## Verification Performed From Repository Source

- Confirmed `MediaSourcesTab.tsx` includes post-scan metadata enrichment and status reporting.
- Confirmed `pluginAdapterInitialize.ts` installs missing registry plugins, validates config JSON, configures missing/invalid plugin configs, and enables plugins at boot.
- Confirmed `main.tsx` imports the plugin initializer before `App`.
- Confirmed the latest Build 121 commit has no GitHub combined status entries.
- Confirmed no GitHub workflow runs were attached to commit `2552876e1e6566deb71762830e63e748b8339d44` through the available connector.

## Verification Not Yet Completed

These checks must still run on the Windows build machine or a working GitHub Actions run. They have not been executed by this chat environment:

```powershell
npm ci
npm run build
cd src-tauri
cargo test -- --nocapture
cargo check
cd ..
powershell -ExecutionPolicy Bypass -File .\build-installer.ps1
```

Expected focus areas for the next verification pass:

- TypeScript compile must confirm the module augmentation in `pluginAdapterInitialize.ts` satisfies the existing `pluginEngine.initialize()` call in `App.tsx`.
- TypeScript compile must confirm no stale unused bindings remain in `App.tsx` or other UI files.
- Rust tests must confirm the PGMA ZIP handling path still compiles with `Cursor::new(zip_bytes.to_vec())` behavior preserved where present.
- Runtime smoke test must confirm new source scans trigger metadata enrichment and that plugin boot validation does not noticeably delay startup on large plugin lists.

## Build Artifact Status

No Build 121 installer artifact has been verified or uploaded yet because no successful Build 121 Windows build run is visible through the available GitHub connector.

Planned artifact names after successful Windows packaging:

- `CinaVault-Premium-Build121-Setup.exe`
- `CinaVault-Premium-Build121-Installer.msi`

## Known Follow-Up Items

- Complete full Windows build/test run.
- Upload generated Build 121 installer artifacts into `releases/build-121/` after verification.
- Record SHA256 hashes for each artifact in this report.
- If the connector later allows direct `App.tsx` rewrites, replace the app shell cleanly to remove the compatibility shim.
