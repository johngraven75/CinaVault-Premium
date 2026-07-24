# CinaVault Branch Isolation Strategy

## Overview

Each app version must be maintained in its own isolated branch to prevent code intermingling between different versions. This ensures version stability, prevents regression, and allows for parallel development.

## Branch Structure

### Main Branches

```
main                    # Latest stable release (read-only after release)
develop                 # Active development for next version
```

### Version-Specific Branches

```
release/v1.6.4         # Version 1.6.4 maintenance branch
release/v1.6.5         # Version 1.6.5 maintenance branch (future)
feature/v1.6.4/*       # Version 1.6.4 specific features
hotfix/v1.6.4/*        # Version 1.6.4 emergency fixes
```

### Isolation Rules

1. **No Cross-Branch Merging**: Version branches cannot merge into other version branches
2. **Strict Upstream Flow**: Only `develop` → `main` flow allowed
3. **Version Locking**: Once a version branch is created, it cannot receive updates from other branches
4. **Code Separation**: Each version branch has its own isolated codebase

## Branch Creation Process

### New Version Branch Creation

```bash
# Create new version branch from develop
git checkout develop
git pull origin develop
git checkout -b release/v1.6.4
git push origin release/v1.6.4
```

### Version Branch Locking

Once a version branch is created:
1. Set branch protection rules
2. Configure workflow isolation
3. Enable code ownership checks
4. Restrict merge permissions

## Branch Protection Rules

### Main Branch
- **Status Checks**: Required (all tests must pass)
- **Pull Requests**: Required
- **Branch Restrictions**: Only maintainers can push
- **Required Reviews**: At least 2 maintainers
- **Upstream Merge**: Only from `develop` branch

### Develop Branch
- **Status Checks**: Required
- **Pull Requests**: Required
- **Branch Restrictions**: Maintainers and contributors
- **Required Reviews**: At least 1 maintainer

### Version Branches (release/vX.Y.Z)
- **Status Checks**: Required
- **Pull Requests**: Required
- **Branch Restrictions**: Only version maintainers
- **Merge Restrictions**: No merges from other version branches
- **Required Reviews**: At least 1 maintainer

## Code Isolation Mechanisms

### 1. Directory Structure Isolation

Each version branch maintains its own structure:

```
CinaVault-Premium/
├── src/                    # Version-specific source code
├── src-tauri/              # Version-specific Rust code
├── package.json            # Version-specific dependencies
├── src-tauri/Cargo.toml   # Version-specific Rust dependencies
└── docs/                  # Version-specific documentation
```

### 2. Dependency Isolation

Each version branch has locked dependencies:
- `package-lock.json` - npm dependencies
- `src-tauri/Cargo.lock` - Rust dependencies
- No shared dependency files across versions

### 3. Configuration Isolation

Version-specific configuration files:
- `src-tauri/tauri.conf.json` - Version-specific app config
- `.github/workflows/` - Version-specific workflows
- Environment variables per version

### 4. Workflow Isolation

Workflows are configured to run only on specific branches:

```yaml
on:
  push:
    branches:
      - 'release/v1.6.4'
      - 'develop'
```

### 5. Artifact Isolation

Build artifacts are version-specific:
- Release artifacts tagged with version
- Separate artifact repositories per version
- Version-specific GitHub Releases

## Merge Policies

### Allowed Merges

