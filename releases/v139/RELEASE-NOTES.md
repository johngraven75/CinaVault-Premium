# CinaVault Premium v139

Version: 1.0.139
Build: 139
Source commit: 32ff58efb843cb2a0a4d0890341168aac141f15b
Installer workflow run: 28235963492
Installer artifact: CinaVault-Premium-Windows-Installer-Build139

## Release artifacts

The Windows installer artifacts are published in `releases/build-139` and in the GitHub Actions artifact named `CinaVault-Premium-Windows-Installer-Build139`.

Posted files:

- `CinaVault Premium_1.0.139_x64_en-US.msi`
- `CinaVault Premium_1.0.139_x64-setup.exe`
- `BUILD_NOTES_PGMA.md`
- `RELEASE-NOTES.md`
- `SHA256SUMS.txt`

## SHA256

```text
00DE9D44BAAF4FD1671D737ADECA36B97F9700F83C4FEFFF49397401A41364F1  BUILD_NOTES_PGMA.md
C7D42F030AC4EDC5AC770472D60815E3241F0A95840127CC3DAA274AC3E1F8D2  CinaVault Premium_1.0.139_x64_en-US.msi
A59CE0F2561D36BFCC1D7FBC4F65CD07579F01A3421143A3CCAB2943425223E2  CinaVault Premium_1.0.139_x64-setup.exe
16C471F7E8D8E5F42BE547549FD5097C51FD340AF0F96BA9EBD6AD446319B781  RELEASE-NOTES.md
```

## Highlights

- Build 139 / v139 is now on the default `main` branch.
- Keeps the Build 137 Hyper-Neon Fusion Cyber HUD, Quantum Grid navigation, holographic library cards, quick stats, and terminal panel experience.
- Restores always-visible per-media Check Metadata actions on library cards and list/table rows.
- Adds the bulk Post Metadata & Posters action that routes through the same backend writer as the row-level Check Metadata action.
- Continues scanning all enabled media sources even when one source is missing or fails.
- Reports per-source scan counts, skipped disabled sources, failed sources, and ingestion errors instead of silently swallowing library upsert failures.
- Sets source item counts from files found, not only newly added files, so rescans no longer zero out source counts.
- Requires scanner ingestion tests plus metadata/poster posting validation before installer creation.
- Publishes Windows NSIS setup EXE, MSI, release notes, build notes, and SHA256 manifest.
