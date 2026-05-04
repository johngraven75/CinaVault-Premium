# CinaVault Premium Build Report (May 4, 2026)

## Completed build artifacts
- NSIS installer: `builds/windows/CinaVault Premium_1.0.0_x64-setup.exe`
- MSI installer artifact: `builds/windows/CinaVault Premium_1.0.0_x64_en-US.msi`

## Build verification
- `npm run build`: passed
- `npm run tauri build -- --bundles nsis`: passed
- `npm run tauri build` (msi + nsis): msi bundling failed with `Access is denied (os error 5)` at MSI output write step.

## Notes
- The new installer was also copied to `C:\Users\johng\OneDrive\Documents\Desktop\cinavault builds`.
- MSI artifact in the desktop builds folder may be from an earlier successful run due write lock on the MSI target path.
