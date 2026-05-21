# Release Artifact Policy

CinaVault source control should contain source code, configuration, tests, and build notes.
Installer payloads belong in GitHub Releases and the local CinaVault build folder, not in the
tracked repository tree.

Keep these files out of git:

- Windows installer executables (`*.exe`)
- Windows installer packages (`*.msi`)
- Packaged release archives (`*.zip`)

Required build outputs for each shipped build:

- Upload installer assets to the matching GitHub Release.
- Upload the build report or build notes to the matching GitHub Release.
- Keep the local installer and build notes copy in the user-facing CinaVault build folder.
- Keep durable source changes, tests, and build notes in the repository.

This keeps future features from disappearing while avoiding duplicate binary artifacts in git.
