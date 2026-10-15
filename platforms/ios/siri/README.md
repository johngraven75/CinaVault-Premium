# CinaVault Premium — Siri / App Intents (iOS)

Voice control for the CinaVault iOS app: play, search, pause, resume, skip, and
ask the holographic agent — by Siri, in the Shortcuts app, Spotlight, and Siri
Suggestions. Built on Apple's **App Intents** framework (iOS 16+), the current
non-deprecated API — not the legacy SiriKit `INIntent` classes.

## Why it is wired this way

The CinaVault iOS app is a **Tauri (WKWebView) build** of the same React + Rust
product as Windows (`src-tauri/src/lib.rs` is the shared mobile entry point).
There is no native Swift view layer to drive directly, so the Siri integration
is split cleanly across the two layers that actually exist:

- **Native (this folder, Swift).** The App Intents and `AppShortcutsProvider`.
  Each intent builds a `cinavault://…` URL and hands it to the app through
  `SiriLink` — stored in a shared app group and opened when the app foregrounds.
  No network call is made to resolve or run an intent.
- **Web (`src/services/siriIntents.ts` + `siriBridge.ts`, already in the repo
  and unit-tested).** The router parses the URL, fuzzy-matches a spoken title
  against the loaded library, and calls the app's existing actions:
  `playback.ts` (`playMedia`, `controlPlayback`) and the holo agent.

```
Siri  ──▶  App Intent (Swift)  ──▶  cinavault://play?title=…  ──▶  window.__cinavaultSiriIntent(url)
                                     (SiriLink + app group)          (siriBridge → store / player / agent)
```

## Files

| File | Role |
|------|------|
| `CinaVaultIntents.swift` | The six intents: Play, Search, Pause, Resume, Skip, AskHolo |
| `CinaVaultShortcuts.swift` | `AppShortcutsProvider` with the spoken phrases |
| `SiriLink.swift` | Builds `cinavault://` URLs; app-group deliver/drain bridge |
| `Info.plist.additions.xml` | `NSSiriUsageDescription` + `CFBundleURLTypes` keys to merge |
| `Tests/CinaVaultIntentsTests.swift` | XCTest happy-path tests for each intent |

## Siri phrases wired

- "Play *<title>* on CinaVault" · "Play CinaVault"
- "Search for *<query>* on CinaVault" · "Find *<query>* on CinaVault"
- "Pause CinaVault" · "Resume CinaVault" · "Skip on CinaVault"
- "Ask CinaVault *<question>*" — CinaVault opens and the agent answers aloud

## Integrating into the Tauri-generated Xcode project

The iOS Xcode project is generated on a Mac (`npm run tauri -- ios init`) into
`src-tauri/gen/apple/` and is not committed. After generating it:

1. **Add the Swift sources** (`SiriLink.swift`, `CinaVaultIntents.swift`,
   `CinaVaultShortcuts.swift`) to the app target in
   `src-tauri/gen/apple/cinavault.xcodeproj`. Add
   `Tests/CinaVaultIntentsTests.swift` to the test target.
2. **Merge `Info.plist.additions.xml`** into
   `src-tauri/gen/apple/cinavault_iOS/Info.plist` (the `NSSiriUsageDescription`
   string and the `CFBundleURLTypes` `cinavault` scheme).
3. **Enable the App Group** `group.com.cinavault.premium` on the app target
   (Signing & Capabilities → App Groups). This is what lets `SiriLink` hand the
   pending URL to the webview process.
4. **Drain the pending URL in the app delegate.** In the generated
   `src-tauri/gen/apple/Sources/cinavault/*` app delegate, on launch and in
   `applicationDidBecomeActive`, call:

   ```swift
   if let url = SiriLink.takePending() {
       let js = "window.__cinavaultSiriIntent && window.__cinavaultSiriIntent(\(jsStringLiteral(url)))"
       webView.evaluateJavaScript(js, completionHandler: nil)
   }
   ```

   Also handle the `cinavault://` open-URL callback the same way (for when the
   app was already foreground and `UIApplication.open` delivered the scheme).
   The web side already installs `window.__cinavaultSiriIntent` and buffers any
   URL that arrives before the app has mounted.

## Building and testing (requires a Mac)

This repository builds iOS only on macOS with Xcode — the same constraint as
the existing App Store build path (`scripts/build-ios-app-store.sh`). The cloud
CI container has no Swift toolchain or Xcode, so the Swift here is **not
compiled or run in CI**; it is compiled and its XCTest suite run as part of the
Xcode app/test target on a Mac.

```bash
# On a Mac, after the steps above:
xcodebuild test -scheme cinavault -destination 'platform=iOS Simulator,name=iPhone 15'
./scripts/build-ios-app-store.sh   # archive + (optional) App Store Connect upload
```

## Accessibility

VoiceOver on the WKWebView reads the web UI's ARIA. The app's main surfaces
already carry labels; this change adds a labelled primary-navigation landmark
and ensures every nav item is announced (`src/components/Sidebar.tsx`), and the
header, search, and player controls expose `aria-label`s. The Siri transport
commands map onto the same player controls VoiceOver users operate.

## Not included on purpose

An `AppEntity`-backed title catalogue (so Siri suggests exact titles before you
speak) would need a small Tauri command to write the library titles into the
app group. It is deliberately **not stubbed in** here — the intents take the
spoken title as a string and the in-app router fuzzy-matches it, so the feature
works on a fresh install with an empty library cache. Adding the catalogue is a
clean follow-up once that sync command exists.
