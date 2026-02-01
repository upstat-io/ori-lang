# Proposal: Package Version Resolution

**Status:** Approved
**Created:** 2026-01-31
**Approved:** 2026-01-31
**Affects:** Package management, dependency resolution
**Related:** stdlib-philosophy-proposal.md (approved)
**Implementation:** `plans/pkg_mgmt/phase-02-resolution.md`

---

## Summary

Ori uses **exact version pinning** with a **single version policy**. No caret, no tilde, no ranges. Each package appears exactly once in the dependency graph at the version specified in `oripk.toml`.

---

## Design Principles

From the Ori Package Management design:

1. **Exact versions only** — No `^1.0`, no `~1.0`, no ranges
2. **Single version policy** — Only one version of each package allowed
3. **Bundled stdlib** — `std.*` packages ship with `ori`, no version needed
4. **Deterministic** — Same manifest produces identical builds
5. **No silent resolution** — Conflicts are errors, not warnings

---

## Resolution Semantics

### Exact Version Matching

Dependencies specify exact versions:

```toml
[dependencies]
@orilib/parser = "3.0.2"    # Exactly 3.0.2
@orilib/utils = "2.0.1"     # Exactly 2.0.1
```

The resolver matches versions exactly. `"1.2.3"` matches only `1.2.3`.

### Single Version Policy

Only one version of each package exists in the dependency graph:

```
my-app
├── @orilib/parser@3.0.2
│   └── @orilib/utils@2.0.1   ← Must match
└── @orilib/utils@2.0.1       ← Same version
```

If two dependencies require different versions of the same package, resolution **fails with an error**.

### Conflict Detection

When packages require incompatible versions:

```toml
# Package A requires
@orilib/utils = "2.0.1"

# Package B requires
@orilib/utils = "2.0.2"
```

The resolver produces an error:

```
error[E0901]: version conflict for @orilib/utils
  --> oripk.toml:5:1
   |
   | @orilib/parser@3.0.2 requires @orilib/utils = "2.0.1"
   | @orilib/cache@1.0.0 requires @orilib/utils = "2.0.2"
   |
   = help: find compatible versions or wait for upstream updates
```

### Conflict Resolution

There is **no patch escape hatch**. Conflicts must be resolved by:

1. Finding compatible versions of your dependencies
2. Waiting for upstream packages to update their dependencies
3. Forking if necessary

This ensures the dependency graph is always consistent and prevents the subtle bugs that can arise from forcing incompatible versions to coexist.

---

## Stdlib Resolution

Stdlib packages (`std.*`) are bundled with `ori` and don't require versions:

```toml
[dependencies]
std.json           # Uses bundled version
std.http           # Uses bundled version
std.crypto         # Uses bundled version
```

Stdlib packages are:
- Not included in `oripk.lock`
- Implied from the `ori` version in `[project]`
- Cannot conflict (always one version per `ori` release)

To override for security patches:

```toml
[dependencies]
std.crypto = "1.0.1"   # Override bundled version
```

---

## Lock File

The `oripk.lock` file contains **only checksums** for security verification:

```toml
[checksums]
"@orilib/parser:3.0.2" = "sha256:abc123..."
"@orilib/utils:2.0.1" = "sha256:def456..."
```

### Purpose

The lock file is for **security**, not version locking:

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

## Pre-release Handling

Pre-release versions are opt-in only:

```toml
@orilib/experimental = "1.0.0-alpha"  # Explicit opt-in
```

A stable version constraint never matches pre-release:
- `"1.0.0"` does NOT match `1.0.0-alpha`
- `"1.0.0"` does NOT match `1.0.0-beta.1`

---

## Feature Resolution

Features are resolved per dependency kind:

- Normal dependencies isolated from dev dependencies
- Platform-specific dependencies isolated

```toml
[dependencies.@orilib/parser]
version = "3.0.2"
features = ["unicode"]
default-features = false
```

Default features apply unless `default-features = false`.

---

## Circular Dependencies

Circular dependencies are detected and produce errors:

```
error[E0902]: circular dependency detected
  |
  | @orilib/a depends on @orilib/b
  | @orilib/b depends on @orilib/c
  | @orilib/c depends on @orilib/a
  |
  = help: refactor to break the cycle
```

---

## Comparison with Other Systems

| Aspect | npm | Cargo | Go | **Ori** |
|--------|-----|-------|-----|---------|
| Version syntax | `^1.0`, `~1.0`, ranges | `^1.0`, `=1.0` | MVS | **Exact only** |
| Multiple versions | Yes (nested) | Yes (semver-incompatible) | No | **No** |
| Conflict resolution | Hoisting | Union | Upgrade | **Error (no patch)** |
| Lock file | package-lock.json | Cargo.lock | go.sum | **oripk.lock** |
| Lock purpose | Version locking | Version locking | Checksums | **Checksums only** |

---

## Implementation

See `plans/pkg_mgmt/phase-02-resolution.md` for implementation tasks:

- 2.1 Dependency Graph — Build and traverse
- 2.2 Version Matching — Exact match semantics
- 2.3 Conflict Detection — Error messages (no patch escape hatch)
- 2.4 Feature Resolution — Per-dependency-kind isolation
- 2.5 Stdlib Handling — Bundled version inference
- 2.6 Incremental Resolution — Checksum verification

---

## References

- `plans/pkg_mgmt/design.md` — Full package management design
- `plans/pkg_mgmt/phase-02-resolution.md` — Implementation plan
- `stdlib-philosophy-proposal.md` — Bundled stdlib versioning
