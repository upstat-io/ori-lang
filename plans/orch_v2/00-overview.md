# CLI Orchestrator V2

> **Modular CLI orchestrator architecture** — Synthesized from 7 reference compilers (Rust, Go, Zig, Gleam, Elm, Roc, TypeScript) into a layered, testable, and extensible system.

## Source Documents

- **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md`
- **Reference analysis**: Research across `~/projects/reference_repos/lang_repos/` (Rust, Go, Zig, Gleam, Elm, Roc, TypeScript)
- **Current code**: `compiler/oric/src/main.rs`, `compiler/oric/src/commands/`, `compiler/oric/src/db.rs`

---

## Design Philosophy

The orchestrator V2 separates **state** (Session) from **behavior** (Pipeline), abstracts **I/O** (CompilerHost) from computation, and validates **configuration** upfront (CompilerConfig). Every module is independently testable and the architecture supports CLI, LSP, and testing contexts through the same compiler core.

Key principles:
1. **I/O abstraction** — No `fs::read_to_string` in compiler core; all I/O through `CompilerHost`
2. **Upfront validation** — All config validated before any compilation begins
3. **Panic safety** — ICE produces diagnostics, not silent crashes
4. **Zero duplication** — Pipeline replaces copy-pasted error-accumulation boilerplate
5. **Incremental migration** — Each module introduced alongside existing code; commands migrate one at a time

---

## Section Overview

### Tier 0: Foundation (Sections 1-3)

Independent modules with no internal dependencies. Can be built in any order.

| Section | Focus | Inspired By |
|---------|-------|-------------|
| 1 | CompilerHost trait | TypeScript (CompilerHost interface) |
| 2 | CompilerConfig | Zig (Config.resolve()) |
| 3 | Session | Rust (Session struct) |

### Tier 1: Pipeline (Sections 4-6)

Core orchestration. Depends on Tier 0.

| Section | Focus | Inspired By |
|---------|-------|-------------|
| 4 | DiagnosticContext | Rust + Gleam (accumulate-then-flush) |
| 5 | Pipeline + CompilerCallbacks | Rust (CompilerCallbacks) + Gleam (phase ordering) |
| 6 | execute_safely() | Rust (catch_unwind + finish_diagnostics) |

### Tier 2: Polish (Sections 7-9)

User-facing improvements. Depends on Tier 1.

| Section | Focus | Inspired By |
|---------|-------|-------------|
| 7 | Telemetry trait | Gleam (Telemetry + NullTelemetry) |
| 8 | Command Table | Elm (declarative command definitions) |
| 9 | Outcome enum | Gleam (Ok / PartialFailure / TotalFailure) |

### Tier 3: Integration (Sections 10-12)

Connect new architecture to LSP, testing, and migrate existing commands.

| Section | Focus | Inspired By |
|---------|-------|-------------|
| 10 | Command Migration | All (proof of architecture) |
| 11 | TestHost + LspHost | TypeScript (host implementations) |
| 12 | Watch Mode | TypeScript (createWatchProgram) |

---

## Dependency Graph

```
Independent (Tier 0):
  Section 1 (CompilerHost)
  Section 2 (CompilerConfig)
  Section 3 (Session) ←── depends on Section 1, Section 2

Pipeline (Tier 1):
  Section 4 (DiagnosticContext) ←── depends on Section 3
  Section 5 (Pipeline) ←── depends on Section 3, Section 4
  Section 6 (execute_safely) ←── depends on Section 3

Polish (Tier 2):
  Section 7 (Telemetry) ←── depends on Section 5
  Section 8 (Command Table) ←── depends on Section 2
  Section 9 (Outcome) ←── no dependencies (pure type)

Integration (Tier 3):
  Section 10 (Command Migration) ←── depends on Sections 3-6
  Section 11 (TestHost + LspHost) ←── depends on Section 1
  Section 12 (Watch Mode) ←── depends on Section 3, Section 5
```

**Critical path**: Section 1 → Section 3 → Section 4 → Section 5 → Section 10

---

## Success Criteria

A section is complete when:

1. **Implemented** — Module exists in `compiler/oric/src/`
2. **Tested** — Unit tests and integration tests pass
3. **Documented** — Public API has `///` docs
4. **Compatible** — `./test-all.sh` passes (no regressions)

---

## Milestones

| Milestone | Tier | Sections | Exit Criteria |
|-----------|------|----------|---------------|
| **M0: Foundation** | 0 | 1-3 | CompilerHost, CompilerConfig, Session exist and are tested |
| **M1: Pipeline** | 1 | 4-6 | Pipeline orchestrates phases; panic safety works |
| **M2: Polish** | 2 | 7-9 | Telemetry, declarative commands, Outcome type |
| **M3: Integration** | 3 | 10-12 | All commands migrated; TestHost/LspHost work |

---

## Quick Reference

| Document | Purpose |
|----------|---------|
| `index.md` | Keyword index for section discovery |
| `00-overview.md` | This file — high-level goals, tiers, dependencies |
| `section-XX-*.md` | Individual section details |

### Source References

| Reference | Location |
|-----------|----------|
| Proposal | `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` |
| Current CLI | `compiler/oric/src/main.rs` |
| Commands | `compiler/oric/src/commands/` |
| Database | `compiler/oric/src/db.rs` |
| Context | `compiler/oric/src/context.rs` |
| Queries | `compiler/oric/src/query/mod.rs` |
