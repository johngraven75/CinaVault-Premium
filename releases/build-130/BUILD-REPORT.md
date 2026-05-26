# CinaVault Premium Build 130

Version: 1.0.0-18
Branch: codex/build-130-server-startup-remote-guide
Date: 2026-05-25

## What Changed

- Added Jellyfin/Emby startup hygiene before CinaVault launches the media server.
- Disables Jellyfin/Emby auto-discovery and UPnP in detected `config/network.xml` files to stop the repeated SSDP socket error against `239.255.255.250:1900`.
- Removes startup triggers from the heavy `RefreshPeople` and `RefreshTrickplayImages` tasks while preserving the tasks for manual or scheduled use.
- Keeps a one-time `.cinavault.bak` backup beside any external server config file before changing it.
- Surfaces startup preparation actions in the Server tab status ticker.
- Added a Remote Access guide explaining how to connect to a remote CinaVault media server with server URL, user ID/password, and issued access token.
- Replaced generated/old branding with the exact provided `Copilot_20260525_223336.png` logo asset.
- Integrated the provided logo into the existing animated startup splash.
- Removed the second startup brand screen so launch now uses one startup screen.
- Carried forward Build 129 behavior and fixes.

## Startup Log Issue Addressed

Observed local Jellyfin evidence showed:

- 688 `Failed to get person` warnings during people validation.
- `Refresh People` and `Generate Trickplay Images` running from startup triggers.
- Repeated SSDP socket errors from `192.168.5.174` to `239.255.255.250:1900`.

Build 130 avoids the recurring startup flood by preventing the two heavy library-maintenance tasks from firing at launch and by disabling discovery/UPnP before the server process starts.

## Verification

- `npm run build` passed.
- `cargo test` passed: 61 tests.
- `npm run tauri build` passed.
- Visual startup check passed: exact provided logo appears on the first splash, and no second brand splash is present.
- Remote Access page check passed: the new guide includes server address, user ID/password, access token, and confirm access steps.
- Branding file hashes matched the provided source PNG.
- Local installer smoke test passed: NSIS installer exit code 0, installed executable version 1.0.0-18, process responding true, window title `CinaVault Premium - Media Server`.

## Artifacts

- `CinaVault Premium_1.0.0-18_x64-setup.exe`
- `CinaVault Premium_1.0.0-18_x64_en-US.msi`
- `SHA256SUMS.txt`
