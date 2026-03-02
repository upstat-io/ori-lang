# Journey 13: COW List Operations

**Code**:
```ori
@main () -> int = {
    let items = [1, 2, 3];
    let items = items.push(4);
    let items = items.push(5);
    let $count = items.length();

    let total = 0;
    for x in items do {
        total += x;
    };

    total + count
}
```
**Source**: 481 bytes, **Expected Result**: 20 (= 1+2+3+4+5 + 5)
**Actual**: Eval = 20 (correct), **AOT = 20 (correct)**

Both execution paths produce the correct result. This is the first journey testing COW (copy-on-write) semantics for list mutation through `push`.

---

## Transformation Timeline

### Stage 1-2: Lexer
```
481 bytes -> 90 tokens (0 errors)
Prelude: 10,331 bytes -> 1516 tokens (0 errors)
```
- 5.3 bytes/token ratio (higher than usual due to lengthy comments)
- CONFIRMED M1: Prelude still 10,331 bytes

### Stage 3: Parser
```
90 tokens -> 1 function, 0 types, 25 expressions, 0 errors
Prelude: 1516 tokens -> 9 functions, 39 traits, 46 expressions
```
- Single `@main` function
- 25 expressions: list literal, 2x method call (push), 1x method call (length), for loop, binary ops
- `.push(4)` and `.push(5)` parsed as method calls
- `.length()` parsed as method call

### Stage 4: Type Checker
```
registration: 9 functions (prelude), 1 function (user), 0 tests, 0 impls
signatures: collected for all functions
body checking: complete (prelude + user)
```
- Hash-first miss on generic prelude functions (len, is_empty, is_some, is_none, is_ok, is_err) -- AST fallback
- Hash-first hit on non-generic prelude functions (compare, min, max)
- `.push()` resolved as list method, `.length()` resolved as list method
- Type: `[int]` -- list of int

### Stage 5: Canonicalizer
```
canon lower_module started (functions=1, source_exprs=25)
canon lower_module complete (canon_nodes=30, roots=1, constants=6, decision_trees=0)
```
- 20% canon expansion (25 -> 30) -- for loop desugaring and compound assignment expansion
- 6 constants: 0, 1, 2, 3, 4, 5
- 0 decision trees (no match expressions)

### Stage 6a: Eval Path
```
64 eval_can calls (from trace)
```
Trace shows clear execution flow:
1. `CanId(3)` -- `List([1, 2, 3])` creation
2. `CanId(7)` -- `MethodCall(items, push, [4])` -> list with [1,2,3,4]
3. `CanId(11)` -- `MethodCall(items, push, [5])` -> list with [1,2,3,4,5]
4. `CanId(14)` -- `MethodCall(items, length, [])` -> 5
5. `CanId(25)` -- `For(x, items, body)` -- iterates 5 times
6. 5x loop iterations: `Add(total, x)` for x = 1,2,3,4,5 -> total becomes 15
7. `CanId(28)` -- `Binary(Add, total, count)` -> 15 + 5 = 20

- Each loop iteration: Block -> Assign -> Binary(Add) -> Ident(total) -> Ident(x)
- Final: `evaluate_binary op=Add left_type="int" right_type="int"` for total(15) + count(5)

### Stage 6b: LLVM Path

#### ARC Trace
```
nounwind analysis: 1 function, 1 pass, 0 nounwind (main NOT marked nounwind)
Type registration: Ordering, PanicInfo, TraceEntry, FormatType, Alignment, Sign enums/structs
Function declaration: _ori_main, C calling convention, Direct return passing
```
- `_ori_main` not marked nounwind (correct -- calls runtime functions that may throw)
- C calling convention (not fastcc -- entry point)

#### Type Representation
```llvm
; List<int> = { len: i64, cap: i64, data: ptr }
; Represented as: { i64, i64, ptr }
; Layout: 24 bytes (3 x 8-byte fields)
```

#### Generated LLVM IR (formatted, key sections)

