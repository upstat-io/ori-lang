# Versioning System

This document describes how version numbers are managed across the Ori project.

## Single Source of Truth

The **workspace `Cargo.toml`** is the single source of truth for the project version:

```toml
# Cargo.toml
[workspace.package]
version = "0.1.0-alpha.1"
```

All other version locations are derived from this value, either automatically at compile time or via synchronization scripts.

## Version Locations

### Automatic (compile-time)

These locations use `env!("CARGO_PKG_VERSION")` and are automatically correct:

| Location | Mechanism |
|----------|-----------|
| `compiler/oric/src/main.rs` | `env!("CARGO_PKG_VERSION")` |
| `website/playground-wasm/src/lib.rs` | `env!("CARGO_PKG_VERSION")` |

### Manual Sync Required

These files need synchronization via `sync-version.sh`:

| File | Version Format |
|------|----------------|
| `compiler/oric/Cargo.toml` | Full (`0.1.0-alpha.1`) |
| `compiler/ori_macros/Cargo.toml` | Full (`0.1.0-alpha.1`) |
| `compiler/ori_llvm/Cargo.toml` | Full (`0.1.0-alpha.1`) |
| `website/playground-wasm/Cargo.toml` | Full (`0.1.0-alpha.1`) |
| `website/package.json` | Base semver (`0.1.0`) |
| `website/src/wasm/package.json` | Base semver (`0.1.0`) |
| `editors/vscode-ori/package.json` | Base semver (`0.1.0`) |

**Note**: NPM package.json files use base semver (without pre-release suffix) because npm has different semantics for pre-release versions.

## Commands

### Check Version Sync

Verify all versions are synchronized (used in CI):

```bash
./scripts/sync-version.sh --check
```

### Synchronize Versions

Update all version files to match the workspace version:

```bash
./scripts/sync-version.sh
```

### Prepare a Release

Bump the version across all files and print next steps:

```bash
./scripts/release.sh 0.1.0-alpha.2
```

This will:
1. Validate the version format
2. Update the workspace `Cargo.toml`
3. Run `sync-version.sh` to propagate the change
4. Print instructions for testing, committing, tagging, and pushing

## CI Integration

### Pull Requests

The CI workflow includes a `version-check` job that runs `sync-version.sh --check` to ensure all versions are synchronized before merging.

### Releases

The release workflow includes a `validate` job that:
1. Validates the git tag matches the `Cargo.toml` version
2. Runs `sync-version.sh --check`

If validation fails, the release is blocked and helpful error messages guide you to fix the issue.

## Pre-commit Hook

If you use [lefthook](https://github.com/evilmartians/lefthook), the `version-sync` command runs automatically when you modify `Cargo.toml` or `package.json` files.

## Release Process

1. **Bump version**: `./scripts/release.sh <new-version>`
2. **Review changes**: `git diff`
3. **Run tests**: `./test-all`
4. **Commit**: `git commit -am "chore: bump version to <new-version>"`
5. **Tag**: `git tag v<new-version>`
6. **Push**: `git push origin master --tags`

The GitHub Actions release workflow will automatically build binaries and create a GitHub release.

## Version Format

We follow [Semantic Versioning](https://semver.org/) with pre-release identifiers:

- **Stable**: `MAJOR.MINOR.PATCH` (e.g., `1.0.0`)
- **Pre-release**: `MAJOR.MINOR.PATCH-PRERELEASE` (e.g., `0.1.0-alpha.1`, `1.0.0-beta.2`, `2.0.0-rc.1`)

During the alpha phase, versions follow the pattern `0.1.0-alpha.N` where N increments with each release.
