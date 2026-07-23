# CinaVault Premium Build 169 Release Status

## Implemented

- Future Horizon Casting Center UI
- Automatic device selection UI without manual device IP entry
- Chromecast, AirPlay, Samsung Smart View, and DLNA device categories
- Native Tauri casting command module
- SSDP discovery for DLNA and Smart View renderers
- mDNS discovery probes for Chromecast and AirPlay receivers
- Native reachability verification and connection state
- AirPlay playback handoff
- Casting session state and playback control bridge
- Version alignment at 1.7.169 across npm, Cargo, Tauri, and backend metadata
- Automated CI, installer build, release publishing, and safe repository maintenance workflows

## Verification gates

Build 169 must not be released until all of these are green:

1. npm clean install
2. TypeScript validation
3. Carry-forward regression tests
4. Production frontend build
5. Cargo validation
6. Windows Tauri MSI build
7. Windows Tauri NSIS build
8. Installer artifact upload
9. GitHub Release publication
10. Release asset verification

## Current infrastructure blocker

GitHub Actions runs are being created but fail before runner allocation. The Actions API exposes no checkout step, no command steps, no logs, and no diagnostic PR comment. This is a repository/account runner availability or billing/Actions-permission condition, not a reported compiler or test failure.

The repository owner must confirm GitHub Actions is enabled and that hosted runner usage/billing is available. Once runner allocation works, the self-reporting CI workflow will post exact install, type, test, frontend, and Rust outcomes to PR #42.