```llvm
define i64 @_ori_main() personality ptr @rust_eh_personality {
bb0:
  ; --- Entry allocas ---
  %iter_next.scratch = alloca i64, align 8
  %push.out8 = alloca { i64, i64, ptr }, align 8
  %push.elem7 = alloca i64, align 8
  %push.out = alloca { i64, i64, ptr }, align 8
  %push.elem = alloca i64, align 8

  ; --- List creation: [1, 2, 3] ---
  %list.data = call ptr @ori_list_alloc_data(i64 3, i64 8)
  store i64 1, ptr %list.elem_ptr, align 4        ; <-- M5: align 4, should be align 8
  store i64 2, ptr %list.elem_ptr1, align 4       ; <-- M5
  store i64 3, ptr %list.elem_ptr2, align 4       ; <-- M5
  %list.2 = insertvalue { i64, i64, ptr } { i64 3, i64 3, ptr undef }, ptr %list.data, 2

  ; --- COW push(4) ---
  store i64 4, ptr %push.elem, align 4            ; <-- M5
  call void @ori_list_push_cow(ptr noalias %list.data3, i64 %list.len, i64 %list.cap,
    ptr %push.elem, i64 8, i64 8, ptr null, i32 1, ptr %push.out)
  ; alloca+store+load roundtrip to extract result struct   <-- M7
  %push.val.s2 = ... insertvalue chain ...
  br label %bb1                                    ; <-- M3: unnecessary branch

bb1:
  ; --- COW push(5) ---
  store i64 5, ptr %push.elem7, align 4           ; <-- M5
  call void @ori_list_push_cow(ptr noalias %list.data4, i64 %list.len5, i64 %list.cap6,
    ptr %push.elem7, i64 8, i64 8, ptr null, i32 1, ptr %push.out8)
  %push.val.s217 = ... insertvalue chain ...
  br label %bb3                                    ; <-- M3

bb3:
  ; --- RC inc for iteration copy ---
  %rc_inc.data = extractvalue { i64, i64, ptr } %push.val.s217, 2
  %rc_inc.cap = extractvalue { i64, i64, ptr } %push.val.s217, 1
  call void @ori_list_rc_inc(ptr %rc_inc.data, i64 %rc_inc.cap)
  %list.len19 = extractvalue { i64, i64, ptr } %push.val.s217, 0
  br label %bb5                                    ; <-- M3

bb5:
  ; --- RC dec (count binding consumes a ref) ---
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr21, ...)
  ; --- RC inc (for..in needs a ref) ---
  call void @ori_list_rc_inc(ptr %rc_inc.data24, ...)

  ; --- Iterator creation ---
  %list.iter = call ptr @ori_iter_from_list(ptr %list.data26, i64 %list.len27,
    i64 %list.cap28, i64 8, ptr null)
  br label %bb7                                    ; <-- M3

bb7:                                    ; loop header
  %v20 = phi i64 [ 0, %bb5 ], [ %add32, %bb8 ]
  %v21 = phi { i64, i64, ptr } [ %push.val.s217, %bb5 ], [ %v21, %bb8 ]
  %iter_next.has = call i8 @ori_iter_next(ptr %list.iter, ptr %iter_next.scratch, i64 8)
  %iter_next.tag = zext i8 %iter_next.has to i64
  %iter_next.elem = load i64, ptr %iter_next.scratch, align 4  ; <-- M5
  ; Option-like construction
  %iter_next.1 = insertvalue { i64, i64 } ... tag ... elem ...
  %proj.0 = extractvalue { i64, i64 } %iter_next.1, 0
  %ne = icmp ne i64 %proj.0, 0
  br i1 %ne, label %bb8, label %bb10

bb8:                                    ; loop body
  %proj.1 = extractvalue { i64, i64 } %iter_next.1, 1
  %add32 = add i64 %v20, %proj.1
  br label %bb7

bb10:                                   ; loop exit -> bb9
  br label %bb9                                    ; <-- L4: single-pred branch

bb9:
  ; RC dec for final list
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr29, ...)
  ; Iterator cleanup
  call void @ori_iter_drop(ptr %list.iter)
  ; Final computation
  %add = add i64 %v23, %list.len19
  ret i64 %add
}
```

#### Runtime Function Declarations

