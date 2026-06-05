# CinaVault Premium iOS App Store Build 130 Handoff

Version: 1.0.0-18
Source baseline: build-130 / 409b5fb
Branch: codex/ios-build-130-app-store
Date: 2026-05-29

## What Changed

- Started from the verified Windows Build 130 tag.
- Converted the Tauri backend from desktop-only `main.rs` into a shared `lib.rs` entry point with `#[cfg_attr(mobile, tauri::mobile_entry_point)]`.
- Kept the Windows app as a thin desktop wrapper calling the same backend, so the iOS app source uses the same React/Tauri product surface as Windows Build 130.
- Added the library crate outputs required by mobile packaging: `staticlib`, `cdylib`, and `rlib`.
- Switched `reqwest` to Rustls TLS to avoid native OpenSSL dependencies in mobile cross-builds.
- Added iOS App Store bundle metadata with sequential `bundleVersion` set to `130`.
- Added `tests/iosAppStoreReadiness.test.mjs` to guard the iOS entry point, bundle version, identifier, and TLS configuration.
- Updated existing backend command-surface tests so they continue checking the shared backend after the desktop/mobile split.
- Added `scripts/build-ios-app-store.sh` for the macOS/Xcode App Store archive and optional App Store Connect upload path.

## Verification Completed On Windows

- `npm ci` passed with 0 vulnerabilities.
- `node --test tests\*.test.mjs` passed: 47 tests.
- `npm run build` passed.
- `cargo fmt` passed.
- `cargo test` passed: 61 Rust tests.
- `npm run tauri -- info` passed and confirmed Tauri 2.11.2 / `@tauri-apps/cli` 2.11.2.
- `npm run tauri build` passed and produced the Windows MSI and NSIS bundles after the backend split.
- `npm run tauri -- ios init --ci` was attempted on Windows and failed because this local CLI exposes Android commands but not iOS commands on Windows.

## App Store Upload Status

No signed `.ipa` was produced on this Windows machine.

The App Store archive and upload step still requires macOS with Xcode, Apple Developer Program signing, an App Store Connect app record whose Bundle ID matches `com.cinavault.premium`, and either Xcode/Transporter or App Store Connect API credentials.

On a Mac, run:

```bash
./scripts/build-ios-app-store.sh
```

If `APPLE_API_KEY_ID` and `APPLE_API_ISSUER` are set and the matching App Store Connect API key is installed for `altool`, the script attempts the upload after building the `.ipa`. Otherwise it leaves the `.ipa` ready for manual upload with Xcode Organizer or Transporter.

## Important Notes

- This is the build-130 source conversion and App Store handoff package, not a signed App Store binary.
- The Windows app still builds successfully after the mobile backend split.
- A final "ready to submit" claim should only be made after the Mac build produces an `.ipa`, Apple accepts the upload, and App Store Connect finishes processing without an Invalid Binary status.
