# Native server integration seam

The executable integration test lives beside the Axum server in
`src-tauri/src/embedded_server.rs`. It exercises the public HTTP boundary on an
ephemeral loopback port with a temporary SQLite database.

Covered behavior:

- access-key login and invalid-key rejection;
- bearer-authorized server information;
- bounded library pagination through `limit` and `offset` query parameters;
- process-local session invalidation when the server state is restarted.

The test deliberately constructs `HttpState` directly instead of using the
global Tauri server runtime. This keeps it deterministic and leaves application
lifecycle ownership to the production startup integration. A future lifecycle
test should separately verify that Tauri startup configures the database path,
starts the listener before remote connectivity, and shuts both down together.

Run the seam with:

```text
cargo test --manifest-path src-tauri/Cargo.toml remote_client_authenticates_and_reads_paginated_library_across_restart
```
