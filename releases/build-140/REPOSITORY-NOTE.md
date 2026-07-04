# Build 140 Repository Note

CinaVault Premium Build 140 is the repository-aligned Windows installer release for version 1.0.140.

Repository alignment completed:
- package.json uses 1.0.140.
- package-lock.json uses 1.0.140.
- src-tauri/tauri.conf.json uses 1.0.140.
- src-tauri/Cargo.toml uses 1.0.140.
- The Settings/About panel displays v1.0.140 / Build 140.
- The Build 140 release folder contains the installer artifacts, release notes, and SHA256 manifest.

Use `scripts/publish-build-140-artifacts.sh` from Git Bash to refresh local artifacts, regenerate notes, update hashes, commit, and push the Build 140 release folder.
