# CinaVault Premium Beta 4 Build 6

- Version: `1.0.0-beta.4+6`
- Date: 2026-05-06
- Platform: Windows x64
- Status: PASS

## Validation

- `npm run build` passed.
- `npm run tauri build` passed.
- Bundles produced for both MSI and NSIS targets.

## Artifacts

- `CinaVault Premium_1.0.0-beta.4+6_x64_en-US.msi`
  - SHA256: `3363E8E829CE3B71921AD4FC4AE8B38A9BF2E7827701FC55F4A46F728192C9B9`
- `CinaVault Premium_1.0.0-beta.4+6_x64-setup.exe`
  - SHA256: `08D00581E0F880B6A6FB61C75099B5F11186FB2EB628B12FECAF798739125581`

## Included fixes

- Library now requests and displays the full media list by default (no 200 item cap).
- Library cards keep reduced animation overhead at large item counts for responsiveness.
- Embedded-title apply path remains active for existing library records.
