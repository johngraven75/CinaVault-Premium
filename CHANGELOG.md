# Changelog

## v1.0.1 Pre-Beta · Build 101

The first pre-beta of CinaVault Premium for Windows.

### New

- **Every Feature Matrix switch works.** All 40 switches in Settings > Advanced change real behavior, each has its own settings panel where it needs one, and a test fails the build if any switch is left unwired.
- **Built-in player.** It plays files directly when it can and transcodes on the fly (with NVENC, Quick Sync or AMF) when it can't. It adds resume, skip intro and credits, a Next Up countdown, gapless playback, audio crossfade, cinema mode and buffering control.
- **Discovery shelves:** Continue Watching, Watchlist, Trending, Recommendations, Similar Titles, Genre Radio and New Releases.
- **Library automation:** smart title matching, Kodi `.nfo` import, metadata lookup after scans, OpenSubtitles downloads, chapter thumbnails, a poster sync folder and automatic collections.
- **Households:**
  - Profiles with their own progress and watchlist, plus a profile switcher.
  - Parental controls based on content ratings, behind a PIN.
  - An activity log and webhooks.
- **Media server:**
  - A Remote Access switch, and API keys you can issue and revoke.
  - A per-stream bandwidth cap and a cache layer for artwork and the library.
  - A GPU acceleration setting for the app window.
- **Holographic 3D AI agent** that can search, play and manage the library.
- **Offline vision model** that identifies films and checks posters with no key or network.
- **Unified library engine**, which shows each work once across every source, and a first-run wizard for provider keys.

### Changed

- The paywall has been removed, so every feature is available.
- Installers are signed with Azure Artifact Signing once the signing secrets are configured, and are unsigned until then.

### Fixed

- Rescans no longer reset the titles of verified or already-identified items to the filename.
- Windows CI now checks out text files with LF line endings, which fixed source-reading tests that failed on Windows.
- The SonarCloud workflow file was invalid and failed on every run. It now skips cleanly when SonarCloud isn't configured.

### Repository

- Docs are organized under `docs/`, with an index, and past build reports are moved to `docs/history/`.
- A new README with architecture, scan, playback and release diagrams.
- Old installers, build artifacts, test logs and one-off release workflows from builds 140 to 170 are removed; they remain in git history.
