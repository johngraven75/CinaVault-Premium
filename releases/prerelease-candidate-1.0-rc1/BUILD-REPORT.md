# CinaVault Premium PreRelease Candidate 1.0 RC1

- Channel: PreRelease Candidate
- Version (MSI-compatible): `1.0.0-1`
- Label: `1.0 RC1`
- Date: 2026-05-11
- Platform: Windows x64
- Status: PASS

## Validation

- `cargo test --manifest-path src-tauri/Cargo.toml ai::tests` passed.
- `cargo test --manifest-path src-tauri/Cargo.toml metadata::tests` passed.
- `npm run build` passed.
- `npm run tauri build` passed.

## Fixed in this RC

- Adult metadata gather now normalizes legacy provider IDs (example: `themoviedb_images` -> `tmdb`, `theporndb` -> `tpdb`) for backward compatibility.
- Adult metadata gather now queries StashDB metadata and merges it with TMDb/OMDb when available.
- Remote posters can replace local placeholder poster files (such as `*-poster.jpg`) to avoid photo-frame posters on media cards.
- Chapter image generation limit was removed so large libraries (9k+ files) are not skipped by a hard cap.
- Adult metadata gather continues writing sidecar metadata files (`*.cinavault.json`) next to media files.

## Artifacts

- `CinaVault Premium_1.0.0-1_x64_en-US.msi`
  - SHA256: `D15033EBA64C351693773CC82F5976CF649964514850B7FB49212E1434C4AAC8`
- `CinaVault Premium_1.0.0-1_x64-setup.exe`
  - SHA256: `96F83D49105C6499C7552698EF4801F51F59F6A1AFCC3DC0452964E90547AECA`
- `CinaVault-Premium-PreRelease-Candidate-1.0-RC1-Windows-x64.zip`
