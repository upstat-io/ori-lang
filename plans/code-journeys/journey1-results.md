# Journey 1: "I am arithmetic"

**Code**:
```ori
// Journey 1: "I am arithmetic"
// Features: int literals, let bindings, arithmetic ops, one function call
// Expected: (3 + 4) * 5 - 2 = 33

@add (a: int, b: int) -> int = a + b;

@main () -> int = {
    let x = 3;
    let y = 4;
    let sum = add(x, y);   // = 7
    let result = sum * 5 - 2;  // = 35 - 2 = 33
    result
}
```
**Source**: 326 bytes, **Expected Result**: 33 (= (3+4)*5-2)
**Actual**: Eval = 33 (correct), AOT = 33 (correct)

---

## Transformation Timeline

### Stage 1-2: Lexer
```
User:    326 bytes → 71 tokens (5 comments, 0 errors, 0 warnings)
Prelude: 10,331 bytes → 1,516 tokens (126 comments, 0 errors, 0 warnings)
```
- User bytes/token ratio: 4.6 — typical for code with comments
- Prelude bytes/token ratio: 6.8 — higher due to comment density (126 comments vs 5)
- **Prelude is 31.7x larger in bytes and 21.4x more tokens than user code**

### Stage 3: Parser
```
User:    71 tokens → 2 functions, 0 tests, 0 types, 0 traits, 0 impls, 0 imports, 16 expressions, 0 errors
Prelude: 1,516 tokens → 9 functions, 0 tests, 0 types, 39 traits, 0 impls, 0 imports, 46 expressions, 0 errors
```
- 39 trait definitions in the prelude — these are the built-in traits (Eq, Printable, Clone, etc.)
- 9 prelude functions (print, panic, hash_combine, comparison helpers, etc.)
- User code: 8 expressions per function average (16 / 2)

### Stage 4: Type Checker
```
Prelude: registration 9 functions, 0 tests, 0 impls → signatures → body checking
User:    registration 2 functions, 0 tests, 0 impls → signatures → body checking
```
- Prelude is type-checked first (as an import), then user code
- Both modules complete all 3 phases (registration, signature collection, body checking) with 0 errors
- No monomorphization recorded — all concrete types (int)

### Stage 5: Canonicalizer
```
User:    2 functions, 16 source_exprs → 20 canon_nodes, 2 roots, 0 method_roots, 6 constants, 0 decision_trees
Prelude: 9 functions, 46 source_exprs → 46 canon_nodes, 9 roots, 0 method_roots, 6 constants, 4 decision_trees
```
- User code expansion: 16 → 20 canon nodes (25% growth) — let-binding pattern desugaring adds 4 nodes
- Prelude expansion: 46 → 46 (1:1) — prelude functions are already in canonical form
- 6 constants in user code: `3`, `4`, `5`, `2`, and the function refs for `add` and `main`
- 4 decision trees in prelude — likely from match expressions in comparison helpers

### Stage 6a: Eval Path
```
Total eval_can calls:    20
Binary operations:       3 (Add, Mul, Sub)
Function calls:          1 (add)
Let bindings:            4 (x, y, sum, result)
Identifier lookups:      8
Integer literals:        4
Block entry:             1
```
Execution trace (simplified):
1. Enter `@main` block
2. `let x = 3` → bind
3. `let y = 4` → bind
4. `let sum = add(x, y)` → lookup `add`, lookup `x`=3, lookup `y`=4 → call add(3, 4)
5. Inside `@add`: `a + b` → lookup `a`=3, lookup `b`=4 → Add(3, 4) = 7
6. `let result = sum * 5 - 2` → lookup `sum`=7 → Mul(7, 5)=35 → Sub(35, 2)=33
7. `result` → lookup = 33
8. Return 33

### Stage 6b: LLVM Path

