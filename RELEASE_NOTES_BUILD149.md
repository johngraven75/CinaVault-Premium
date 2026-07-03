# CinaVault Premium Build 149

## Security Fixes
- Added npm overrides for vulnerable protobufjs dependency.
- Added npm overrides for vulnerable debug dependency.
- Refreshed package-lock.json.
- Updated Rust glib dependency lockfile where available.

## Addresses Dependabot Alerts
- protobufjs arbitrary code execution / code injection / DoS / prototype pollution alerts
- debug ReDoS alerts
- glib VariantStrIter advisory where Cargo can resolve an update

## Downloads
- Windows EXE installer
- Windows MSI installer
- Build/security test results log
