# CinaVault Premium Android Native

This is a from-scratch native Android implementation of CinaVault Premium.

It is intentionally not a patched copy of the Windows/Tauri app and does not import or alias the desktop React/Tauri source. The app is written as a native Kotlin + Jetpack Compose Android project with its own app architecture, UI model, state model, theme, and verification plan.

## Product target

Replicate the Windows CinaVault Premium experience for Android while using Android-native patterns:

- Premium media-server dashboard.
- Matching dark cinematic/glass skin.
- Home, Sources, Downloads, Live TV, Server, Security, Remote, Advanced, Cloud NAS, Plugins, AI, and Settings sections.
- Persistent settings-ready app state.
- Server/VPN/scanning/status simulation hooks ready for real service integration.
- Android-native responsive phone/tablet layout.

## Build

Use Android Studio Hedgehog/newer or command line with Android Gradle Plugin support.

```bash
cd android-cinavault-native
./gradlew assembleDebug
```

If the repo has no Gradle wrapper available locally, open the folder in Android Studio and let it sync Gradle.

## Status

Initial native source is committed. I could not run an emulator/device verification from this chat environment because Android SDK/emulator execution is not available here. The included verification plan is written as a user-like acceptance checklist for the first local/device pass.
