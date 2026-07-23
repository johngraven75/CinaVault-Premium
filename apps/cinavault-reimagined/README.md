# CinaVault ReImagined

**Version:** 0.0.1  
**Platform:** Windows 10/11  
**Architecture:** Tauri 2 + Rust + React + TypeScript + Vite

CinaVault ReImagined is the next-generation edition of CinaVault Premium. It is intentionally isolated under `apps/cinavault-reimagined` so the current production application remains stable while features are migrated behind modern service interfaces.

## Product goals

- Instant, fluid navigation and startup
- Modern animated glass-and-aurora visual system
- Keyboard, mouse, touch and controller-friendly interaction
- Local, remote and cloud-backed media libraries
- Automatic Chromecast, AirPlay and Smart View discovery
- Serverless control-plane compatibility
- CDN-backed adaptive streaming
- Strong multi-user and parental-control foundations
- Windows MSI and NSIS installers

## Included in v0.0.1

- Buildable Tauri 2 Windows application foundation
- Animated home dashboard and navigation shell
- Responsive visual design tokens and component states
- Feature registry covering the current and planned product surface
- Rust desktop entry point and optimized release profile
- MSI/NSIS bundle configuration

## Run locally

```bash
cd apps/cinavault-reimagined
npm install
npm run desktop:dev
```

## Build Windows installers

```bash
cd apps/cinavault-reimagined
npm ci
npm run desktop:build
```

The generated installers are written beneath `src-tauri/target/release/bundle/`.

## Migration principle

The existing CinaVault Premium application remains the behavioral reference. Features will be migrated module-by-module into typed providers for libraries, playback, metadata, casting, users, live television, cloud storage and remote access.
