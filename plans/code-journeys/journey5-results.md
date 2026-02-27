# Journey 5: "I am a closure"

**Code**:
```ori
@apply (f: (int) -> int, x: int) -> int = f(x);
@make_adder (n: int) -> (int) -> int = x -> x + n;

@main () -> int = {
    let double = x -> x * 2;
    let a = apply(double, 5);      // = 10
    let add10 = make_adder(10);
    let b = add10(7);              // = 17
    a + b                          // = 27
}
```
**Source**: 485 bytes, **Expected Result**: 27 (= 10 + 17)
**Actual**: Eval = 27 (correct), **AOT = CRASH** (LLVM verification error)

---

## Transformation Timeline

### Stage 1-2: Lexer
```
User:    485 bytes → 106 tokens (6 comments, 0 errors)
Prelude: 10,331 bytes → 1,516 tokens (unchanged)
```

### Stage 3: Parser
```
User:    106 tokens → 3 functions, 27 expressions, 0 errors
```

### Stage 5: Canonicalizer
```
User:    3 functions, 27 source_exprs → 29 canon_nodes, 3 roots, 6 constants, 0 decision_trees
```
- 7.4% expansion — lambda bodies add canon nodes

### Stage 6a: Eval Path
```
Total eval_can calls:  29
```
- Works correctly. Closures capture by value, lambda invocation works.

### Stage 6b: LLVM Path — CRASH

#### Error Output
```
ERROR ori_llvm::codegen::ir_builder::phi_types_blocks:
  parameter index out of bounds — returning zero
  func=_ori___lambda_0 param_index=2 param_count=2

thread 'main' panicked at inkwell-0.8.0/src/module.rs:1654:9:
  Cloning a Module seems to segfault when module is not valid.
  Error: "Incorrect number of arguments passed to called function!
    %result = call i64 @_ori___lambda_1(i64 %cap.0)"
```

#### Root Cause Analysis
1. **Lambda numbering collision**: When both non-capturing (`lambda_0`: `x -> x * 2`) and capturing (`lambda_1`: `x -> x + n`) lambdas exist in the same module, the closure calling convention gets confused
2. **The capturing closure `lambda_1` expects 2 args**: `(i64 %cap.n, i64 %x)` — the captured `n` and the parameter `x`
3. **But the generated call only passes 1 arg**: `call i64 @_ori___lambda_1(i64 %cap.0)` — only the capture, missing the actual parameter
4. **param_index out of bounds**: `_ori___lambda_0` (non-capturing, 2 params expected) tries to access param_index=2 when only 2 params exist (0-indexed, so index 2 is OOB)

#### Isolation Tests
| Test | Result |
|------|--------|
| Non-capturing lambda alone (`x -> x * 2`) | AOT works (exit 10) |
| Capturing closure alone (`x -> x + n`) | AOT works (exit 17) |
| Both in same module | **AOT crashes** |

This confirms the bug is in lambda numbering or calling convention assignment when multiple lambda types coexist.

---

## Issues Found

### CRITICAL

**C1 (NEW): AOT crashes when non-capturing lambda AND capturing closure coexist in same module**
- Eval returns 27 (correct), AOT panics with LLVM verification error
- Root cause: lambda argument count mismatch — capturing closure called with only capture args, missing actual parameter
- The `phi_types_blocks` module logs "parameter index out of bounds — returning zero" — the codegen emits a zero instead of the correct value
- Severity: CRITICAL — this is a behavioral mismatch (eval works, AOT crashes) and affects any real program that uses both types of closures
- Workaround: Use only one type of closure per module (either all capturing or all non-capturing)
- Affected component: `ori_llvm::codegen::ir_builder::phi_types_blocks`

### MEDIUM
**CONFIRMED M1**: Prelude overhead (10,331 bytes)
**CONFIRMED M3**: Dead branches (eval path, as usual)

### LOW
**L1**: Canon expansion 7.4% for closure code

### CONFIRMED FROM PREVIOUS JOURNEYS
- M1: Prelude overhead
- M2: No `nsw` (not triggered — crash before IR emission)
- M3: Dead branches (eval side)

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | 27 (correct) | **CRASH** (LLVM verification error) |
| Non-capturing lambda | Works | Works (alone) |
| Capturing closure | Works | Works (alone) |
| Both together | Works | **FAILS** |
| Exit code | 27 | 101 (panic) |
