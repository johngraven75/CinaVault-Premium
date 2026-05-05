# CinaVault Premium

Windows desktop media application built with Tauri + React + Rust.

## Structure

- `src/` React UI and app state
- `src-tauri/` Rust backend and Tauri packaging config
- `releases/` versioned installer artifacts

## Local Development

```bash
npm install --legacy-peer-deps
npm run build
npm run tauri build
```

## Build Artifacts

Current sequential installer build:

- `releases/build-112/CinaVault-Premium-Build112-Installer.exe`
- `releases/build-112/CinaVault-Premium-Build112-Installer.msi`
- `releases/build-112/SHA256SUMS.txt`
