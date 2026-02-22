# Section 02 — Wire JIT to ARC Pipeline

## File: `compiler/ori_llvm/src/evaluator.rs`

### Changes

1. Add ARC pipeline setup before `FunctionCompiler::new()`:
   - Create `ArcClassifier::new(pool)`
   - Lower module functions + imported functions + impl methods to ARC IR
   - Run `ori_arc::infer_borrows()` to get `annotated_sigs`

2. Update `FunctionCompiler::new()` call:
   - Pass `&annotated_sigs` and `&classifier` instead of `None, None`

3. Rewrite `compile_tests()` to use ARC path:
   - Currently creates `ExprLowerer` directly
   - Must use ARC pipeline (lower -> pipeline -> ArcIrEmitter)
