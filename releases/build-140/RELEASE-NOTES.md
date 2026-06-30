# CinaVault Premium Build 140 Notes

Build: 140
Source commit: b71d47bcfa48f1a8dbb8058592e328df1548abc2
Workflow run: 28471944346
Artifact name: CinaVault-Premium-Windows-Installer-Build140

Carries forward the Build 137 Cyber HUD redesign and adds the Build 140 source-scan, metadata-provider, and validation cleanup set.

Build 140 highlights:
- Keeps the Hyper-Neon Fusion Cyber HUD, Quantum Grid navigation, holographic cards, quick stats, and terminal panel experience from Build 137.
- Restores PGMA Modernized and Porn Site Nuxt metadata provider routing through Tauri command handlers.
- Requires the JavaScript surface regression test before installer creation so metadata provider wiring regressions are caught.
- Continues scanning all enabled media sources even when one source is missing or fails.
- Reports per-source scan counts, skipped disabled sources, failed sources, and ingestion errors instead of silently swallowing library upsert failures.
- Sets source item counts from files found, not only newly added files, so rescans no longer zero out source counts.
- Publishes Windows NSIS setup EXE, MSI, release notes, build notes, and SHA256 manifest.
