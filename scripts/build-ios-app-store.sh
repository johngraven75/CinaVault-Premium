#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "CinaVault iOS App Store builds must run on macOS with Xcode installed." >&2
  exit 1
fi

if ! command -v xcodebuild >/dev/null 2>&1; then
  echo "xcodebuild was not found. Install Xcode and select it with xcode-select." >&2
  exit 1
fi

if ! xcodebuild -version >/dev/null 2>&1; then
  echo "Xcode is not ready. Open Xcode once and accept the license/install components." >&2
  exit 1
fi

npm ci
npm run build
node --test tests/*.test.mjs

if [[ ! -d src-tauri/gen/apple ]]; then
  npm run tauri -- ios init --ci
fi

npm run tauri -- ios build -- --export-method app-store-connect

ipa_path="$(find src-tauri/gen/apple/build -type f -name '*.ipa' | sort | tail -n 1)"
if [[ -z "${ipa_path}" ]]; then
  echo "No .ipa was produced under src-tauri/gen/apple/build." >&2
  exit 1
fi

echo "Built App Store IPA: ${ipa_path}"

if [[ -n "${APPLE_API_KEY_ID:-}" && -n "${APPLE_API_ISSUER:-}" ]]; then
  xcrun altool \
    --upload-app \
    --type ios \
    --file "${ipa_path}" \
    --apiKey "${APPLE_API_KEY_ID}" \
    --apiIssuer "${APPLE_API_ISSUER}"
else
  echo "APPLE_API_KEY_ID or APPLE_API_ISSUER is not set, so upload was skipped."
  echo "Upload the IPA with Xcode Organizer, Transporter, or rerun with App Store Connect API credentials."
fi
