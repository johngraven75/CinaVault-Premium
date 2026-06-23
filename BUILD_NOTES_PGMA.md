# CinaVault Premium Build 132 Installer Rebuild Notes

Build: 132
Date: 2026-06-23
Branch: main
Workflow: `.github/workflows/windows-installer.yml`
Expected artifact: `CinaVault-Premium-Windows-Installer-Build132`
Repository output folder: `releases/build-132/`

## Rebuild request

Rebuild the Windows installer from the current `main` branch and publish the installer artifacts back into the repository under `releases/build-132/` with generated SHA256 sums and an installer upload report.

This file intentionally lives outside `releases/**` so the push triggers the Windows installer workflow. The workflow copies this file into the staged installer artifact folder as `BUILD_NOTES_PGMA.md` before upload and publication.

## Required feature carry-forward

This rebuild must preserve the Build 132 forward-facing UI skin redesign:

- Futuristic application shell using `app-shell`, `app-shell-orb`, and `app-shell-noise` layers.
- Rounded glass command-deck workspace container.
- Animated tab transitions with blur/lift/scale state.
- Futuristic glass sidebar navigation rail with active panel and active rail glow.
- Cinematic command header with Build 132 identity text, tab subtitles, search, live clock, fullscreen control, notification command feed, and animated canvas background.

## Metadata provider additions required in this rebuild

- PGMA Modernized metadata provider is exposed in the frontend provider list as `pgma` / `PGMA Modernized`.
- PGMA uses the native CinaVault PGMA bridge for local sidecar/artwork metadata behavior.
- Porn Site Nuxt metadata provider is exposed as `porn_site_nuxt` / `Porn Site Nuxt`.
- Porn Site Nuxt aliases include `porn-site-nuxt`, `porn site nuxt`, `pornsite_nuxt`, `pornhub-irene`, `pornhub_irene`, `irenehub`, `irene_hub`, and `nuxt_porn_site`.
- Porn Site Nuxt default API base URL is `http://localhost:42069/` and search calls use `/search?q=...`.

## Verification expected before artifact publication

Run the normal Windows installer workflow checks and packaging path:

```powershell
npm run test:build132
npm run build
cargo test -- --nocapture
cargo check
npm run tauri build
```

The workflow should then stage `.exe`, `.msi`, `.zip` if present, this build note file, and `SHA256SUMS.txt`, upload the GitHub Actions artifact, and commit the generated installer files into `releases/build-132/`.

## Artifact publication rule

Do not manually place placeholder installer binaries in `releases/build-132/`. Only publish generated installer artifacts from the Windows build workflow. The repository output folder should contain the real installer files, `SHA256SUMS.txt`, `BUILD_NOTES_PGMA.md`, and `INSTALLER-UPLOAD-REPORT.md` after the workflow finishes.
