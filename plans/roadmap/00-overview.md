# Ori Compiler Roadmap

> **Unified Implementation Plan** — Merges V3 implementation and language gap fixes into a single dependency-ordered roadmap.

## Source Plans

This roadmap consolidates:
- `plans/oric-v3-implementation/` — Core compiler implementation
- `plans/fix-lang-gaps/` — Language gap analysis and fixes

Both source plans are preserved for reference. This roadmap provides the authoritative execution order.

---

## Design Philosophy

From CLAUDE.md — Ori's core tenets:

1. **Code that proves itself** — Mandatory tests bound to functions
2. **Dependency-aware integrity** — Change propagates to tests automatically
3. **Explicit effects** — Capabilities make side effects visible and mockable
4. **Lean core** — Only essentials in compiler; rest in stdlib
5. **Immutable by default** — Mutation is explicit
6. **Structured concurrency** — No orphan tasks
7. **Type safety** — No null, no unhandled errors, compile-time data race prevention

---

## Phase Overview

### Tier 1: Foundation (Phases 1-5)

Must be completed first. Everything else depends on these.

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Type System Foundation | ✅ Complete |
| 2 | Type Inference | ✅ Complete |
| 3 | Traits | 🔶 ~80% complete |
| 4 | Modules | ✅ Core complete |
| 5 | Type Declarations | ✅ Core complete |

### Tier 2: Capabilities & Stdlib (Phases 6-7)

Capabilities must come before Stdlib to unblock Pattern cache (Phase 8) and FFI (Phase 11).

| Phase | Focus | Status |
|-------|-------|--------|
| 6 | Capabilities | ⏳ Not started |
| 7 | Standard Library | ⏳ Blocked on Phase 3, 6 |

### Tier 3: Core Patterns (Phases 8-10)

Pattern evaluation and control flow, now with Capabilities available.

| Phase | Focus | Status |
|-------|-------|--------|
| 8 | Pattern Evaluation | 🔶 ~95% complete (cache blocked until Phase 6) |
| 9 | Match Expressions | 🔶 Partial |
| 10 | Control Flow | ⏳ Not started |

### Tier 4: FFI & Interop (Phases 11-12)

Foreign code integration.

| Phase | Focus | Dependencies |
|-------|-------|--------------|
| 11 | FFI | Phase 6 (Capabilities - Unsafe) |
| 12 | Variadic Functions | Phase 11 (FFI for C variadics) |

### Tier 5: Language Completion (Phases 13-15)

Platform support, testing framework, and syntax finalization.

| Phase | Focus | Dependencies |
|-------|-------|--------------|
| 13 | Conditional Compilation | Phase 8 (Patterns) |
| 14 | Testing Framework | Phase 6 (Capabilities), Phase 7 (Stdlib) |
| 15 | Approved Syntax Proposals | Core language (Phases 1-10) |

### Tier 6: Async & Concurrency (Phases 16-17)

Async runtime and concurrent programming.

| Phase | Focus | Dependencies |
|-------|-------|--------------|
| 16 | Async Support | Phase 6 (Capabilities) |
| 17 | Concurrency Extended | Phase 16 (Async) |

### Tier 7: Advanced Type System (Phases 18-19)

Type system extensions. Note: These have minimal dependencies (Phase 2 and 3 respectively) but are deferred for practical reasons—const generics and existential types are advanced features better implemented after core language completion.

| Phase | Focus | Dependencies |
|-------|-------|--------------|
| 18 | Const Generics | Phase 2 (Type Inference) |
| 19 | Existential Types | Phase 3 (Traits) |

### Tier 8: Advanced Features (Phases 20-22)

Power-user features and tooling.

| Phase | Focus | Dependencies |
|-------|-------|--------------|
| 20 | Reflection | Phase 3 (Traits), Phase 11 (FFI) |
| 21 | Code Generation | Core complete (Phases 1-15) |
| 22 | Tooling | Core complete (Phases 1-15) |

---

## Dependency Graph

