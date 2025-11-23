# CI/CD Workflows

base-d uses GitHub Actions for automated testing, building, and publishing.

## Workflow Pipeline

### 1. Prepare Release (`auto-release.yml`)

**Trigger:** Push to `main` branch

**Actions:**
- Auto-bumps patch version in `Cargo.toml` (unless Cargo.toml was manually changed)
- Commits and pushes version bump
- Force-updates `release` branch to match `main`

**Required Secret:** `PAT_TOKEN`

### 2. Build and Release (`release.yml`)

**Trigger:** Push to `release` branch

**Actions:**
- Builds binaries for 9 platforms:
  - Linux: x86_64 (glibc + musl), aarch64 (glibc + musl)
  - macOS: x86_64, aarch64
  - Windows: x86_64, aarch64
  - FreeBSD: x86_64
- Creates GitHub release with version tag
- Uploads packaged binaries (.tar.gz for Unix, .zip for Windows)

**Required Secret:** `GITHUB_TOKEN` (automatically provided)

### 3. Publish Crate (`publish-crate.yml`)

**Trigger:** After successful release workflow

**Actions:**
- Publishes crate to crates.io

**Required Secret:** `CARGO_REGISTRY_TOKEN`

## Setup Instructions

### 1. Create GitHub Secrets

Go to your repository settings → Secrets and variables → Actions

#### PAT_TOKEN
1. Go to GitHub Settings → Developer settings → Personal access tokens → Tokens (classic)
2. Generate new token with `repo` scope
3. Add as repository secret named `PAT_TOKEN`

#### CARGO_REGISTRY_TOKEN
1. Get token from https://crates.io/me
2. Add as repository secret named `CARGO_REGISTRY_TOKEN`

### 2. Create Release Branch

```bash
git checkout -b release
git push -u origin release
```

## Usage

### Manual Release Process

```bash
# Make changes and commit to main
git add .
git commit -m "Add new feature"
git push origin main

# Workflow automatically:
# 1. Bumps version (e.g., 0.1.0 → 0.1.1)
# 2. Pushes to release branch
# 3. Builds binaries
# 4. Creates GitHub release
# 5. Publishes to crates.io
```

### Manual Version Bump (Major/Minor)

```bash
# Edit Cargo.toml manually for major/minor bumps
sed -i 's/version = "0.1.0"/version = "0.2.0"/' Cargo.toml

git add Cargo.toml
git commit -m "Bump to v0.2.0"
git push origin main

# Auto-release will skip auto-bump since Cargo.toml changed
# But will still trigger release workflow
```

### Skip Release

```bash
# Include [skip ci] in commit message
git commit -m "Update docs [skip ci]"
git push origin main
```

## Build Matrix

| Platform | Target | Binary Name |
|----------|--------|-------------|
| Linux x86_64 | x86_64-unknown-linux-gnu | base-d |
| Linux x86_64 (musl) | x86_64-unknown-linux-musl | base-d |
| Linux aarch64 | aarch64-unknown-linux-gnu | base-d |
| Linux aarch64 (musl) | aarch64-unknown-linux-musl | base-d |
| macOS x86_64 | x86_64-apple-darwin | base-d |
| macOS aarch64 | aarch64-apple-darwin | base-d |
| Windows x86_64 | x86_64-pc-windows-msvc | base-d.exe |
| Windows aarch64 | aarch64-pc-windows-msvc | base-d.exe |
| FreeBSD x86_64 | x86_64-unknown-freebsd | base-d |

## Troubleshooting

### Build Failures

**Cross-compilation issues:**
- The workflows use `cross` for non-native targets
- Check Cross.toml if you need custom build configuration

**Dependency issues:**
- Ensure all dependencies support the target platforms
- Check Cargo.toml for platform-specific dependencies

### Release Failures

**Tag already exists:**
- Workflow automatically deletes and recreates tags
- Manually delete with: `git push origin :refs/tags/v0.1.0`

**Release already exists:**
- Workflow skips if release exists
- Manually delete release and tag to retry

### Publish Failures

**Crate name taken:**
- Change name in Cargo.toml
- Update workflows if binary name changes

**Invalid token:**
- Regenerate CARGO_REGISTRY_TOKEN at crates.io
- Update GitHub secret

## Local Testing

Test builds locally before pushing:

```bash
# Test standard build
cargo build --release

# Test specific target (requires target installed)
cargo build --release --target x86_64-unknown-linux-musl

# Test with cross
cargo install cross --git https://github.com/cross-rs/cross
cross build --release --target aarch64-unknown-linux-gnu
```

## Continuous Improvement

The workflows are designed to be:
- ✅ Fully automated
- ✅ Zero-downtime releases
- ✅ Multi-platform support
- ✅ Crates.io integration

Customize as needed for your project.
