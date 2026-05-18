# CinaVault Build 118 - Library Load Pagination Fix

Date: 2026-05-17
Version: 1.0.0-5
GitHub tag: build-118

## Summary

Build 118 addresses the reported hang while loading the media library. The root cause was the Home tab asking the backend for every media row and then rendering the full result set into the UI. On this machine, the live database has 17,781 media rows; the database fetch itself was fast, but rendering the full library at once can stall the desktop webview.

## Implementation

- Added `src/utils/libraryLoadPolicy.ts`.
- Updated `src/components/tabs/HomeTab.tsx` to load 240 media items at a time.
- Added a Load Next 240 button to page through the library.
- Updated `src/components/tabs/AIDiagnosticsTab.tsx` so enrichment refreshes do not repopulate the UI with every media row.
- Added `tests/libraryLoadPolicy.test.mjs`.

## Verification

- Frontend build passed.
- Rust check passed.
- Rust unit tests passed: 38 tests.
- Node safety tests passed: 14 tests.
- Tauri MSI and NSIS packaging passed.

## Installer Copies

- Repository release folder: `releases/build-118`
- Local build folder: `C:\Users\johng\OneDrive\Documents\Desktop\John\cinavault builds\build-118`
