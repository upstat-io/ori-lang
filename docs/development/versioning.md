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
| `compiler/oric/src/main.rs` | `env!("CARGO_PKG_VERSION")` + `include_str!("../../../BUILD")` |
| `website/playground-wasm/src/lib.rs` | `include_str!("../../../BUILD")` |

### Manual Sync Required

These files need synchronization via `sync-version.sh`:

| File | Version Format |
|------|----------------|
| `compiler/oric/Cargo.toml` | Full (`0.1.0-alpha.1`) |
| `compiler/ori_macros/Cargo.toml` | Full (`0.1.0-alpha.1`) |
| `compiler/ori_llvm/Cargo.toml` | Full (`0.1.0-alpha.1`) |
| `website/playground-wasm/Cargo.toml` | Full (`0.1.0-alpha.1`) |
| `website/src/layouts/BaseLayout.astro` | Full (`0.1.0-alpha.1`) |
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
3. **Run tests**: `./test-all.sh`
4. **Commit**: `git commit -am "chore: bump version to <new-version>"`
5. **Tag**: `git tag v<new-version>`
6. **Push**: `git push origin master --tags`

The GitHub Actions release workflow will automatically build binaries and create a GitHub release.

## Version Format

We follow [Semantic Versioning](https://semver.org/) with pre-release identifiers:

- **Stable**: `MAJOR.MINOR.PATCH` (e.g., `1.0.0`)
- **Pre-release**: `MAJOR.MINOR.PATCH-PRERELEASE` (e.g., `0.1.0-alpha.1`, `1.0.0-beta.2`, `2.0.0-rc.1`)

During the alpha phase, versions follow the pattern `0.1.0-alpha.N` where N increments with each release.

## Build Number

Separate from the release version, a **build number** tracks every merge to master. This is an internal number for identifying exactly which build is running.

### Format

```
YYYY.MM.DD.N
```

- `YYYY.MM.DD` — UTC date of the build
- `N` — daily counter (starts at 1, increments with each merge on the same day)

Example: `2026.02.05.3` = third build on February 5, 2026.

### Storage

The build number lives in the `BUILD` file at the repo root. It is committed to git and read at compile time via `include_str!`.

### Where It Appears

| Location | Format |
|----------|--------|
| `ori --version` | `Ori Compiler 0.1.0-alpha.8 (build 2026.02.05.3)` |
| `ori help` | `Ori Compiler 0.1.0-alpha.8 (build 2026.02.05.3)` |
| Playground footer | `Ori build 2026.02.05.3` |

### CI Workflow

The `bump-build.yml` workflow runs on every push to master:

1. Reads the current `BUILD` file
2. If the date matches today (UTC), increments the counter
3. If it's a new day, resets to `<today>.1`
4. Commits the updated `BUILD` file with `[skip ci]` to avoid infinite loops

### Commands

```bash
# Dry-run: see what the next build number would be
./scripts/bump-build.sh --check

# Bump the build number (normally done by CI)
./scripts/bump-build.sh
```

### Local Development

The `BUILD` file may be stale on local clones. This is expected — the build number is an internal tracking number, not user-facing release information. Pull from master to get the latest value.
