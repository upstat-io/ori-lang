# Ori Compiler Roadmap

> **Unified Implementation Plan** — Merges V3 implementation and language gap fixes into a single dependency-ordered roadmap.

> **ACTIVE REROUTE — Trait Architecture Refactor**: All roadmap work is suspended until `plans/trait_arch/` is complete. See `plans/trait_arch/00-overview.md` for the full plan. This reroute was added because 35% of recent work was reactive sync-drift fixes — the trait infrastructure must be restructured before adding more traits. Remove this block when all 7 trait_arch sections are complete.

> **ACTIVE REROUTE — Block Unification Refactor**: All roadmap work is suspended until `plans/block_unify/` is complete. See `plans/block_unify/00-overview.md` for the full plan. This reroute was added because the block-syntax migration (commit `4e0c1611`) revealed a fundamental duality: `FunctionSeq::Run` + `SeqBinding` and `ExprKind::Block` + `StmtKind` are two parallel representations of sequential code, creating 27 dispatch sites across 7 crates. Adopting the Gleam pattern (one block type, one statement type) eliminates ~13 redundant dispatch sites and prevents future block-like constructs from doubling the maintenance burden. Remove this block when all 5 block_unify sections are complete.

> **ACTIVE REROUTE — ARC Optimization**: ARC-related roadmap items (Section 21A ARC codegen, Section 17 atomic refcounts) are rerouted to `plans/arc_optimization/`. See `plans/arc_optimization/00-overview.md` for the full plan, `plans/dpr_arc-optimization_02212026.md` for prior art analysis. This reroute was added because `arc_emitter.rs` has 5 significant stubs (`IsShared` always false, `Reuse` falls back to alloc, `RcDec` null drop, `PartialApply` null env, non-atomic refcounts) that must be closed before ARC-managed types can work in AOT. The plan covers 3 phases: codegen completeness (Swift/Lean 4 patterns), optimization enhancements (RC identity normalization, known-safe elimination), and verification (Koka-inspired `@fbip` enforcement, dual-execution testing). Remove this block when all 3 arc_optimization sections are complete.

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
5. **Value semantics** — No in-place mutation; reassignment replaces values
6. **Structured concurrency** — No orphan tasks
7. **Type safety** — No null, no unhandled errors, compile-time data race prevention

---

## Section Overview

### Tier 0: Parser Foundation (Section 0)

Independent section — can be worked on at any time. Ensures the parser handles all spec syntax.

| Section | Focus |
|-------|-------|
| 0 | Full Parser Support |

### Tier 1: Foundation (Sections 1-5)

Must be completed first. Everything else depends on these.

| Section | Focus |
|-------|-------|
| 1 | Type System Foundation |
| 2 | Type Inference |
| 3 | Traits |
| 4 | Modules |
| 5 | Type Declarations |

### Tier 2: Capabilities & Stdlib (Sections 6-7)

Capabilities must come before Stdlib to unblock Pattern cache (Section 8) and FFI (Section 11).

| Section | Focus |
|-------|-------|
| 6 | Capabilities |
| 7A | Core Built-ins |
| 7B | Option & Result |
| 7C | Collections & Iteration |
| 7D | Stdlib Modules |

### Tier 3: Core Patterns (Sections 8-10)

Pattern evaluation and control flow, now with Capabilities available.

| Section | Focus |
|-------|-------|
| 8 | Pattern Evaluation |
| 9 | Match Expressions |
| 10 | Control Flow |

### Tier 4: FFI & Interop (Sections 11-12)

Foreign code integration.

| Section | Focus |
|-------|-------|
| 11 | FFI |
| 12 | Variadic Functions |

### Tier 5: Language Completion (Sections 13-15)

Platform support, testing framework, and syntax finalization.

| Section | Focus |
|-------|-------|
| 13 | Conditional Compilation |
| 14 | Testing Framework |
| 15A | Attributes & Comments |
| 15B | Function Syntax |
| 15C | Literals & Operators |
| 15D | Bindings & Types |

### Tier 6: Async & Concurrency (Sections 16-17)

Async runtime and concurrent programming.

