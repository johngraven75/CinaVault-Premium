# CinaVault Premium Build 137 Installer Rebuild Notes

Build: 137
Date: 2026-06-25
Branch: main
Workflow: `.github/workflows/windows-installer.yml`
Expected artifact: `CinaVault-Premium-Windows-Installer-Build137`
Repository output folder: `releases/build-137/`

## Rebuild request

Rebuild the Windows installer from the current `main` branch and publish the installer artifacts back into the repository under `releases/build-137/` with generated SHA256 sums and an installer upload report.

The workflow copies this file into the staged installer artifact folder as `BUILD_NOTES_PGMA.md` before upload and publication.

## Required feature carry-forward

Preserve the Build 137 Cyber HUD UI while carrying forward the Build 132/136 media, metadata, plugin, and installer behavior.

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

Only publish real generated installer artifacts, hashes, build notes, and installer upload reports into `releases/build-137/`.