1. **feature/v1.6.4/* → release/v1.6.4**
   - Feature branches merge into their version branch
   - Requires PR and review
   - Must pass all tests

2. **hotfix/v1.6.4/* → release/v1.6.4**
   - Emergency fixes merge into version branch
   - Fast-track review process
   - Must pass critical tests

3. **develop → main**
   - Only when ready for release
   - Requires comprehensive testing
   - Creates new version branch

### Forbidden Merges

1. **release/v1.6.4 → release/v1.6.5**
   - No cross-version merging
   - Prevents code intermingling

2. **main → release/v1.6.4**
   - No backporting from main
   - Use cherry-pick if needed

3. **develop → release/v1.6.4**
   - No direct develop to version branch
   - Create feature branch first

## Version Lifecycle

### Active Development
- Branch: `develop`
- Status: Open for changes
- Workflow: Full CI/CD pipeline

### Release Candidate
- Branch: `release/vX.Y.Z`
- Status: Feature freeze, bug fixes only
- Workflow: Release-specific CI/CD

### Maintenance
- Branch: `release/vX.Y.Z`
- Status: Security fixes only
- Workflow: Minimal CI/CD

### End of Life
- Branch: `release/vX.Y.Z`
- Status: Read-only, archived
- Workflow: Disabled

## Implementation Steps

### 1. Current Repository Restructuring

```bash
# Create develop branch from current main
git checkout main
git checkout -b develop
git push origin develop

# Create version branch for current version
git checkout -b release/v1.6.4
git push origin release/v1.6.4

# Return to develop for future work
git checkout develop
```

### 2. Branch Protection Configuration

Configure in GitHub repository settings:
- Enable protection for `main`, `develop`, and `release/*` branches
- Set required status checks
- Configure branch restrictions
- Enable required pull request reviews

### 3. Workflow Updates

Update all workflows to respect branch isolation:
- Add branch filters to triggers
- Configure version-specific artifact handling
- Implement branch-specific deployment

### 4. Team Permissions

Configure team access:
- **Maintainers**: Full access to all branches
- **Version Maintainers**: Access to specific version branches
- **Contributors**: Access to develop and feature branches

## Monitoring and Enforcement

### Automated Checks

1. **Branch Validation**
   - Verify no cross-branch dependencies
   - Check for version conflicts
   - Validate branch naming conventions

2. **Merge Prevention**
   - Block forbidden merges
   - Require explicit approval for exceptions
   - Log all merge attempts

3. **Code Ownership**
   - Enforce CODEOWNERS file
   - Require version-specific reviewers
   - Track code changes per version

### Manual Reviews

1. **Branch Audits**
   - Regular review of branch structure
   - Verify isolation compliance
   - Check for code intermingling

2. **Dependency Audits**
   - Review dependency changes
   - Verify no cross-version dependencies
   - Check for security vulnerabilities

## Rollback Procedures

### If Code Intermingling Detected

1. **Immediate Isolation**
   - Identify affected branches
   - Lock affected branches
   - Prevent further merges

2. **Impact Assessment**
   - Determine scope of intermingling
   - Identify affected versions
   - Assess security implications

3. **Remediation**
   - Revert intermingled changes
   - Restore branch isolation
   - Update protection rules

4. **Prevention**
   - Review root cause
   - Update isolation mechanisms
   - Enhance monitoring

## Documentation Requirements

Each version branch must include:
- Version-specific README
- Release notes
- Known issues
- Upgrade instructions
- Compatibility matrix

## Benefits

1. **Version Stability**: Each version isolated from others
2. **Parallel Development**: Multiple versions can be maintained simultaneously
3. **Regression Prevention**: Changes in one version don't affect others
4. **Clear Ownership**: Specific teams responsible for specific versions
5. **Simplified Testing**: Test suites specific to each version
6. **Controlled Releases**: Release process per version

## Migration Plan

### Phase 1: Setup (Week 1)
- Create branch structure
- Configure protection rules
- Update workflows
- Document procedures

### Phase 2: Migration (Week 2)
- Migrate current code to develop branch
- Create version branch for v1.6.4
- Update team permissions
- Train team on new process

### Phase 3: Validation (Week 3)
- Test branch isolation
- Verify merge policies
- Validate workflow updates
- Monitor for issues

### Phase 4: Full Implementation (Week 4)
- Enable all protection rules
- Begin using new branch strategy
- Monitor compliance
- Adjust as needed

## Support and Training

### Team Training
- Branch strategy overview
- Merge policy training
- Workflow usage training
- Troubleshooting procedures

### Documentation
- Branch creation guide
- Merge request process
- Isolation troubleshooting
- Emergency procedures

## Summary

This branch isolation strategy ensures that each app version maintains its own isolated codebase, preventing code intermingling and enabling stable, parallel development of multiple versions.
