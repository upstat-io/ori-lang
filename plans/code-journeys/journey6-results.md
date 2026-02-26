# Journey 6: "I am pattern matching"

**Code**:
```ori
@classify (n: int) -> int =
  match n {
    0 -> 0,
    x if x > 0 -> 1,
    _ -> 2,
  };

@main () -> int = classify(0) + classify(5) + classify(-3);
```
**Source**: 271 bytes, **Expected Result**: 3 (= 0 + 1 + 2)
**Actual**: Eval = 3 (correct), AOT = 3 (correct)

## Transformation Timeline

### Stage 1-2: Lexer
```
271 bytes → 65 tokens (0 errors, 2 comments)
Prelude: 10331 bytes → 1516 tokens (126 comments)
```
Bytes/token ratio: 4.2 (user code). Pattern matching syntax (`match`, `{`, `->`, `if`, `_`) is concise.

### Stage 3: Parser
```
65 tokens → 2 functions, 20 expressions (0 errors)
Prelude: 1516 tokens → 9 functions, 39 traits, 46 expressions
```
Match expression with 3 arms and a guard compiles to 20 expressions — compact AST.

### Stage 4: Type Checker
```
registration: 2 functions, 0 tests, 0 impls
signatures: 2 functions
body checking: 2 functions
Prelude: 9 functions, 39 traits
```
No monomorphization needed — all types are concrete `int`.

### Stage 5: Canonicalizer
```
canon lower_module started (functions=2, source_exprs=20)
canon lower_module complete (canon_nodes=19, roots=2, constants=6, decision_trees=1)
Prelude: canon_nodes=46, roots=9, constants=6, decision_trees=4
```
**Key**: `decision_trees=1` — the `match` expression compiles to a single decision tree. The 6 constants are the literal values used in the match arms and main body (0, 1, 2, 5, -3, and 0 for comparison).

### Stage 6a: Eval Path
```
classify(0): Match(scrutinee=0) → literal 0 matches → arm 0 → result 0
classify(5): Match(scrutinee=5) → literal 0 fails → guard (5 > 0 = true) → arm 1 → result 1
classify(-3): Match(scrutinee=-3) → literal 0 fails → guard (-3 > 0 = false) → wildcard → arm 2 → result 2
main: 0 + 1 + 2 = 3
```
Total: ~30 eval_can calls, 3 function calls, 2 binary Gt comparisons, 2 binary Add operations.

### Stage 6b: LLVM Path

#### Generated LLVM IR
```llvm
; Function Attrs: nounwind
define fastcc i64 @_ori_classify(i64 %0) #1 {
bb0:
  switch i64 %0, label %bb3 [
    i64 0, label %bb2
  ]

bb1:                                              ; preds = %bb5, %bb4, %bb2
  %v2 = phi i64 [ 0, %bb2 ], [ 1, %bb4 ], [ 2, %bb5 ]
  ret i64 %v2

bb2:                                              ; preds = %bb0
  br label %bb1

bb3:                                              ; preds = %bb0
  %gt = icmp sgt i64 %0, 0
  br i1 %gt, label %bb4, label %bb5

bb4:                                              ; preds = %bb3
  br label %bb1

bb5:                                              ; preds = %bb3
  br label %bb1
}

; Function Attrs: nounwind
define i64 @_ori_main() #1 {
bb0:
  %call = call fastcc i64 @_ori_classify(i64 0)
  br label %bb1

bb1:
  %call1 = call fastcc i64 @_ori_classify(i64 5)
  br label %bb3

bb2:                                              ; No predecessors!
  unreachable

bb3:
  %add = add i64 %call, %call1
  %call2 = call fastcc i64 @_ori_classify(i64 -3)
  br label %bb5

bb4:                                              ; No predecessors!
  unreachable

bb5:
  %add3 = add i64 %add, %call2
  ret i64 %add3

bb6:                                              ; No predecessors!
  unreachable
}
```

#### Key Observations
1. **Textbook match compilation** — `switch` for literal matching + `icmp sgt`/`br` for the guard condition + `phi` node to merge all arm results. This is exactly how LLVM-targeting compilers typically lower pattern matching.
2. **Zero binding overhead** — The guard variable `x` is not a separate alloca or copy; the match scrutinee `%0` is used directly in the guard `icmp sgt i64 %0, 0`. The compiler recognizes that `x` is just an alias for `n`.
3. **Decision tree → switch+branch** — The single `DecisionTreeId(0)` from the canonicalizer maps cleanly to the switch/branch structure. The literal `0` case is a switch arm, the guarded case is a conditional branch in the default path, and the wildcard is the else branch.
4. **fastcc correctly propagated** — All 3 calls from `_ori_main` use `call fastcc` (verified: the Journey 2 calling convention fix is working).
5. **Nounwind analysis correct** — `_ori_classify` is correctly marked `nounwind` (pure integer arithmetic, no panics possible). `_ori_main` uses `call` (not `invoke`) for all callees.

---

## Issues Found

### MEDIUM
1. **[NEW] Redundant unconditional branches in match arms** — `bb2`, `bb4`, `bb5` each contain only `br label %bb1`. LLVM's SimplifyCFG pass will fold these at `-O1`+, but at `-O0` they remain as three unnecessary basic blocks. The codegen could emit the phi predecessors directly from the switch targets and guard branches without intermediate blocks.

### CONFIRMED FROM PREVIOUS JOURNEYS
2. **[CONFIRMED] 98 eager runtime declarations** — Still all present for a program that uses zero runtime functions.
3. **[CONFIRMED] Dead unreachable blocks in nounwind functions** — `_ori_main` has 3 dead blocks (bb2, bb4, bb6) from invoke→call downgrade. One per call site.

---

## Eval vs LLVM Behavioral Mismatch
None — both produce 3.
