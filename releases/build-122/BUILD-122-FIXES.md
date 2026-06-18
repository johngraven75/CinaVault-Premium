# CinaVault Premium Build 122 Fixes

Build: 122
Date: 2026-06-17
Branch: main

## Fixes Applied

### Installer workflow hardening

Workflow file: `.github/workflows/windows-installer.yml`
Commit: `4e4ebcc7dd40bd53a7bdc2a5fd7dfeae9240f20a`

The workflow was rewritten to:

- Run on pushes to `main` and `codex/cinavault-pgma-plex-plugin`.
- Ignore `releases/**` paths so artifact-publishing commits do not loop the workflow.
- Grant `contents: write` permission for publishing generated installer files back into the repository.
- Use Build 122 environment values.
- Run `build-installer.ps1` on `windows-latest`.
- Stage generated `.exe`, `.msi`, and `.zip` files from `src-tauri/target/release/bundle`.
- Generate `SHA256SUMS.txt`.
- Upload the staged files as a GitHub Actions artifact named `CinaVault-Premium-Windows-Installer-Build122`.
- Publish successful outputs into `releases/build-122/`.

### App shell TypeScript build blockers

File: `src/App.tsx`
Commits:

- `9186573073c1da19598a8683f730b9f9c723a001`
- `8d15e758ed4cfdbcc9ac14691413c0de9ce70e92`

The app shell was cleanly rewritten to:

- Remove unused `setSettings` and `setTheme` bindings that conflict with `noUnusedLocals: true`.
- Await `pluginEngine.initialize()` during application startup.
- Correct wheel-scroll helper calls so `getWheelDeltaPixels` and `getWheelScrolledTop` receive their required numeric arguments.
- Keep the current shell layout, tab routing, theme application, status messages, and transition behavior intact.

## Verification Performed Through Repository Source

- Confirmed `build-installer.ps1` already runs JavaScript dependency installation, TypeScript build, Rust checks/tests, Tauri packaging, and installer discovery.
- Confirmed `tsconfig.json` enforces `noUnusedLocals: true`, making the unused app-shell bindings a real build risk.
- Confirmed `src/App.tsx` now uses the numeric wheel helper API correctly.
- Confirmed the workflow file now contains Build 122 artifact staging, hashing, upload, and repository-publish steps.

## Still Not Verified By This Chat Environment

The chat connector does not expose a completed Actions run for the latest Build 122 commit, and no generated installer upload report is present yet at `releases/build-122/INSTALLER-UPLOAD-REPORT.md`.

A successful GitHub Actions run or local Windows run is still required to prove final installer generation.
