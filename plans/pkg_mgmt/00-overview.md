# Ori Package Management Roadmap

> **ROADMAP**: Section 22.11 in `plans/roadmap/section-22-tooling.md`
> **Implementation Plan** — Package management system for Ori

---

## Design Philosophy

From the proposal — Key differentiators:

1. **Exact versions only** — No caret, no tilde, no ranges
2. **Single version policy** — One version per package in graph
3. **Bundled stdlib** — `std.*` comes with ori, not registry
4. **Self-contained** — Distributed only via `ori self-update`
5. **No telemetry** — Privacy first
6. **No arbitrary code** — No build scripts, no post-install hooks
7. **Scripting mode** — Run single files without project
8. **Lock is for security** — Checksums only, not version locking
9. **No patching** — Conflicts must be resolved properly

---

## Section Overview

### Tier 1: Foundation (Sections 1-3)

Core manifest, lock file, and resolution.

| Section | Focus |
|---------|-------|
| 1 | Manifest & Lock File |
| 2 | Version Resolution |
| 3 | Cache & Installation |

### Tier 2: Registry (Sections 4-5)

Registry protocol and client.

| Section | Focus |
|---------|-------|
| 4 | Registry Protocol |
| 5 | Registry Client |

### Tier 3: Commands (Sections 6-8)

CLI commands for package management.

| Section | Focus |
|---------|-------|
| 6 | Dependency Commands |
| 7 | Publishing |
| 8 | Workspaces |

### Tier 4: Developer Experience (Sections 9-10)

Scripts, REPL, and tooling.

| Section | Focus |
|---------|-------|
| 9 | Scripts |
| 10 | Tooling (REPL, docs, etc.) |

### Tier 5: Infrastructure (Section 11)

Cloudflare deployment.

| Section | Focus |
|---------|-------|
| 11 | Registry Infrastructure |

---

## Dependency Graph

```
Section 1 (Manifest) → Section 2 (Resolution) → Section 3 (Cache)
    → Section 4 (Registry Protocol) → Section 5 (Registry Client)
    → Section 6 (Dep Commands) → Section 7 (Publishing)
    → Section 8 (Workspaces)
    → Section 9 (Scripts)
    → Section 10 (Tooling)

Section 4-5 (Registry) → Section 11 (Infrastructure)
```

---

## Success Criteria

A section is complete when:

1. **Implemented** — Code in `compiler/oric/` or `ori_pkg/`
2. **Tested** — Tests in `tests/spec/pkg/` or Rust unit tests
3. **Documented** — Spec updated, CLAUDE.md if syntax affected

---

## Milestones

| Milestone | Sections | Exit Criteria |
|-----------|----------|---------------|
| **M1: Local Projects** | 1-3 | oripk.toml, oripk.lock, local deps work |
| **M2: Registry** | 4-5 | Can fetch packages from registry |
| **M3: Full CLI** | 6-8 | install, remove, upgrade, sync, check, publish, workspaces |
| **M4: Developer UX** | 9-10 | Scripts, REPL, ori docs |
| **M5: Production** | 11 | Registry deployed on Cloudflare |

---

## Quick Reference

| Document | Purpose |
|----------|---------|
| `00-overview.md` | This file |
| `index.md` | Keyword index for quick finding |
| `design.md` | Full design specification |
| `section-XX-*.md` | Individual section details |
