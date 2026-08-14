The Windows release preparation script stages wireguard.exe in this directory.

Keeping the directory present allows Rust unit and integration tests to compile
without downloading a platform executable. Release builds still run
scripts/prepare-wireguard.ps1 and bundle every staged file from this directory.
