# Proposal: Ori Package Management System

**Status:** Draft
**Created:** 2026-01-31
**Iteration:** 7
**Affects:** Package management, tooling, registry, build system
**Supersedes:** iteration-6.md

---

## Summary

A complete package management system for Ori prioritizing: deterministic builds, exact version pinning, security by default, and unified tooling.

**Key differentiators:**
- **Exact versions only** - No caret, no tilde, no ranges
- **Bundled stdlib** - `std.*` comes with ori
- **Self-contained** - Distributed only via `ori self-update`
- **No telemetry** - Privacy first
- **Scripting mode** - Run single files without a project

---

## Installation

```bash
curl -fsSL https://ori-lang.org/install.sh | sh
```

Only distribution method.

```bash
ori self-update    # Update ori
```

---

## Scripting Mode

Run single `.ori` files without a project:

```bash
ori run script.ori              # Run single file
./script.ori                    # With shebang
```

### Shebang Support

```ori
#!/usr/bin/env ori

@main () -> void =
    print(msg: "Hello from script!")
```

```bash
chmod +x script.ori
./script.ori
```

### Limitations

Single-file mode:
- **Stdlib only** - Can use `std.*`, no external deps
- **No manifest** - No oripk.toml required
- **Quick scripts** - For experiments and utilities

To use external dependencies, create a project with `ori init`.

---

## Package Naming

### Rules

- **Characters:** lowercase a-z, 0-9, hyphens
- **Format:** `@scope/package-name`
- **Max length:** 64 characters
- **Version format:** Strict semver `major.minor.patch` (no extras)

### Reserved Names

Cannot be claimed: `test`, `tests`, `example`, `examples`, `internal`, `private`, `temp`, `tmp`, `build`, `dist`, `out`, anything starting with `std`.

### Disputes

First come first served. No trademark disputes. Whoever registers first owns it.

---

## Manifest: `oripk.toml`

```toml
[project]
name = "@myorg/my-project"
version = "1.0.0"                      # Strict semver only
description = "A useful library"       # Required for publish
license = "MIT"
authors = ["Alice <alice@example.com>"]
repository = "https://github.com/user/repo"
keywords = ["web", "http"]
categories = ["networking"]
funding = "https://github.com/sponsors/user"
edition = "2026"

ori = "0.5.0"                          # Required, warns if mismatch

[project.entry]
lib = "src/lib.ori"
main = "src/main.ori"

[dependencies]
std.json                               # Bundled, no version
std.http
@orilib/utils = "2.0.1"                # Exact version

[dependencies.@orilib/parser]
version = "3.0.2"
features = ["unicode"]

[dev-dependencies]
@orilib/test-helpers = "1.0.0"

[features]
default = ["json"]
json = ["std.json"]

[registries.company]
url = "https://registry.company.com"
auth = "token"

[workspace]
members = ["crates/core", "crates/cli"]

[scripts]
dev = "ori build --watch"
lint = "ori fmt --check && ori check"

[publish]
registry = "default"
include = ["src/**", "README.md", "LICENSE"]
```

**Note:** No `[patch]` section. Version conflicts must be resolved by finding compatible versions or waiting for upstream updates.

---

## Lock File: `oripk.lock`

The lock file contains **only checksums** for security verification. It is NOT for version locking (the manifest has exact versions).

```toml
[checksums]
"@orilib/parser:3.0.2" = "sha256:abc123..."
"@orilib/utils:2.0.1" = "sha256:def456..."
```

### Purpose

- **Security verification** - Protects against supply chain attacks
- **Team consistency** - Everyone verifies against the same checksums
- **Committed to repo** - Should be checked into version control

### What it does NOT do

- Lock versions (manifest has exact versions already)
- Store dependency graphs (resolved from manifest)
- Store metadata (fetched from registry)

### Behavior

