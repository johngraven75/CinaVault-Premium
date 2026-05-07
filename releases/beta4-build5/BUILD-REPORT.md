# CinaVault Premium Beta 4 Build 5

- Version: `1.0.0-beta.4+5`
- Date: 2026-05-06
- Platform: Windows x64
- Status: PASS

## Validation

- `npm run build` passed.
- `npm run tauri build` passed.
- Bundles produced for both MSI and NSIS targets.

## Artifacts

- `CinaVault Premium_1.0.0-beta.4+5_x64_en-US.msi`
  - SHA256: `743617E311D223CE7AAB7247259DD09295F2B73BD74D0D8623A870CF21F7E075`
- `CinaVault Premium_1.0.0-beta.4+5_x64-setup.exe`
  - SHA256: `FBBB2CDDAF5C7812855C865E37CF4EA61F472FECA94E93339D5B48AACD77A761`

## Included fixes

- Library can go past the old 200-item cap with load-more/load-all pagination.
- Embedded title application now updates existing library records when enabled.
- Adult metadata enrichment writes fetched details back to files in the library.
- Source scanning is more stable and avoids recursive junction/symlink loops.
