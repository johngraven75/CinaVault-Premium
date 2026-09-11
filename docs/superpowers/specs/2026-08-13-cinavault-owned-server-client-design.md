# CinaVault Owned Server and Client Design

## Purpose

Replace CinaVault's Jellyfin-first runtime with a native, owner-operated CinaVault Server and produce a client mode for a user's other devices. The experience should resemble Plex: sign in, claim a server, discover it from another device, and stream directly whenever possible. A lightweight central control plane may coordinate identity and discovery, but it must not store the media library or become the default streaming path.

## Current State

CinaVault Premium is a React/Tauri desktop application backed by SQLite. It already contains a native Axum server with password/access-key authentication, bearer sessions, server information, library, artwork, metadata-provider contract, and byte-range streaming interfaces. Native scanning, metadata enrichment, artwork caching, and opaque media identifiers also exist.

The native path is not operational as the product default:

- The canonical database path is never passed to the native server's configuration interface, so server startup fails.
- Remote connectivity can expose or relay port 32400 without starting or health-checking the native server.
- Server UI and persisted defaults still select Jellyfin on port 8096.
- A declarative CinaVault provider claims port 8097 while the native listener uses 32400.
- Remote-user errors appear only in a general status feed, and creating an existing email silently rotates its key.
- There is no account-linked server registry, device registration, discovery, client pairing, adaptive streaming, or native transcoding.

## Product Architecture

### Owned server

Each CinaVault Server installation owns its library, metadata, artwork, authorization decisions, active sessions, direct streaming, and eventual transcoding. It has a stable server identity and can be claimed by one account. The server exposes one versioned client interface regardless of whether the client is on the same machine, LAN, public direct route, or relay.

### Rendezvous service

The central control plane is deliberately narrow. It authenticates accounts, records server claims and registered devices, manages device authorization, and returns ordered connection candidates. It may allocate an encrypted relay when direct connection fails. It stores no media and does not inspect stream contents.

The control plane must be stateless at the request tier, backed by a durable relational database, and safe to scale horizontally. Account, device, and server records use non-guessable public identifiers. Authentication and claim operations are rate-limited and audited.

### Client

The first client is a desktop client mode derived from the existing CinaVault UI. It contains no server-management controls and does not scan a local library. After account login it lists accessible servers, chooses the best connection candidate, performs device authorization, and uses the native server interface for browsing and playback.

Mobile clients reuse the same client interface through platform adapters after desktop pairing and playback are stable. Android and iOS are not considered complete until they pass real paired-server integration tests.

## Native Server Modules and Seams

### Server lifecycle module

Interface: configure with the canonical database path, start on a requested or ephemeral port, report health and bound address, and stop idempotently.

The implementation owns Axum startup, database availability, port binding, graceful shutdown, and status. Remote connectivity may consume this interface but may not independently assume a listener exists.

### Remote account module

Interface: create account, rotate access key, disable account, list safe account summaries, authenticate credentials, revoke sessions.

Creation and rotation are distinct. Creating an existing normalized email returns a conflict and never silently changes credentials. Plaintext access keys are returned exactly once; only salted hashes are persisted. UI errors and success state appear beside the form and remain available until dismissed.

### Connection module

Interface: return ordered LAN, public-direct, and relay connection candidates with expiry and health metadata.

Remote connectivity first verifies native-server health, then establishes mapping or relay, verifies the external route, and finally advertises the candidate. Partial failure rolls back the mapping/tunnel and reports an actionable state.

### Library and playback module

Interface: paginated library queries, item details, artwork, playback decision, and ranged media delivery by opaque media key.

The first release supports direct play. Direct stream and transcode are later adapters selected through capability negotiation; clients must not depend on their internal implementation.

## First Implementation Slice

The first bounded subproject is: **make one native server reachable and authenticate one client reliably**.

It includes:

