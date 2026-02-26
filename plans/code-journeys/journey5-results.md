# Journey 5: "I am a struct"

**Code**:
```ori
type Point = { x: int, y: int };
@sum (p: Point) -> int = p.x + p.y;
@main () -> int = { let p = Point { x: 3, y: 4 }; sum(p) };
```
**Source**: 186 bytes, **Expected Result**: 7 (= 3 + 4)
**Actual**: Eval = 7 (correct), AOT = 7 (correct)

## Stage 6b: LLVM Path

```llvm
define fastcc i64 @_ori_sum(%ori.3 %0) #1 {
bb0:
  %proj.0 = extractvalue %ori.3 %0, 0
  %proj.1 = extractvalue %ori.3 %0, 1
  %add = add i64 %proj.0, %proj.1
  ret i64 %add
}

define i64 @_ori_main() #1 {
bb0:
  %call = call fastcc i64 @_ori_sum(%ori.3 { i64 3, i64 4 })
  br label %bb1
bb1:
  ret i64 %call
bb2:
  unreachable
}
```

#### Key Observations
1. **Value-type struct**: `%ori.3 = { i64, i64 }` — 16 bytes, passed by value (Direct ABI)
2. **Field access via extractvalue**: Clean, no GEP+load — perfect for value types
3. **Constant struct folding**: `{ i64 3, i64 4 }` passed as a constant literal — no stack allocation
4. **Nounwind correct**: `_ori_main` uses `call fastcc` for `_ori_sum`

---

## Issues Found

### LOW
1. **[NEW] Opaque struct type names** — `%ori.3` instead of `%Point`. LLVM type names are cosmetic (don't affect codegen), but readable names would help with IR debugging. Minor.

### CONFIRMED
2. **[CONFIRMED] 98 eager runtime declarations**
3. **[CONFIRMED] Dead unreachable blocks in nounwind functions** (bb2)

---

## Eval vs LLVM Behavioral Mismatch
None — both produce 7.
