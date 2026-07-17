# CinaVault Premium Build 165

Build 165 restores real, observable work to AI media-management commands and strengthens the permanent carry-forward contract.

## Restored and improved

- FFmpeg, FFprobe, yt-dlp, MediaInfo, and MKVToolNix are checked and silently installed/loaded at application startup without an in-app authorization step.
- AI metadata enrichment and title cleanup now invoke native library automation and report actual changed/error counts.
- AI source discovery now finds real media directories and persists enabled SQLite sources.
- WD My Cloud username/password authentication reuses a live session and creates reachable scanner-compatible shares.
- Synology QuickConnect resolves, authenticates, mounts, and persists reachable shares as sources.
- Adult metadata startup/runtime routing covers TPDB, StashDB, Porn Site Nuxt, IAFD, PhoenixAdult, and PGMA.
- Provider and plugin JSON files are CI-validated for syntax, identity, enablement, uniqueness, and usable endpoints.
- Poster acquisition validates image payloads, writes sidecars atomically, persists local paths, and handles card rendering failures.
- Regression tests verify actual database and filesystem effects, not success strings.
- Installer release publication is main-branch-only and produces MSI, NSIS EXE, and SHA-256 checksums.

No prior feature was intentionally removed. See `docs/BUILD_165_CARRY_FORWARD_AUDIT.md` for the release-by-release audit and acceptance gates.
