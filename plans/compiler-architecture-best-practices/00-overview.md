---
plan: "compiler-architecture-best-practices"
title: "Compiler Architecture Best Practices — Industry-Standard Foundation"
status: not-started
references:
  - ".claude/rules/impl-hygiene.md"
  - ".claude/rules/tests.md"
  - ".claude/rules/typeck.md"
  - ".claude/rules/codegen-rules.md"
  - ".claude/rules/diagnostic.md"
  - ".claude/rules/types.md"
  - ".claude/rules/parse.md"
  - ".claude/rules/canon.md"
---

# Compiler Architecture Best Practices — Industry-Standard Foundation

## Mission

Implement verified industry best practices and documented aspirational patterns as enforceable rules with supporting infrastructure, creating the foundational correctness-enforcement layer that ALL downstream plans build on. This plan closes 6 confirmed gaps surfaced by a dual-source (Codex + Gemini) TPR review against 10 reference compilers (Rust, Go, TypeScript, Zig, Swift, Gleam, Roc, Elm, Koka, Lean 4), plus 3 aspirational patterns already documented in `impl-hygiene.md` §Aspirational Patterns. Effort is not a constraint — this is about building the best possible architectural foundation.

## Mission Success Criteria

- [ ] Type solver terminates predictably with fuel limits — no nontermination on pathological generics (Section 03)
- [ ] Diagnostics emit in deterministic source-position order with (error_code, span) dedup and child-span TyError suppression (Section 04)
- [ ] Salsa cache invalidation is tested via multi-revision edit-sequence tests; new Salsa-touching PRs require revision tests (Section 05)
- [ ] Cross-target codegen changes require FileCheck ABI verification for at least one non-host target (Section 06)
- [ ] TypeFolder trait eliminates 4+ substitution duplications — new type transformations implement fold_*, not ad-hoc recursion (Section 07)
- [ ] Symbol type encodes module provenance — cross-module lookups are O(1) without secondary Name→Module maps (Section 08)
- [ ] Layout computation is a Salsa-tracked query — all consumers call `layout_of()`, none recompute from the type pool (Section 09)
- [ ] All new rules are documented in `.claude/rules/*.md` with enforcement mechanisms (Sections 01-06)
- [ ] AST/IR immutability is explicitly documented as a rule with type-system enforcement at phase boundaries (Section 02)
- [ ] `./test-all.sh` green — no regressions
- [ ] All section success criteria met

## Architecture

```
Rule Files (.claude/rules/*.md)        Compiler Infrastructure
┌────────────────────────────────┐    ┌──────────────────────────────────┐
│ §01 Foundation                 │    │                                  │
│  ├─ perf gate rule             │    │ §03 Solver Budgets (ori_types)   │
│  ├─ crash regression rule      │    │  ├─ unify fuel counter           │
│  └─ phase documentation        │    │  ├─ substitute depth limit       │
│                                │    │  └─ E2042/E2043 overflow codes   │
│ §02 Immutability Contract      │    │                                  │
│  ├─ impl-hygiene.md update     │    │ §04 Diagnostic Ordering          │
│  └─ parse.md update            │    │  (ori_diagnostic)                │
│                                │    │  ├─ (code, span) dedup           │
│ §05 Incremental Testing Rule   │    │  └─ child-span suppression       │
│  └─ tests.md update            │    │                                  │
│                                │    │ §05 Edit-Sequence Harness        │
│ §06 Cross-Target Rule          │    │  (ori_test_harness/revision)     │
│  └─ codegen-rules.md update    │    │  └─ multi-step Salsa tests       │
└────────────────────────────────┘    │                                  │
                                      │ §06 Cross-Target FileCheck       │
Aspirational Patterns                 │  (ori_llvm/tests/codegen)        │
┌────────────────────────────────┐    │  └─ --target ABI assertions      │
│ §07 TypeFolder (ori_types)     │    │                                  │
│  └─ Extract from 4+ impls     │    │ §07 TypeFolder (ori_types)       │
│                                │    │  └─ fold_var, fold_named, etc.   │
│ §08 Packed Symbol (ori_ir)     │    │                                  │
│  └─ Symbol = (ModuleId, Name) │    │ §08 Symbol + ModuleId (ori_ir)   │
│                                │    │  └─ cross-module O(1) lookup     │
│ §09 Layout Query               │    │                                  │
│  └─ Salsa layout_of(Idx)      │    │ §09 layout_of (ori_repr→shared)  │
└────────────────────────────────┘    │  └─ Salsa memoized query         │
                                      └──────────────────────────────────┘
```

## Design Principles

1. **Rules and enforcement are one deliverable** — each section writes its rule AND builds the enforcement in the same section. No front-loaded rule document that drifts from implementation (Codex + Gemini consensus).

2. **Type-system enforcement over runtime assertions** — where Rust's type system already enforces an invariant (e.g., `&ExprArena` immutability at phase boundaries), document it as a rule. Don't add redundant `debug_assert!` (Gemini insight: arena-based `debug_assert!` is impractical and wasteful).

3. **Narrow before broad** — solver budgets before full type folding; diagnostic ordering before packed symbols. Correctness-enforcement gaps first, then architectural patterns.

