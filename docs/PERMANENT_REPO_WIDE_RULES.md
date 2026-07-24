# Permanent Repository-Wide Rules

## Overview

These are the permanent, non-negotiable rules that govern all build and release operations in the CinaVault-Premium repository. These rules ensure professional quality, consistency, and reliability across all automated processes.

## Rule 1: Sequential Build Enforcement

### Rule Statement
**All builds must be sequential when a new build is produced.**

### Implementation
- **Workflow**: `build-sequencer.yml`
- **Enforcement**: Checks for running builds before allowing new builds
- **Override**: Manual override available with `force_build=true` parameter

### How It Works
1. Before any build starts, the sequencer checks for running builds
2. If a build is already in progress, new builds are blocked
3. Only after the current build completes can a new one start
4. Emergency builds can override with manual intervention

### Workflow Integration
- `app-start-release.yml` is triggered through the sequencer
- Automatic builds on version changes go through sequencer
- Manual builds should use the sequencer workflow

### Violation Handling
- Automatic blocking of concurrent builds
- GitHub Actions concurrency settings as backup
- Manual override requires explicit confirmation

## Rule 2: Comprehensive Release Publishing

### Rule Statement
**All produced builds are to be published with build notes and test results along with artifacts and MSI and EXE releases.**

### Implementation
- **Workflow**: `app-start-release.yml` (enhanced)
- **Artifacts**: MSI installer, EXE installer, SHA256 checksums
- **Documentation**: Build notes, test results, coverage reports

### Required Release Components
1. **Installers**
   - NSIS installer (.exe) - Recommended for most users
   - MSI installer (.msi) - For enterprise deployment

2. **Verification**
   - SHA256 checksums for all installers
   - Verification instructions in release notes

3. **Build Notes**
   - Version validation results
   - Code quality check results
   - Build process steps
   - Version correction status

4. **Test Results**
   - TypeScript unit test results
   - Rust unit test results
   - Integration test results
   - Coverage reports

5. **Documentation**
   - Architecture documentation links
   - Release notes
   - Workflow documentation
   - Installation instructions

### Release Process
1. Build completes successfully
2. All artifacts are staged and checksummed
3. Test results are collected and packaged
4. Comprehensive release notes are generated
5. GitHub release is created with all components
6. Release is published with proper tagging

### Quality Gates
- All tests must pass before release
- All artifacts must be present
- Checksums must be generated
- Documentation must be complete

## Rule 3: Failure Handling and Retry

### Rule Statement
**If a workflow fails it must be fixed and rerun until build successfully publishes and releases with artifacts.**

### Implementation
- **Workflow**: `build-retry-handler.yml`
- **Automatic Retry**: Up to 3 automatic retry attempts
- **Manual Intervention**: Required for non-retryable failures
- **Escalation**: Critical issues created when retries exhausted

### Failure Classification
1. **Retryable Failures**
   - Build failures (compilation, packaging)
   - Release failures (GitHub API issues)
   - Test failures (flaky tests)
   - Network/infrastructure issues

2. **Non-Retryable Failures**
   - Validation failures (code quality, linting)
   - Version mismatches (requires manual correction)
   - Configuration errors
   - Dependency conflicts

### Retry Process
1. Failure detected in build workflow
2. Failure type analyzed and classified
3. If retryable: automatic retry with delay
4. If non-retryable: manual intervention issue created
5. After max retries: critical escalation issue created

### Manual Intervention Requirements
- Review failure logs
- Identify root cause
- Implement fix
- Test locally
- Manually trigger new build
- Verify successful release

### Escalation Procedures
- After 3 failed retry attempts
- Critical issue created with urgent priority
- Build process paused until resolved
- Repository maintainers notified

## Rule 4: Post-Release Cleanup

### Rule Statement
**After successful publishing and releasing make sure all unneeded repo files and outdated unneeded work is cleaned and repo is left organized as if a pro managed it.**

### Implementation
- **Workflow**: `post-release-cleanup.yml`
- **Trigger**: Automatic after successful release
- **Levels**: Basic, Standard, Thorough

### Cleanup Operations

#### Artifact Cleanup
- Remove build artifacts older than 7 days
- Keep release artifacts in GitHub Releases
- Clean up temporary build files
- Remove intermediate build outputs

#### Workflow Run Cleanup
- Remove failed workflow runs older than 30 days
- Keep successful release runs
- Clean up test run logs
- Remove debug workflow runs

#### Repository Organization
- Remove temporary directories
- Clean up old release artifact directories
- Remove outdated build trigger files
- Organize documentation structure

#### Log Cleanup
- Remove log files older than 30 days
- Clean up test result files
- Remove debug logs
- Archive important logs

#### Documentation Updates
- Create release notes for new version
- Update build summaries
- Update workflow documentation
- Maintain architecture docs

### Professional Standards
- Repository structure follows best practices
- No temporary or debug files left in repo
- Documentation is current and accurate
- Build artifacts are properly organized
- Clean separation of source, build, and release artifacts

