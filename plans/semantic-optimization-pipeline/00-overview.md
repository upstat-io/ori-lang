---
plan: "semantic-optimization-pipeline"
title: "Semantic Optimization Pipeline: Exhaustive Implementation Plan"
status: not-started
supersedes: []
references:
  - "docs/ori_lang/proposals/approved/checks-proposal.md"
  - "plans/roadmap/section-21A-llvm.md"
  - "plans/roadmap/section-15D-bindings-types.md"
  - "plans/roadmap/section-23-evaluator.md"
  - ".claude/rules/arc.md"
  - ".claude/rules/llvm.md"
---

# Semantic Optimization Pipeline: Exhaustive Implementation Plan

## Mission

Make proven facts survive into optimization — including algebraic structure. Ori's compiler already proves rich semantic facts about types, ownership, purity, and structure through the trait system, AIMS lattice, ARC pipeline, and type checker. But too many of these facts are consumed internally and discarded before they can benefit LLVM's optimizer. This plan extends existing infrastructure so that facts the compiler already proves travel one phase farther in the pipeline, and adds algebraic law metadata so that user-defined types (matrix, vector, bignum) receive the same algebraic optimizations that built-in types get today.

Inspired by Tang 2013 ("Lifting the Abstraction Level of Compiler Transformations") — but organically integrated into Ori's existing trait system, AIMS pipeline, and LLVM codegen rather than bolted on as a separate framework.

## Mission Success Criteria

- [ ] LLVM IR emitted by `ori build` contains `!tbaa` metadata on struct field accesses, `!range` on bounded returns (Ordering, bool, enum tags), and `!invariant.load` on immutable borrowed params — verified by `ORI_DUMP_AFTER_LLVM=1` inspection
- [ ] AIMS ownership facts (uniqueness, disjointness, effect summaries) survive into LLVM as `noalias`, `alias.scope`, and refined `memory()` attributes — verified by `ORI_DUMP_AFTER_LLVM=1` + `codegen-audit.sh`
- [ ] Derived Eq/Comparable/Hashable methods are marked `memory(read)` — enabling LLVM GVN/CSE on repeated calls
- [ ] `ori_str_concat("hello", "")` returns the original string (rc_inc, not fresh copy) — verified by `ORI_TRACE_RC=1` showing no allocation
- [ ] `ori_list_concat_cow([], x)` returns `x` unchanged (ownership transfer, no copy) — verified by `ORI_TRACE_RC=1`
- [ ] Operator traits support `algebra { }` blocks and impls support `laws [ ]` declarations — parser, IR, type checker, and registry all store and query algebraic metadata
- [ ] `Zero` and `One` traits exist in the prelude with `zero()` and `one()` methods
- [ ] Stdlib `impl Add for int`, `impl Mul for int`, `impl Add for str` (etc.) have algebraic laws installed (identity, associativity, commutativity as appropriate)
- [ ] A pre-ARC algebraic normalization pass eliminates identity operations (`x + 0 → x`, `x + "" → x`, `x * 1 → x`, `x & -1 → x`, `x | 0 → x`), double negation (`-(-x) → x`, `!!x → x`), and canonicalizes commutative operand order
- [ ] Associativity rewrites enable better CSE/LICM for chains: `(a + b) + c` and `a + (b + c)` normalize to the same canonical form
- [ ] Distributivity with cost model: `a*c + b*c → (a+b)*c` fires when profitable (reduces operation count)
- [ ] All optimizations are sound: float excluded from reassociation unless opt-in, integer overflow handled correctly, user operators purity-gated
- [ ] `./test-all.sh` green — no regressions
- [ ] All plan annotation cleanup verified: zero code annotations referencing `semantic-optimization-pipeline` remain
- [ ] All section success criteria met

## Architecture

