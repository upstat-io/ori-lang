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
  │     ├── insert_rc_ops_with_ownership()← RC insertion
  │     ├── detect_reset_reuse_cfg()      ← reset/reuse detection
  │     ├── expand_reset_reuse()          ← reuse expansion
  │     ├── propagate_rc_identity()       ← RC identity normalization (NEW)
  │     └── eliminate_rc_ops_dataflow()   ← RC elimination (extended)
  │
  ├── ori_llvm: FunctionCompiler
  │     ├── declare_all() → function signatures + ABI
  │     └── define_all() → ArcIrEmitter per function
  │           ├── emit_block() → walk ARC IR blocks
  │           ├── emit_instr() → produce EmittedValue (NEW)
  │           ├── emit_terminator() → branch/return/switch
  │           ├── emit_builtin() → BuiltinTable dispatch (NEW)
  │           └── emit_drop_fn() → type-specialized drop functions
  │
  └── ori_rt: C-ABI runtime
        ├── ori_rc_{alloc,inc,dec,free,is_shared}
        ├── ori_str_*, ori_list_*, ori_map_*, ori_set_*
        ├── ori_iter_*, ori_channel_*
        └── ori_panic, ori_format_*
```

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
| 01 Emission Layer Typing | ~500 | Medium | — |
| 02 ARC Lowerer Gap Closure | ~200 | Low | — |
| 03 Closure Codegen | ~150 | Medium | — |
| 04 Borrow Inference Hardening | ~80 | Low | — |
| 05 Builtin Method Architecture | ~300 | Medium | — |
| 06 RC Identity Propagation | ~200 | Medium | — |
| 07 Cross-Block RC Elimination | ~400 | High | 06 |
| 08 Salsa Integration | ~150 | Medium | 04 |
| 09 FBIP Enforcement | ~100 | Low | — |
| 10 Legacy Cleanup | ~-11,000 (deletion) | Low | 01-05 |
| 11 Comprehensive Verification | ~500 | Medium | All |
| **Total new** | **~2,600** | | |
| **Total deleted** | **~11,000** | | |

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
