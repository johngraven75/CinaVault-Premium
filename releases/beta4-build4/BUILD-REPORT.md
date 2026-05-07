# CinaVault Premium Beta 4 Build 4

- Version: `1.0.0-beta.4+4`
- Date: 2026-05-06
- Platform: Windows x64
- Status: PASS

## Validation

- `npm run tauri build` passed.
- Bundles produced for both MSI and NSIS targets.

## Artifacts

- `CinaVault Premium_1.0.0-beta.4+4_x64_en-US.msi`
  - SHA256: `81F0B75BAF7C3FB9B64BC49CD564F8BC671A6F82F48B821C84E535C2A467216F`
- `CinaVault Premium_1.0.0-beta.4+4_x64-setup.exe`
  - SHA256: `F9E9C8163801C9DF03B11129CAFDCE98D2A38B087415EC6F3238325934A43EC8`

## Included fixes

- Library no longer capped at 200 items.
- Source scan stability improved (no recursive symlink/junction loops).
- Embedded title extraction improved from media tags.
- Adult metadata gather now writes back fetched metadata fields.