```
Source Code (.ori)
    |
    v
Parser ──[algebra {} / laws []]──> ori_ir TraitDef + ImplDef (with AlgebraDecl, AlgebraLaw)
    |
    v
Type Checker ──> ori_types TraitEntry + ImplEntry (with AlgebraLawSet)
    |                                |
    v                                v
Canonicalization ──> CanExpr    TraitRegistry (law queries)
    |                                |
    v                                v
ARC Lowering ──> ArcFunction ──> AIMS Pipeline
    |                    |              |
    |              [step 3b: Algebraic  |
    |               Normalization]      |
    |              (rewrites PrimOp     |
    |               instructions)       |
    |                    |              v
    |                    v         AimsStateMap + MemoryContract
    |              ArcFunction          |
    |              (normalized)         |
    v                    |              v
LLVM Codegen <───────────┘──────────────┘
                         ^
                         |
              AlgebraLawIndex (from TraitRegistry)
    |
    v
LLVM IR (with TBAA, !range, !invariant.load, noalias, alias.scope, memory(read))
    |
    v
LLVM Optimizer (LICM, GVN, SROA, InstCombine work BETTER with metadata)
    |
    v
Machine Code
```

## Design Principles

1. **Metadata-first, strategy-driven** — Follow the DerivedTrait pattern: define algebraic laws as a fixed enum (`AlgebraLaw`), store them as metadata in `TraitEntry`/`ImplEntry`, and have consumers dispatch on the enum. No per-law code duplication between evaluator and LLVM. Adding a new law with an existing rewrite pattern requires zero new consumer code.

2. **Proven facts only — no speculation** — Every optimization must be justified by a fact the compiler already proved or the programmer explicitly declared. AIMS uniqueness → `noalias`. Structural derived Eq → `memory(read)`. Programmer-declared `laws [associative]` → reassociation. No heuristic guessing.

3. **Sound by construction** — Float excluded from reassociation unless explicitly opted in via `laws []`. Integer identity rewrites only for types where the identity operation is truly an identity (runtime must be fixed first). User operator rewrites gated on purity analysis. Unsound optimization is worse than no optimization.

## Section Dependency Graph

```
01 Hygiene ────────────────────────────> 07 Algebra Schema ─────┐
                                           |                    │
02 Metadata Infra ──┬──> 03 AIMS Export    |                    │ (law-export
                    |                      v                    │  plumbing)
                    └──> 04 TBAA/Range  08 Adoption ──> 09 Normalization ──> 10 Advanced
                                           ^
05 Derive Norms (05.1 independent;         |
   05.2-05.3 share infra with 09;         |
   implement 05.2/05.3 AS PART OF 09)     |
                                           |
06 Runtime Identity ───────────────────────┘
```

- **01** (hygiene) gates **07** (algebra schema needs traits/mod.rs split) and practically gates **04** (TBAA emission adds code to instr_dispatch.rs which is currently 587 lines -- must be split first)
- **02** (metadata infra) gates **03** and **04** (both need metadata emission layer)
- **03** and **04** are parallel (AIMS export and TBAA/range are independent)
- **05** (derive normalization) is fully independent
- **06** (runtime identity) gates **08** (adoption installs stdlib identity laws, which require correct runtime)
- **07** (schema) gates **08** (adoption needs the schema to exist)
- **08** (adoption) gates **09** (normalization consumes installed laws)
- **09** (normalization) gates **10** (advanced rewrites build on normalization framework)