- `ori sync` auto-regenerates lock if stale (no error)
- No `--frozen` flag (pointless with exact versioning)
- No separate `ori lock` command (sync handles it)

---

## Package Archive: `.oripk`

Packages are distributed as `.oripk` archives (similar to Rust's `.crate`).

---

## Registry

### API (v1)

```
GET  /v1/packages/{scope}/{name}/versions
GET  /v1/packages/{scope}/{name}/{version}/metadata
GET  /v1/packages/{scope}/{name}/{version}/download
POST /v1/packages/{scope}/{name}/publish
POST /v1/packages/{scope}/{name}/{version}/yank
POST /v1/packages/{scope}/{name}/{version}/unyank
POST /v1/packages/{scope}/{name}/{version}/deprecate
POST /v1/packages/{scope}/{name}/transfer
GET  /v1/search?q=json
GET  /v1/advisories
```

### Token Scopes

- **read** - Download, search
- **publish** - Publish to owned scopes
- **admin** - Manage owners, transfer

### Package Lifecycle

| Action | Who | Effect |
|--------|-----|--------|
| Publish | Owner | Available |
| Deprecate | Owner | Warning shown |
| Yank | Owner | Hidden from resolution |
| Unyank | Owner | Restore yanked version |
| Emergency Remove | Admin | Immediate removal (security) |
| Delete | Admin | Permanent removal (legal) |
| Transfer | Both parties | Change ownership |

### Policies

- **Abandoned packages:** No policy. Packages exist forever.
- **Namespace disputes:** First come first served.
- **Coverage:** Not required. Tests must pass, that's all.
- **Debug info:** Not included. Packages are release builds.
- **Webhooks:** Not supported. Keep registry simple.

### Rate Limits

- Authenticated: 1000 req/min
- Unauthenticated: 100 req/min

---

## Commands

### Project

```bash
# Create (generates .gitignore)
ori new my-project
ori new my-lib --lib
ori init

# Dependencies
ori sync                      # Sync to manifest, auto-regenerates lock if stale
ori check                     # Show available updates (informational only)
ori check @orilib/foo         # Show available versions for specific package
ori install @orilib/foo       # Add NEW dep - prompts for version, errors if exists
ori remove @orilib/foo        # Remove dep from manifest + sync
ori upgrade @orilib/foo:1.2.3 # Update EXISTING dep - version required
ori upgrade @orilib/foo       # Without version: shows available versions

# Build
ori build
ori build --release
ori start

# Format
ori fmt
ori fmt --check

# Run
ori run                    # List scripts (in project)
ori run dev                # Run script
ori run file.ori           # Run single file (stdlib only)

# Test
ori test
ori bench

# Version
ori version patch/minor/major

# Publish
ori login
ori publish
ori publish --dry-run
ori yank 1.0.0
ori unyank 1.0.0
ori deprecate 1.0.0 "Use 2.x"

# Audit
ori audit

# Workspace
ori workspace list
ori build --workspace

# Cleanup
ori clean                     # Wipe local cache
```

### Analysis

```bash
ori deps                   # Tree
ori deps --sizes           # Size breakdown
ori deps --graph           # DOT format
ori why @orilib/utils      # Why included
ori diff 1.0.0 1.1.0       # Compare versions
ori licenses               # License summary
ori search json            # Find packages
ori info @orilib/parser    # Metadata
```

### Documentation

```bash
ori docs                   # Open Ori docs in browser
ori docs @orilib/parser    # Open package repository
```

### Interactive

```bash
ori repl                   # Interactive Ori shell
```

### System

```bash
ori self-update
ori doctor
ori completions bash/zsh/fish
```

---

## IDE Support

Official VS Code extension (LSP-based):
- Syntax highlighting
- Autocomplete
- Go to definition
- Error diagnostics
- Formatting

Ori provides LSP server; extension wraps it.

---

## Infrastructure: Cloudflare

```
┌─────────────────────────────────────┐
│         Cloudflare Edge             │
│                                     │
│  Workers (Ori/WASM)                 │
│    ├── API endpoints                │
│    ├── Auth / rate limiting         │
│    ├── Search                       │
│    │                                │
│  Containers (Ori native)            │
│    ├── Package processing           │
│    ├── Advisory scanning            │
│    │                                │
│   R2              KV                │
│  (packages)    (metadata)           │
└─────────────────────────────────────┘
```

Registry written in Ori (dogfooding).

---

## Security

- No build scripts
- No post-install hooks
- No third-party CLI tools
- No telemetry
- No signing (transparency log sufficient)
- No provenance
- Official advisory database
- Admin emergency removal for critical security

---

## Decisions Made (Iteration 7)

| Decision | Choice |
|----------|--------|
| Abandoned packages | No policy |
| Unyank | Owner can unyank |
| Version format | Strict major.minor.patch |
| Patch section | None (removed) |
| Lock file purpose | Checksums only (security) |
| Coverage requirement | None |
| Namespace disputes | First come first served |
| Emergency removal | Admin-only |
| Debug info | Not in packages |
| Cache proxy | Use Cloudflare mirroring |
| API versioning | Path-based (/v1/) |
| Webhooks | Not supported |
| IDE extension | Official, LSP-based |
| REPL | Yes, ori repl |
| Playground | Exists at ori-lang.com |
| Search filters | Basic only |
| ori docs | Opens repository |
| Single-file run | ori run file.ori |
| Single-file deps | Stdlib only |
| Shebang | #!/usr/bin/env ori |
| Init from file | No, just ori init |

---

## All Decisions

| Category | Decision | Choice |
|----------|----------|--------|
| **Files** | Manifest | `oripk.toml` |
| | Lock file | `oripk.lock` (checksums only) |
| | Archive | `.oripk` |
| **Versions** | Semantics | Exact only |
| | Format | Strict major.minor.patch |
| | Pre-releases | Opt-in |
| | Workspace | Independent |
| | Patch section | None (removed) |
| | Conflicts | Error, no escape hatch |
| **Commands** | Sync | `ori sync` (auto-regenerates lock) |
| | Check updates | `ori check` (informational) |
| | Install | `ori install` (new deps, prompts) |
| | Upgrade | `ori upgrade` (existing, version required) |
| | Remove | `ori remove` |
| | Clean | `ori clean` |
| | Update | None (removed, use check+upgrade) |
| | Lock | None (sync handles it) |
| | Frozen flag | None (pointless with exact versions) |
| **Stdlib** | Distribution | Bundled |
| **Distribution** | Method | Self-update only |
| **Registry** | Metrics | None |
| | Webhooks | None |
| | Disputes | First come first served |
| | Abandoned | No policy |
| | Emergency | Admin removal |
| **Tokens** | Scopes | read/publish/admin |
| **Publishing** | Tests | Must pass |
| | Coverage | Not required |
| | Debug info | Not included |
| **Yank** | Unyank | Owner can |
| **Tooling** | REPL | ori repl |
| | IDE | Official extension |
| | Docs command | Opens repo |
| **Scripting** | Single-file | ori run file.ori |
| | Deps | Stdlib only |
| | Shebang | Supported |

---

## Comparison

| Aspect | npm | Cargo | Go | **Ori** |
|--------|-----|-------|-----|---------|
| Versions | Caret | Caret | MVS | **Exact** |
| Stdlib | N/A | Separate | Bundled | **Bundled** |
| REPL | node | No | No | **Yes** |
| Single-file | node file.js | No | go run | **ori run** |
| IDE | Community | rust-analyzer | gopls | **Official** |
| Playground | No | play.rust-lang.org | go.dev/play | **ori-lang.com** |

---

## References

- [Cloudflare Containers](https://developers.cloudflare.com/containers/)
- [Ori Playground](https://ori-lang.com/playground/)
