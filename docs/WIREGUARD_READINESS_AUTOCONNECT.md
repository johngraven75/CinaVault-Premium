# WireGuard readiness and auto-connect slice

This slice uses only authentic official WireGuard Windows engines. Discovery
checks an installed client before bundled resources and verifies file size,
product identity, Authenticode validity, and signer identity before execution.
The UI reports whether an engine is ready and links to the official installer;
it never reports that CinaVault installed software when it did not.

Profile import parses sections and fields, validates 32-byte base64 keys,
addresses, routes, endpoints, duplicates, and safe profile names. List and
status responses expose only the name, address/route summary, endpoint,
verification state, default state, and activity state. Private keys and profile
paths are not returned.

A profile becomes eligible as the default only after a manual tunnel start and
a profile-specific handshake. Failed verification removes the partial tunnel
service. Startup auto-connect runs asynchronously with a 15-second bound and
always attempts cleanup after failure or timeout, so it cannot block application
startup.

## Remaining server-peer work

Owned-server peer issuance is intentionally not part of this slice. The next
server-side project must own the host private key, allocate non-overlapping
device addresses, issue a unique one-time profile per authorized device, apply
peer changes atomically, and support deterministic rotation and revocation with
audit events. It must not reuse or export the host private key.