**Cross-section interactions (must be co-implemented):**
- **07 + 08**: Schema and adoption must agree on `AlgebraLaw` enum variants and prelude trait definitions. A law variant in the schema with no stdlib adoption is dead code.
- **06 + 08**: Runtime identity fixes and stdlib identity law adoption must agree on which operations are true identities. Installing a law for an unfixed runtime operation is unsound.
- **07 + 09 (CRITICAL plumbing)**: Laws stored in `ori_types` TraitRegistry have NO path to `ori_arc` where the normalization pass runs. The pipeline receives `pool`, `interner`, `contracts` but NOT the TraitRegistry. **Section 07 owns this**: subsection 07.5 MUST add the `AlgebraLawIndex` extraction step: build a compact law summary in `ori_types`, enrich it with exact operator provenance/purity keys, and thread it through `ori_llvm`'s `FunctionCompiler` into `run_arc_pipeline()` → `AimsPipelineConfig`. Without this plumbing, Section 09's normalization pass cannot query laws or prove purity of overloaded operators. Section 09.1 merely consumes the plumbing; it does not build it.
- **08 + 09 (builtin bool logic)**: logical `&&` / `||` are hardcoded language operators, not overloadable traits. Furthermore, **`&&`/`||` are NEVER PrimOps in ARC IR** — `lower_binary()` intercepts `BinaryOp::And`/`Or` and routes them to `lower_short_circuit_and()`/`lower_short_circuit_or()`, producing control-flow IR (branches). Only bitwise `&`/`|` (`BitAnd`/`BitOr`) reach PrimOp. Sections 07-08 must NOT try to express them as `And` / `Or` trait laws. Section 09 CANNOT normalize `x && true` / `x || false` because these patterns do not exist in ARC IR. Section 09 normalizes bitwise identity patterns (`x & -1 → x`, `x | 0 → x`) instead.

## Implementation Sequence

```
Phase 0 - Prerequisites
  └─ 01: Split bloated files (traits/mod.rs, aims_pipeline.rs, instr_dispatch.rs; BUG-04-029 already fixed)

Phase 1 - LLVM Plumbing (parallel)
  ├─ 02: LLVM metadata infrastructure (llvm-sys layer)
  ├─ 05.1: Structural derive memory(read) attribute (05.2/05.3 deferred to Phase 4, subsection 09)
  └─ 06: Runtime identity fixes (str concat, list concat)
  Gate: metadata helpers exist, derives marked memory(read), identity ops are true identities

Phase 2 - Metadata Emission (parallel, requires Phase 1)
  ├─ 03: AIMS state export to codegen (noalias, alias.scope, effect annotations)
  └─ 04: TBAA, range, invariant.load metadata emission
  Gate: ORI_DUMP_AFTER_LLVM=1 shows TBAA/range/noalias metadata on appropriate instructions

Phase 3 - Algebra Foundation (requires Phase 0 + Phase 1)
  ├─ 07: Algebra law schema (parser/IR/registry/types)
  └─ 08: Algebra law adoption (prelude/stdlib instances)
  Gate: algebra {} parses, laws [] validates, Zero/One traits exist, stdlib laws installed

Phase 4 - Algebraic Optimization (requires Phase 3)
  └─ 09: Algebraic normalization pass (step 3b in AIMS pipeline)
  Gate: identity elimination, double negation, canonical ordering, associativity all fire on test programs

Phase 5 - Advanced & Verification (requires Phase 4)
  └─ 10: Distributivity, multiplicative inverse, full verification suite
  Gate: ./test-all.sh green, all mission success criteria met
```

**Why this order:**
- Phase 0-1 are pure additions — no behavioral changes to existing code paths (except runtime identity fixes, which are correctness improvements).
- Phase 2 emits metadata that LLVM already knows how to consume — immediate optimization benefit with zero risk.
- Phase 3 adds language surface (algebra blocks) — must be after hygiene (file splits) and runtime fixes (identity soundness).
- Phase 4-5 add compiler-internal optimization passes — highest complexity, highest reward, latest in sequence.

**Known failing tests (expected until plan completion):**
- None expected — each phase is additive. Runtime identity fixes (Phase 1) may change allocation counts in existing tests; those test updates are part of Section 06.

## Metrics (Current State)

