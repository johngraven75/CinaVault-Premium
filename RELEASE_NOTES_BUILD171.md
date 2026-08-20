# CinaVault Premium — Build 171 Release Notes

## LumaSift Exact Media Resolution

Build 171 adds **LumaSift**, a cinematic duplicate-resolution workspace for indexed files. Before scanning, the owner selects one or more scopes—**videos, MP3 audio, DOCX documents, PDFs, and images**—then LumaSift scans selected internal folders, external drives, and existing authenticated NAS sources through the established CinaVault source pipeline. LumaSift uses sampled content grouping followed by complete SHA-256 verification; only exact-content matches can enter a cleanup plan.

The workspace shows a live phase, processed/total count, percentage, current file, exact duplicate groups, candidate names, score evidence, file disposition, and projected recoverable storage. The highest ranked copy is retained using transparent media evidence: pixel count, bitrate, color depth, duration, file size, and a deterministic path tie-breaker.

## Safety and Security

LumaSift never deletes as a side effect of scanning. A user must review and approve the plan before lower-ranked copies move to an application-managed quarantine. Every file is re-hashed immediately before moving. Permanent erase is deliberately a separate action requiring the exact confirmation phrase `ERASE LUMASIFT QUARANTINE`.

The embedded authenticated server now exposes path-redacted LumaSift status, plan, scan-start, and quarantine-application endpoints for the iOS and Android companion apps. NAS credentials remain in the established host-only connection flow and are not logged, transmitted to companion clients, or added to repository state.

## Compatibility and Rollback

This release is additive. Existing CinaVault source scanning, duplicate listing, manual duplicate removal, and quarantine commands remain available. The scanner now indexes common owner image formats, MP3 audio, DOCX documents, and PDFs while retaining exclusions for generated chapter art and video sidecar artwork. To roll back a cleanup operation, restore files from the LumaSift quarantine directory before deliberately emptying it. To roll back the application feature, revert the Build 171 commit; no database schema migration is required.

## Validation Scope

The release candidate requires Rust formatting and tests, TypeScript checks, component tests, carry-forward governance, Android checks, iOS XCTest/Xcode validation where the platform toolchain is available, and current-commit build evidence before release or merge.
