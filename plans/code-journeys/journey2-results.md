# Journey 2: "I am a branch"

**Code**:
```ori
@my_abs (n: int) -> int = if n < 0 then -n else n;
@my_max (a: int, b: int) -> int = if a > b then a else b;
@my_sign (n: int) -> int =
    if n > 0 then 1
    else (if n < 0 then -1 else 0);

@main () -> int = {
    let a = my_abs(-7);       // = 7
    let b = my_max(3, 10);    // = 10
    let c = my_sign(0);       // = 0
    a + b + c                 // = 17
}
```
**Source**: 552 bytes, **Expected Result**: 17 (= 7 + 10 + 0)
**Actual**: Eval = 17 (correct), AOT = 17 (correct)

---

## Transformation Timeline

### Stage 1-2: Lexer
```
User:    552 bytes → 132 tokens (7 comments, 0 errors)
Prelude: 10,331 bytes → 1,516 tokens (126 comments, 0 errors)
```
- User bytes/token: 4.2 — slightly denser than J1 (4.6)
- Prelude unchanged (confirmed: same 10,331 bytes every run)

### Stage 3: Parser
```
User:    132 tokens → 4 functions, 40 expressions, 0 errors
Prelude: 1,516 tokens → 9 functions, 39 traits, 46 expressions, 0 errors
```
- 10 expressions per function average
- 40 expressions for 4 functions — if/else and comparisons add expression volume

### Stage 4: Type Checker
```
Prelude: registration 9 functions, 0 tests, 0 impls → signatures → body checking
User:    registration 4 functions, 0 tests, 0 impls → signatures → body checking
```
- Clean pass, no monomorphization needed (all concrete int types)
- Comparison operators (`<`, `>`) correctly type-check without explicit trait dispatch

### Stage 5: Canonicalizer
```
User:    4 functions, 40 source_exprs → 43 canon_nodes, 4 roots, 6 constants, 0 decision_trees
Prelude: 9 functions, 46 source_exprs → 46 canon_nodes, 9 roots, 6 constants, 4 decision_trees
```
- 7.5% expansion (40→43), less than J1's 25% — if/else doesn't expand much
- 0 decision trees — if/else doesn't generate decision trees (only `match` does)

### Stage 6a: Eval Path
```
Total eval_can calls:  39
Binary operations:     6
Function calls:        3 (my_abs, my_max, my_sign)
```
Breakdown: my_abs → 1 comparison + 1 negation; my_max → 1 comparison; my_sign → 2 comparisons; main → 2 additions

### Stage 6b: LLVM Path

#### Generated LLVM IR (formatted)
```llvm
; Function Attrs: nounwind
define fastcc i64 @_ori_my_abs(i64 %0) #0 {
bb0:
  %lt = icmp slt i64 %0, 0
  br i1 %lt, label %bb1, label %bb2
bb1:
  %neg = sub i64 0, %0
  br label %bb3
bb2:
  br label %bb3
bb3:
  %v7 = phi i64 [ %neg, %bb1 ], [ %0, %bb2 ]
  ret i64 %v7
}

define fastcc i64 @_ori_my_max(i64 %0, i64 %1) #0 {
bb0:
  %gt = icmp sgt i64 %0, %1
  br i1 %gt, label %bb1, label %bb2
bb1:
  br label %bb3
bb2:
  br label %bb3
bb3:
  %v7 = phi i64 [ %0, %bb1 ], [ %1, %bb2 ]
  ret i64 %v7
}

define fastcc i64 @_ori_my_sign(i64 %0) #0 {
bb0:
  %gt = icmp sgt i64 %0, 0
  br i1 %gt, label %bb1, label %bb2
bb1:
  br label %bb3
bb2:
  %lt = icmp slt i64 %0, 0
  br i1 %lt, label %bb4, label %bb5
bb3:
  %v11 = phi i64 [ 1, %bb1 ], [ %v10, %bb6 ]
  ret i64 %v11
bb4:
  br label %bb6
bb5:
  br label %bb6
bb6:
  %v10 = phi i64 [ -1, %bb4 ], [ 0, %bb5 ]
  br label %bb3
}

define i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_my_abs(i64 -7)
  br label %bb1
bb1:
  %call1 = call fastcc i64 @_ori_my_max(i64 3, i64 10)
  br label %bb3
bb3:
  %call2 = call fastcc i64 @_ori_my_sign(i64 0)
  br label %bb5
bb5:
  %add = add i64 %call, %call1
  %add3 = add i64 %add, %call2
  ret i64 %add3
}
```

#### Key Observations
1. **Correct `icmp slt/sgt` for signed comparisons** — operators `<` and `>` compile to proper signed integer compare
2. **Correct `phi` nodes for if/else** — SSA construction is correct, merge points properly select values
3. **Nested if/else compiles correctly** — `my_sign` chains two branches with phi cascade (bb6→bb3)
4. **Unary negation via `sub i64 0, %0`** — standard LLVM pattern for negation
5. **CONFIRMED: Dead `br label` after every call in `_ori_main`** — 3 redundant branches (one after each call)
6. **Trivial if/else branches could use `select`** — `my_max`'s bb1/bb2 just branch to bb3. A `select` instruction would be more efficient pre-optimization, though LLVM's SimplifyCFG likely optimizes this.
7. **Zero runtime declarations** — still pure arithmetic, clean
8. **No overflow check on negation** — `sub i64 0, %0` for `-n` has no `nsw` flag; `my_abs(INT_MIN)` would silently wrap in AOT

---

## Issues Found

### CRITICAL
None.

### HIGH
None.

### MEDIUM
**CONFIRMED M2**: No `nsw` on negation (`sub i64 0, %0`) — `my_abs(INT_MIN)` would overflow silently in AOT
**CONFIRMED M3**: Dead `br label` after every function call — now 3 instances in `_ori_main` alone

### LOW
**L3 (NEW)**: Trivial if/else branches compile to branch+phi instead of `select` — LLVM optimizer handles this but pre-opt IR is noisy

### CONFIRMED FROM PREVIOUS JOURNEYS
- M1: Prelude overhead unchanged (10,331 bytes for every program)
- M2: No `nsw` flags on arithmetic
- M3: Dead branches after function calls (worse in J2 — 3 instances vs 1 in J1)

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | 17 | 17 |
| Exit code | 17 | 17 |
| if/else branching | Correct | Correct |
| Nested if/else | Correct | Correct |
