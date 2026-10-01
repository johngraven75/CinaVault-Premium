# CinaVault foundation audit (2026-10-01)

This is the starting point for the next three workstreams: the unified library
engine, the UI overhaul, and the AI agent. It records which repository is the
base, what was merged into it, what state it builds in, and what exists versus
what is still missing.

## The base

`johngraven75/CinaVault-Premium` (`main`) is the single base. It is a Tauri v2
desktop app: React 19 + Vite 6 + Tailwind 3 + framer-motion 12 + zustand on the
front end, and a Rust back end (~20k lines, 167 Tauri commands, SQLite via
rusqlite, an embedded Axum media server).

Other repositories checked:

| Repository | What it is | Outcome |
|---|---|---|
| `CinaVault-MS-v1` (private) | Older fork of Premium (React 18) with a Microsoft Store "store-safe" edition that hides adult content | Folded in as a build mode, see below |
| `cinavault-android` | Native Kotlin/Compose client (19 files) that logs in to an owned server and browses the library | Left separate; it is a client of the server API, not part of the desktop build |
| `Cinavault-Server-Premium-Edition-iOS`, `Cinavault-3.0*`, `Cinavault-Reimagined` | Earlier or parallel attempts | Not merged; no features missing from Premium were found |

**Paywall and feature locks were not found** in any branch of any of these
repositories (searched every branch tip for paywall, entitlement, feature lock,
billing, purchase, unlock). That work still needs to be located or rebuilt.

## Changes in this foundation pass

- **Store edition merged.** `npm run build:ms-v1` (Vite mode `store-safe`) and
  `npm run tauri:build:ms-v1:windows` build the store-safe edition from this
  same codebase. `src/config/edition.ts` exposes `IS_STORE_SAFE`; adult sources,
  media, providers, plugins, filters and AI actions are gated on it, and the
  store bundle ships only `plugins/configs/store-safe/*.json`. Vite drops the
  gated code from the store bundle entirely (verified: no "Adult Media" string in
  the store build output). The Premium build is unchanged.
- **Rust back end compiles again.** `main` failed to compile: the restored
  `duplicates::find_duplicates` returns `DuplicateScanResult`, but `ai.rs` and
  `ai_automation.rs` still treated it as JSON. Both callers now use the struct.
- **App icons are RGBA again.** An ImgBot pass had saved the PNG icons as RGB,
  which makes `tauri::generate_context!` panic at compile time.
- **Carry-forward guards follow the real casting code.** Casting moved from a
  floating `CastButton` + Node `castv2-client` (which cannot run in a WebView) to
  the Casting tab and native `rust_cast` in `casting.rs`. The guards and the
  Build 140 cast test now check that implementation, and the sidebar entry is
  labelled "Casting Center" again.
- **Windows installer workflow runs the regression suite** (`npm test`) before
  building, restoring the Build 155 "Auto Build, Test, Cleanup" gate.

## Build and test state (Linux, this branch)

| Check | Result |
|---|---|
| `npm run build` (Premium) | passes |
| `npm run build:ms-v1` (store-safe) | passes |
| `npm test` (preflight gates + 28 regression tests + remote-user form tests) | 32 pass, 0 fail |
| Every `tests/*.test.mjs` file | all pass |
| `cargo test` (src-tauri) | 100 pass, 2 ignored (live network acceptance) |

Not verified here: the Windows MSI/NSIS build itself (needs the Windows runner
and the signed WireGuard download in `scripts/prepare-wireguard.ps1`).

## What exists vs. what is missing

### Unified library engine (next workstream)

Exists:
- Folder/drive/NAS/cloud sources (`scanner.rs`, `nas_devices.rs`, `cloud_storage.rs`), adult sources routed only to adult providers.
- Metadata providers wired in Rust: TMDb, OMDb, TVDb, Fanart, Trakt, TVMaze, Cinemeta (keyless), ThePornDB/TPDB, StashDB, IAFD, PGMA bridge, adult site scraper (`metadata*.rs`, `enrichment.rs`, `adult_site_provider.rs`, `pgma_bridge.rs`). Credentials stored via the OS keyring (`secure_credentials.rs`).
- Duplicate detection by name+size with SHA-256 helpers, plus remove/quarantine commands (`duplicates.rs`).
- Poster/sidecar artwork caching, NFO writing, chapter thumbnails.

Missing:
- Collapsing copies of the same title into one card. Items from every source already share one `media_items` table (unique per file path) and one grid, so two copies of a film on different drives show as two cards.
- Content-level duplicate detection (perceptual video/poster hashing); current matching is name+size only, and `similarity_threshold` is ignored.
- Zero-touch first boot: providers that need keys (ThePornDB, StashDB, TMDb, etc.) have no first-run setup flow; enrichment silently skips them.
- A bundled, local, free vision model for poster/scene identification. Today the AI path calls the Hugging Face router and needs an HF token.

### UI (workstream after that)

Exists: a "Spatial Media OS" shell with command palette (Ctrl K), orbital sidebar, Kodi-style home, cyber-HUD and v2 "future skin" CSS, framer-motion transitions, poster card standards, reduced-motion handling, and many regression guards that pin class names and tokens (`tests/build170*`, `tests/v2Build10*`).

Missing: holographic media cards, a cohesive design system (styling is spread over 14 CSS layers that patch each other), and any 3D layer (no three.js / WebGL dependency).

Watch out: the carry-forward suite (`tests/carryForwardVerification.test.mjs`, `docs/CARRY_FORWARD.md`) fails the build if listed tokens disappear. A redesign must update that registry deliberately rather than delete guards.

### AI agent (final workstream)

Exists: `ai.rs` (HF router chat completions, provider fallback), `ai_automation.rs` (`ai_library_manage`: scan, enrich, posters, NFO, duplicates, normalize, tags), `aiMediaAgent.ts` / `aiMediaAutopilot.ts` front-end services, an HF model picker tab.

Missing: a local multimodal model that works offline at install, tool-calling with a schema (actions are a fixed task list today), computer/browser use, file tools, and the animated 3D head avatar.

### Cloudless server, paywall, feature locks

Exists: embedded Axum server with password/access-key auth, opaque media keys, library/artwork/stream routes (`embedded_server.rs`); UPnP, Cloudflare tunnel relay and WireGuard device profiles (`remote_connectivity.rs`, `vpn.rs`); ADR 0001 describes the Plex-like owned-server + rendezvous design.

Missing: the rendezvous service itself (designed in ADR 0001, no code in any repo), and all paywall, licensing and feature-lock code.
