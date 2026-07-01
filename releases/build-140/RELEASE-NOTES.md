# CinaVault Premium Build 140 Notes

Build: 140
Source commit: 984a860585d6f82b3664ac24703d66e030c8ed37
Workflow run: 28490225580
Artifact name: CinaVault-Premium-Windows-Installer-Build140

Carries forward the Cyber HUD redesign and adds the Build 140 source-scan, metadata-provider, and validation cleanup set.

Build 140 highlights:
- Keeps the Hyper-Neon Fusion Cyber HUD, Quantum Grid navigation, holographic cards, quick stats, and terminal panel experience.
- Restores PGMA Modernized and Porn Site Nuxt metadata provider routing through Tauri command handlers.
- Requires JavaScript governance, production web build, Rust module validation, and Tauri packaging before artifact upload.
- Continues scanning all enabled media sources even when one source is missing or fails.
- Reports per-source scan counts, skipped disabled sources, failed sources, and ingestion errors instead of silently swallowing library upsert failures.
- Sets source item counts from files found, not only newly added files, so rescans no longer zero out source counts.
- Publishes Windows NSIS setup EXE, MSI, release notes, build notes, and SHA256 manifest.
