# ADR 0001: Owned servers with a lightweight rendezvous service

## Status

Accepted

## Context

CinaVault currently mixes a Jellyfin-oriented desktop workflow with an incomplete native Axum server. The product needs Plex-like account-linked server discovery and clients on a user's other devices without centralizing the user's media library.

Three broad architectures were considered:

1. A centralized media service that stores or proxies all libraries.
2. Independent owned servers with no shared account or discovery layer.
3. Independent owned servers plus a lightweight central rendezvous service.

The centralized option scales operational responsibility and media bandwidth with every stream and conflicts with user-owned libraries. Fully independent servers preserve ownership but make account login, device discovery, revocation, and reliable remote connection difficult.

## Decision

CinaVault will use independent owned servers plus a lightweight central rendezvous service.

The rendezvous service owns account authentication, server claims, device registration, device authorization, connection-candidate discovery, and relay allocation. It does not store media or act as the default media path.

Owned servers own libraries, metadata, users, authorization enforcement, sessions, streaming, and eventual transcoding. Clients connect directly over LAN or public routes when possible and use an encrypted relay only as fallback.

Jellyfin will remain temporarily available as an import/migration adapter. It will cease to be the default runtime and will be removed only after native-server compatibility and migration tests pass.

## Consequences

- Media ownership and primary bandwidth remain distributed across owned servers.
- The control plane can scale independently of streaming and transcoding capacity.
- Direct connection, device trust, revocation, and relay behavior become explicit product concepts.
- CinaVault must operate account infrastructure and optional relay capacity.
- The native server requires substantial hardening: lifecycle, durable sessions, rate limits, pagination, device authorization, connection negotiation, and transcoding.
- Client applications can share one server interface across desktop and mobile adapters.

