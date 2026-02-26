# Journey 2: "I am function calls"

**Code**:
```ori
// Journey 2: Function calls, if/else, booleans
// Expected: double(5) + pick(true) = 10 + 1 = 11
@double (x: int) -> int = x * 2;

@pick (flag: bool) -> int =
  if flag then 1 else 0;

@main () -> int = double(5) + pick(true);
```
**Source**: 228 bytes, **Expected Result**: 11 (= 5*2 + 1)
**Actual**: Eval = 11 (correct), AOT = 80 (WRONG → **FIXED** to 11)

## Transformation Timeline

### Stage 1-2: Lexer
```
User: 228 bytes → 57 tokens (2 comments, 0 errors) — 4.0 bytes/token
Prelude: 10,331 bytes → 1,516 tokens (126 comments, 0 errors) — 6.8 bytes/token
```

### Stage 3: Parser
```
User: 57 tokens → 3 functions, 14 expressions, 0 errors
Prelude: 1,516 tokens → 9 functions, 39 traits, 46 expressions, 0 errors
```

### Stage 4: Type Checker
```
User: registration=3 functions; signatures=3; body checking=3
Prelude: registration=9 functions; signatures=9; body checking=9
```

### Stage 5: Canonicalizer
```
User: functions=3, source_exprs=14 → canon_nodes=14, roots=3, constants=6, decision_trees=0
Prelude: functions=9, source_exprs=46 → canon_nodes=46, roots=9, constants=6, decision_trees=4
```
14 source expressions → 14 canon nodes (perfect 1:1 — no expansion for simple functions).

### Stage 6a: Eval Path
```
15 total traced steps (eval_can + eval_call + binary ops)
- 2 function calls (double, pick)
- 1 conditional (if flag then 1 else 0)
- 2 binary ops (Mul in double, Add in main)
```

### Stage 6b: LLVM Path

#### Generated LLVM IR (after fix)
```llvm
define fastcc i64 @_ori_double(i64 %0) #1 {
bb0:
  %mul = mul i64 %0, 2
  ret i64 %mul
}

; Function Attrs: nounwind
define fastcc i64 @_ori_pick(i1 %0) #1 {
bb0:
  br i1 %0, label %bb1, label %bb2
bb1:
  br label %bb3
bb2:
  br label %bb3
bb3:
  %v4 = phi i64 [ 1, %bb1 ], [ 0, %bb2 ]
  ret i64 %v4
}

; Function Attrs: nounwind
define i64 @_ori_main() #1 {
bb0:
  %call = call fastcc i64 @_ori_double(i64 5)
  br label %bb1
bb1:
  %call1 = call fastcc i64 @_ori_pick(i1 true)
  br label %bb3
bb2:                        ; No predecessors!
  unreachable
bb3:
  %add = add i64 %call, %call1
  ret i64 %add
bb4:                        ; No predecessors!
  unreachable
}
```

#### Key Observations
1. **Nounwind call downgrade**: `_ori_main` uses `call` + `br` instead of `invoke` for nounwind callees — landing pads eliminated
2. **Dead blocks**: bb2 and bb4 are dead unwind blocks emitting `unreachable` (correct for nounwind)
3. **No constant folding**: Unlike Journey 1, the compiler doesn't constant-fold across function boundaries. `double(5)` is still a call, not `10`. This is expected — cross-function constant propagation requires interprocedural analysis.
4. **Missing nounwind on `_ori_double`**: `_ori_double` has `#1` (nounwind) but no `; Function Attrs: nounwind` comment before it (cosmetic only — attribute IS applied)
5. **`_ori_pick` phi node**: The if/else correctly lowers to a phi node with two branches — clean IR
6. **98 runtime declarations** — still all eagerly declared

---

## Issues Found

### CRITICAL
1. **[NEW → FIXED] Calling convention lost on nounwind call downgrade** — When `invoke` was downgraded to `call` for nounwind functions, the `fastcc` calling convention was lost. `builder.call()` did not explicitly set the calling convention, unlike `builder.invoke()` which had an explicit `set_call_convention()`. Result: `fastcc`-defined functions called with C convention → wrong return values.
   - **Root cause**: `IrBuilder::call()` relied on an incorrect assumption that inkwell's `build_call` auto-propagates the callee's calling convention
   - **Fix**: Added `call_val.set_call_convention(func.get_call_conventions())` to `call()` and `call_tail()` methods in `ir_builder/calls.rs`
   - **Impact**: All nounwind user function calls were broken. This was a regression from the nounwind analysis added in the previous session.

### MEDIUM
2. **[NEW] Dead unreachable blocks in nounwind functions** — `_ori_main` has 2 dead blocks (bb2, bb4) that just emit `unreachable`. These are the former unwind targets whose sole predecessors were downgraded from `invoke` to `call`. LLVM will DCE them, but they're unnecessary IR clutter. A more aggressive implementation could skip emitting these blocks entirely.

3. **[CONFIRMED] 98 eager runtime declarations** — Same as Journey 1. Zero runtime functions called.

### LOW
4. **[NEW] Missing `; Function Attrs` comment on some nounwind functions** — `_ori_double` gets `#1` (nounwind) but lacks the comment prefix. Cosmetic only.

---

## Eval vs LLVM Behavioral Mismatch
| Metric | Eval | AOT (before fix) | AOT (after fix) |
|--------|------|-------------------|-----------------|
| Exit code | 11 | 80 | 11 |
| Correct? | Yes | **NO** | Yes |

**Root cause**: fastcc calling convention mismatch — fixed by propagating callee CC to call instructions.
