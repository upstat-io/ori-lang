# Versioning System

This document describes how version numbers are managed across the Ori project.

## Single Source of Truth

The **`BUILD_NUMBER`** file at the repo root is the single source of truth for the project version:

```
2026.02.27.5-alpha
```

### Format

```
YYYY.MM.DD.N-STAGE
```

- `YYYY.MM.DD` — UTC date of the build
- `N` — daily counter (starts at 1, increments with each merge on the same day)
- `STAGE` — release stage from the `RELEASE_STAGE` file (`alpha`, `beta`, `rc`, or empty for stable)

Example: `2026.02.27.3-alpha` = third build on February 27, 2026, alpha stage.

### How It Works

The build number is **derived from git history** — no persistent counter needed. The script `bump-build.sh` counts first-parent commits to master on the current UTC date and appends the release stage:

```bash
git log --first-parent --oneline --since="<midnight-utc>" master | wc -l
```

The `RELEASE_STAGE` file at the repo root controls the suffix. Change it to `beta`, `rc`, or leave it empty for stable releases.

## Version Locations

### Automatic (compile-time)

| Location | Mechanism |
|----------|-----------|
| `compiler/oric/src/main.rs` | `include_str!("../../../BUILD_NUMBER")` |
| `website/playground-wasm/src/lib.rs` | `include_str!("../../../BUILD_NUMBER")` |
| `website/src/components/landing/Hero.astro` | Reads `BUILD_NUMBER` at build time |

### Cargo.toml (Rust tooling)

The workspace `Cargo.toml` maintains a separate semver version (`0.1.0-alpha.N`) required by Cargo. This is a Rust tooling concern, not the project version. Synchronized via `sync-version.sh`.

| File | Version Format |
|------|----------------|
| `Cargo.toml` (workspace) | Full (`0.1.0-alpha.10`) |
| `compiler/oric/Cargo.toml` | Full (`0.1.0-alpha.10`) |
| `compiler/ori_llvm/Cargo.toml` | Full (`0.1.0-alpha.10`) |
| `website/playground-wasm/Cargo.toml` | Full (`0.1.0-alpha.10`) |
| `website/src/layouts/BaseLayout.astro` | Full (`0.1.0-alpha.10`) |
| `website/package.json` | Base semver (`0.1.0`) |
| `website/src/wasm/package.json` | Base semver (`0.1.0`) |
| `editors/vscode-ori/package.json` | Base semver (`0.1.0`) |

## Where It Appears

| Location | Format |
|----------|--------|
| `ori --version` | `Ori Compiler 0.1.0-alpha.10 (build 2026.02.27.5-alpha)` |
| `ori help` | `Ori Compiler 0.1.0-alpha.10 (build 2026.02.27.5-alpha)` |
| Website hero badge | `v2026.02.27.5-alpha` |
| Playground footer | `Ori build 2026.02.27.5-alpha` |
| GitHub release tag | `v2026.02.27.5-alpha` |
| GitHub release title | `Ori 2026.02.27.5-alpha` |

## Release Pipeline

### Automatic (every merge to master)

1. `auto-release.yml` reads `RELEASE_STAGE`, derives next CalVer tag
2. Tags the merge commit: `v2026.02.27.N-alpha`
3. `release.yml` triggers on the tag, extracts full version (including stage) into `BUILD_NUMBER`
4. Builds binaries with the correct version baked in

### Nightly

`nightly.yml` runs daily at midnight UTC, creates a PR from `dev` → `master`, auto-merges if CI passes, which triggers the auto-release.

## Commands

```bash
# Dry-run: see what the next build number would be
./scripts/bump-build.sh --check

# Bump the build number (normally done by CI)
./scripts/bump-build.sh

# Check Cargo.toml version sync (CI)
./scripts/sync-version.sh --check

# Synchronize Cargo.toml versions
./scripts/sync-version.sh

# Bump Cargo.toml version
./scripts/release.sh 0.1.0-alpha.11
```

## Changing Release Stage

To move from alpha to beta:

1. Edit `RELEASE_STAGE`: change `alpha` to `beta`
2. Commit and merge to master
3. All subsequent builds will be `YYYY.MM.DD.N-beta`

To go stable, empty the file or delete it.