## Section Dependency Graph

```
§01 Foundation
 ├─→ §02 Immutability (independent)
 ├─→ §03 Solver Budgets (independent)
 ├─→ §04 Diagnostic Ordering (independent)
 ├─→ §05 Incremental Testing (independent)
 └─→ §06 Cross-Target (independent)

§02-§06 all independent of each other

§07 TypeFolder (depends on §01 for policy language)
§08 Packed Symbol (depends on §01 for policy language)

§09 Layout Query (depends on §01; benefits from §07 TypeFolder patterns)
```

## Implementation Sequence

```
Phase 0 — Foundation
  └─ §01: Policy rules, phase documentation, crash regression rule

Phase 1 — Correctness Enforcement (parallel)
  ├─ §02: AST/IR immutability contract (rules only — no code changes)
  ├─ §03: Type solver budget infrastructure (ori_types changes)
  ├─ §04: Diagnostic ordering & suppression (ori_diagnostic changes)
  ├─ §05: Incremental edit-sequence test harness (ori_test_harness changes)
  └─ §06: Cross-target codegen verification (ori_llvm test additions)
  Gate: test-all.sh green, all new rules documented, solver has fuel limits

Phase 2 — Aspirational Patterns (parallel where possible)
  ├─ §07: TypeFolder trait (ori_types refactor)
  ├─ §08: Packed Symbol representation (ori_ir + cross-crate migration)
  Gate: 4+ substitution implementations consolidated, ModuleId exists

Phase 3 — Layout Architecture
  └─ §09: Layout caching via Salsa query (ori_repr → shared crate)
  Gate: all layout consumers use layout_of(), no manual recomputation
```

**Why this order:**
- Phase 0 establishes shared policy language — all subsequent sections reference it.
- Phase 1 closes correctness gaps — these are the "rising tide" that benefits all downstream plans immediately.
- Phase 2 tackles structural patterns — TypeFolder and Symbol are independent but both inform how the codebase's type manipulation evolves.
- Phase 3 is last because layout caching benefits from TypeFolder patterns and is the most architecturally significant refactor.

## Metrics (Current State)

| Area | Production LOC | Test LOC | Files |
|------|---------------|----------|-------|
| `ori_types/pool/substitute/` | ~360 | ~100 | 2 |
| `ori_types/unify/substitute.rs` | ~270 | — | 1 |
| `ori_diagnostic/queue/` | ~394 | ~100 | 2 |
| `ori_test_harness/revision/` | ~88 | ~50 | 2 |
| `ori_llvm/tests/aot/cross.rs` | ~802 | — | 1 |
| `ori_ir/name/` | ~84 | ~30 | 2 |
| `ori_repr/layout/` | ~285 | ~80 | 4 |
| **Estimated total change** | **~2000-3000** | **~1500-2000** | |

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Depends On |
|---------|-------------------|------------|------------|
| 01 Foundation & Policy | ~200 (rules) | Low | — |
| 02 AST/IR Immutability | ~50 (rules) | Low | 01 |
| 03 Solver Budgets | ~400 | Medium | 01 |
| 04 Diagnostic Ordering | ~300 | Medium | 01 |
| 05 Incremental Testing | ~500 | Medium | 01 |
| 06 Cross-Target Codegen | ~300 | Medium | 01 |
| 07 TypeFolder Trait | ~600 | High | 01 |
| 08 Packed Symbol | ~800 | High | 01 |
| 09 Layout Query | ~700 | High | 01, benefits from 07 |
| **Total** | **~3850** | | |

## Cross-Plan Benefits

| Downstream Plan | Benefits From |
|----------------|---------------|
| `perf-engineering` (queued, order 10) | Solver budgets (§03), incremental testing (§05), layout query (§09) |
| `semantic-optimization-pipeline` (active, order 8) | Immutability contract (§02), diagnostic ordering (§04), TypeFolder (§07) |
| `repr-opt` §08-§12 (active, order 4) | Cross-target verification (§06), layout query (§09), packed symbol (§08) |
| `hygiene-full-2` (active, order 6) | TypeFolder eliminates substitution LEAKs they'd otherwise have to fix |
| ALL future plans | Crash regression rule (§01), perf gate rule (§01), incremental testing (§05) |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Foundation & Policy Rules | `section-01-foundation.md` | Not Started |
| 02 | AST/IR Immutability Contract | `section-02-immutability.md` | Not Started |
| 03 | Type Solver Budget Infrastructure | `section-03-solver-budgets.md` | Not Started |
| 04 | Diagnostic Ordering & Suppression | `section-04-diagnostic-ordering.md` | Not Started |
| 05 | Incremental Edit-Sequence Testing | `section-05-incremental-testing.md` | Not Started |
| 06 | Cross-Target Codegen Verification | `section-06-cross-target.md` | Not Started |
| 07 | TypeFolder Trait | `section-07-type-folder.md` | Not Started |
| 08 | Packed Symbol Representation | `section-08-packed-symbol.md` | Not Started |
| 09 | Layout Caching via Salsa Query | `section-09-layout-query.md` | Not Started |
