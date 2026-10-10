# CinaVault Premium documentation

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
| [promotions/](promotions/) | Promo video blueprints and assets |
| [ai-diagnostics/](ai-diagnostics/) · [weekly-summaries/](weekly-summaries/) | Reports written by the scheduled maintenance workflows |
| [history/](history/) | Past build audits, release notes, status reports and design plans, kept for reference |
