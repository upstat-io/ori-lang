# ARC Codegen Unification — Keyword Index

## Sections

| Section | Title | Keywords |
|---------|-------|----------|
| [00](00-overview.md) | Overview | goals, dependencies, key files, architecture |
| [01](section-01-fix-arc-lowerer-gaps.md) | Fix ARC Lowerer Gaps | FunctionExp, FunctionRef, HashLength, FormatWith, Await, WithCapability, CanExpr, ArcLowerer |
| [02](section-02-wire-jit-arc-pipeline.md) | Wire JIT to ARC Pipeline | evaluator.rs, compile_module_with_tests, ArcClassifier, borrow inference, JIT |
| [03](section-03-remove-tier1.md) | Remove Tier 1 | ExprLowerer, Scope, feature flag, use_arc_codegen, delete files, 11K lines |
| [04](section-04-cleanup-and-verify.md) | Cleanup & Verify | SYNC comments, llvm.md, arc.md, dead references, final verification |

## Key Terms

- **Tier 1**: `ExprLowerer`-based codegen (direct CanExpr -> LLVM IR, no RC). Being removed.
- **Tier 2**: `ArcIrEmitter`-based codegen (CanExpr -> ARC IR -> LLVM IR, with RC). Becoming the only path.
- **ARC pipeline**: classify -> lower -> borrow infer -> liveness -> RC insert -> detect/expand reuse -> RC eliminate
- **Feature flag**: `FunctionCompiler::use_arc_codegen: bool` (defaults false, never enabled). Being removed.
