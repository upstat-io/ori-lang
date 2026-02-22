---
plan: "aot_codegen_pipeline"
title: "AOT Codegen Pipeline: Exhaustive Implementation Plan"
status: in-progress
supersedes:
  - "plans/arc_optimization/"
  - "plans/arc_codegen_unification/"
references:
  - "plans/dpr_aot-codegen-pipeline_02222026.md"
---

# AOT Codegen Pipeline: Exhaustive Implementation Plan

## Mission

Complete Ori's AOT compilation pipeline as one cohesive system: from typed AST through ARC-optimized IR to native code via LLVM. This plan unifies all prior arc_optimization and arc_codegen_unification work into a single sequenced roadmap.

## Architecture

```
CanExpr (typed AST)
  │
  ├── ori_arc: lower_function_can()
  │     ↓
  │   ArcFunction (basic blocks, SSA, ownership annotations, ValueRepr)
  │     │
  │     ├── infer_derived_ownership()     ← borrow inference (Lean 4 fixed-point)
  │     ├── compute_refined_liveness()    ← liveness analysis
  │     ├── insert_rc_ops_with_ownership()← RC insertion (with RcStrategy + CallOwnership)
  │     ├── detect_reset_reuse_cfg()      ← reset/reuse detection
  │     ├── expand_reset_reuse()          ← reuse expansion
  │     ├── propagate_rc_identity()       ← RC identity normalization
  │     └── eliminate_rc_ops_dataflow()   ← RC elimination (extended)
  │
  ├── ori_llvm: FunctionCompiler
  │     ├── declare_all() → function signatures + ABI
  │     └── define_all() → ArcIrEmitter per function
  │           ├── emit_block() → walk ARC IR blocks
  │           ├── emit_instr() → produce EmittedValue (typed)
  │           ├── emit_rc_op() → match on RcStrategy (no Pool queries)
  │           ├── emit_terminator() → branch/return/switch
  │           ├── emit_builtin() → BuiltinTable dispatch
  │           └── emit_drop_fn() → type-specialized drop functions
  │
  └── ori_rt: C-ABI runtime
        ├── ori_rc_{alloc,inc,dec,free,is_shared}
        ├── ori_str_*, ori_list_*, ori_map_*, ori_set_*
        ├── ori_iter_*, ori_channel_*
        └── ori_panic, ori_format_*
```

## Information Contract Chain

The core architectural principle: **each stage enriches the IR for the next stage.** No downstream stage should need to re-derive information that an upstream stage already computed. The chain flows left to right:

```
Pool + ArcClassifier        ← type system (source of truth)
  ↓
ValueRepr (per variable)    ← Section 01.1: Scalar / RcPointer / Aggregate / FatValue
  ↓
RcStrategy (per RC op)      ← Section 01.3: how to inc/dec (heap / closure / enum / fat / skip)
  ↓
CallOwnership (per arg)     ← Section 04.4: borrowing vs consuming at each call site
  ↓
EmittedValue (per LLVM val) ← Section 01.2: tagged LLVM value (Immediate / RcPointer / Aggregate / Pair)
```

**Why this matters:** The 5-hour ARC leak debugging session on 2026-02-22 exposed a design flaw at the boundary between `ori_arc` and `ori_llvm`. The ARC IR carried only raw type indices (`Idx`) on RC operations — the LLVM emitter had to reach back into the Pool to determine the cleanup strategy (closure vs enum vs heap pointer). A misclassification (e.g., treating a `Result<int, str>` as a heap pointer instead of an inline enum) silently corrupted memory. With the contract chain, each RC instruction carries its strategy, and the emitter pattern-matches on it — the wrong strategy is **impossible** because the decision was made during lowering when the full type information was available.

## Section Dependency Graph

```
  01 Emission Typing ──────┐
  02 Lowerer Gaps ─────────┤
  03 Closure Codegen ──────┤
  04 Borrow Hardening ─────┼──→ 10 Legacy Cleanup ──→ 11 Verification
  05 Builtin Architecture ─┤
  06 RC Identity ──────────┼──→ 07 Cross-Block Elim
  08 Salsa Integration ────┘
  09 FBIP Enforcement ─────────→ 11 Verification
```

