# CinaVault Premium on Linux (Debian / Ubuntu / Zorin OS)

CinaVault Premium ships a Debian package (`.deb`) and an AppImage built from the
same Tauri codebase as the Windows edition. The baseline target is **Ubuntu
22.04 LTS** (glibc + `webkit2gtk-4.1`), which also covers **Debian 12** and
**Zorin OS 17 Core**.

## Install

Download `CinaVault-Premium_<version>_amd64.deb` from the release, then:

```bash
sudo apt install ./CinaVault-Premium_*.deb
```

`apt` pulls in the runtime dependencies automatically. The AppImage
(`CinaVault-Premium_<version>_amd64.AppImage`) is a no-install alternative:

```bash
chmod +x CinaVault-Premium_*.AppImage
./CinaVault-Premium_*.AppImage
```

Verify a download against the published `SHA256SUMS.txt`:

```bash
sha256sum -c SHA256SUMS.txt
```

## Runtime dependencies

The `.deb` declares these; on a stock Zorin OS 17 Core / Ubuntu 22.04 box they
install automatically with the command above:

| Package | Why |
|---|---|
| `libwebkit2gtk-4.1-0` | Native WebView that renders the UI (auto-detected by the packager) |
| `libgtk-3-0`, `libglib2.0-0` | GTK runtime (auto-detected) |
| `libayatana-appindicator3-1` | Tray/indicator runtime loaded by the WebView stack |
| `wireguard-tools` | `wg-quick`, used for the WireGuard VPN client |

If you run the AppImage instead of the `.deb`, install `wireguard-tools`
yourself for VPN support: `sudo apt install wireguard-tools`.

## Feature parity with Windows

Everything in the Windows build is present on Linux — the unified library
engine (free tier), adult-only metadata providers (ThePornDB, StashDB, Porn
Site Nuxt, IAFD, PhoenixAdult, PGMA), the offline CLIP poster-vision skill, the
holo agent panel, the IPTV player with EPG, remote-access user accounts, and the
scan/key behaviour. Two areas use a Linux-native mechanism, and one Windows-only
feature has no Linux equivalent.

### Secret storage — Freedesktop Secret Service

Metadata provider keys and remote-access credentials are stored in the system
keyring over D-Bus: **GNOME Keyring** on GNOME/Zorin, **KWallet** on KDE (the
Windows build uses the Credential Manager). The desktop session normally runs a
Secret Service provider already. On a headless or minimal install without one,
key storage reports an error at save time rather than silently losing the key —
install and start `gnome-keyring` (or run inside a desktop session) if that
happens.

### WireGuard — `wg-quick` with a polkit prompt

The VPN client drives `wg-quick up/down` from `wireguard-tools` (the Windows
build installs a per-tunnel Windows service). Bringing a tunnel up or down needs
root, so CinaVault elevates through **`pkexec`** (polkit), which shows a
graphical authentication dialog on GNOME/Zorin and KDE — the Linux counterpart
of the Windows UAC prompt. On a system without polkit it falls back to
non-interactive `sudo` (useful with a NOPASSWD rule). Imported profiles are
stored in your user data directory with `0600` permissions.

A WireGuard interface name must stay within the kernel's 15-character limit, so
a long profile name is shortened to a stable, unique interface name; this is
internal and does not change the profile.

### Windows Defender scan — not available on Linux

The "Run antivirus scan" / "Update AV signatures" actions call Windows Defender
and return an *unsupported on this platform* status on Linux. There is no
first-party Defender equivalent; use your distribution's own tools (for example
ClamAV) if you want on-demand scanning.

### GPU-accelerated video decode

Hardware-accelerated decode (VA-API) is not wired up yet on Linux; playback
falls back to CPU decode, which is fine for most content. This is tracked as a
follow-up.

## Troubleshooting

### Blank white window on launch

On many Zorin 17 / Ubuntu 22.04 GPU and driver combinations the WebKitGTK
accelerated render path crashes the web view process at launch — the window
opens but stays blank white and the terminal shows `WebProcess didn't exit as
expected after the UI process connection was closed`. CinaVault starts the web
view with software compositing to avoid this: `gpu_startup::apply()` sets both
`WEBKIT_DISABLE_DMABUF_RENDERER=1` and `WEBKIT_DISABLE_COMPOSITING_MODE=1`
before the window is created. Video playback is unaffected — it uses the
player's own hardware decoding, not the WebKit compositor.

If a window still opens white on an unusual driver, launch from a terminal and
share the full output so the real cause is visible:

```bash
cinavault-premium 2>&1 | tail -40
```

Both variables are already exported by the app; setting them yourself on the
command line is only needed when debugging a non-standard setup.

## Building from source on Linux

**Requirements:** Ubuntu 22.04+ (or Debian 12 / Zorin OS 17), Node.js 22, the
Rust stable toolchain, and the Tauri Linux prerequisites.

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  patchelf libssl-dev libgtk-3-dev libglib2.0-dev libfuse2

git clone https://github.com/johngraven75/CinaVault-Premium.git
cd CinaVault-Premium
npm ci
npm run fetch:ai-models                 # offline vision model (~149 MB)
npx tauri build --bundles deb appimage  # produces the .deb and AppImage
```

The packages land under `src-tauri/target/release/bundle/deb/` and
`src-tauri/target/release/bundle/appimage/`.

## Continuous integration

`.github/workflows/linux-build.yml` builds the `.deb` (required) and AppImage
(best effort) on `ubuntu-22.04`, publishes them with a `SHA256SUMS.txt` as a
workflow artifact on every pull request, and — on a `v*` tag — attaches them to
the GitHub release so they sit alongside the Windows installers. The Windows
release pipeline is unchanged; the Linux build is additive.

## Code signing

The `.deb` and AppImage are unpackaged unsigned for now. A follow-up can sign
the `.deb` with `dpkg-sig` (and publish a signed `Release`/`InRelease` if an apt
repository is ever offered), mirroring the Windows code-signing setup.
