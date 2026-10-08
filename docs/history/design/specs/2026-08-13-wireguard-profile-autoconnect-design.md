# CinaVault WireGuard Profile and Auto-Connect Design

## Purpose

Make CinaVault's WireGuard function installable, configurable, testable, and useful for private access to an owned server. Server mode can issue a revocable profile for another device; client mode can import that profile, select it as the default, and connect automatically after a verified setup.

## Current State

- Windows build scripts download or locate the official WireGuard MSI, verify its Authenticode publisher, extract `wireguard.exe`, and stage it as a bundled resource.
- The application does not install the normal WireGuard client. Its current install command merely reports whether the bundled engine exists.
- Profile import validates a small set of required fields and copies `.conf` files into a restricted app-data directory.
- Connect and disconnect install or uninstall Windows tunnel services using the bundled executable.
- There is no profile creation, device peer lifecycle, default profile, auto-connect preference, successful-connection prerequisite, privilege guidance, or safe startup state machine.

## Product Behavior

### Setup choices

The VPN setup view offers two explicit paths:

1. **Connect to my CinaVault Server**: the owned server creates a VPN device profile for this authorized client.
2. **Import an existing WireGuard profile**: the user selects a `.conf` supplied by another server, administrator, or provider.

CinaVault never claims that an arbitrary locally generated profile can reach the internet or a remote server. A profile is usable only when its peer endpoint exists and is reachable.

### Official engine installation

On Windows, CinaVault checks in this order:

1. An installed, officially signed WireGuard client.
2. A bundled, officially signed WireGuard engine.
3. An explicit install action for the signed official MSI.

Download, installation, and elevation are never silent. Before elevation, the UI states what will be installed and why administrator privileges are required. Signature, publisher, expected product, and minimum-size checks run before execution. A failed or cancelled install leaves the application usable and reports a persistent actionable error.

### Server-issued device profiles

The VPN host owns the server private key and never exports it. For each authorized device it creates a unique peer public key, assigned tunnel address, allowed route set, creation time, and revocation state. A device profile contains only that device's private key, server public key, endpoint, allowed IPs, and optional DNS.

Profiles are issued once, displayed/exported once, and stored with current-user-only permissions on the client. Revoking a device removes its peer from the host and disconnects future handshakes. Rotation creates a new device key pair and invalidates the old peer.

### Default profile and startup

A profile may become the default only after:

- structural validation,
- official engine readiness,
- required privilege availability,
- successful tunnel-service installation,
- an active WireGuard handshake or an authenticated CinaVault server health request through the tunnel.

After success, the user can enable **Connect this profile when CinaVault starts**. The preference stores the profile identifier, never its secret content.

Startup behavior is fail-safe:

1. Start CinaVault normally.
2. Resolve the selected profile and engine.
3. Attempt connection with a bounded timeout.
4. Verify handshake/server health.
5. Show connected state, or disconnect/clean up the failed service and show a persistent error.

Failure never changes the system default route beyond the profile's declared routes, blocks application startup, or repeatedly prompts for elevation in a loop.

## Modules and Interfaces

### WireGuard engine module

Interface: discover official engine, verify authenticity, request installation, install tunnel service, uninstall tunnel service, and inspect service/handshake state.

The implementation hides installed-versus-bundled executable paths and Windows process details. Callers receive typed readiness and failure results.

### VPN profile module

Interface: import, validate, store, list safe summaries, export once, delete, choose default, and read the default identifier.

Profile contents and private keys never appear in list/status results. Validation parses WireGuard fields rather than searching for substrings. It rejects empty values, duplicate sections, malformed addresses, invalid keys, invalid endpoints, and unsafe names.

### VPN host peer module

Interface: issue device peer, list safe peer summaries, revoke peer, and rotate peer.

The host implementation updates the active WireGuard configuration atomically and rolls back persisted peer state when activation fails.

### VPN startup module

Interface: attempt default connection, return the final connection state, and disconnect/clean up idempotently.

This module owns timeouts, privilege outcomes, verification, and the rule that auto-connect requires a prior successful manual test.

## Networking Defaults

- Server UDP listen port is configurable and defaults to the established WireGuard port only when available.
- Device tunnel addresses come from a configurable private subnet and must not overlap detected LAN routes.
- A CinaVault device profile defaults to routes required for the owned-server tunnel, not `0.0.0.0/0` or `::/0`.
- Full-tunnel routing is an advanced explicit option with a clear warning and DNS configuration requirement.
- Endpoint changes can be updated without rotating device identity.
- Keepalive is enabled only where needed for NAT traversal.

## Error Handling

The UI distinguishes missing engine, untrusted binary, elevation cancelled, invalid profile, conflicting subnet, unreachable endpoint, service-install failure, handshake timeout, server-health failure, and revoked peer. Errors remain visible until dismissed or resolved and include a direct next action.

Secret values are redacted from errors and logs. Temporary profile/export files are removed after use. Partial tunnel services are uninstalled after failed verification.

## Security

- Accept only authentic official WireGuard binaries/installers.
- Never commit, log, transmit to telemetry, or place private keys in URLs.
- Store profiles with current-user-only filesystem permissions.
- Use one peer per device; never share one client private key among devices.
- Require local owner authorization before issuing, exporting, rotating, or revoking profiles.
- Use narrow server routes by default and avoid silently becoming a full-device VPN.
- Rate-limit remote peer enrollment and audit safe device/profile events without secrets.

## Validation and Acceptance

The first WireGuard slice is accepted when:

- Tests run without requiring a bundled executable to exist in the source tree.
- An installed official client or bundled engine is discovered and verified correctly.
- An unsigned or wrong-publisher executable is rejected.
- A valid profile can be imported and stored securely; invalid profiles produce field-specific errors.
- A server can issue two distinct device profiles and revoke one without affecting the other.
- Duplicate issuance, rotation, and revocation are deterministic and tested.
- A successful manual connection can be selected as the default.
- Auto-connect starts the selected tunnel, verifies it, and reports connected state.
- Failed auto-connect cleans up and leaves CinaVault and normal networking usable.
- Relevant Rust, TypeScript, UI, Windows, security, and workflow checks pass.

## Delivery Order

1. Repair build-resource handling and official engine detection/install feedback.
2. Replace substring validation with parsed profile validation and safe summaries.
3. Add default-profile persistence and manual successful-test requirement.
4. Add bounded fail-safe auto-connect at application startup.
5. Add owned-server peer issuance, rotation, and revocation.
6. Integrate device authorization and remote server discovery from the broader client/server program.