Sections 01-06 are independent of each other and can be worked in any order.
Section 07 requires 06. Section 10 requires 01-05. Section 11 requires all.

**Cross-section interactions (must be co-implemented):**
- **01.3 (RcStrategy) + 04.4 (ArgOwnership)**: The Result/Enum ARC leaks require BOTH fixes. RcStrategy::InlineEnum makes Inc a documented no-op and Dec a tag-switch. But without ArgOwnership recognizing builtin methods (is_err, unwrap, etc.) as borrowing, the RC inserter never emits cleanup RcDec for Result args — so inner RC fields leak regardless of strategy. These two subsections must land together.
- **02 (Lowerer) + 04.4 (ArgOwnership)**: The preferred fix for builtin method borrowing (option a: lower as PrimOp) requires changes in the ARC lowerer's `emit_call_or_invoke` in `lower/calls/mod.rs` — NOT `lower_method_call`, because the canonical IR desugars `r.is_err()` to `is_err(r)` as a direct call. This is a Section 02 change that resolves a Section 04.4 bug. If option (b) is used instead (synthetic `annotated_sigs` entries), the lowerer changes are not needed.
- **01.3 (RcStrategy) + 04.4 (Project Borrowing)**: Even after fixing builtin methods, the `?` operator's `lower_try` uses raw `Project` for tag extraction. `Project` isn't classified as borrowing → Perceus consumes the parent Result → inner RC fields leak. The fix (Lean 4 model: `proj` borrows parent) requires THREE co-implementations: (1) `is_borrowing_instr` classifies Project as borrowing, (2) `compute_refined_liveness` correctly propagates parent liveness through borrowing projections so Dec is placed at last use per-path (not at branch point), (3) the Dec for the parent Result uses RcStrategy::InlineEnum (tag-switch + per-variant field Dec). Missing any one causes either leaks (#1), heap corruption (#2), or wrong cleanup strategy (#3).
- **01.3 (RcStrategy) + 02 (Lowerer Gaps)**: The Idx::ERROR bug in lower_try() (now fixed in working tree) caused the Err payload to be projected as i64 instead of the actual error type. Without this fix, InlineEnum Dec would tag-switch on correct tag but access truncated payload data.

**Internal dependency within Section 01:** 01.1 (ValueRepr) feeds 01.3 (RcStrategy) feeds 01.2 (EmittedValue).
**Cross-section dependency:** Section 04.4 (CallOwnership) uses ValueRepr from 01.1 to classify borrowing args.

## Implementation Sequence

The section dependency graph (above) says Sections 01-06 are independent. The cross-section interactions (above) say they're not — at least not for the ARC correctness path. This sequence resolves the contradictions into a concrete build order. Each phase gates the next; items within a phase can be parallelized.

```
Phase 0 ─ Commit prerequisites
  └─ 02: Commit Idx::ERROR fix (already in working tree)

Phase 1 ─ Type foundation
  └─ 01.1: ValueRepr enum + classify()
  └─ 01.4: Propagate ValueRepr through existing passes

Phase 2 ─ RC strategy
  └─ 01.3: RcStrategy enum + classify_rc_strategy()
       ├─ InlineEnum Inc → documented no-op
       ├─ InlineEnum Dec → tag-switch + per-variant field traversal
       └─ Delete dead emit_inline_enum_inc (159 lines)

Phase 3 ─ Liveness + Project borrowing  [CRITICAL PATH]
  └─ 04.4a: Fix compute_refined_liveness for borrowing projections
       └─ Borrowing source → GEN set, NOT KILL set
       └─ Fixed-point propagates parent liveness across blocks
  └─ 04.4b: Add Project to is_borrowing_instr (Lean 4 model)
       └─ Scalar result → borrows parent, no Inc
       └─ RC-typed result → borrows parent, Inc on result
  └─ 04.4c: Fix process_block_rc Dec placement
       └─ Variable in live_out → defer Dec to successors
       └─ Dec at LAST use per control-flow path independently
  Gate: ? operator pattern emits correct ARC IR (no Dec in B3,
        Dec in B4 and B5 using InlineEnum strategy from Phase 2)

Phase 4 ─ Builtin PrimOp lowering
  └─ 02/04.4: try_lower_tag_check in emit_call_or_invoke
       └─ Intercept is_err/is_ok/is_some/is_none
       └─ Emit Project(tag) + PrimOp::Eq instead of Invoke
  Gate: never_propagation.ori ARC leak tests pass

Phase 5 ─ IR contract cleanup
  └─ 04.4: ArgOwnership enum + embed in Invoke/Apply
  └─ 04.4: Simplify insert_external_invoke_cleanup
  └─ 04.4: Remove interner from RcContext
  └─ 01.2: EmittedValue (tagged LLVM values)
  Gate: grep -r "interner" ori_arc/src/ returns zero results

Phase 6 ─ Independent hardening (any order)
  ├─ 04.1-04.3: Diagnostics, O(1) dispatch, debug_assert
  ├─ 03: Closure codegen
  ├─ 05: Builtin architecture (BuiltinTable)
  ├─ 06: RC identity propagation
  └─ 01.5: Emission layer tests

Phase 7 ─ Downstream
  ├─ 07: Cross-block RC elimination (requires 06)
  ├─ 08: Salsa integration (requires 04)
  └─ 09: FBIP enforcement

Phase 8 ─ Cleanup + verification
  ├─ 10: Legacy cleanup (~11K lines deleted, requires 01-05)
  └─ 11: Comprehensive verification (requires all)
```

**Why this order:**
- Phase 0-1 are pure additions — no behavioral changes, nothing can break.
- Phase 2 (RcStrategy) must precede Phase 3 because the InlineEnum Dec strategy is required for correct cleanup when borrowing projections emit Dec.
- Phase 3 is the critical path. The three sub-steps (liveness, borrowing classification, Dec placement) are tightly coupled and must land as one atomic change. The heap corruption from the 2026-02-22 session proves that #3b without #3a corrupts memory.
- Phase 4 (PrimOp) depends on Phase 3 for the `?` operator to work, but the `try_lower_tag_check` code itself can be written during Phase 2-3 and tested once Phase 3 lands.
- Phase 5 is cleanup — making the IR carry the decisions that earlier phases computed, then deleting the runtime re-derivation code.
- Phase 6 is independent work that doesn't interact with the ARC correctness path.

**Known failing tests (expected until plan completion):**

AOT tests involving Result/Option with RC-typed inner fields are expected to fail until Phase 4 is complete. These are not regressions — they are symptoms of the missing infrastructure documented in this plan. Specifically:

- **`never_propagation.ori`** (3 tests: `test_try_chain_first_err`, `test_try_chain_second_err`, `test_nested_try_err`) — ARC leaks on Result<int, str> passed through `?` operator. Root cause: builtin methods not classified as borrowing (Phase 4) AND Project not classified as borrowing (Phase 3). Both must be fixed.
- **Any AOT test using `?` on `Result<T, E>` where E contains RC fields** — `lower_try` emits `Project` for tag extraction, which consumes the parent Result without cleanup. Root cause: Phase 3 (Project borrowing + liveness fix).
- **Any AOT test using `is_err`/`is_ok`/`is_some`/`is_none` on values with RC fields** — these are lowered as `Invoke` (function call) but emitted inline as tag reads, so nobody emits Dec. Root cause: Phase 4 (PrimOp lowering).
- **`Option<T>` where T is RC-typed and value is `None`** — `extract_rc_data_ptrs` reads garbage payload. Root cause: Phase 2 (InlineEnum strategy with tag-switch).

Do NOT attempt to fix these tests individually. They share infrastructure dependencies that must be built bottom-up through Phases 1-4. Attempting point fixes leads to the heap corruption observed in the 2026-02-22 session.

## Metrics (Current State — 2026-02-22)

| Crate | Production LOC | Test LOC | Total |
|-------|---------------|----------|-------|
| `ori_arc` | ~10,800 | ~10,700 | ~21,500 |
| `arc_emitter` | ~4,300 | ~1,000 | ~5,300 |
| `ori_rt` | ~2,100 | ~400 | ~2,500 |
| **Total** | **~17,200** | **~12,100** | **~29,300** |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 Emission Layer Typing | ~650 | Medium-High | — |
|   ↳ 01.1 ValueRepr | ~150 | Low | — |
|   ↳ 01.2 EmittedValue | ~200 | Medium | 01.1 |
|   ↳ 01.3 RcStrategy | ~200 | Medium | 01.1 |
|   ↳ 01.4 Pass propagation | ~50 | Low | 01.1 |
|   ↳ 01.5 Tests | ~50 | Low | 01.1-01.4 |
| 02 ARC Lowerer Gap Closure | ~200 | Low | — |
| 03 Closure Codegen | ~150 | Medium | — |
| 04 Borrow Inference Hardening | ~250 | Medium | — |
|   ↳ 04.1-04.3 Diagnostics & O(1) | ~80 | Low | — |
|   ↳ 04.4 CallOwnership in IR | ~170 | Medium | 01.1 |
| 05 Builtin Method Architecture | ~300 | Medium | — |
| 06 RC Identity Propagation | ~200 | Medium | — |
| 07 Cross-Block RC Elimination | ~400 | High | 06 |
| 08 Salsa Integration | ~150 | Medium | 04 |
| 09 FBIP Enforcement | ~100 | Low | — |
| 10 Legacy Cleanup | ~-11,000 (deletion) | Low | 01-05 |
| 11 Comprehensive Verification | ~500 | Medium | All |
| **Total new** | **~2,900** | | |
| **Total deleted** | **~11,000** | | |

## Known Bugs (Pre-existing, Discovered 2026-02-22)

These bugs were discovered during the debugging session. They affect multiple sections and must be fixed as prerequisites or as part of the listed sections.

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| ARC leak: Result args to builtin methods | Canonical IR desugars `r.is_err()` to `is_err(r)` (direct call, CanExpr::Ident), so it goes through `lower_call` → `emit_call_or_invoke` → `Invoke`. RC inserter sees unknown callee → treats as consuming; LLVM emitter inlines as tag read → nobody emits Dec. First fix attempt in `lower_method_call` failed (wrong interception point). | Section 04.4 (preferred: intercept in `emit_call_or_invoke`) | Not Started |
| `Idx::ERROR` in `lower_try()` | Err payload projected as `Idx::ERROR` (i64) instead of actual error type | Section 02 | **Fixed** (uncommitted) |
| "variable not yet defined" errors | Pre-existing; invoke destination vars sometimes not registered before successor blocks run | Section 01.3 (guard) | Guarded, root cause TBD |
| InlineEnum Inc crashes on undefined vars | `emit_inline_enum_inc` operates on `ValueId::NONE` | Section 01.3 (guard + root cause) | Guarded |
| Option<T> RC on None variant | `extract_rc_data_ptrs` checks type but not runtime tag — reads garbage payload from None | Section 01.3 | Not Started |
| Project not classified as borrowing | `is_borrowing_instr` returns false for `Project`, so Perceus consumes parent on scalar field extraction (e.g., tag from Result). Inner RC fields leak. Lean 4 classifies `proj` as borrowing. Initial blanket fix (`is_scalar(dst)`) caused heap corruption due to cross-block liveness bug — the liveness analysis must be fixed to propagate parent variable liveness through borrowing projections (see Section 04.4). | Section 04.4 (Project Borrowing) + liveness fix in `compute_refined_liveness` | Investigated, not fixed |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Emission Layer Typing | `section-01-emission-layer-typing.md` | Not Started |
| 02 | ARC Lowerer Gap Closure | `section-02-lowerer-gaps.md` | Not Started |
| 03 | Closure Codegen Completion | `section-03-closure-codegen.md` | Not Started |
| 04 | Borrow Inference Hardening | `section-04-borrow-hardening.md` | Not Started |
| 05 | Builtin Method Architecture | `section-05-builtin-architecture.md` | Not Started |
| 06 | RC Identity Propagation | `section-06-rc-identity.md` | Not Started |
| 07 | Cross-Block RC Elimination | `section-07-cross-block-elim.md` | Not Started |
| 08 | Salsa-Integrated Borrow Inference | `section-08-salsa-integration.md` | Not Started |
| 09 | FBIP Enforcement | `section-09-fbip-enforcement.md` | Not Started |
| 10 | Legacy Cleanup & Unification | `section-10-legacy-cleanup.md` | Not Started |
| 11 | Comprehensive Verification | `section-11-verification.md` | Not Started |
