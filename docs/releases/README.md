# Release milestones

Per-release reference notes for CinaVault Premium's shipped milestones, newest
last. Each doc covers the version and build, release date, user-facing
highlights, bug fixes, breaking changes and migration, and known issues. Content
is drawn from the pull requests actually merged for each release, not from
planned work.

These are the curated milestone docs for the **1.0.x** line. Raw, build-by-build
notes from earlier builds are kept under [../history/releases/](../history/releases/).

| Milestone | Build | Date | Notes |
|---|---|---|---|
| [v1.0.1 Pre-Beta](v1.0.1-pre-beta-build-101.md) | 101 | 2026-10-10 | Foundation reset: one buildable base, paywall removed, Store edition, working library controls |
| [v1.0.2 Pre-Beta](v1.0.2-pre-beta-build-102.md) | 102 | 2026-10-10 | Holo agent gains a Claude brain (vision, library actions); MediaInfo freeze fix |
| [v1.0.3 Pre-Beta](v1.0.3-pre-beta-build-103.md) | 103 | 2026-10-11 | Optional animated photo face for the agent, fully on-device |
| [v1.0.4 Pre-Beta](v1.0.4-pre-beta-build-104.md) | 104 | 2026-10-11 | AI agent on Android/iOS (read-only); Windows scan/metadata/poster fixes |
| [v1.0.5 Public Beta](v1.0.5-public-beta-build-105.md) | 105 | releasing | Adult-only library metadata (full edition); free AI poster vision; Pre-Beta → Public Beta |

## Conventions

- **Reference, not tutorial.** These docs describe what each release contains.
  How-to guides live in the main [docs/](../README.md) folder.
- **Plain language.** Written for people using the app, not for the build system.
- **Platform.** All 1.0.x builds so far ship Windows installers (MSI and
  NSIS/EXE). Installers are currently unsigned; see
  [../CODE_SIGNING.md](../CODE_SIGNING.md).
