# CinaVault Premium — Debian / Zorin Linux parity

This document is the Linux platform record for the Windows edition
(CinaVault-Premium, main `7bd0b75c`, release `v2.1.7-beta-3`, which includes
the six fixes from PRs #190/#191 and vault encryption from PR #192).
It states, in one place, what the Linux `.deb` duplicates, what is
Linux-specific, and what is still left behind — so a Linux build is never
reported as a full platform duplicate while any item below is unported.

## Standing parity rule (2026-11-02)

All OS versions (Debian/Zorin Linux, Android, iOS) must mirror the current
CinaVault Premium Windows build. Every change to the current build carries
over to ALL versions on EVERY new build, nothing left behind, unless
Johnathan specifically says otherwise for that change. Nothing counts as
green until ALL OS versions are greenlit — checked and passing workflow
all the way through publish and release, on every platform.

Consequences, stated plainly:

- A green Linux workflow does **not** make the platform set green. Windows
  beta-3 alone does not either. The set is green only when every platform
  is greenlit through publish and release.
- A Windows change with no Linux counterpart is **left behind** until it is
  ported or explicitly exempted by Johnathan. The "Left behind" section
  below is that list for Linux. It is not a footnote; it is the open work.
- This PR is a **draft**. It builds a `.deb` artifact for inspection. It
  does not publish a release, does not submit anything to any store, and
  must not be merged as a claim of platform parity while the left-behind
  items are open.

## How the Linux build is wired (no Windows behaviour changed)

- `src-tauri/tauri.linux.conf.json` is a Tauri platform overlay. Tauri
  auto-loads `tauri.<platform>.conf.json` and merges it over
  `src-tauri/tauri.conf.json` with JSON Merge Patch (RFC 7396): objects
  merge key by key, arrays are replaced wholesale. On Linux, and only on
  Linux, `bundle.targets` becomes `["deb"]` and `bundle.resources` drops
  the Windows WireGuard bundle (`tools/wireguard/*`) in favour of
  `tools/cloudflared/*` plus the plugin configs. The main
  `tauri.conf.json` is untouched, so the Windows MSI/NSIS build, the
  store-safe (`tauri.msv1.conf.json`) build, and every test that reads the
  main config are unaffected.
- `.github/workflows/linux-installer.yml` builds on `ubuntu-latest`,
  installs the same Tauri Linux prerequisites the existing CI installs on
  Ubuntu, runs `npm ci`, bundles the local AI vision model and runs its
  end-to-end smoke test exactly as the Windows workflow does, then runs
  `npx --no-install tauri build --config src-tauri/tauri.linux.conf.json`,
  stages the resulting `.deb` with a `SHA256SUMS.txt`, prints
  `dpkg-deb --info` for it, and uploads it as an artifact
  (`CinaVault-Premium-Linux-deb`, 15 days). It publishes no release.
- The full regression suite, TypeScript checks, Rust tests and
  live-metadata acceptance already run on Ubuntu in
  `.github/workflows/ci.yml` on every pull request, so the Linux workflow
  does not duplicate them — it adds the bundle step CI does not do.

## Build and install (Debian / Zorin)

Build prerequisites (same list the workflow installs):

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf \
  libssl-dev libgtk-3-dev libglib2.0-dev
```

Then, from the repository root:

```bash
npm ci
npm run fetch:ai-models
npx --no-install tauri build --config src-tauri/tauri.linux.conf.json
# output: src-tauri/target/release/bundle/deb/*.deb
```

Install the package (apt resolves the declared runtime dependencies):

```bash
sudo dpkg -i 'src-tauri/target/release/bundle/deb/'*.deb
sudo apt-get install -f   # only needed if dpkg reports missing dependencies
```

Do **not** run `scripts/prepare-wireguard.ps1` on Linux. It throws on
non-Windows by design, and the Windows WireGuard client it stages cannot
run on Linux (see "Left behind").

## Genuinely duplicated (same code, same behaviour)

Everything in the shared Tauri + React codebase that is not
platform-gated, including:

- The full library, metadata enrichment, poster/AI vision, casting,
  discovery, downloads, collections, subtitles, chapters and HLS features
  from the Windows build, including the six PR #190/#191 fixes.
- App Vault encryption from PR #192: user-selected passphrase,
  XChaCha20-Poly1305 AEAD, Argon2id, salt + verification tag only stored,
  key in memory only (cleared on lock), sealed database + artwork while
  locked, startup gate, backup-first migration for legacy plaintext
  libraries. This is platform-neutral Rust and carries over unchanged.
- Window, security policy (CSP, asset protocol scope
  `$APPDATA/artwork/**/*`, which Tauri resolves per platform), product
  name, version `2.1.7` and bundle identifier `com.cinavault.premium`
  from the main config. On Linux the app data directory is therefore
  `~/.local/share/com.cinavault.premium` (or `$XDG_DATA_HOME` equivalent),
  not a Windows path.
- GPU switch handling in `src-tauri/src/gpu_startup.rs`, which already
  has a Linux branch: with GPU off it sets
  `WEBKIT_DISABLE_COMPOSITING_MODE=1` for WebKitGTK (the
  `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` value it also sets is ignored
  on Linux, as WebView2 does not exist there).
- VPN profile import, validation and listing
  (`src-tauri/src/vpn_profile_store.rs`): profile files, CIDR/key
  validation and the per-user-restricted profile directory work on Linux.
  Only actually bringing a tunnel up/down does not (below).
- Media-tool probing (`src-tauri/src/media_tools.rs`): `ffmpeg`,
  `ffprobe`, `yt-dlp`, `mediainfo` and `mkvmerge` are probed from `PATH`
  on Linux exactly as on Windows. See the Linux-specific note below for
  installation, because auto-install is Windows-only.
- Trash/recycle handling via the `trash` crate, which is documented in
  `Cargo.toml` as cross-platform (Windows/macOS/Linux).

## Linux-specific (same feature, different mechanism — not a gap)

- **Bundle format:** `.deb` instead of MSI/NSIS. Runtime dependencies
  are declared in `tauri.linux.conf.json` with distro alternatives
  (`libgtk-3-0t64 | libgtk-3-0`, `libappindicator3-1 |
  libayatana-appindicator3-1`) so one package installs on Debian 12,
  Ubuntu 22.04 / 24.04, Zorin 17 (Ubuntu 22.04 based) and Zorin 18
  (Ubuntu 24.04 based).
- **Web rendering:** WebKitGTK 4.1 instead of WebView2. The build
  prerequisites above are the cost of that; the UI itself is the same
  React frontend.
- **Media tools:** install via apt instead of winget, e.g.
  `sudo apt-get install -y ffmpeg mediainfo mkvtoolnix yt-dlp`.
  Until they are on `PATH`, `ensure_media_tools` reports
  `missing_tools`, which is the honest Linux equivalent of the Windows
  pre-install state — it does not silently mark tools ready.
- **Signing:** the `.deb` is unsigned. There is no Linux signing step
  configured, and this PR adds none. That matches the current Windows
  position (beta-3 is unsigned because the Azure Trusted Signing
  account/profile/Entra app do not exist yet), but it is a separate
  fact: a future Linux signing story (e.g. a signed apt repository)
  is still to be decided, not assumed.
- **Icons:** the Linux overlay lists PNG icons only (`32x32`,
  `128x128`, `128x128@2x`); the `.ico` entry in the main config is
  Windows-only and is dropped by the overlay's wholesale array replace.
  The largest icon shipped is 256 px (`icon-256.png` /
  `128x128@2x.png`); whether the `.deb` bundler accepts that without a
  512 px source is proven or disproven by the workflow run, not assumed.

## Left behind (Windows features NOT duplicated on Linux — open work)

These are flagged explicitly under the standing parity rule. None of
them blocks producing a `.deb`, and none of them may be described as
working on Linux until its own fix lands and is verified on Linux.

1. **WireGuard tunnel connect/disconnect — NOT ported.**
   `src-tauri/src/vpn.rs` gates tunnel bring-up on Windows: on any other
   OS `vpn_connect` / `vpn_disconnect` return
   "bundled WireGuard tunnels are currently supported on Windows only",
   `official_engine` always returns false off Windows, and the tunnel
   scripts are PowerShell + the Windows `WireGuardTunnel$` service
   model. The `.deb` therefore contains no WireGuard engine, and the
   system `wg` / `wg-quick` tools are not used by the app. Porting this
   means a Linux tunnel path (system WireGuard via `wg-quick` with a
   privilege model, plus a Linux engine-readiness check) — a feature
   change, not a packaging change, and it is out of scope for this
   draft. The frontend must not present VPN connect as available on
   Linux until that lands.
2. **OS keychain persistence for provider secrets — NOT ported.**
   `src-tauri/Cargo.toml` pins `keyring = { version = "3", features =
   ["windows-native"] }`, and the comment above it states the reason:
   keyring 3 has no platform store by default and silently falls back
   to an in-memory mock. The crate's documented Linux store is the
   Secret Service (`sync-secret-service` + `crypto-rust`), selected per
   target in `Cargo.toml`. On Linux today, `secure_credentials`
   (`src-tauri/src/secure_credentials.rs`) therefore writes to the mock:
   a set followed by a read-back in the same process can succeed, but
   the secret does not survive an app restart, and on a headless or
   keychain-less session even that is not guaranteed. The correct fix
   is a target-gated keyring dependency (Windows: `windows-native`;
   Linux: `sync-secret-service` + `crypto-rust`) **plus** a
   cargo-regenerated `Cargo.lock` — the lock currently lists only the
   Windows keyring backend (`windows-sys`). That lock regeneration was
   deliberately not hand-edited in this PR: several npm/Rust flows use
   `--locked`, so a hand-written lock entry risks breaking them, and
   the change must be generated by Cargo and checked on both platforms
   in its own PR. Until then, Linux provider keys must be treated as
   non-persisted, and no Linux release may claim keychain parity.
3. **Windows Defender scan / signature update — not applicable.**
   `run_antivirus_scan` / `update_av_signatures` in `vpn.rs` return
   `status: "unsupported"` with a Windows-only message off Windows.
   There is no Linux antivirus integration to port to; the honest
   Linux behaviour is that reported status, not a pretend scan. If a
   Linux scanner integration is ever wanted, it is a new feature, not
   a parity fix.
4. **Media-tool auto-install — Windows-only by design.**
   `ensure_media_tools` only attempts installation on Windows (winget).
   On Linux it reports missing tools and installs nothing. The apt
   mapping above is the user-facing equivalent; an in-app apt install
   prompt is not attempted here (it would need privilege handling and
   distro detection, and must not be smuggled into a packaging PR).
5. **Release publication — not done.**
   The Linux workflow uploads a build artifact only. No GitHub Release
   is created for Linux, no `.deb` is published, and no version is
   greenlit through release on Linux. That step needs its own explicit
   authorisation, after the left-behind items above are resolved or
   explicitly exempted, and after a `.deb` has been installed and smoke
   tested on Debian and on Zorin (not just built on Ubuntu).

## What "done" looks like for Linux

- [ ] This draft's workflow passes and produces a `.deb` + `SHA256SUMS.txt`.
- [ ] The `.deb` installs with apt-resolved dependencies on Debian and on Zorin.
- [ ] The installed app launches, unlocks a vault, and passes the same
      library/metadata/vision smoke checks as Windows beta-3.
- [ ] Keyring Secret Service port landed (own PR, Cargo-generated lock,
      verified on Windows + Linux) — or explicitly exempted by Johnathan.
- [ ] WireGuard Linux tunnel path landed — or explicitly exempted by Johnathan.
- [ ] A Linux release is published from a green workflow run, and only
      then is Linux described as greenlit — and even then, the platform
      set is green only when Android and iOS are greenlit too.

## Not targets

The dead mirrors `Cinavault-3.0-Linux` and `cinavault-premium-linu`
are not targets of this work and were not touched. The Linux duplicate
comes from the CinaVault-Premium Tauri codebase itself, on a branch/PR
against its main, exactly as the Windows build does.