**Main Sequence** (dependency-ordered for sequential execution):
```
Phase 1 (Types) → Phase 2 (Inference) → Phase 3 (Traits) → Phase 4 (Modules)
    → Phase 5 (Type Decls) → Phase 6 (Capabilities) → Phase 7 (Stdlib)
    → Phase 8 (Patterns) → Phase 9 (Match) → Phase 10 (Control Flow)
    → Phase 11 (FFI) → Phase 12 (Variadics) → Phase 13 (Conditional Compilation)
    → Phase 14 (Testing) → Phase 15 (Syntax Proposals)
```

> **Note**: Phase 6 (Capabilities) comes BEFORE Phase 7 (Stdlib) because:
> - Phase 8 cache requires Capabilities
> - Phase 11 FFI requires Unsafe capability
> - Phase 14 Testing requires Capabilities for mocking

**Branches from Main Sequence**:
```
Phase 6 (Capabilities) ──→ Phase 16 (Async) → Phase 17 (Concurrency)

Phase 3 (Traits) ──┬──→ Phase 19 (Existential Types) [deferred to Tier 7]
                   │
Phase 2 (Inference) ──→ Phase 18 (Const Generics) [deferred to Tier 7]

Phase 11 (FFI) ──→ Phase 20 (Reflection)

Core Complete (1-15) ──→ Phase 21 (Codegen) → Phase 22 (Tooling)
```

**Key Dependencies**:
- Phase 6 (Capabilities) requires Phase 3 (Traits) — placed after Phase 5 to unblock Phase 8 cache
- Phase 7 (Stdlib) requires Phase 3 (Traits) AND Phase 6 (Capabilities)
- Phase 8 (Patterns) cache feature requires Phase 6 (Capabilities)
- Phase 11 (FFI) requires Phase 6 (Unsafe capability)
- Phase 14 (Testing) requires Phase 6 (Capabilities) and Phase 7 (Stdlib)
- Phase 18 (Const Generics) requires Phase 2 — deferred to Tier 7 as advanced feature
- Phase 19 (Existential Types) requires Phase 3 — deferred to Tier 7 as advanced feature
- Phase 20 (Reflection) requires Phase 3 (Traits) and Phase 11 (FFI)
- **Core Complete** is defined as Phases 1-15

---

## Success Criteria

A phase is complete when:

1. **Implemented** — Compiler support in `compiler/oric/`
2. **Specified** — Spec updated in `docs/ori_lang/0.1-alpha/spec/`
3. **Tested** — Tests in `tests/spec/`
4. **Documented** — CLAUDE.md updated if syntax affected

---

## Milestones

Milestones align with tiers for consistent tracking.

| Milestone | Tier | Phases | Exit Criteria |
|-----------|------|--------|---------------|
| **M1: Foundation** | 1 | 1-5 | Types, inference, traits, modules, type declarations |
| **M2: Capabilities & Stdlib** | 2 | 6-7 | Capability system, standard library |
| **M3: Core Patterns** | 3 | 8-10 | Pattern evaluation, match, control flow |
| **M4: FFI & Interop** | 4 | 11-12 | Foreign function interface, variadics |
| **M5: Language Complete** | 5 | 13-15 | Conditional compilation, testing, syntax finalization |
| **M6: Production Async** | 6 | 16-17 | Async with select/cancel |
| **M7: Advanced Types** | 7 | 18-19 | Const generics, impl Trait |
| **M8: Full Featured** | 8 | 20-22 | Reflection, codegen, tooling |

---

## Quick Reference

| Document | Purpose |
|----------|---------|
| `plan.md` | How to use this plan |
| `priority-and-tracking.md` | Current status and tracking |
| `phase-XX-*.md` | Individual phase details |

### Source References

| Reference | Location |
|-----------|----------|
| Spec | `docs/ori_lang/0.1-alpha/spec/` |
| Proposals | `docs/ori_lang/proposals/` |
| Compiler | `compiler/oric/` |
| Tests | `tests/spec/` |
