# Journey 4: "I am a struct"

**Code**:
```ori
type Point = { x: int, y: int }
type Rect = { origin: Point, width: int, height: int }

@area (r: Rect) -> int = r.width * r.height;

@main () -> int = {
    let p = Point { x: 3, y: 4 };
    let r = Rect { origin: p, width: 10, height: 5 };
    p.x + p.y + area(r)    // = 3 + 4 + 50 = 57
}
```
**Source**: 459 bytes, **Expected Result**: 57 (= 3 + 4 + 50)
**Actual**: Eval = 57 (correct), AOT = 57 (correct)

---

## Transformation Timeline

### Stage 1-2: Lexer
```
User:    459 bytes → 114 tokens (4 comments, 0 errors)
Prelude: 10,331 bytes → 1,516 tokens (unchanged)
```

### Stage 3: Parser
```
User:    114 tokens → 2 functions, 2 types, 24 expressions, 0 errors
```
- First time `types` count > 0 (Point and Rect)

### Stage 5: Canonicalizer
```
User:    2 functions, 24 source_exprs → 24 canon_nodes, 2 roots, 6 constants, 0 decision_trees
```
- **Perfect 1:1 mapping** — no expansion for struct code! Best ratio yet.

### Stage 6a: Eval Path
```
Total eval_can calls:  24
Binary operations:     3 (mul, add, add)
```
- Very efficient: 24 calls for 24 source expressions — 1:1

### Stage 6b: LLVM Path

#### Generated LLVM IR (formatted)
```llvm
%ori.Rect = type { %ori.Point, i64, i64 }
%ori.Point = type { i64, i64 }

define fastcc i64 @_ori_area(ptr %0) #0 {
bb0:
  ; Full per-field GEP+load+insertvalue for the entire Rect (including nested Point)
  %param.load.f0.ptr = getelementptr inbounds %ori.Rect, ptr %0, i32 0, i32 0
  %param.load.f0.f0.ptr = getelementptr inbounds %ori.Point, ptr %param.load.f0.ptr, i32 0, i32 0
  %param.load.f0.f0 = load i64, ptr %param.load.f0.f0.ptr, align 4
  %param.load.f0.s0 = insertvalue %ori.Point zeroinitializer, i64 %param.load.f0.f0, 0
  %param.load.f0.f1.ptr = getelementptr inbounds %ori.Point, ptr %param.load.f0.ptr, i32 0, i32 1
  %param.load.f0.f1 = load i64, ptr %param.load.f0.f1.ptr, align 4
  %param.load.f0.s1 = insertvalue %ori.Point %param.load.f0.s0, i64 %param.load.f0.f1, 1
  %param.load.s0 = insertvalue %ori.Rect zeroinitializer, %ori.Point %param.load.f0.s1, 0
  %param.load.f1.ptr = getelementptr inbounds %ori.Rect, ptr %0, i32 0, i32 1
  %param.load.f1 = load i64, ptr %param.load.f1.ptr, align 4
  %param.load.s1 = insertvalue %ori.Rect %param.load.s0, i64 %param.load.f1, 1
  %param.load.f2.ptr = getelementptr inbounds %ori.Rect, ptr %0, i32 0, i32 2
  %param.load.f2 = load i64, ptr %param.load.f2.ptr, align 4
  %param.load.s2 = insertvalue %ori.Rect %param.load.s1, i64 %param.load.f2, 2
  ; Then extract only the two fields actually needed
  %proj.1 = extractvalue %ori.Rect %param.load.s2, 1
  %proj.2 = extractvalue %ori.Rect %param.load.s2, 2
  %mul = mul i64 %proj.1, %proj.2
  ret i64 %mul
}

define i64 @_ori_main() #0 {
bb0:
  %ref_arg = alloca %ori.Rect, align 8
  store %ori.Rect { %ori.Point { i64 3, i64 4 }, i64 10, i64 5 }, ptr %ref_arg, align 4
  %call = call fastcc i64 @_ori_area(ptr %ref_arg)
  br label %bb1
bb1:
  %add = add i64 7, %call    ; <-- p.x + p.y constant-folded to 7!
  ret i64 %add
}
```

#### Key Observations
1. **Correct struct type layout**: `%ori.Rect = type { %ori.Point, i64, i64 }` with nested `%ori.Point = type { i64, i64 }` — proper LLVM named struct types
2. **Indirect parameter passing**: `@_ori_area` takes `ptr %0` — Rect is 32 bytes (4×i64), exceeds direct-passing threshold. This is the JIT FastISel workaround (documented in MEMORY.md).
3. **Full struct load for partial access**: `@_ori_area` only needs `width` and `height` but loads ALL fields (including nested Point's x and y). 17 instructions to load the whole struct, then 2 extractvalue instructions. Could be just 2 GEP+load instructions.
4. **`align 4` on i64 loads**: All `load i64` instructions use `align 4` instead of the natural `align 8`. i64 fields in these structs ARE 8-byte aligned, so this is a conservative understatement that may prevent LLVM from emitting aligned load instructions.
5. **Constant folding**: `p.x + p.y` is folded to literal `7` in the IR — the compiler propagated struct field values through to the addition. Excellent optimization.
6. **Constant struct construction**: `store %ori.Rect { %ori.Point { i64 3, i64 4 }, i64 10, i64 5 }` — the struct is constructed as a constant and stored in one instruction.
7. **Stack alloca for pass-by-ref**: `%ref_arg = alloca %ori.Rect, align 8` — correct stack allocation with proper 8-byte alignment (for the alloca itself, though loads use 4).
8. **`nounwind` + `call` (not invoke)** — no recursion, so correctly marked `nounwind`. Confirms H1 only affects recursive code.
9. **Zero runtime declarations** — struct operations need no runtime support
10. **CONFIRMED M3**: Dead `br label %bb1` after call

---

## Issues Found

### CRITICAL
None.

### HIGH
None. (H1 not triggered — non-recursive)

### MEDIUM

**M5 (NEW): `align 4` on i64 struct field loads — should be `align 8`**
- All `load i64, ptr %..., align 4` in `@_ori_area` use alignment 4
- These i64 fields are naturally 8-byte aligned in the struct layout
- `align 4` prevents LLVM from emitting efficient aligned loads on architectures that benefit (x86 SSE, ARM NEON)
- May also cause faults on strict-alignment architectures

**M6 (NEW): Full struct loading for partial field access**
- `@_ori_area` only uses `r.width` and `r.height` (fields 1 and 2)
- But the codegen loads ALL 4 fields (including nested Point's x and y)
- 17 GEP+load+insertvalue instructions vs 4 needed (2 GEP + 2 load)
- Root cause: the `load_indirect_param` pattern always loads the entire struct into an SSA aggregate, then uses extractvalue. It doesn't know which fields will be accessed.

**CONFIRMED M2**: No `nsw` on `mul i64`
**CONFIRMED M3**: Dead branch after call

### LOW
**CONFIRMED L1**: Canon expansion 0% for struct code (24→24) — best ratio
**CONFIRMED L3**: Dead `br label` pattern

### What Works Well (NEW)
- **Constant propagation through structs**: `p.x + p.y = 3 + 4 = 7` folded at compile time
- **Struct type layout**: Correct nested struct representation
- **Pass-by-reference ABI**: Correct for large structs (>16 bytes)

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | 57 | 57 |
| Struct construction | Works | Works (constant folded) |
| Nested field access | Works | Works (GEP chains) |
| Partial field access | Natural (only evaluates accessed fields) | Loads entire struct then extracts |