1. Configure the native server with the canonical SQLite path during Tauri initialization.
2. Consolidate start, stop, status, health, and port selection behind the server lifecycle module.
3. Make remote-connect start conditional on a healthy native listener and roll back on failure.
4. Unify configuration on port 32400 for the migration period.
5. Change remote-user creation to reject an existing email and add an explicit key-rotation action.
6. Add persistent inline form validation, saving state, success state, copy control, and one-time key warning.
7. Add executable integration coverage that creates a temporary database, creates a user, starts Axum on an ephemeral loopback port, authenticates by access key, requests server information and a paginated library, rejects an invalid key, and verifies restart/session semantics.
8. Add UI coverage for save success, validation failure, duplicate email, explicit rotation, and visible key handling.
9. Remove the build-time dependency on a missing bundled WireGuard executable from unit/integration test execution.

Cloud accounts, server claiming, production relay allocation, transcoding, and mobile clients remain outside this slice. They require separate approved specifications after the native interface is proven.

## Migration Away from Jellyfin

Migration is incremental:

1. Make native server lifecycle and authentication reliable.
2. Make CinaVault Server the default for new installations while preserving existing stored selections.
3. Move the Server UI to the native lifecycle interface.
4. Add import adapters for Jellyfin libraries and settings where useful.
5. Run compatibility tests for library counts, metadata, artwork, users, and playback.
6. Remove Jellyfin process management and runtime defaults only after native equivalence gates pass.
7. Retain Jellyfin-specific import code only while supported by an explicit migration policy.

No existing installation is silently switched until its native server passes a local health check and its database backup is verified.

## Scale and Reliability

"Several thousand users" refers to accounts and owned servers coordinated by the control plane, not thousands of concurrent streams through one home server.

- Control-plane request tiers scale horizontally and keep no session state in process memory.
- Owned-server capacity is explicit: concurrent-stream limits, bandwidth limits, transcode slots, and storage health.
- Library listing is paginated and indexed; no uncapped full-library response is allowed.
- Database access uses a bounded pool instead of reopening SQLite for every request.
- Server sessions become durable, expiring, revocable records; restarts do not create ambiguous authorization state.
- Access-key lookup uses an indexed key identifier plus constant-time hash verification rather than scanning all users.
- Login, key, claim, and pairing operations are rate-limited and audited.
- CORS uses an explicit origin policy appropriate to desktop/mobile clients; wildcard origin is removed before remote production use.
- Health, connection, stream, error-rate, and relay metrics contain no media paths, keys, or personal content.

## Error Handling

Every user action returns a typed success or failure result with an actionable message. UI forms retain entered non-secret values after recoverable failure. Plaintext passwords and access keys are never logged. Tunnel, mapping, and server lifecycle errors distinguish configuration, unavailable dependency, port conflict, authentication, and external reachability failures.

Connection selection falls back from LAN to public direct to relay. It never silently uses a paid or cloud media path. When no candidate works, the client shows which route failed and what the server owner can do.

## Security

- Media remains on the owned server unless sent to an authorized client.
- The rendezvous service issues short-lived authorization artifacts scoped to account, device, server, and purpose.
- Server claims require proof from both the signed-in account and the local server installation.
- Device trust is visible and revocable from both account and server views.
- Access keys are server-local recovery/invitation credentials, not substitutes for account/device authorization.
- Secrets never enter frontend bundles, repository files, telemetry, or URLs.
- Forceful migration, protection bypass, and automatic credential rotation are prohibited.

## Validation and Acceptance

The first slice is accepted when:

- A clean installation starts the native server without Jellyfin installed.
- Server status reports the actual bound listener and database health.
- Creating a valid remote user persists it and returns one access key.
- Creating the same email again does not rotate credentials.
- Explicit rotation invalidates the old key and returns one replacement.
- A client integration test authenticates and reads server information/library data.
- Invalid keys and expired/revoked sessions are rejected.
- Remote connectivity cannot report success when the native server is unhealthy.
- Relevant Rust, TypeScript, UI, build, and Windows validation checks pass.

The broader architecture is accepted when an account can claim an owned server, authorize a second device, discover the server locally or remotely, and direct-play media without Jellyfin or a centralized media path.

