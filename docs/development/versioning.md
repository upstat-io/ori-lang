# Versioning System

This document describes how version numbers are managed across the Ori project.

## Single Source of Truth

The **`BUILD_NUMBER`** file at the repo root is the single source of truth for the project version:

```
2026.02.28.1-alpha
```

### Format

```
YYYY.MM.DD.N-STAGE
```

- `YYYY.MM.DD` — UTC date of the build
- `N` — daily counter (starts at 1, increments with each merge on the same day)
- `STAGE` — release stage (`alpha`, `beta`, `rc`, or empty for stable), embedded in `BUILD_NUMBER` itself

Example: `2026.02.27.3-alpha` = third build on February 27, 2026, alpha stage.

### How It Works

The build number is **derived from git history** — no persistent counter needed. The script `bump-build.sh` counts first-parent commits to master on the current UTC date and appends the release stage (extracted from the existing `BUILD_NUMBER`):

```bash
git log --first-parent --oneline --since="<midnight-utc>" master | wc -l
```

### CalVer to Cargo SemVer Mapping

Cargo requires valid SemVer. The `sync-version.sh` script converts CalVer to Cargo-compatible format:

| Context | Format | Example |
|---------|--------|---------|
| BUILD_NUMBER (human, tags) | `YYYY.MM.DD.N-STAGE` | `2026.02.28.1-alpha` |
| Cargo.toml (valid SemVer) | `YYYY.M.D-STAGE.N` | `2026.2.28-alpha.1` |
| NPM package.json | `YYYY.M.D` | `2026.2.28` |
| Git tag | `vYYYY.MM.DD.N-STAGE` | `v2026.02.28.1-alpha` |

SemVer forbids leading zeros, so `02` becomes `2`, `06` becomes `6` in Cargo versions.

## Version Locations

### Automatic (compile-time)

| Location | Mechanism |
|----------|-----------|
| `compiler/oric/src/main.rs` | `include_str!("../../../BUILD_NUMBER")` |
| `website/playground-wasm/src/lib.rs` | `include_str!("../../../BUILD_NUMBER")` |
| `website/src/components/landing/Hero.astro` | Reads `BUILD_NUMBER` at build time |

### Cargo.toml (derived from BUILD_NUMBER)

All Cargo.toml versions are derived from `BUILD_NUMBER` via `sync-version.sh`. Workspace members inherit the version; excluded crates are synced explicitly.

| File | Version Source |
|------|----------------|
| `Cargo.toml` (workspace) | CalVer → Cargo (`2026.2.28-alpha.1`) |
| `compiler/oric/Cargo.toml` | `version.workspace = true` |
| `compiler/ori_lexer_core/Cargo.toml` | `version.workspace = true` |
| `compiler/ori_llvm/Cargo.toml` | CalVer → Cargo (workspace member, not in default-members) |
| `compiler/ori_rt/Cargo.toml` | CalVer → Cargo (workspace member, not in default-members) |
| `tools/ori-lsp/Cargo.toml` | CalVer → Cargo (excluded from workspace) |
| `website/playground-wasm/Cargo.toml` | CalVer → Cargo (standalone) |
| `website/src/layouts/BaseLayout.astro` | CalVer → Cargo |
| `website/package.json` | CalVer → NPM (`2026.2.28`) |
| `website/src/wasm/package.json` | CalVer → NPM |
| `editors/vscode-ori/package.json` | CalVer → NPM |

## Where It Appears

| Location | Format |
|----------|--------|
| `ori --version` | `Ori Compiler 2026.02.28.1-alpha` |
| `ori help` | `Ori Compiler 2026.02.28.1-alpha` |
| Website hero badge | `v2026.02.28.1-alpha` |
| Playground footer | `Ori build 2026.02.28.1-alpha` |
| GitHub release tag | `v2026.02.28.1-alpha` |
| GitHub release title | `Ori 2026.02.28.1-alpha` |

## Release Pipeline

### Automatic (every merge to master)

1. `auto-release.yml` reads stage from `BUILD_NUMBER`, derives next CalVer tag
2. Tags the merge commit: `v2026.02.27.N-alpha`
3. `release.yml` triggers on the tag, extracts full version into `BUILD_NUMBER`
4. Runs `sync-version.sh` to set Cargo.toml versions from the tag
5. Builds binaries with the correct version baked in

### Nightly

`nightly.yml` runs daily at midnight UTC, creates a PR from `dev` -> `master`, auto-merges if CI passes, which triggers the auto-release.

## Commands

```bash
# Dry-run: see what the next build number would be
./scripts/bump-build.sh --check

# Bump the build number (normally done by CI)
./scripts/bump-build.sh

# Change release stage (alpha -> beta -> rc -> stable)
./scripts/bump-build.sh --set-stage beta

# Check all versions are in sync (CI)
./scripts/sync-version.sh --check

# Synchronize all manifests from BUILD_NUMBER
./scripts/sync-version.sh

# Full release preparation (bump + sync + next steps)
./scripts/release.sh
./scripts/release.sh --set-stage beta
./scripts/release.sh --yes  # Non-interactive (CI)
```

## Changing Release Stage

To move from alpha to beta:

```bash
./scripts/release.sh --set-stage beta
```

This updates `BUILD_NUMBER` with the new stage and syncs all manifests. All subsequent builds will be `YYYY.MM.DD.N-beta`.

To go stable:

```bash
./scripts/release.sh --set-stage stable
```