### Cleanup Levels
- **Basic**: Artifact cleanup only
- **Standard**: Artifacts + workflow runs + logs
- **Thorough**: Standard + repository organization + documentation

## Rule 5: Version Validation and Correction

### Rule Statement
**Make sure that all version numbers are current before starting a workflow if not correct them before building.**

### Implementation
- **Workflow**: Integrated into all build workflows
- **Files Checked**: `package.json`, `tauri.conf.json`, `Cargo.toml`
- **Correction**: Automatic synchronization before build

### Version Validation Process
1. Extract version from `package.json` (source of truth)
2. Compare with `tauri.conf.json` version
3. Compare with `Cargo.toml` version
4. If mismatches found, correct automatically
5. Commit corrections with standardized message
6. Push corrections before build proceeds

### Version Sources
- **Primary Source**: `package.json` (npm version)
- **Secondary Sources**: Must match primary
- `src-tauri/tauri.conf.json` (Tauri version)
- `src-tauri/Cargo.toml` (Cargo version)

### Correction Rules
- All version files must match exactly
- Corrections are automatic and immediate
- Changes are committed before build
- Build uses corrected versions
- Correction status noted in build logs

### Version Format
- Semantic versioning: `MAJOR.MINOR.PATCH`
- Build number extracted from PATCH
- Example: `1.6.4` → build number `164`

### Validation Gates
- Versions must be valid semantic versions
- All version files must exist
- Version strings must be parseable
- No version conflicts allowed

## Enforcement Mechanisms

### Automated Enforcement
- GitHub Actions workflows enforce rules automatically
- No manual builds bypass the rules
- All builds go through validation gates
- Failures trigger appropriate handling

### Manual Override Procedures
- Sequential builds: `force_build=true` parameter
- Version correction: Manual edit before build
- Cleanup: Manual workflow trigger with level selection
- Retry: Manual trigger with specific run ID

### Monitoring and Alerts
- Rule violations create GitHub issues
- Build failures trigger notifications
- Cleanup failures generate reports
- Version mismatches logged

### Compliance Checking
- Each workflow logs rule compliance
- Release notes include compliance status
- Cleanup reports verify organization
- Build summaries show validation results

## Workflow Integration

### Build Workflow Chain
1. **build-sequencer.yml** - Enforces sequential builds
2. **app-start-release.yml** - Validates versions, builds, releases
3. **build-retry-handler.yml** - Handles failures and retries
4. **post-release-cleanup.yml** - Cleans and organizes repository

### Dependency Chain
- Sequential build enforcement → Version validation → Build process → Release publishing → Failure handling → Cleanup
- Each step depends on previous step success
- Failures trigger appropriate handling
- Success triggers next step

### Quality Gates
- Version validation must pass
- Code quality checks must pass
- Tests must pass
- Artifacts must be complete
- Documentation must be current

## Compliance Verification

### Automated Checks
- Each workflow includes compliance status
- Release notes show rule compliance
- Cleanup reports verify organization
- Build logs show validation results

### Manual Verification
- Review release notes for completeness
- Check GitHub Releases for all artifacts
- Verify repository organization
- Confirm documentation is current

### Audit Trail
- All version corrections logged
- All retry attempts documented
- All cleanup operations recorded
- All compliance checks tracked

## Best Practices

### Before Starting a Build
1. Ensure repository is in clean state
2. Verify version numbers are consistent
3. Check no other builds are running
4. Review recent commits for issues
5. Verify branch protection rules

### During Build Process
1. Monitor workflow progress
2. Review validation results
3. Check for version corrections
4. Verify artifact generation
5. Confirm test results

### After Release
1. Verify release completeness
2. Check all artifacts are present
3. Review build notes
4. Confirm cleanup executed
5. Verify repository organization

### When Failures Occur
1. Review failure classification
2. Check retry status
3. Implement required fixes
4. Manually trigger retry if needed
5. Verify successful resolution

## Maintenance

### Rule Updates
- Changes to rules require workflow updates
- Documentation must be updated simultaneously
- Test changes thoroughly
- Communicate changes to team

### Workflow Maintenance
- Regular workflow reviews
- Update dependencies
- Monitor performance
- Adjust retry parameters as needed

### Documentation Maintenance
- Keep this document current
- Update examples as workflows change
- Maintain compliance checklists
- Document any exceptions

## Support and Escalation

### Rule Violations
- Automatic issue creation for violations
- Urgent priority for build failures
- Critical escalation for persistent issues
- Team notification for manual intervention

### Questions and Issues
- Review this documentation first
- Check workflow logs for details
- Review GitHub issues for context
- Contact maintainers for clarification

## Summary

These permanent repository-wide rules ensure:

✅ **Sequential Builds**: No concurrent builds, proper ordering
✅ **Comprehensive Releases**: All artifacts, notes, and test results included
✅ **Failure Handling**: Automatic retries, manual intervention when needed
✅ **Professional Organization**: Clean repository after every release
✅ **Version Consistency**: All version files synchronized before builds

These rules are enforced through automated workflows and cannot be bypassed without explicit override procedures. They ensure the CinaVault-Premium repository maintains professional standards and reliable release processes.