```llvm
declare ptr @ori_list_alloc_data(i64, i64)           ; no attrs
declare void @ori_list_push_cow(ptr, i64, i64,        ; noalias on data param only
    ptr, i64, i64, ptr, i32, ptr noalias)
declare void @ori_list_rc_inc(ptr, i64)                ; nounwind memory(inaccessiblemem: readwrite)
declare void @ori_buffer_rc_dec(ptr, i64, i64, i64, ptr) ; nounwind memory(inaccessiblemem: readwrite)
declare ptr @ori_iter_from_list(ptr, i64, i64, i64, ptr) ; no attrs
declare i8 @ori_iter_next(ptr, ptr, i64)              ; no attrs
declare void @ori_iter_drop(ptr)                       ; no attrs
```

---

## LLVM Deep Scrutiny

### 1. Instruction Purity

**Actual IR instruction count** (bb0-bb10, excluding allocas and declarations): ~80 instructions

**Optimal IR** for this program:
```llvm
define i64 @_ori_main() {
  %data = call ptr @ori_list_alloc_data(i64 3, i64 8)
  store i64 1, ptr %data, align 8
  %p1 = getelementptr i64, ptr %data, i64 1
  store i64 2, ptr %p1, align 8
  %p2 = getelementptr i64, ptr %data, i64 2
  store i64 3, ptr %p2, align 8
  ; push(4) -- inline if unique
  call void @ori_list_push_cow(ptr %data, i64 3, i64 3, ...)
  ; push(5) -- inline from cow result
  call void @ori_list_push_cow(...)
  ; length = extractvalue .0 -> 5
  ; iter
  %iter = call ptr @ori_iter_from_list(...)
  br label %loop
loop:
  %total = phi i64 [0, %entry], [%next_total, %body]
  %has = call i8 @ori_iter_next(...)
  %done = icmp eq i8 %has, 0
  br i1 %done, label %exit, label %body
body:
  %x = load i64, ptr %scratch, align 8
  %next_total = add i64 %total, %x
  br label %loop
exit:
  call void @ori_iter_drop(ptr %iter)
  %result = add i64 %total, 5       ; length is known constant
  ret i64 %result
}
```
Optimal: ~22 instructions. Actual: ~80 instructions. **Ratio: ~3.6x** (HIGH).

Key sources of overhead:
- alloca+store+load roundtrip for push output structs (M7): ~20 instructions for 2 pushes
- insertvalue chains to reconstruct SSA structs from alloca loads: ~12 instructions
- RC inc/dec pairs: 4 runtime calls (2x inc, 2x dec) -- see ARC purity below
- Option-like {tag, elem} construction in iterator loop: ~6 instructions per iteration (M13)
- Dead branches (M3): 4 unnecessary `br label` instructions
- Phi for list value through loop body when never modified in loop (L7/M15)

**Severity: HIGH** -- instruction overhead ratio exceeds 2.0

### 2. ARC Purity

RC operations in generated IR:
1. `ori_list_rc_inc` (bb3) -- inc before `.length()` extraction
2. `ori_buffer_rc_dec` (bb5) -- dec after `.length()` consumes list ref
3. `ori_list_rc_inc` (bb5) -- inc before iterator creation
4. `ori_buffer_rc_dec` (bb9) -- dec after loop completes

**Analysis**:
- 2 inc + 2 dec = balanced. No leak, no double-free.
- However, the `inc` at bb3 followed by `dec` at bb5 (for the `$count` binding) is **unnecessary**. The list is used again immediately for the for loop. The pipeline does not recognize that `$count = items.length()` only extracts a scalar from the list and does not create a new reference. Instead it treats `.length()` as consuming the list, requiring an inc before and dec after.
- The second `inc` at bb5 (for the iterator) is correct -- the iterator needs its own reference to the list data.
- The final `dec` at bb9 is correct -- releases the list after the loop.
- **Net: 1 unnecessary inc/dec pair** around the length extraction.

The `ori_list_push_cow` calls receive `ptr null` for `inc_fn` (element RC increment function) because `int` elements are scalars -- correct, no RC needed for elements.

