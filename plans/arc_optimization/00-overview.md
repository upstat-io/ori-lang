# ARC Optimization Plan

> **Design Reference:** `plans/dpr_arc-optimization_02212026.md` — Full prior art analysis and proposed design.

## Goal

Close the gaps between Ori's well-architected ARC analysis pipeline (`ori_arc`) and its LLVM codegen layer (`arc_emitter.rs`) / runtime (`ori_rt`), then enhance the optimization passes with Swift-inspired RC identity normalization and Koka-inspired FBIP enforcement.

## Current State

Ori's ARC system (~2.6k lines in `ori_arc`) is modeled on Lean 4's LCNF approach. The analysis pipeline is complete: borrow inference, derived ownership, liveness, RC insertion, reset/reuse detection, reuse expansion, RC elimination, and FBIP diagnostics. **The gaps are concentrated in the codegen/runtime layer** — the analysis produces correct ARC IR but the LLVM emission has significant stubs (`IsShared` always false, `Reuse` falls back to fresh alloc, `RcDec` passes null drop function, `PartialApply` emits null closures).

## Scope

Three phases, each independently valuable:

1. **Codegen Completeness** (Section 01) — Make existing ARC pipeline production-ready in LLVM
2. **Optimization Enhancements** (Section 02) — Adopt Swift's "Known Safe" and RC identity patterns
3. **Verification & Enforcement** (Section 03) — Add Koka-inspired `@fbip` enforcement + dual-execution verification

## Dependencies

- **Requires**: Section 21A (LLVM Backend) partial — `arc_emitter.rs` and `ori_rt` must exist (they do)
- **Requires**: `ori_arc` pipeline complete (it is — all analysis passes implemented)
- **Blocked by**: Nothing — codegen completeness can proceed immediately
- **Blocks**: Section 17 (Concurrency) needs atomic refcounts from Section 01
- **Blocks**: Any program using heap-allocated user types in AOT needs drop functions from Section 01

## Key Files

| File | Role |
|------|------|
| `compiler/ori_arc/src/lib.rs` | Pipeline orchestration |
| `compiler/ori_arc/src/ir/mod.rs` | ARC IR: `ArcFunction`, `ArcInstr`, `ArcTerminator` |
| `compiler/ori_arc/src/borrow/mod.rs` | Borrow inference, derived ownership |
| `compiler/ori_arc/src/rc_elim/mod.rs` | RC elimination passes |
| `compiler/ori_arc/src/fbip/mod.rs` | FBIP analysis |
| `compiler/ori_arc/src/drop/mod.rs` | Drop descriptors |
| `compiler/ori_llvm/src/codegen/arc_emitter.rs` | ARC IR -> LLVM IR translation |
| `compiler/ori_rt/src/lib.rs` | Runtime: `ori_rc_alloc`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_free` |

## Section Overview

| Section | Focus | Status |
|---------|-------|--------|
| 01 | LLVM Codegen Completeness | Not Started |
| 02 | Optimization Enhancements | Not Started |
| 03 | Verification & Enforcement | Not Started |

## Success Criteria

The plan is complete when:

1. **Codegen complete** — All ARC IR instructions emit correct LLVM IR (no stubs)
2. **Runtime correct** — `ori_rc_inc`/`ori_rc_dec` use atomic operations for thread safety
3. **Optimization enhanced** — RC identity propagation and known-safe elimination reduce redundant RC ops
4. **Verification enabled** — `@fbip` annotation enforces in-place reuse at compile time
5. **Tests passing** — All existing AOT tests pass, new tests cover each completed item
