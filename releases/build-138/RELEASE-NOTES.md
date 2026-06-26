# CinaVault Premium Build 138 Notes

Build: 138
Source commit: 34d9a4fe86bf7220cc982b09442f03423baf1eb0
Workflow run: 28212713158
Artifact name: CinaVault-Premium-Windows-Installer-Build138

Carries forward the Build 137 Cyber HUD redesign and adds the Build 138 metadata repair validation set.

Build 138 highlights:
- Keeps the Hyper-Neon Fusion Cyber HUD, Quantum Grid navigation, holographic cards, quick stats, and terminal panel experience from Build 137.
- Restores always-visible per-media Check Metadata actions on library cards and list/table surfaces.
- Reports metadata gather results with scanned, enriched, updated-field, poster, sidecar, and chapter-image counts instead of a generic completion message.
- Requires a Rust regression test proving metadata and poster URLs are posted to an actual media row before installer creation.
- Publishes Windows NSIS setup EXE, MSI, release notes, build notes, and SHA256 manifest.