The `cow_mode` parameter is `i32 1` -- indicating "always COW" mode. The uniqueness check happens inside the runtime function, not in the generated IR.

**Missing**: There is no explicit `ori_rc_is_unique` call in the generated IR. The uniqueness check is encapsulated within `ori_list_push_cow`. This is architecturally correct for the COW-push approach (runtime handles it), but means the compiler cannot optimize the fast path at compile time when uniqueness is statically provable.

**Severity: MEDIUM** -- 1 unnecessary RC pair, but balanced overall

### 3. Attribute Audit

| Function | Expected Attrs | Actual Attrs | Status |
|----------|---------------|--------------|--------|
| `_ori_main` | personality (has EH) | personality ptr @rust_eh_personality | OK |
| `_ori_main` | NOT nounwind | (none) | OK -- calls may-throw functions |
| `ori_list_alloc_data` | nounwind, noalias ret | (none) | **MISSING** |
| `ori_list_push_cow` | (complex side effects) | noalias on out_ptr only | Partial |
| `ori_list_rc_inc` | nounwind mem(inaccessible:rw) | nounwind memory(inaccessiblemem: readwrite) | OK |
| `ori_buffer_rc_dec` | nounwind mem(inaccessible:rw) | nounwind memory(inaccessiblemem: readwrite) | OK |
| `ori_iter_from_list` | nounwind, noalias ret | (none) | **MISSING** |
| `ori_iter_next` | nounwind | (none) | **MISSING** |
| `ori_iter_drop` | nounwind | (none) | **MISSING** |

**Missing attributes on allocation/iterator functions** (H3):
- `ori_list_alloc_data`: Should have `nounwind` (allocation failure = abort, not throw) and `noalias` return (fresh allocation)
- `ori_iter_from_list`: Should have `nounwind` and `noalias` return
- `ori_iter_next`: Should have `nounwind` (never throws)
- `ori_iter_drop`: Should have `nounwind` (never throws)

Because `_ori_main` calls functions without `nounwind`, the entire function requires `personality` and landing pads even though no catch/cleanup logic exists. Adding `nounwind` to these runtime declarations would allow LLVM to eliminate the landing pad overhead.

