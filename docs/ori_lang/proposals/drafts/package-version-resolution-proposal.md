# Proposal: Package Version Resolution

**Status:** Draft
**Created:** 2026-01-31
**Affects:** Package management, dependency resolution
**Related:** stdlib-philosophy-proposal.md (approved)

---

## Summary

This proposal defines how the Ori package manager resolves version conflicts when multiple dependencies require different versions of the same stdlib or community package.

---

## Problem Statement

The stdlib-philosophy-proposal establishes that `std.*` packages are semver-independent from the compiler. When two dependencies require different versions of the same package:

```toml
# Package A requires
std.crypto = "1.0"

# Package B requires
std.crypto = "1.1"
```

What version gets used? This proposal defines the resolution semantics.

---

## Open Questions

1. **Single version vs. multiple versions**: Should Ori allow multiple versions of the same package in a dependency tree?
2. **Semver resolution**: Should `^1.0` automatically resolve to `1.1` if available?
3. **Lock files**: Format and behavior of `ori.lock`
4. **Override syntax**: How do users force a specific version?
5. **Conflict reporting**: What errors/warnings should be shown?

---

## Design Space

### Option A: Single Version (Go-style)

Only one version of each package allowed. Diamond dependencies resolve to highest compatible version.

**Pros**: Simple mental model, smaller binaries
**Cons**: May break if packages have incompatible requirements

### Option B: Multiple Versions (Cargo-style)

Allow multiple semver-incompatible versions. `1.x` and `2.x` can coexist.

**Pros**: Handles breaking changes gracefully
**Cons**: Binary size, potential confusion

### Option C: Strict Single Version (Explicit Override Required)

Single version, but compiler error on conflict until user explicitly overrides.

**Pros**: No silent resolution surprises
**Cons**: More user intervention required

---

## Implementation Notes

TBD pending design decisions.

---

## References

- stdlib-philosophy-proposal.md — establishes independent versioning
- Go modules: https://go.dev/ref/mod
- Cargo resolver: https://doc.rust-lang.org/cargo/reference/resolver.html
- npm resolution: https://docs.npmjs.com/cli/v9/configuring-npm/package-lock-json
