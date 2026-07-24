# CinaVault GitHub Actions Workflows

This directory contains GitHub Actions workflows for automating the CinaVault Premium build and release process.

## Workflows

### app-start-release.yml
**Primary release workflow** for creating and publishing CinaVault Premium releases.

#### Triggers
- **Manual Dispatch**: Trigger manually from GitHub Actions UI with custom version/build parameters
- **Automatic**: Triggers on push to `main` branch when version files change (`package.json`, `tauri.conf.json`, `Cargo.toml`)

#### Features
- **Version Detection**: Automatically extracts version from `package.json` or uses manual input
- **Multi-Stage Validation**: TypeScript linting, governance tests, Rust compilation checks
- **Windows Build**: Creates both NSIS (`.exe`) and MSI (`.msi`) installers
- **SHA256 Checksums**: Generates cryptographic hashes for all installer files
- **GitHub Release**: Creates tagged releases with auto-generated release notes
- **Draft/Prerelease Support**: Optional flags for testing releases

#### Manual Trigger Parameters
- `version`: Release version (e.g., `1.6.4`)
- `build_number`: Build number (e.g., `164`)
- `prerelease`: Mark as prerelease (default: `false`)
- `draft`: Create as draft release (default: `false`)

#### Workflow Stages
1. **Environment Validation**: Determines version and build numbers
2. **Code Quality Validation**: TypeScript checks and governance tests
3. **Windows Build**: Compiles frontend, runs Rust tests, builds Tauri installers
4. **Release Creation**: Publishes GitHub release with artifacts and notes
5. **Notification**: Generates completion summary

### publish.yml
**Automated publisher** that triggers after successful app-start-release completion.

#### Triggers
- **Workflow Run**: Automatically triggers when `app-start-release` completes successfully
- **Manual Dispatch**: Manual trigger for republishing specific versions

#### Features
- **Artifact Download**: Retrieves installer artifacts from completed workflow
- **Verification**: Validates presence of installer files and checksums
- **Summary Generation**: Creates detailed release summary with file listings

### build-164-release.yml
**Legacy build workflow** for Build 164 (kept for backward compatibility).

### windows-installer.yml
**Windows-specific installer build** with CI/CD integration.

#### Triggers
- Push to `main` branch (excluding docs and releases)
- Pull requests to `main`
- Scheduled runs (every 15 minutes)
- Manual dispatch

#### Features
- Concurrency control with cancellation
- Comprehensive testing suite
- Artifact staging with checksums
- Conditional release publishing (only on main branch pushes)

### Other Workflows
- `build-sequencer.yml`: Sequential build enforcement and version validation
- `build-retry-handler.yml`: Automatic retry logic and failure handling
- `post-release-cleanup.yml`: Repository cleanup and organization after releases
- `metadata-enhancement-workflow.yml`: Automated metadata enhancement and validation
- `dependency-updates.yml`: Automated dependency updates and security auditing
- `automated-testing.yml`: Comprehensive testing suite (unit, integration, E2E, visual, performance)
- `documentation-generation.yml`: Automated documentation generation and deployment
- `apisec-scan.yml`: API security scanning
- `library-maintenance.yml`: Library dependency maintenance
- `maintenance.yml`: General maintenance tasks
- `stale.yml`: Issue and PR stale management
- `summary.yml`: Build summary generation

## Usage

### Creating a New Release

#### Option 1: Automatic (Recommended)
1. Update version in `package.json` and `src-tauri/tauri.conf.json`
2. Commit and push to `main` branch
3. Workflow automatically triggers and creates release

#### Option 2: Manual
1. Go to Actions tab in GitHub
2. Select "CinaVault App Start Release Workflow"
3. Click "Run workflow"
4. Fill in parameters:
   - Version: `1.6.4` (or your version)
   - Build Number: `164` (or your build number)
   - Prerelease: `false` (for production) or `true` (for testing)
   - Draft: `false` (for immediate publish) or `true` (for review)

### Local Testing

Use the local build script before triggering workflows:

```powershell
.\scripts\build-installer.ps1
```

This will:
- Validate TypeScript compilation
- Run production frontend build
- Execute Rust tests
- Build Windows installers
- Copy artifacts to Desktop

## Artifacts

### Installer Types
- **NSIS Installer** (`.exe`): Recommended for most users, includes uninstaller
- **MSI Installer** (`.msi`): For enterprise deployment and managed environments

### Artifact Locations
- **GitHub Actions**: Available as workflow artifacts for 30 days
- **GitHub Releases**: Published permanently in Releases section
- **Local Build**: `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/`

### Verification
Each release includes `SHA256SUMS.txt` for file verification:

```bash
sha256sum -c SHA256SUMS.txt
```

## Build Requirements

### Dependencies
- Node.js 20+
- Rust stable toolchain
- npm/yarn package manager
- Tauri CLI

### GitHub Actions runners use:
- `ubuntu-latest` for validation and release steps
- `windows-latest` for Windows installer builds

## Troubleshooting

### Build Failures
1. Check workflow logs for specific error messages
2. Verify version numbers match across `package.json` and `tauri.conf.json`
3. Ensure all tests pass locally before triggering workflow

### Release Issues
1. Verify GitHub token has `contents: write` permission
2. Check if tag already exists (delete old tag if needed)
3. Ensure workflow has completed successfully before publishing

### Artifact Issues
1. Confirm installer files were produced in bundle directory
2. Verify SHA256 checksums match generated file
3. Check artifact retention period (30 days default)

## Security

- API keys and secrets are managed via GitHub Secrets
- Installers are signed with appropriate certificates
- SHA256 checksums provide integrity verification
- No sensitive data is included in workflow logs

## Maintenance

### Version Updates
When creating a new release:
1. Update `package.json` version
2. Update `src-tauri/tauri.conf.json` version
3. Update workflow environment variables if needed
4. Update this README with any workflow changes

### Workflow Updates
1. Test changes in a feature branch first
2. Use manual dispatch for testing
3. Review workflow logs for errors
4. Update documentation accordingly

## Support

For issues with:
- **Build failures**: Check workflow logs and local build script
- **Release problems**: Verify GitHub permissions and token access
- **Installer issues**: Test with local build script first
- **Workflow questions**: Review this README and workflow comments
