# CinaVault Premium documentation

This index covers the guides in this folder. The root README is the
starting point for the app itself; the project policies live at the
repository root, not in this folder.

## Project policies

| Document | Topic |
|---|---|
| [../SECURITY.md](../SECURITY.md) | Supported version and private vulnerability reporting |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | Branches, pull requests, and the validation gate |
| [../AGENTS.md](../AGENTS.md) | Engineering rules for contributors |

## Release identity

`../build-version.json` is the authoritative version/build identity: the
semantic version, display build, and release tag in that file must agree
with `package.json`, `package-lock.json`, the Tauri config, and the Cargo
manifest (enforced by `tests/buildNumberDriftGuard.test.mjs`). Docs must
not rename a release on their own.

Known label mismatch, flagged rather than silently renamed: the in-code
release cycle still reads "Beta 1", while the latest published GitHub
release is tagged `v2.1.7-beta-2`. The root README therefore uses the
version and build number only. Resolving the mismatch is a
version-identity change, not a docs edit.

## Guides

| Document | Topic |
|---|---|
| [LIBRARY_ENGINE.md](LIBRARY_ENGINE.md) | Unified library, duplicate finder and first-run setup |
| [LOCAL_AI.md](LOCAL_AI.md) | The offline CLIP vision model: bundling, runtime and offline behavior |
| [HLS_STREAM_SUPPORT.md](HLS_STREAM_SUPPORT.md) | HLS and live stream playback |
| [WIREGUARD_READINESS_AUTOCONNECT.md](WIREGUARD_READINESS_AUTOCONNECT.md) | WireGuard profiles and auto-connect for remote access |
| [NATIVE_SERVER_INTEGRATION_TESTS.md](NATIVE_SERVER_INTEGRATION_TESTS.md) | How the embedded server is tested end to end |
| [CODE_SIGNING.md](CODE_SIGNING.md) | Turning on Azure Artifact Signing for installers |
| [ANDROID.md](ANDROID.md) | Building the shared project for Android |
| [DOMAIN_LANGUAGE.md](DOMAIN_LANGUAGE.md) | Terms used for accounts, servers, clients and remote access |

## Process and quality gates

| Document | Topic |
|---|---|
| [MASTER_BUILD_COMPLETION_GATE.md](MASTER_BUILD_COMPLETION_GATE.md) | What must pass before a build counts as complete |
| [CARRY_FORWARD.md](CARRY_FORWARD.md) | Registry of accepted features that every build must keep (checked in CI) |
| [platform-parity.json](platform-parity.json) | Feature parity across platforms (checked in CI) |
| [AGENTS.md](AGENTS.md) | Owner's standing build and upload instructions |

## Architecture decisions

| Document | Decision |
|---|---|
| [adr/0001-owned-server-and-rendezvous-architecture.md](adr/0001-owned-server-and-rendezvous-architecture.md) | Owned server with a rendezvous control plane |
| [adr/0002-wireguard-device-profiles.md](adr/0002-wireguard-device-profiles.md) | WireGuard device profiles |

## Other folders

| Folder | Contents |
|---|---|
| [research/](research/) | Provider research notes |
| [promotions/](promotions/) | Promo video blueprints and assets — see the size note below |
| [ai-diagnostics/](ai-diagnostics/) · [weekly-summaries/](weekly-summaries/) | Reports written by the scheduled maintenance workflows; reference material, not current guides |
| [history/](history/) | Past build audits, release notes, status reports and design plans, kept for reference — not current guides |

## Size note (flagged, not changed here)

`docs/promotions/` contains committed video, audio, and image assets
totalling roughly 76 MB, including four promo videos of about 3–18 MB
each. They are kept in this pull request: removing them would rewrite
what the history folders document, and a snapshot push cannot delete
tracked files in any case. A future cleanup should decide whether to
move promo assets to a release attachment or Git LFS instead of keeping
large binaries in the repository.
