# CinaVault Premium — Media Server

**Engine:** Tauri v2 + Rust + React/TypeScript  
**Build:** v109 Premium Rewrite — Public Beta 1  
**Brand:** CinaVault Emby Fusion

---

## Features

### 11-Tab Premium UI
- **Library** — Card/list views, search, filters, shelves (Recent, Verified, Unverified, Favorites), media spotlight
- **Media Sources** — Folder/drive/file sources, AI source discovery, web/playlist download links
- **Downloads** — yt-dlp + ffmpeg engine, playlist mode, tool auto-install via winget
- **Live TV** — Xtream Codes IPTV profiles, channel groups, EPG sync, stream playback
- **Server** — Jellyfin/Emby server start/stop, compatibility check, library import, admin console
- **Security** — Windscribe VPN (10 locations), Windows Defender quick scan, AV signature updates
- **Advanced** — Emby SDK feature matrix (40+ toggles), media requests, user groups, integrations
- **Cloud & NAS** — WD My Cloud, Synology QuickConnect, remote endpoints, Dropbox/OneDrive/GDrive placeholders
- **Plugins & Metadata** — 30+ metadata providers, API key management, plugin repository system
- **AI Diagnostics** — HuggingFace inference, network diagnostics, source checks, provider checks, AI visualizer
- **Settings** — 6 premium themes, playback config, library options, duplicate finder, power/safety controls

### Visual Design
- Glassmorphism panels with backdrop blur
- Animated starfield + nebula + comet header
- Particle field effects
- AI activity visualizer (rotating ring + waveform)
- 6 theme presets (Emby SDK Classic, Jellyfin Aurora, Windows 11 Glass, Neon Night, Ocean Drive, Sunset Pulse)
- Smooth tab transitions via Framer Motion
- Animated sidebar with layout transitions
- Premium input/button/toggle components
- Zebra-striped data rows with hover effects

### Tech Stack
- **Frontend:** React 18 + TypeScript + Tailwind CSS + Framer Motion + Zustand + Lucide Icons
- **Backend:** Rust + rusqlite + reqwest + tokio + walkdir + sha2
- **Framework:** Tauri v2 with plugins (dialog, fs, shell, process, os, clipboard)
- **Build:** Vite + esbuild (fast builds, tree-shaking, code splitting)
- **Installer:** NSIS + MSI via Tauri bundler

---

## Quick Start

### Prerequisites
- [Node.js 20+](https://nodejs.org/)
- [Rust / Cargo](https://rustup.rs/)
- [Tauri CLI](https://tauri.app/start/): `cargo install tauri-cli --version "^2.0"`

### Development
```bash
npm install
npm run tauri dev
```

### Build Windows Installer
```bash
npm install
npm run tauri build
```

Installers will be in:
- `src-tauri/target/release/bundle/nsis/` (NSIS .exe)
- `src-tauri/target/release/bundle/msi/` (MSI)

### Automated CI/CD
Push to the `tauri-premium` branch or create a version tag to trigger the GitHub Actions workflow that automatically builds the Windows installer and creates a GitHub release.

---

## Project Structure

```
cinavault-premium/
├── src/                          # React/TypeScript frontend
│   ├── main.tsx                  # Entry point + splash screen
│   ├── App.tsx                   # Main shell (sidebar, header, tab router)
│   ├── themes.ts                 # 6 theme presets + CSS variable system
│   ├── store/appStore.ts         # Zustand global state
│   ├── styles/index.css          # Tailwind + premium CSS
│   └── components/
│       ├── Sidebar.tsx           # Animated sidebar navigation
│       ├── Header.tsx            # Starfield + nebula + comet header
│       ├── effects/
│       │   ├── ParticleField.tsx # Particle animation canvas
│       │   └── AIVisualizer.tsx  # AI ring + waveform canvas
│       └── tabs/
│           ├── HomeTab.tsx
│           ├── MediaSourcesTab.tsx
│           ├── DownloadsTab.tsx
│           ├── LiveTVTab.tsx
│           ├── ServerTab.tsx
│           ├── SecurityTab.tsx
│           ├── AdvancedTab.tsx
│           ├── CloudNASTab.tsx
│           ├── PluginsTab.tsx
│           ├── AIDiagnosticsTab.tsx
│           └── SettingsTab.tsx
├── src-tauri/                    # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs               # App setup + command registration
│       ├── db.rs                  # SQLite database layer
│       ├── scanner.rs             # Media file scanner
│       ├── iptv.rs                # Xtream Codes IPTV
│       ├── jellyfin.rs            # Jellyfin/Emby server mgmt
│       ├── plugins.rs             # Plugin repository system
│       ├── player.rs              # Media player detection
│       ├── metadata.rs            # 30+ metadata providers
│       ├── chapters.rs            # Chapter thumbnail gen
│       ├── duplicates.rs          # Duplicate file finder
│       ├── vpn.rs                 # Windscribe VPN + AV
│       ├── downloads.rs           # yt-dlp download engine
│       └── ai.rs                  # HuggingFace AI inference
├── .github/workflows/
│   └── build-windows.yml         # CI/CD for Windows installer
├── package.json
├── vite.config.ts
├── tailwind.config.js
├── tsconfig.json
└── index.html
```

---

## License

Proprietary — CinaVault Premium
