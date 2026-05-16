# CinaVault Premium Build 115

Date: 2026-05-16

## Corrective Changes

- Fixed adult metadata gather routing so the prompt `Run adult metadata gather for installed providers and generate posters and chapter images` now runs the adult metadata gather path instead of being intercepted by the generic media/source check.
- Added progress tracking for library metadata enrichment, filename normalization, and adult metadata gathering.
- Added a user-facing percentage popup in AI Diagnostics that reports metadata task status, item counts, and completion.
- Updated scanner poster handling so later scans can repair existing media rows that still have a blank poster path.
- Updated database scan upsert behavior so a newly discovered poster fills an empty `poster_path` without overwriting existing artwork.
- Added local project instructions requiring fresh, intentional fixes and user-facing UI verification.

## Verification

- `cargo test --manifest-path src-tauri\Cargo.toml`: 26 passed.
- `node --test tests\*.test.mjs`: 11 passed.
- `npm run build`: passed.
- `npm run tauri -- build`: passed and produced MSI plus NSIS installers.
- Rendered desktop UI QA at `http://127.0.0.1:1420/`: AI Diagnostics -> Adult Metadata Gather showed the popup at 25% and 100%.
- Rendered mobile UI QA at `390x844`: AI Diagnostics -> Adult Metadata Gather showed the popup at 50% without blocking the screen.
- Browser plugin path timed out during setup twice, so UI QA used Playwright against installed Chrome with mocked Tauri command responses for deterministic progress states.

## Artifacts

- `CinaVault-Premium-Build115-Installer.exe`
- `CinaVault-Premium-Build115-Installer.msi`
- Desktop build copy: `C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\Cinavault-Standalone-Installer-v115.exe`
- Desktop MSI copy: `C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\CinaVault-Premium-Build115-Installer.msi`

## SHA256

- `491817C32CAE072C598A7642B26EB2972389607516AB24D36C152A5AF118601F`  `CinaVault-Premium-Build115-Installer.exe`
- `6F9055EE62DFA442C29EF467027A20DFBA3CBEE7AB0CAB97838F30FD707AFC63`  `CinaVault-Premium-Build115-Installer.msi`
