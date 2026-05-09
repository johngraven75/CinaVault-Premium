# CinaVault Premium

Windows desktop media application built with Tauri + React + Rust.

## Structure

- `src/` React UI and app state
- `src-tauri/` Rust backend and Tauri packaging config
- `releases/` versioned installer artifacts

## Local Development

```bash
npm install --legacy-peer-deps
npm run dev
npm run tauri dev
```

## Build Commands

```bash
npm run build
npm run tauri build
```

## Build Artifacts

Current sequential installer build:

- `releases/beta4-build7/CinaVault Premium_1.0.0-beta.4+7_x64-setup.exe`
- `releases/beta4-build7/CinaVault Premium_1.0.0-beta.4+7_x64_en-US.msi`
- `releases/beta4-build7/SHA256SUMS.txt`
