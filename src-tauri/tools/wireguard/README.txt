The Windows release preparation script stages the signed WireGuard executable in
this directory. Keeping the directory present allows development, Rust unit, and
integration tests to compile without downloading a platform binary. Release builds
still run scripts/prepare-wireguard.ps1 and bundle every staged file from here.
