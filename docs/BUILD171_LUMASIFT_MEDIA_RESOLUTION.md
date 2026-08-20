# Build 171 — LumaSift Media Resolution

## Purpose

**LumaSift** is the cinematic duplicate-resolution experience for CinaVault Premium. It scans owner-selected local folders, attached external volumes, and previously authorized NAS shares for a user-selected set of videos, MP3 audio, DOCX documents, PDFs, and images; proves exact duplicates through content hashing; ranks each duplicate group with an explainable quality score; and prepares a reversible resolution plan that retains the best copy while quarantining lower-ranked copies.

The target user is a media collector who needs a clear answer to three questions: what was scanned, which identical files were found, and exactly what will happen to every duplicate. The intended outcome is a fast, trustworthy workflow with continuous percentage progress, source-level and file-level dispositions, a human review gate, and a safe quarantine-first resolution path.

## Measurable Success Criteria

| Criterion | Acceptance condition |
|---|---|
| Source coverage | The scan accepts existing Windows folders, discovered internal/external drive folders, and authenticated/mounted NAS library paths. |
| Media coverage | The selection menu scopes each scan to videos, MP3 audio, DOCX documents, PDFs, and/or images; generated chapter images and video artwork sidecars remain excluded. |
| Exactness | A group is eligible for automatic resolution only after every candidate has the same complete SHA-256 content digest. |
| Best-copy selection | Every resolution item exposes its score and score factors, then retains one deterministic winner and marks all other files for quarantine. |
| Guarded deletion | A destructive remove is not an automatic side effect of scanning. Lower-ranked files are moved to quarantine first, with a two-step confirmation for permanent deletion. |
| Progress and disposition | The interface reports active phase, processed/total item count, percentage, current path, and a file-level disposition log. |
| NAS privacy | Credentials are used only to establish a session/mapped share and are never logged or persisted in plaintext. |

## Non-goals and Constraints

The first release does not treat visually similar but non-identical media as safe duplicates; it therefore avoids perceptual hashes and false-positive deletion. It does not delete the sole member of any group, delete a file outside an approved plan, follow symbolic links during source walking, or bypass Windows file-lock and permission failures. Existing CinaVault scanning, NAS connectivity, duplicate listing, quarantine, and library behavior remain available and compatible.

The mobile applications are companion clients for the approved Windows server. They may monitor a scan and approve a server-created resolution plan, but the Windows host remains the authority that reads local volumes, accesses NAS shares, and moves/deletes remote files. This keeps the operation aligned with each mobile operating system’s file-system permissions and the current CinaVault client/server architecture.

## Architecture

### Front end

A new **LumaSift** duplicate-resolution workspace presents the active scan, a precise percentage meter, the current file, source cards, duplicate groups, quality reasons, estimated reclaimable storage, and file disposition rows. The visual identity uses an obsidian surface with electric cyan, ultraviolet violet, hot magenta, and restrained gold accents. Scan can be cancelled. A user reviews the plan before action; resolution defaults to quarantine and permanent removal remains a separate explicit action.

### Connector / integration

The implementation reuses existing media-source discovery and NAS share setup. Mounted NAS paths flow through the same source pipeline as local folders. The embedded authenticated server exposes read-only status and plan endpoints plus an authenticated plan-application endpoint so iOS and Android can request the same server-side operation. API responses never contain NAS passwords and paths are only returned to authenticated local-network clients.

### Back end

The scanner indexes video, MP3 audio, DOCX/PDF document, and image records while preserving existing exclusions for generated chapter images and video sidecar art. The LumaSift selection contract records exactly which of these categories the owner chose for a given plan. The duplicate engine creates a deterministic two-phase proof: candidates are grouped by byte length and a sampled digest, then complete SHA-256 is calculated before a group is classified as exact. The quality score is explainable and orders candidates by media metadata first, then dimensions, bitrate, color depth, duration, file size, and stable normalized path as final tie-breaker. The exact weighting is returned with every candidate rather than hidden.

A resolution plan is immutable once created. Applying it validates that the candidate still exists, that its current content digest equals the planned digest, that the chosen winner remains in its group, and that the selected action is valid. Files move to CinaVault quarantine atomically where possible, otherwise copy-and-remove with rollback. All resulting state transitions are logged as per-file dispositions.

