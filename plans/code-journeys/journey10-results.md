# Journey 10: "I am a list"

**Code**:
```ori
@count_items (xs: [int]) -> int = xs.length();

@check_length () -> int = {
    let a = [10, 20, 30];
    let b = [40, 50];
    let c = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    a.length() + b.length() + count_items(xs: c) - count_items(xs: b)
    // = 3 + 2 + 10 - 2 = 13
}

@check_iteration () -> int = {
    let xs = [1, 2, 3, 4, 5];
    let total = 0;
    for x in xs do total += x;
    total
    // = 1 + 2 + 3 + 4 + 5 = 15
}

@check_passing () -> int = count_items(xs: [100, 200, 300, 400, 500]);
// = 5

@main () -> int = {
    let a = check_length();     // = 13
    let b = check_iteration();  // = 15
    let c = check_passing();    // = 5
    a + b + c                   // = 33
}
```
**Source**: 949 bytes, **Expected Result**: 33 (= 13 + 15 + 5)
**Actual**: Eval = 33 (correct), AOT = 33 (correct)

**CRITICAL discovery**: List indexing (`xs[0]`) crashes AOT — `__index` function unresolved. Tested separately; does NOT affect this journey's code (which avoids indexing).

---

## Transformation Timeline

### Stages
```
Lexer:   949 bytes → 228 tokens (11 comments, 0 errors)
Parser:  228 tokens → 5 functions, 0 types, 70 expressions, 0 errors
TypeCk:  5 functions registered, signatures collected, bodies checked — no mono instances
Canon:   5 functions, 70 source_exprs → 78 canon_nodes, 5 roots, 6 constants, 0 decision_trees
Eval:    107 eval_can calls — highest so far, driven by list element iteration
```
- 11.4% canon expansion (70→78) — moderate, from list construction and for-loop desugaring
- 107 eval_can calls — list iteration (5 elements × next call + body) drives the count

### Stage 6b: LLVM Path

