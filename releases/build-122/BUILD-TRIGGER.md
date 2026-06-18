# CinaVault Premium Build 122 Installer Trigger

Build: 122
Date: 2026-06-17
Branch: main
Purpose: trigger the GitHub Actions Windows installer workflow after the Build 121 source-side fixes.

Expected workflow: `.github/workflows/windows-installer.yml`
Expected artifact: `CinaVault-Premium-Windows-Installer-PGMA`

Build 122 should compile the current `main` branch, run the repository installer script on `windows-latest`, stage generated `.exe`, `.msi`, and `.zip` files from `src-tauri/target/release/bundle`, and upload them as a GitHub Actions artifact.

Verification required after the workflow completes:

- Confirm the workflow exits successfully.
- Confirm the artifact contains the generated Windows installer file or files.
- Download the artifact ZIP from GitHub Actions.
- Record installer filenames and SHA256 hashes in the build report.