## Public Contracts

| Contract | Direction | Responsibility |
|---|---|---|
| `start_lumasift_scan` | Front end → Tauri | Begins a cancellable scan and exact-duplicate plan build for enabled sources. |
| `get_lumasift_progress` | Front end → Tauri | Returns phase, current, total, percentage, current path, source name, and terminal state. |
| `get_lumasift_plan` | Front end → Tauri | Returns grouped candidates, winner, disposition, score explanation, and storage impact. |
| `apply_lumasift_plan` | Front end → Tauri | Applies owner-approved quarantine actions only after content revalidation. |
| `purge_lumasift_quarantine` | Front end → Tauri | Performs explicit, separately confirmed permanent removal of quarantined files. |
| `/api/lumasift/status` | Mobile → Windows server | Returns authenticated progress and summary status. |
| `/api/lumasift/plan` | Mobile → Windows server | Returns the authenticated proposed resolution plan. |
| `/api/lumasift/plan/apply` | Mobile → Windows server | Applies an authenticated owner-approved quarantine plan. |

## Quality Policy

The ranking algorithm is deterministic and disclosed in the UI. A candidate score is built from available metadata as follows.

| Factor | Video treatment | Image treatment | Why it matters |
|---|---|---|---|
| Pixel count | Width × height | Width × height | Favors higher-resolution media. |
| Video bitrate | Higher bitrate is preferred when known. | Not applicable. | Distinguishes encodes at the same dimensions. |
| Color depth | Higher bit depth is preferred when known. | Higher bit depth is preferred when known. | Retains richer masters when metadata is available. |
| Duration | Longer duration is preferred when candidates otherwise match. | Not applicable. | Helps retain a complete video over a truncated copy. |
| File size | Larger bytes are preferred only after media metadata factors. | Larger bytes are preferred after dimensions and depth. | A conservative proxy when metadata is unavailable. |
| Stable path | Lexical normalized path is the final tie-breaker. | Same. | Makes a tie repeatable and auditable. |

## Security, Reliability, and Observability

Every scan maintains cancellation checkpoints between file discovery, digest reads, metadata probes, and file moves. Scan work is bounded and avoids loading entire media files into memory. Path comparisons are normalized for Windows long-path behavior. Failure to read metadata reduces score confidence but never causes a delete. File locks, permission failures, source disconnects, and digest mismatches become explicit error dispositions and leave files in place.

No credential, access key, raw password, session token, or personally identifying network address is written to application logs. The audit log stores an operation identifier, UTC timestamp, source label, file display name, planned action, final disposition, bytes affected, and error category.

## Validation Plan

| Layer | Required evidence |
|---|---|
| Rust unit tests | Extension classification, full hashing, collision rejection, deterministic ranking, plan validation, progress percentage, cancellation, and quarantine rollback. |
| Filesystem integration tests | Image/video fixture scan, duplicate proof, NAS-style mounted path, file lock/error path, move/copy fallback, and no-survivor regression. |
| Server contract tests | Authentication, response redaction, status/plan/apply payloads, and rejected unapproved or stale plans. |
| React UI tests | Progress meter, disposition table, scoring explanation, review gate, quarantine action, and permanent-delete confirmation. |
| Build validation | Repository format, type, unit, integration, governance, build, and packaging checks documented in the release evidence. |

## Rollback and Compatibility

The feature is additive. Existing `find_duplicates`, `get_duplicate_groups`, `remove_duplicate`, and `quarantine` contracts remain available. LumaSift actions default to quarantine, and the existing quarantine location provides the rollback boundary. A failed plan application stops at the failed file, preserves earlier successful disposition records, and reports the exact remaining work rather than masking partial success.

## Brand System

The product name is **LumaSift**. Its mark is a three-facet prism with a central checkmark and film perforations. It conveys a single decision: keep the luminous best copy. The visual palette uses obsidian `#050914`, cyan `#28D7FF`, ultraviolet `#7A3DFF`, magenta `#F51393`, and gold `#FFD166`.