| Area | Key Files | Production LOC | Test LOC |
|------|-----------|---------------|----------|
| Trait IR | `ori_ir/src/ast/items/traits.rs` | ~400 | ~0 |
| Trait Registry | `ori_types/src/registry/traits/mod.rs` | ~765 | ~0 |
| AIMS Pipeline | `ori_arc/src/pipeline/aims_pipeline.rs` | ~590 | ~0 |
| LLVM Attributes | `ori_llvm/src/codegen/ir_builder/attributes.rs` | ~287 | ~0 |
| ARC Emitter | `ori_llvm/src/codegen/arc_emitter/mod.rs` | ~250 | ~0 |
| Instr Dispatch | `ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` | ~587 | ~0 |
| Operator Codegen | `ori_llvm/src/codegen/arc_emitter/operators/` | ~660 | ~0 |
| String Ops | `ori_rt/src/string/ops.rs` | ~273 | ~0 |
| List COW Sort | `ori_rt/src/list/cow_sort/mod.rs` | ~221 | ~0 |
| Iterator Builtins | `ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs` | ~449 | ~0 |
| Prelude | `library/std/prelude.ori` | ~367 | ~0 |
| **Estimated New** | | **~3,000-4,000** | **~2,000-3,000** |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 Hygiene & Targeted Fixes | ~250 (refactor) | Low | — |
| 02 LLVM Metadata Infrastructure | ~300 | Medium | — |
| 03 AIMS State Export | ~250 | Medium | 02 |
| 04 TBAA, Range, Invariant Metadata | ~350 | Medium | 02 |
| 05 Structural Derive Normalization | ~200 | Low | — (05.1 in Phase 1; 05.2/05.3 done in 09) |
| 06 Runtime Identity Fixes | ~150 | Medium | — |
| 07 Algebra Law Schema | ~500 | High | 01 |
| 08 Algebra Law Adoption | ~300 | Medium | 06, 07 |
| 09 Algebraic Normalization Pass | ~600 | High | 08 |
| 10 Advanced Rewrites & Verification | ~500 | High | 09 |
| **Total new** | **~3,350** | | |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| BUG-04-029: LLVM missing shift overflow checks | Interpreter has checks but codegen emits raw shl/ashr | `checked_ops.rs` | **ALREADY FIXED** — `checked_shl()`/`checked_shr()` emit negative count, width, and overflow checks |
| `ori_str_concat` copies on empty operand | `OriStr::from_bytes()` creates copy, not alias | Section 06 | Not Started |
| `ori_list_concat_cow` copies `[] + x` when shared | Shared/slice path doesn't do ownership transfer | Section 06 | Not Started |

## Cross-Plan References

| External Item | Location | Relationship |
|---------------|----------|--------------|
| Contract enforcement (pre/post) | `plans/roadmap/section-15D`, `section-23`, `section-21A` | Separate plan — identified as blocker but orthogonal to algebraic optimization |
| Iterator specialization | Consensus workstream D | Separate plan — backend-performance track |
| Escape analysis | `plans/repr-opt/section-08-escape-analysis.md` | Section 03 lays groundwork for future escape analysis integration. repr-opt §08 also extends `AimsPipelineConfig` — coordinate field additions to avoid merge conflicts. |
| Range metadata (Tier 3) | `plans/roadmap/section-21A-llvm.md` line 859 | Absorbed by Section 04 — `<!-- resolved-by: plans/semantic-optimization-pipeline/section-04 -->` |
| Impl colon syntax | `plans/roadmap/section-03-traits.md` §3.23 | If impl syntax migration (`impl Trait for Type` → `impl Type: Trait`) lands during this plan, Section 08 stdlib law declarations must use the new syntax. Not a blocker — just match whatever syntax `prelude.ori` uses at implementation time. |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Hygiene & Targeted Fixes | `section-01-hygiene.md` | Not Started |
| 02 | LLVM Metadata Infrastructure | `section-02-metadata-infra.md` | Not Started |
| 03 | AIMS State Export | `section-03-aims-export.md` | Not Started |
| 04 | TBAA, Range, Invariant Metadata | `section-04-tbaa-range.md` | Not Started |
| 05 | Structural Derive Normalization | `section-05-derive-norms.md` | Not Started |
| 06 | Runtime Identity Fixes | `section-06-runtime-identity.md` | Not Started |
| 07 | Algebra Law Schema | `section-07-algebra-schema.md` | Not Started |
| 08 | Algebra Law Adoption | `section-08-algebra-adoption.md` | Not Started |
| 09 | Algebraic Normalization Pass | `section-09-normalization.md` | Not Started |
| 10 | Advanced Rewrites & Verification | `section-10-advanced-verification.md` | Not Started |
