# ADR 0002: Self-hosted WireGuard device profiles with bring-your-own support

## Status

Accepted

## Context

CinaVault currently stages an official WireGuard executable and can import profiles, but it neither installs the official client nor creates a usable peer configuration. A valid WireGuard tunnel cannot be invented locally: it requires a reachable peer, keys, addresses, allowed routes, and usually NAT or firewall configuration.

Three options were considered:

1. Require profiles from a commercial VPN provider.
2. Operate a centralized CinaVault VPN service.
3. Let an owned server host WireGuard peers while also accepting profiles from an existing provider or administrator.

A commercial-only dependency does not solve private access to an owned media server. A centralized VPN service would carry user traffic and create a new high-cost, high-risk infrastructure obligation. Owned-server peers preserve the distributed architecture and make device access revocable by the server owner.

## Decision

CinaVault will support self-hosted WireGuard device profiles issued by an owned server, plus import of valid third-party WireGuard profiles.

The server creates one peer identity per authorized device. The client stores the resulting profile with current-user-only permissions, validates it, performs one explicit connection test, and may then mark it as the default auto-connect profile.

Auto-connect is enabled only after a successful test and remains user-controllable. Failure never blocks ordinary networking or application startup. Official WireGuard binaries and installers must pass publisher/signature verification. Installation and tunnel-service changes require explicit Windows elevation.

## Consequences

- CinaVault can provide private client-to-server access without carrying media through a central VPN.
- Server owners must expose a reachable UDP endpoint or use another connection candidate.
- Device profile lifecycle and revocation become part of device trust.
- Private keys require restricted storage, one-time display/export behavior, and zero telemetry/logging.
- Startup needs a fail-safe connection state machine rather than an unconditional tunnel command.
- Commercial or administrator-managed profiles remain usable through the same client profile interface.

