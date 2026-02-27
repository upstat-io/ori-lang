# Journey 7: "I am a loop"

**Code**:
```ori
@sum_loop (n: int) -> int = {
    let i = 0; let total = 0;
    loop { if i >= n then break; total += i + 1; i += 1 };
    total
}
@sum_for (n: int) -> int = {
    let total = 0; for x in 1..=n do total += x; total
}
@main () -> int = { let a = sum_loop(n: 5); let b = sum_for(n: 5); a + b }
```
**Source**: 577 bytes, **Expected Result**: 30 (= 15 + 15)
**Actual**: Eval = 30 (correct), AOT = 30 (correct)

---

## Transformation Timeline

### Stages
```
Lexer:  577 bytes → 132 tokens (6 comments, 0 errors)
Parser: 132 tokens → 3 functions, 45 expressions, 0 errors
Canon:  3 functions, 45 source_exprs → 50 canon_nodes, 3 roots, 6 constants, 0 decision_trees
Eval:   128 eval_can calls (loops iterate 5 times each, ~13 nodes per iteration)
```
- 11.1% canon expansion (45→50) — loop desugaring adds nodes
- 128 eval_can calls — reasonable for 2 loops of 5 iterations each

### Stage 6b: LLVM Path

#### `sum_loop` — loop + break compiles to phi-based SSA loop
```llvm
define fastcc i64 @_ori_sum_loop(i64 %0) #0 {
bb0:
  br label %bb1
bb1:                                    ; loop header
  %v5 = phi i64 [ 0, %bb0 ], [ %add2, %bb5 ]  ; i
  %v6 = phi i64 [ 0, %bb0 ], [ %add1, %bb5 ]  ; total
  %ge = icmp sge i64 %v5, %0           ; i >= n
  br i1 %ge, label %bb3, label %bb4
bb3:                                    ; break path
  br label %bb2                         ; dead branch
bb2:                                    ; exit block
  %v26 = phi i64 [ 0, %bb3 ]           ; unused (break value = void)
  %v27 = phi i64 [ %v5, %bb3 ]         ; unused (carries i)
  %v28 = phi i64 [ %v6, %bb3 ]         ; total
  ret i64 %v28
bb4:                                    ; continue path
  br label %bb5                         ; dead branch
bb5:                                    ; loop body
  %add = add i64 %v5, 1                ; i + 1 (for total)
  %add1 = add i64 %v6, %add            ; total += i + 1
  %add2 = add i64 %v5, 1               ; i + 1 (duplicate!)
  br label %bb1                         ; back edge
}
```

#### `sum_for` — range creates a 4-field struct, then destructures
```llvm
define fastcc i64 @_ori_sum_for(i64 %0) #0 {
bb0:
  ; Range 1..=n materialized as { start, end, step, current }
  %ctor.1 = insertvalue { i64, i64, i64, i64 } { i64 1, i64 undef, ... }, i64 %0, 1
  %ctor.2 = insertvalue ... i64 1, 2    ; step = 1
  %ctor.3 = insertvalue ... i64 1, 3    ; current = 1
  %proj.0 = extractvalue ... 0          ; start = 1
  %proj.1 = extractvalue ... 1          ; end = n
  %proj.3 = extractvalue ... 3          ; step = 1 (unused? duplicates start)
  %add = add i64 %proj.1, %proj.3       ; end + step = n + 1 (inclusive→exclusive)
  br label %bb1
bb1:                                     ; loop header
  %v8 = phi i64 [ %proj.0, %bb0 ], [ %add2, %bb3 ]  ; current
  %v9 = phi i64 [ 0, %bb0 ], [ %v10, %bb3 ]          ; total
  %lt = icmp slt i64 %v8, %add                        ; current < n+1
  br i1 %lt, label %bb2, label %bb5
bb2:
  %add1 = add i64 %v9, %v8             ; total += current
  br label %bb3
bb3:
  %add2 = add i64 %v8, 1               ; current += 1
  br label %bb1                         ; back edge
bb5→bb4:                                ; exit (with dead indirection)
  ret i64 %v12                          ; return total
}
```

#### Key Observations
1. **Phi-based SSA loops** — Mutable variables (`i`, `total`) correctly compile to phi nodes at the loop header. This is the canonical way to represent mutation in SSA.
2. **Loop structure is correct** — back edge (bb5→bb1 / bb3→bb1), exit condition, break path all work.
3. **Inclusive range → exclusive**: `1..=n` computes `end + step` = `n + 1` as the exclusive bound. Correct but has overflow risk for `INT_MAX`.
4. **Range struct materialized then destructured**: 3 `insertvalue` + 3 `extractvalue` creates a `{1, n, 1, 1}` struct only to immediately pull out the fields. The intermediate aggregate is dead.
5. **Duplicate `i + 1` computation**: In `sum_loop`, `%add` and `%add2` both compute `i + 1`. LLVM CSE handles this, but codegen emits it twice.
6. **Dead phi values at loop exit**: `sum_loop`'s exit block has 3 phis, but only 1 (`%v28` = total) is used. The other 2 (`%v26`, `%v27`) are dead code.
7. **Dead branch indirections**: bb3→bb2, bb4→bb5, bb5→bb4 are single-target branches with no other predecessors.
8. **`nounwind` + `call`** — non-recursive loops correctly get `nounwind`. Good.
9. **Zero runtime declarations** — range iteration is pure
10. **CONFIRMED M3**: Dead branches in main after calls

---

## Issues Found

### CRITICAL
None.

### MEDIUM
**M9 (NEW): Inclusive range `..=` computes `end + step` — overflow for `INT_MAX`**
- `1..=INT_MAX` would compute `INT_MAX + 1` → overflow (wrapping to `INT_MIN`)
- Loop would iterate incorrectly (or infinitely)
- Eval path likely handles this differently

**CONFIRMED M2**: No `nsw` on loop arithmetic
**CONFIRMED M3**: Dead branches

### LOW
**L5 (NEW): Range struct materialized then immediately destructured**
- `{ 1, n, 1, 1 }` created via 3 insertvalue, then 3 extractvalue pulls fields back out
- The struct serves no purpose — fields should be directly used

**L6 (NEW): Duplicate computation in loops (`i + 1` computed twice)**
- LLVM CSE eliminates this but it's wasted codegen effort

**L7 (NEW): Dead phi values at loop exit (2 of 3 phis unused)**

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | 30 | 30 |
| Loop iteration | Works (128 eval_can) | Works (phi-based SSA) |
| For..in range | Works | Works (range struct → loop) |
| Mutable let + compound assignment | Works | Works (phi at loop header) |
