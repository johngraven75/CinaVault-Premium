# CinaVault Premium Beta 4 Build 7

- Version: `1.0.0-beta.4+7`
- Date: 2026-05-09
- Platform: Windows x64
- Status: PASS

## Validation

- `npm run build` passed.
- `cargo test --manifest-path src-tauri/Cargo.toml ai::tests` passed.
- `cargo test --manifest-path src-tauri/Cargo.toml metadata::tests` passed.
- `npm run tauri build` passed.
- Bundles produced for both MSI and NSIS targets.

## Artifacts

- `CinaVault Premium_1.0.0-beta.4+7_x64_en-US.msi`
  - SHA256: `15F3A5D50511A48AE8411A54D31BE16D4684D6DFFCD7A530A757FEA37BDDEC13`
- `CinaVault Premium_1.0.0-beta.4+7_x64-setup.exe`
  - SHA256: `2D6A3BE69081DC2A123E305D6BB8A4368830C596324D9C88B3DE19F9FC8B3252`
- `CinaVault-Premium-Beta4-Build7-Windows-x64.zip`

## Included fixes

- Fixed CinaVault/CineVault naming typo in remote access UI copy.
- Hardened API key testing so unknown providers are no longer auto-validated.
- Adult metadata gather now writes metadata sidecar files (`*.cinavault.json`) beside library media files.
- Poster/backdrop image paths are now normalized for local filesystem rendering in the library UI.
- Updated local development docs with explicit dev-run commands.
- Added Rust unit tests for provider validity assumptions and adult sidecar path derivation.