**Source file**: `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

**Severity: HIGH** -- missing nounwind on iterator/alloc functions forces unnecessary EH overhead (H2 scope expansion)

### 4. Constant Folding Opportunities

1. **List length after pushes**: The program creates `[1,2,3]` (len=3), pushes 4 (len=4), pushes 5 (len=5). The length is always 5 at the `.length()` call. The compiler does NOT fold this -- it extracts length at runtime via `extractvalue { i64, i64, ptr } %push.val.s217, 0`. This is reasonable since `push_cow` is a runtime function and the compiler cannot see through it.

2. **Loop sum**: `1+2+3+4+5 = 15` could theoretically be constant-folded if the compiler could see through the iterator. Not feasible with runtime-opaque iterators.

3. **Final result**: `total + count` where both are runtime values. Cannot be folded.

No missed constant folding opportunities that are practically achievable with the current architecture.

**Severity: None**

### 5. Alignment Audit

| Location | Type | Actual Align | Correct Align | Status |
|----------|------|-------------|---------------|--------|
| List element stores (bb0) | i64 | align 4 | align 8 | **WRONG** (M5) |
| Push elem stores (bb0, bb1) | i64 | align 4 | align 8 | **WRONG** (M5) |
| Push output loads (bb0, bb1) | i64 | align 4 | align 8 | **WRONG** (M5) |
| Iterator scratch load (bb7) | i64 | align 4 | align 8 | **WRONG** (M5) |
| Entry allocas | i64 / struct | align 8 | align 8 | OK |

**CONFIRMED M5**: `align 4` on i64 loads/stores throughout. This affects list element stores, push element stores, push output field loads, and iterator scratch loads. All should be `align 8` for i64.

**Source file**: `compiler/ori_llvm/src/codegen/ir_builder/` (alignment computation)

**Severity: MEDIUM** (CONFIRMED from J4, J6, J7, J9, J10, J12 -- persistent across 9 journeys)

### 6. Control Flow Analysis

| Block | Predecessors | Purpose | Status |
|-------|-------------|---------|--------|
| bb0 | (entry) | List creation + first push | OK |
| bb1 | bb0 | Second push | **M3**: unnecessary branch from bb0 |
| bb2 | (none) | Landing pad | **M11**: orphaned |
| bb3 | bb1 | RC inc for length | **M3**: unnecessary branch from bb1 |
| bb4 | (none) | Landing pad | **M11**: orphaned |
| bb5 | bb3 | RC dec + RC inc + iterator creation | **M3**: unnecessary branch from bb3 |
| bb6 | (none) | Landing pad with cleanup | **M11**: orphaned (but has real cleanup code) |
| bb7 | bb5, bb8 | Loop header (iterator next) | OK -- phi merge |
| bb8 | bb7 | Loop body (add) | OK |
| bb9 | bb10 | Loop exit (cleanup + return) | **L4**: single-pred phi |
| bb10 | bb7 | Loop exit trampoline | **L4**: unnecessary intermediate block |

**Orphaned landing pads** (CONFIRMED M11): bb2, bb4 have `No predecessors!` -- dead code from EH infrastructure that LLVM cannot reach. bb6 has actual cleanup code (RC dec) but also has no predecessors.

**Dead branches** (CONFIRMED M3): 4 instances of `br label %bbN` where the target is the immediately next block and the branch is the only path.

**Single-predecessor redundancy** (CONFIRMED L4): bb10 branches unconditionally to bb9; bb9 has phi nodes with single incoming edge from bb10.

**Severity: MEDIUM (M3, M11), LOW (L4)**

### 7. Binary Analysis

```
Binary size: 6,723,440 bytes (6.4 MB)
.text section: 948,490 bytes (922 KB)
_ori_main function: 252 bytes (0x1eb00 to 0x1edfb)
```

- Function size: 252 bytes for ~80 IR instructions = ~3.1 bytes/IR instruction
- Stack frame: `sub $0x128, %rsp` = 296 bytes -- quite large for a simple program
- The 296-byte stack is due to: 2x push output structs (24 bytes each), 2x push elem allocas (8 each), 1x iterator scratch (8), plus spill slots for loop variables carried through the phi nodes

**Runtime symbols linked** (COW-specific):
- `ori_list_alloc_data` (0x20e50): 196 bytes
- `ori_list_push_cow` (0x1f520): 2897 bytes -- substantial, handles uniqueness check + realloc + copy
- `ori_list_rc_inc` (0x28fe0): 145 bytes
- `ori_buffer_rc_dec` (0x28b50): 1162 bytes
- `ori_iter_from_list` (0x2e630): 281 bytes
- `ori_iter_next` (0x2d0a0): 264 bytes
- `ori_iter_drop` (0x2d020): 121 bytes

Total runtime code pulled in for this program: ~5066 bytes of runtime support.

`ori_list_push_cow` is the largest at 2897 bytes -- it contains the full COW logic: uniqueness check, capacity check, reallocation path, element copy path, and in-place append path.

### 8. COW-Specific Analysis

**COW push call signature**:
```llvm
call void @ori_list_push_cow(
    ptr noalias %data,     ; list data pointer
    i64 %len,              ; current length
    i64 %cap,              ; current capacity
    ptr %push.elem,        ; pointer to element to push
    i64 8,                 ; element size
    i64 8,                 ; element alignment
    ptr null,              ; element RC inc function (null = scalar)
    i32 1,                 ; cow_mode (1 = COW enabled)
    ptr %push.out          ; output struct pointer
)
```

Key observations:
1. **`ptr null` for inc_fn**: Correct -- `int` elements are scalars, no RC needed.
2. **`i32 1` for cow_mode**: COW mode enabled. The runtime checks `ori_rc_is_unique` internally.
3. **`ptr noalias` on data param**: Correct -- the data pointer passed in should not alias the output.
4. **No `noalias` on output ptr in declaration**: The declaration has `ptr noalias` on the 9th parameter (out_ptr). The call site also marks data as `noalias`. Good.
5. **Element passed by pointer**: `store i64 4, ptr %push.elem` then pass `%push.elem`. This is correct for the generic interface (works for any element size) but adds an unnecessary store+pointer indirection for scalar elements.

**Uniqueness check location**: Inside `ori_list_push_cow` runtime function (not in generated IR). This means:
- The compiler CANNOT optimize away the COW check even when it can statically prove uniqueness
- In this program, both pushes operate on unique lists (let-rebinding creates fresh owners), so the COW check will always take the fast path at runtime
- A future optimization could inline the uniqueness check or emit a "push_unique" fast path when the ARC analysis proves uniqueness

**Consecutive push optimization missed**: The two pushes happen sequentially on the same (rebound) list. Ideally, after the first push proves/ensures capacity, the second push could skip reallocation. But since each push goes through the full runtime function, this optimization is not available.

### 9. For-In List Iteration Analysis

The for..in loop generates a runtime iterator pattern:

```llvm
; Create iterator (RC-inc'd copy of list data)
%list.iter = call ptr @ori_iter_from_list(ptr %data, i64 %len, i64 %cap, i64 8, ptr null)

; Loop header
bb7:
  %v20 = phi i64 [ 0, %bb5 ], [ %add32, %bb8 ]         ; total accumulator
  %v21 = phi { i64, i64, ptr } [ %push.val.s217, %bb5 ], [ %v21, %bb8 ]  ; <-- M15: unused list phi
  %has = call i8 @ori_iter_next(ptr %list.iter, ptr %scratch, i64 8)
  %tag = zext i8 %has to i64
  %elem = load i64, ptr %scratch, align 4
  ; Option-like construction
  %opt = insertvalue { i64, i64 } undef, i64 %tag, 0    ; <-- M13: unnecessary
  %opt2 = insertvalue { i64, i64 } %opt, i64 %elem, 1   ; <-- M13: unnecessary
  %proj = extractvalue { i64, i64 } %opt2, 0             ; <-- M13: extracts what was just inserted
  %ne = icmp ne i64 %proj, 0
  br i1 %ne, label %bb8, label %bb10

; Body
bb8:
  %x = extractvalue { i64, i64 } %opt2, 1               ; <-- M13: extracts what was just inserted
  %add = add i64 %v20, %x
  br label %bb7
```

**M13 CONFIRMED**: The iterator loop constructs an Option-like `{ tag, elem }` struct via insertvalue, then immediately destructures it via extractvalue. The tag (`%has`) and element (`%elem`) are already available as SSA values -- the struct round-trip adds ~6 instructions per loop iteration (insertvalue x2, extractvalue x2, plus the struct type overhead). Should use the tag/elem directly.

**M15 (NEW): Unused list struct phi in loop header**: `%v21 = phi { i64, i64, ptr } [ %push.val.s217, %bb5 ], [ %v21, %bb8 ]` carries the 24-byte list struct through every loop iteration but it is only used AFTER the loop exits (in bb9 for the final RC dec). The phi keeps the struct "alive" through the loop but adds register pressure for 3 values (len, cap, ptr) that are not touched in the loop body. Ideally, these would be extracted before the loop and only the data ptr / len / cap would be carried as individual i64/ptr values, or the list struct would be stored once before the loop and reloaded after.

**Iterator drop**: `ori_iter_drop` is correctly called at bb9 after the loop exits.

---

## Issues Found

### HIGH

**H3 (NEW): Missing nounwind/noalias on allocation and iterator runtime functions**
- `ori_list_alloc_data`: Missing `nounwind`, missing `noalias` return attribute
- `ori_iter_from_list`: Missing `nounwind`, missing `noalias` return attribute
- `ori_iter_next`: Missing `nounwind`
- `ori_iter_drop`: Missing `nounwind`
- `ori_list_push_cow`: Missing `nounwind` (push cannot throw -- panics on OOM abort, not throw)
- **Impact**: Forces `_ori_main` to use `personality` and emit landing pads for all calls. With `nounwind` on these, the entire function could be `nounwind` and EH overhead would vanish.
- **Source**: `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs` -- `attrs: &[]` for all these functions
- **Scope expansion of H2**: This is the same class as H2 (nounwind unsoundness) but in the opposite direction -- these functions genuinely never throw, and marking them `nounwind` would be SOUND and beneficial.
- **Cross-ref**: H2 from J10

**Instruction overhead ratio 3.6x** (80 actual vs 22 optimal)
- Primary contributors: alloca+store+load roundtrips (M7), Option-like struct construction in loop (M13), unnecessary RC pair (see ARC purity), dead branches (M3)
- This is the highest overhead ratio seen in any journey

### MEDIUM

**M15 (NEW): Unused list struct carried through loop phi**
- `%v21 = phi { i64, i64, ptr } [ %push.val.s217, %bb5 ], [ %v21, %bb8 ]` is a 24-byte struct carried through every loop iteration but only used after loop exit for cleanup
- Wastes 3 registers (or spills to stack) in the hot loop
- Should be extracted before loop, stored to stack slot, and reloaded after
- **Source**: `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` (loop phi generation)

**M16 (NEW): Unnecessary RC inc/dec pair around length extraction**
- `ori_list_rc_inc` in bb3 + `ori_buffer_rc_dec` in bb5 bracket the `.length()` extraction
- `.length()` is a pure field extraction (`extractvalue ...  0`) -- no reference is created or consumed
- The compiler's borrow analysis treats `.length()` as consuming the list, requiring protect+release
- Should recognize scalar field extraction as non-consuming
- **Source**: `compiler/ori_arc/src/borrow/mod.rs` or `rc_insert/mod.rs`

**CONFIRMED M3**: Dead `br label` after calls -- 4 instances (bb0->bb1, bb1->bb3, bb3->bb5, bb5->bb7)
**CONFIRMED M5**: `align 4` on i64 stores/loads -- 8+ instances (list elements, push elems, push outputs, iterator scratch)
**CONFIRMED M7**: alloca+store+load roundtrip for push output structs -- 2 instances (one per push)
**CONFIRMED M11**: Orphaned landing pads -- 3 instances (bb2, bb4, bb6)
**CONFIRMED M13**: Option-like struct construction in iterator loop -- same pattern as J10

### LOW

**CONFIRMED L4**: Single-predecessor phi nodes -- bb10 -> bb9 with phi

---

## Eval vs LLVM Behavioral Comparison

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | **20** | **20** |
| List creation [1,2,3] | Correct | Correct |
| push(4) -> [1,2,3,4] | Correct | Correct (via COW runtime) |
| push(5) -> [1,2,3,4,5] | Correct | Correct (via COW runtime) |
| .length() -> 5 | Correct | Correct (extractvalue) |
| for..in sum -> 15 | Correct | Correct (phi + add loop) |
| total + count -> 20 | Correct | Correct |
| Runtime declarations | N/A | 8 (alloc, push_cow x1 decl, rc_inc, rc_dec, iter_from_list, iter_next, iter_drop, personality) |
| COW behavior | Interpreter COW | Runtime COW (ori_list_push_cow) |
| RC operations | Automatic | 2x inc + 2x dec (1 pair unnecessary) |

**No eval-vs-AOT divergence** -- both produce 20. First fully correct COW journey.

---

## What Works Exceptionally Well

- **COW push codegen**: Clean call to `ori_list_push_cow` with correct parameters. Element passed by pointer, null inc_fn for scalars, cow_mode=1 enabled.
- **List literal creation**: Direct `ori_list_alloc_data` + element stores. No unnecessary allocation overhead.
- **List struct as fat pointer `{ i64, i64, ptr }`**: Clean 24-byte representation with O(1) length access.
- **Iterator loop SSA**: Correct phi nodes for mutable accumulator (`total`). Loop structure is sound.
- **RC balance**: 2 inc + 2 dec = balanced. No leak, no double-free.
- **Element RC optimization**: `ptr null` for int element inc_fn -- correctly skips RC for scalar elements.
- **AOT cache**: Reliable compilation with 0.25-0.38s compile time.
- **Iterator cleanup**: `ori_iter_drop` correctly called at loop exit.
- **No fastcc needed**: Single-function program, C calling convention correct for entry point.
