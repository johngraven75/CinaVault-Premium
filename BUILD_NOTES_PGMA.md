# CinaVault Premium PGMA Build Notes

Branch: `codex/cinavault-pgma-plex-plugin`

## One-step Windows build command

From the repository root in PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File .\build-installer.ps1
```

The script will:

1. Verify Node.js, npm, Rust, Cargo, and rustc are installed.
2. Install JavaScript dependencies with `npm ci` when `package-lock.json` exists, otherwise `npm install`.
3. Run the frontend TypeScript/Vite build.
4. Run Rust compile checks.
5. Run PGMA bridge tests.
6. Run PGMA plugin deployer tests.
7. Build Windows installers with Tauri.
8. Copy generated `.exe`, `.msi`, and `.zip` installer artifacts to a timestamped Desktop folder.
9. Copy this build-notes file beside the installer.

Optional faster build:

```powershell
powershell -ExecutionPolicy Bypass -File .\build-installer.ps1 -SkipTests
```

## PGMA integration summary

This build makes PGMA Modernized preinstalled and available in CinaVault.

Implemented behavior:

- PGMA is injected into the plugin catalog when the app loads.
- PGMA is kept installed/enabled by default.
- PGMA uninstall attempts keep it enabled instead of removing it.
- PGMA deploy/update downloads the upstream PGMA Modernized archive.
- Only `.bundle` folders are extracted from the ZIP archive.
- ZIP path traversal is rejected before extraction.
- Blank Plex path deploys to CinaVault's local plugin staging folder by default.
- No NAS Plex path is hardcoded.
- A configured Plex Plug-ins path can still be used when the user explicitly sets one.
- PGMA Run uses the native CinaVault PGMA bridge to refresh library metadata.

## Native PGMA bridge

CinaVault cannot directly embed Plex Media Server's private plugin runtime, so this build adds a native Rust bridge that performs the practical CinaVault equivalent:

- Scans rows from the `media_items` library table.
- Looks for sidecar metadata near media files.
- Supports `<media-name>.nfo`, `movie.nfo`, and `metadata.nfo`.
- Reads title, overview, year, rating, genre, and artwork references.
- Finds local artwork such as `poster.jpg`, `folder.jpg`, `cover.jpg`, and matching media-stem artwork.
- Downloads remote artwork URLs when enabled.
- Writes matched metadata and artwork paths back into CinaVault's `media_items` database table.

## How to verify after installing

1. Install the generated CinaVault Premium installer.
2. Open CinaVault.
3. Confirm PGMA Modernized appears in Plugins as active/installed.
4. Scan or confirm media exists in the library.
5. Place an `.nfo` file beside a media item, for example `movie.nfo`.
6. Include fields such as:

```xml
<movie>
  <title>Example Title</title>
  <plot>Example overview</plot>
  <year>2024</year>
  <rating>7.5</rating>
  <genre>Drama</genre>
</movie>
```

7. Add optional artwork such as `poster.jpg` in the same folder.
8. Run PGMA from the Plugins tab.
9. Confirm the library item updates title, overview, year, rating, genre, and poster/artwork path.

## Important note

Plex `.bundle` agents normally depend on Plex Media Server runtime APIs like agent objects, preferences, search/update lifecycle, and metadata objects. This build does not pretend those private Plex APIs exist inside CinaVault. Instead, it adds a native CinaVault metadata bridge and keeps bundle deployment available for users who also want to stage or deploy PGMA into a real Plex Plug-ins folder.
