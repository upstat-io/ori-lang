# Journey 4: "I am closures"

**Code**:
```ori
@apply (f: (int) -> int, x: int) -> int = f(x);
@main () -> int = {
  let offset = 5;
  let add_one = (x: int) -> int = x + 1;
  apply(add_one, 10) + offset
};
```
**Source**: 239 bytes, **Expected Result**: 16 (= (10+1) + 5)
**Actual**: Eval = 16 (correct), AOT = 16 (correct)

## Transformation Timeline

### Stage 4: Type Checker
```
User: registration=2 functions (apply + main; lambda is anonymous)
```

### Stage 6b: LLVM Path

#### Generated LLVM IR
```llvm
define fastcc i64 @_ori_apply({ ptr, ptr } %0, i64 %1) #1 {
bb0:
  %closure.fn_ptr = extractvalue { ptr, ptr } %0, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %0, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 %1)
  ret i64 %icall
}

define i64 @_ori_main() #1 {
bb0:
  %call = call fastcc i64 @_ori_apply({ ptr, ptr } { ptr @_ori_partial_0, ptr null }, i64 10)
  br label %bb1
bb1:
  %add = add i64 %call, 5
  ret i64 %add
bb2:
  unreachable
}

; Function Attrs: nounwind
define fastcc i64 @_ori___lambda_0(i64 %0) #1 {
bb0:
  %add = add i64 %0, 1
  ret i64 %add
}

define i64 @_ori_partial_0(ptr %0, i64 %1) {
entry:
  %result = call fastcc i64 @_ori___lambda_0(i64 %1)
  ret i64 %result
}
```

#### Key Observations
1. **Closure representation**: `{ ptr, ptr }` — (fn_ptr, env_ptr) fat pointer pair
2. **Trampoline pattern**: `_ori_partial_0` bridges calling conventions (ccc → fastcc)
3. **Non-capturing lambda**: env_ptr is `null` (no captures), but the full closure machinery is still used
4. **Constant folding**: `offset` (5) is constant-folded into the `add` instruction — the let binding is eliminated
5. **Nounwind on `_ori_main`**: Uses `call` (not `invoke`) for `_ori_apply` — correctly nounwind

---

## Issues Found

### HIGH
1. **[NEW] Nounwind analysis unsound for indirect calls through closures** — `_ori_apply` is marked `nounwind` (attribute #1), but it calls through a function pointer (`%closure.fn_ptr`). If the lambda body could panic (e.g., contains `panic()` or division by zero), the unwind would cross a `call` (not `invoke`) in a `nounwind` function — undefined behavior. For THIS lambda (`x + 1`) it's fine, but the analysis doesn't account for the possibility that the closure target could panic.
   - **Impact**: Any higher-order function receiving a panicking closure would silently produce UB instead of unwinding correctly
   - **Root cause**: `is_arc_function_nounwind()` only checks `ArcTerminator::Invoke` callees and `ArcInstr::Apply` for `ori_panic*`. Indirect calls through closure pointers are `Apply` instructions that don't match the `ori_panic` prefix check, so they're silently treated as nounwind.

### MEDIUM
2. **[NEW] Trampoline overhead for non-capturing lambdas** — `add_one` captures nothing, yet requires: (a) a `{fn_ptr, env_ptr}` closure pair with null env_ptr, (b) a trampoline function `_ori_partial_0` that just forwards the call. A non-capturing lambda could be passed as a bare function pointer, avoiding the closure allocation and trampoline indirection.

3. **[CONFIRMED] 98 eager runtime declarations** — Still all present.

### LOW
4. **[NEW] `_ori_partial_0` missing nounwind** — The trampoline has no `nounwind` attribute even though its body just calls a nounwind lambda. Cosmetic for this case but would affect optimization if the trampoline were called from another nounwind function.

---

## Eval vs LLVM Behavioral Mismatch
None — both produce 16.
