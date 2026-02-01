# Ori Package Management Roadmap

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

## Phase Overview

### Tier 1: Foundation (Phases 1-3)

Core manifest, lock file, and resolution.

| Phase | Focus |
|-------|-------|
| 1 | Manifest & Lock File |
| 2 | Version Resolution |
| 3 | Cache & Installation |

### Tier 2: Registry (Phases 4-5)

Registry protocol and client.

| Phase | Focus |
|-------|-------|
| 4 | Registry Protocol |
| 5 | Registry Client |

### Tier 3: Commands (Phases 6-8)

CLI commands for package management.

| Phase | Focus |
|-------|-------|
| 6 | Dependency Commands |
| 7 | Publishing |
| 8 | Workspaces |

### Tier 4: Developer Experience (Phases 9-10)

Scripts, REPL, and tooling.

| Phase | Focus |
|-------|-------|
| 9 | Scripts |
| 10 | Tooling (REPL, docs, etc.) |

### Tier 5: Infrastructure (Phase 11)

Cloudflare deployment.

| Phase | Focus |
|-------|-------|
| 11 | Registry Infrastructure |

---

## Dependency Graph

```
Phase 1 (Manifest) → Phase 2 (Resolution) → Phase 3 (Cache)
    → Phase 4 (Registry Protocol) → Phase 5 (Registry Client)
    → Phase 6 (Dep Commands) → Phase 7 (Publishing)
    → Phase 8 (Workspaces)
    → Phase 9 (Scripts)
    → Phase 10 (Tooling)

Phase 4-5 (Registry) → Phase 11 (Infrastructure)
```

---

## Success Criteria

A phase is complete when:

1. **Implemented** — Code in `compiler/oric/` or `ori_pkg/`
2. **Tested** — Tests in `tests/spec/pkg/` or Rust unit tests
3. **Documented** — Spec updated, CLAUDE.md if syntax affected

---

## Milestones

| Milestone | Phases | Exit Criteria |
|-----------|--------|---------------|
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
| `design.md` | Full design specification |
| `phase-XX-*.md` | Individual phase details |
