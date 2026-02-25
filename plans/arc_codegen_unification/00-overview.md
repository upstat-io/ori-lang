# Section 00 — Overview

> **SUPERSEDED (2026-02-25):** This plan has been superseded by [`plans/aot_codegen_pipeline/`](../aot_codegen_pipeline/00-overview.md). The Tier 1 removal, ARC unification, and all codegen gap closure work was completed in Sections 01-10 of the AOT codegen pipeline. Refer to that plan for the current state.

## Goal

Remove the Tier 1 / Tier 2 feature flag and unify to a single ARC-based codegen pipeline.
Delete ~11,000 lines of Tier 1 code (ExprLowerer + 25 files). ARC codegen becomes the only path.

## Current State

- `FunctionCompiler::use_arc_codegen: bool` defaults to `false`
- `set_arc_codegen(true)` is never called anywhere
- ARC pipeline runs in AOT but output is never consumed (annotated_sigs computed, but Tier 1 ignores them)
- JIT path passes `None` for both classifier and annotated_sigs

## Dependencies

- `ori_arc` — ARC IR lowering, borrow inference, RC pipeline
- `ori_llvm` — LLVM codegen (FunctionCompiler, ArcIrEmitter, ExprLowerer)
- `oric` — AOT compilation (compile_common.rs)

## Key Files

| File | Role |
|------|------|
| `ori_arc/src/lower/expr/mod.rs` | ARC IR expression lowering (6 gaps to fix) |
| `ori_llvm/src/evaluator.rs` | JIT pipeline (needs ARC setup) |
| `ori_llvm/src/codegen/function_compiler/mod.rs` | Feature flag, dispatch, tests |
| `oric/src/commands/compile_common.rs` | AOT FunctionCompiler construction |
| `ori_llvm/src/codegen/mod.rs` | Module declarations for Tier 1 files |
| `ori_llvm/src/codegen/arc_emitter/mod.rs` | Tier 2 emitter (SYNC comments to clean) |

## Ordering

1. Fix ARC lowerer gaps (6 CanExpr variants)
2. Wire JIT to ARC pipeline
3. Remove feature flag
4. Delete Tier 1 code
5. Cleanup references and documentation
6. Final verification