#### Generated LLVM IR
```llvm
; ModuleID = 'journey1'
source_filename = "journey1"

; Function Attrs: nounwind
define fastcc i64 @_ori_add(i64 %0, i64 %1) #0 {
bb0:
  %add = add i64 %0, %1
  ret i64 %add
}

; Function Attrs: nounwind
define i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_add(i64 3, i64 4)
  br label %bb1

bb1:                                              ; preds = %bb0
  %mul = mul i64 %call, 5
  %sub = sub i64 %mul, 2
  ret i64 %sub
}

define i32 @main() {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}

attributes #0 = { nounwind }
```

#### Key Observations
1. **Zero runtime declarations** — pure arithmetic needs no runtime. Very clean.
2. **`fastcc` on `@_ori_add`** — internal functions correctly use fast calling convention
3. **C calling convention on `@_ori_main`** — correct, called from C entry point `@main`
4. **Constants inlined as immediates** — `i64 3`, `i64 4`, `i64 5`, `i64 2` directly in instructions
5. **Unnecessary `br label %bb1`** in `@_ori_main` — after the call to `@_ori_add`, there's a branch to the immediately following basic block. This is a dead branch that LLVM's optimizer would eliminate, but it's unnecessary codegen noise.
6. **No `nsw`/`nuw` flags on arithmetic** — `add`, `mul`, `sub` lack `nsw` (no signed wrap) flags. Since Ori panics on integer overflow, these could carry `nsw` to enable better LLVM optimization (constant folding, loop unrolling, etc.). Without `nsw`, LLVM must assume wrapping semantics.
7. **`trunc i64 to i32` for exit code** — correct for converting Ori's `int` (i64) to C's `int` (i32)
8. **No ARC operations** — no RC needed for pure integers. Correct.
9. **AOT cache works** — second run produces no "Compiling" message, executes from cache

---

## Issues Found

### CRITICAL
None.

### HIGH
None.

### MEDIUM

**M1 (NEW): Prelude overhead — 31.7x source bytes for a trivial program**
- User code: 326 bytes, 71 tokens, 2 functions
- Prelude: 10,331 bytes, 1,516 tokens, 9 functions, 39 traits
- The prelude is lexed, parsed, type-checked, and canonicalized for every program
- Impact: latency floor for simple programs; particularly visible in REPL/watch mode
- Note: Prelude processing is likely cached by Salsa across compilations, so this is a cold-start cost

**M2 (NEW): No `nsw` flags on integer arithmetic in LLVM IR**
- `add`, `mul`, `sub` emit without `nsw` (no signed wrap)
- Ori has overflow-panics semantics — arithmetic *should* trap on overflow
- Without `nsw`, LLVM can't optimize based on no-overflow assumption
- Two options: (a) add `nsw` for optimization (requires matching runtime panic behavior), or (b) emit `llvm.sadd.with.overflow` intrinsics for checked arithmetic
- Current behavior: silent wrapping on overflow in AOT (behavioral mismatch with eval if eval checks)

**M3 (NEW): Unnecessary `br label` in LLVM IR codegen**
- `@_ori_main` emits `br label %bb1` after the call to `@_ori_add` before the arithmetic
- The target block `%bb1` immediately follows — this is a no-op branch
- Likely caused by the block-structured codegen emitting a new BB after every function call
- LLVM's optimizer eliminates this, but it adds noise to debug IR and slows verification

### LOW

**L1 (NEW): Canonicalizer 25% node expansion for let bindings**
- 16 source expressions → 20 canon nodes
- 4 extra nodes from let-binding pattern desugaring (each `let` creates a pattern + binding node)
- Expected behavior, not a bug — but worth tracking as complexity grows

**L2 (NEW): 4 decision trees generated for prelude (not needed at runtime)**
- The prelude's comparison helpers generate decision trees during canonicalization
- These are only needed if the prelude functions are actually called
- Lazy canonicalization could avoid this work

### CONFIRMED FROM PREVIOUS JOURNEYS
N/A (first journey)

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | 33 | 33 |
| Exit code | 33 | 33 |
| Overflow behavior | Unknown (not tested) | Silent wrapping (no `nsw`) |

**Potential mismatch**: Integer overflow semantics may differ — eval likely panics, AOT silently wraps. This needs a dedicated overflow test in a future journey.
