# Historical GitHub Build 132 Workflow Trigger Note

This file preserves the former active `.github/build-132-run.txt` trigger note as historical Build 132 release context. It was moved out of `.github/` during Build 140 governance cleanup so active workflow files do not carry stale build-number drift.

Original content:

```text
CinaVault Premium Build 132 workflow trigger

Purpose: trigger the Windows installer workflow from a non-release path after App shell fixes, npm diagnostics, TypeScript 5.4.5 simulation fix, and direct simulation build script.
Requested by: Johnathan
Date: 2026-06-18
Run revision: 5

Expected workflow:
- .github/workflows/windows-installer.yml
- Build number: 132
- Artifact: CinaVault-Premium-Windows-Installer-Build132
- Output directory: releases/build-132
```