#### Generated LLVM IR (formatted)
```llvm
; --- List representation: { i64 length, i64 capacity, ptr data } ---

; --- count_items: load list parameter per-field, extract length ---
define fastcc i64 @_ori_count_items(ptr %0) personality ptr @rust_eh_personality {
bb0:
  %param.load.f0.ptr = getelementptr inbounds { i64, i64, ptr }, ptr %0, i32 0, i32 0
  %param.load.f0 = load i64, ptr %param.load.f0.ptr, align 4    ; ← CONFIRMED M5: align 4
  %param.load.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %param.load.f0, 0
  %param.load.f1.ptr = getelementptr inbounds { i64, i64, ptr }, ptr %0, i32 0, i32 1
  %param.load.f1 = load i64, ptr %param.load.f1.ptr, align 4    ; ← align 4 again
  %param.load.s1 = insertvalue { i64, i64, ptr } %param.load.s0, i64 %param.load.f1, 1
  %param.load.f2.ptr = getelementptr inbounds { i64, i64, ptr }, ptr %0, i32 0, i32 2
  %param.load.f2 = load ptr, ptr %param.load.f2.ptr, align 8    ; ptr load: align 8 correct
  %param.load.s2 = insertvalue { i64, i64, ptr } %param.load.s1, ptr %param.load.f2, 2
  %list.len = extractvalue { i64, i64, ptr } %param.load.s2, 0  ; .length() = field 0 extract
  br label %bb1                                                   ; ← CONFIRMED M3: dead branch
bb1:
  ret i64 %list.len
bb2:                                              ; No predecessors!  ← CONFIRMED M11: orphaned LP
  %lp = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp
}

; --- check_length: allocate 3 lists, extract lengths, call count_items ---
define fastcc i64 @_ori_check_length() personality ptr @rust_eh_personality {
bb0:
  ; alloca for passing lists by reference
  %ref_arg29 = alloca { i64, i64, ptr }, align 8
  %ref_arg = alloca { i64, i64, ptr }, align 8

  ; List a = [10, 20, 30] — 3 runtime alloc calls + per-element stores
  %list.data = call ptr @ori_list_alloc_data(i64 3, i64 8)
  ; ... store 10, 20, 30 to list.data[0..2], each align 4 (M5) ...
  %list.2 = insertvalue { i64, i64, ptr } { i64 3, i64 3, ptr undef }, ptr %list.data, 2

  ; List b = [40, 50] — same pattern
  %list.data3 = call ptr @ori_list_alloc_data(i64 2, i64 8)
  ; ... store 40, 50 ...
  %list.26 = insertvalue { i64, i64, ptr } { i64 2, i64 2, ptr undef }, ptr %list.data3, 2

  ; List c = [1..10] — 10-element list
  %list.data7 = call ptr @ori_list_alloc_data(i64 10, i64 8)
  ; ... store 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 ...
  %list.218 = insertvalue { i64, i64, ptr } { i64 10, i64 10, ptr undef }, ptr %list.data7, 2

  ; a.length()
  %list.len = extractvalue { i64, i64, ptr } %list.2, 0

  ; --- ARC lifecycle begins ---
bb1:
  ; Drop list a (length already extracted)
  %rc.data_ptr = extractvalue { i64, i64, ptr } %list.2, 2
  call void @ori_rc_dec(ptr %rc.data_ptr, ptr @"_ori_drop$202")
  ; Inc list b (will be used again later for count_items call)
  call void @ori_rc_inc(ptr ...)
  ; b.length()
  %list.len20 = extractvalue { i64, i64, ptr } %list.26, 0

bb3:
  ; Drop list b (one copy)
  call void @ori_rc_dec(...)
  %add = add i64 %list.len, %list.len20
  ; count_items(xs: c) via invoke
  store { i64, i64, ptr } %list.218, ptr %ref_arg, align 8
  %call = invoke fastcc i64 @_ori_count_items(ptr %ref_arg) to label %bb5 unwind label %bb6

bb5:
  ; Drop list c
  call void @ori_rc_dec(...)
  %add28 = add i64 %add, %call
  ; count_items(xs: b) via invoke
  store { i64, i64, ptr } %list.26, ptr %ref_arg29, align 8
  %call30 = invoke fastcc i64 @_ori_count_items(ptr %ref_arg29) to label %bb7 unwind label %bb8

bb7:
  ; Final drop of list b
  call void @ori_rc_dec(...)
  %sub = sub i64 %add28, %call30
  ret i64 %sub

  ; 3 orphaned landing pads (bb2, bb4, bb6/bb8)
}

; --- check_iteration: for..in list using runtime iterator ---
define fastcc i64 @_ori_check_iteration() #0 {  ; ← nounwind! correct (loop is pure)
bb0:
  %iter_next.scratch = alloca i64, align 8       ; scratch space for iterator output
  ; Build list [1, 2, 3, 4, 5]
  %list.data = call ptr @ori_list_alloc_data(i64 5, i64 8)
  ; ... store elements ...
  %list.2 = insertvalue { i64, i64, ptr } { i64 5, i64 5, ptr undef }, ptr %list.data, 2
  ; RC inc before creating iterator (iterator borrows data)
  call void @ori_rc_inc(ptr %rc.data_ptr)
  ; Create runtime iterator
  %list.iter = call ptr @ori_iter_from_list(ptr %list.data5, i64 %list.len, i64 8)

bb1:                                              ; Loop header
  %v11 = phi { i64, i64, ptr } [ %list.2, %bb0 ], [ %v11, %bb2 ]  ; list (for RC)
  %v12 = phi i64 [ 0, %bb0 ], [ %add, %bb2 ]                       ; total accumulator
  ; Call runtime: next element
  %iter_next.has = call i8 @ori_iter_next(ptr %list.iter, ptr %iter_next.scratch, i64 8)
  %iter_next.tag = zext i8 %iter_next.has to i64
  %iter_next.elem = load i64, ptr %iter_next.scratch, align 4  ; ← M5 again
  ; Build Option-like { tag, value } tuple
  %iter_next.0 = insertvalue { i64, i64 } undef, i64 %iter_next.tag, 0
  %iter_next.1 = insertvalue { i64, i64 } %iter_next.0, i64 %iter_next.elem, 1
  ; Check if iterator has more elements
  %proj.0 = extractvalue { i64, i64 } %iter_next.1, 0
  %ne = icmp ne i64 %proj.0, 0
  br i1 %ne, label %bb2, label %bb4

bb2:                                              ; Loop body
  %proj.1 = extractvalue { i64, i64 } %iter_next.1, 1  ; extract element value
  %add = add i64 %v12, %proj.1                          ; total += x
  br label %bb1                                          ; back to header

bb3:                                              ; Post-loop (via bb4)
  %v13 = phi i64 [ 0, %bb4 ]                    ; ← CONFIRMED L7: dead phi (unused)
  %v14 = phi { i64, i64, ptr } [ %v11, %bb4 ]   ; list for RC cleanup
  %v15 = phi i64 [ %v12, %bb4 ]                  ; total
  ; Drop list
  call void @ori_rc_dec(ptr %rc.data_ptr6, ptr @"_ori_drop$202.1")
  ret i64 %v15

bb4:                                              ; ← CONFIRMED M3: dead branch exit→bb3
  br label %bb3
}

; --- check_passing: inline list literal passed to function ---
define fastcc i64 @_ori_check_passing() personality ptr @rust_eh_personality {
  ; Build [100, 200, 300, 400, 500], store to alloca, invoke count_items
  ; Single landing pad (orphaned)
}

; --- main: invoke check_length + call check_iteration + invoke check_passing ---
define i64 @_ori_main() personality ptr @rust_eh_personality {
  ; invoke check_length (has personality → ARC cleanup)
  ; call check_iteration (nounwind → no need for invoke)
  ; invoke check_passing (has personality)
  ; add results, ret
}

; --- 7 runtime declarations ---
declare i32 @rust_eh_personality(i32) #0
declare ptr @ori_list_alloc_data(i64, i64)       ; NEW: allocate list data buffer
declare void @ori_rc_free(ptr, i64, i64) #0
declare void @ori_rc_dec(ptr, ptr) #2
declare void @ori_rc_inc(ptr) #2
declare ptr @ori_iter_from_list(ptr, i64, i64)    ; NEW: create list iterator
declare i8 @ori_iter_next(ptr, ptr, i64)          ; NEW: advance iterator

; --- 3 identical drop functions (should be deduplicated) ---
define void @"_ori_drop$202"(ptr %0) #1 { call @ori_rc_free(ptr, i64 24, i64 8); ret void }
define void @"_ori_drop$202.1"(ptr %0) #1 { ... identical ... }
define void @"_ori_drop$202.2"(ptr %0) #1 { ... identical ... }
```