| Section | Focus |
|-------|-------|
| 16 | Async Support |
| 17 | Concurrency Extended |

### Tier 7: Advanced Type System (Sections 18-19)

Type system extensions. Note: These have minimal dependencies (Section 2 and 3 respectively) but are deferred for practical reasons—const generics and existential types are advanced features better implemented after core language completion.

| Section | Focus |
|-------|-------|
| 18 | Const Generics |
| 19 | Existential Types |

### Tier 8: Advanced Features (Sections 20-22)

Power-user features and tooling.

| Section | Focus |
|-------|-------|
| 20 | Reflection |
| 21A | LLVM Backend |
| 21B | AOT Compilation |
| 22 | Tooling |

---

## Dependency Graph

**Independent Section** (no dependencies, can run in parallel with anything):
```
Section 0 (Parser) — Full syntax support for all spec grammar
```

**Main Sequence** (dependency-ordered for sequential execution):
```
Section 1 (Types) → Section 2 (Inference) → Section 3 (Traits) → Section 4 (Modules)
    → Section 5 (Type Decls) → Section 6 (Capabilities) → Section 7A-D (Stdlib)
    → Section 8 (Patterns) → Section 9 (Match) → Section 10 (Control Flow)
    → Section 11 (FFI) → Section 12 (Variadics) → Section 13 (Conditional Compilation)
    → Section 14 (Testing) → Section 15A-D (Syntax Proposals)
```

> **Note**: Section 6 (Capabilities) comes BEFORE Section 7 (Stdlib) because:
> - Section 8 cache requires Capabilities
> - Section 11 FFI requires Unsafe capability
> - Section 14 Testing requires Capabilities for mocking

**Branches from Main Sequence**:
```
Section 6 (Capabilities) ──→ Section 16 (Async) → Section 17 (Concurrency)

Section 3 (Traits) ──┬──→ Section 19 (Existential Types) [deferred to Tier 7]
                   │
Section 2 (Inference) ──→ Section 18 (Const Generics) [deferred to Tier 7]

Section 11 (FFI) ──→ Section 20 (Reflection)

Core Complete (1-15) ──→ Section 21A-B (Codegen) → Section 22 (Tooling)
```

**Key Dependencies**:
- Section 6 (Capabilities) requires Section 3 (Traits) — placed after Section 5 to unblock Section 8 cache
- Section 7A-D (Stdlib) requires Section 3 (Traits) AND Section 6 (Capabilities)
- Section 8 (Patterns) cache feature requires Section 6 (Capabilities)
- Section 11 (FFI) requires Section 6 (Unsafe capability)
- Section 14 (Testing) requires Section 6 (Capabilities) and Section 7 (Stdlib)
- Section 18 (Const Generics) requires Section 2 — deferred to Tier 7 as advanced feature
- Section 19 (Existential Types) requires Section 3 — deferred to Tier 7 as advanced feature
- Section 20 (Reflection) requires Section 3 (Traits) and Section 11 (FFI)
- **Core Complete** is defined as Sections 1-15

---

## Success Criteria

A section is complete when:

1. **Implemented** — Compiler support in `compiler/oric/`
2. **Specified** — Spec updated in `docs/ori_lang/0.1-alpha/spec/`
3. **Tested** — Tests in `tests/spec/`
4. **Documented** — CLAUDE.md updated if syntax affected

---

## Milestones

Milestones align with tiers for consistent tracking.

| Milestone | Tier | Sections | Exit Criteria |
|-----------|------|--------|---------------|
| **M0: Parser Complete** | 0 | 0 | All spec syntax parses correctly |
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
| `section-XX-*.md` | Individual section details |

### Source References

| Reference | Location |
|-----------|----------|
| Spec | `docs/ori_lang/0.1-alpha/spec/` |
| Proposals | `docs/ori_lang/proposals/` |
| Compiler | `compiler/oric/` |
| Tests | `tests/spec/` |

---

## Zero Reset Log

| Date | Notes |
|------|-------|
| 2026-02-08 14:37 UTC | Full reset — re-verify all features after LLVM V2 codegen rewrite |
