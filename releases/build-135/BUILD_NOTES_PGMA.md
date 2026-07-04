# CinaVault Premium Build 133 Installer Rebuild Notes

Build: 133
Date: 2026-06-23
Branch: main
Workflow: `.github/workflows/windows-installer.yml`
Expected artifact: `CinaVault-Premium-Windows-Installer-Build133`
Repository output folder: `releases/build-133/`

## Rebuild request

Rebuild the Windows installer from the current `main` branch and publish the installer artifacts back into the repository under `releases/build-133/` with generated SHA256 sums and an installer upload report.

This file intentionally lives outside `releases/**` so the push triggers the Windows installer workflow. The workflow copies this file into the staged installer artifact folder as `BUILD_NOTES_PGMA.md` before upload and publication.

## Required feature carry-forward

Preserve the Build 132 forward-facing UI redesign while publishing as Build 133.

## Metadata provider requirements

- PGMA Modernized metadata provider.
- Porn Site Nuxt metadata provider.
- Local Nuxt endpoint default: `http://localhost:42069/`.
- PGMA native bridge support retained.

## Verification

```powershell
npm run test:build132
npm run build
cargo test -- --nocapture
cargo check
npm run tauri build
```

## Artifact publication rule

Only publish real generated installer artifacts, hashes, build notes, and installer upload reports into `releases/build-133/`.