#### Key Observations
1. **List representation: `{ i64, i64, ptr }`** — 3-field fat struct: length (i64), capacity (i64), data pointer (ptr to RC-managed buffer). 24 bytes total (per drop function's `ori_rc_free(ptr, i64 24, i64 8)`).
2. **`.length()` is zero-cost** — compiles to `extractvalue { i64, i64, ptr } %list, 0`. Identical pattern to string `.length()` from J9.
3. **List construction: per-element stores** — `ori_list_alloc_data(count, elem_size)` allocates buffer, then each element is stored via `getelementptr inbounds i64, ptr, i64 index` + `store`. No batch initialization.
4. **List literal constant propagation** — Lengths and capacities are embedded as constants: `{ i64 3, i64 3, ptr undef }`, `{ i64 10, i64 10, ptr undef }`. The `undef` is immediately overwritten by `insertvalue` with the alloc'd pointer.
5. **List passed by reference** — `alloca { i64, i64, ptr }` + `store` + pass ptr. Functions receive `ptr` and load fields via GEP. Matches the Indirect ABI for >16-byte structs.
6. **for..in compiles to runtime iterator** — `ori_iter_from_list(ptr data, i64 len, i64 elem_size)` creates an iterator, `ori_iter_next(ptr iter, ptr scratch, i64 elem_size)` returns `i8` (0=done, 1=has_next) and writes element to scratch buffer.
7. **Iterator protocol: Option-like { tag, value }** — The `i8` result is zero-extended to `i64`, paired with the loaded element value using `insertvalue`, then checked with `icmp ne`. This builds an Option representation at the IR level.
8. **3 identical drop functions generated** — `_ori_drop$202`, `_ori_drop$202.1`, `_ori_drop$202.2` all do `ori_rc_free(ptr, i64 24, i64 8)`. They should be deduplicated (NEW finding M12).
9. **ARC lifecycle correct with RC inc** — When list `b` is used after its first `.length()` call, the compiler correctly inserts `ori_rc_inc` to extend its lifetime before passing to `count_items`. After the last use, `ori_rc_dec` drops it.
10. **`check_iteration()` has `nounwind`** — Correct! The loop body is pure integer arithmetic. The list iterator runtime calls are marked `nounwind` (attribute group #0).
11. **Wait — `check_iteration()` calls `ori_iter_from_list` and `ori_iter_next` which are NOT marked nounwind** — Those declarations have no attributes! Yet the function IS marked `nounwind`. This may be an UNSOUND nounwind analysis — if the runtime iterator functions can panic (e.g., allocation failure), `check_iteration` should NOT be nounwind. (Potential H2)
12. **CONFIRMED M5**: `align 4` on i64 loads from list struct fields (should be `align 8`)
13. **CONFIRMED M3**: Dead branches after calls (at least 4 instances)
14. **CONFIRMED M11**: Orphaned landing pads in `count_items` (1), `check_length` (3), `check_passing` (1)
15. **CONFIRMED L7**: Dead phi `%v13 = phi i64 [ 0, %bb4 ]` in loop post-block (unused)
16. **Element store alignment**: `store i64 10, ptr %list.elem_ptr, align 4` — CONFIRMED M5 for list elements too, not just struct fields

---

## Issues Found

### CRITICAL
**C2 (NEW): List indexing (`xs[0]`) crashes AOT — `__index` function unresolved**
- `xs[0]` on `[int]` triggers "unresolved function `__index` in apply — missing mono instance?" warning
- Followed by "ArcIrEmitter: variable not yet defined" error
- Crashes with `ValueId 4294967295 out of bounds` panic
- **Root cause**: The `__index` builtin function for lists is not registered as a mono instance in the LLVM codegen path
- **Impact**: All list element access is broken in AOT. Only `.length()` and `for..in` iteration work.
- **Tested with**: `@main () -> int = { let xs = [10, 20, 30]; xs[0] }` — crashes immediately

### HIGH
**H2 (NEW): Potentially unsound `nounwind` on `check_iteration` — calls non-nounwind runtime functions**
- `_ori_check_iteration` is marked `nounwind` (attribute #0)
- But it calls `ori_iter_from_list` and `ori_iter_next` which have NO function attributes
- If either can panic (OOM, bounds violation), the nounwind guarantee is violated
- **Risk**: If a panic propagates through a nounwind boundary, LLVM assumes UB — may crash or corrupt state
- **Note**: This may be safe IF the runtime guarantees those functions never panic, but the lack of `nounwind` on the declarations is suspicious

### MEDIUM
**M12 (NEW): Duplicate identical drop functions — 3 copies of same code**
- `_ori_drop$202`, `_ori_drop$202.1`, `_ori_drop$202.2` are all identical: `ori_rc_free(ptr, i64 24, i64 8)`
- Each list literal gets its own drop function despite all being `[int]` (same element type, same layout)
- Impact: IR bloat, potential instruction cache pollution. Should emit one shared drop per unique layout.

**M13 (NEW): Unnecessary Option-like { tag, value } construction in iterator loop**
- The iterator result is: `zext i8 → i64`, `load i64 scratch`, then `insertvalue { i64, i64 } undef` × 2, then `extractvalue` to check tag, then `extractvalue` again to get value
- This builds and immediately destructures a 2-field aggregate on every iteration
- Could instead: check `i8` directly, load element only when needed (in bb2)

**CONFIRMED M3**: Dead branches after calls (4+ instances)
**CONFIRMED M5**: `align 4` on i64 loads — now confirmed for list struct fields AND element stores
**CONFIRMED M11**: Orphaned landing pads (5 instances across 3 functions)

### LOW
**CONFIRMED L7**: Dead phi values at loop exit (`%v13 = phi i64 [ 0, %bb4 ]` — unused)

### What Works Exceptionally Well
- **List fat pointer `{ i64, i64, ptr }`**: Clean 3-field representation with O(1) length access
- **Zero-cost `.length()`**: Same `extractvalue` pattern as strings
- **for..in compilation**: Runtime iterator with correct SSA phi loop
- **ARC lifecycle**: Correct RC inc/dec for multi-use lists across function calls
- **Pass-by-reference**: Lists correctly passed via alloca+store for >16-byte structs
- **`nounwind` propagation**: `check_iteration` correctly analyzed as nounwind (modulo H2)
- **Calling convention**: `invoke` for ARC functions, `call` for nounwind
- **List literal construction**: Constants correctly inlined (`{ i64 3, i64 3, ptr undef }`)

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | 33 | 33 |
| List indexing (`xs[0]`) | Works correctly | **CRASHES** (C2) |
| List length | Runtime method dispatch | Zero-cost extractvalue |
| List iteration | For-loop desugaring | Runtime iterator (ori_iter_from_list/next) |
| List construction | Allocate + fill | ori_list_alloc_data + per-element GEP/store |
| ARC lifecycle | GC-free (eval manages internally) | Explicit ARC (inc/dec/free) |
| Runtime declarations | 0 | 7 (ARC + list alloc + iterator) |
| Drop functions | N/A | 3 identical copies (should be 1) |
