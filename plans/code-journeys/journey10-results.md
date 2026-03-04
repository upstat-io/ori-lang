# Journey 10: "I am a list"

**Features**: list literals, list `.length()`, list as function parameter, `for..in` iteration over lists, ARC reference counting for heap-allocated collections

**Expected**: `check_length() + check_iteration() + check_passing() = 13 + 15 + 5 = 33`

## Results

| Backend | Output | Exit Code | Status |
|---------|--------|-----------|--------|
| Eval    | 33     | 33        | PASS   |
| AOT     | 33     | 33        | PASS   |

## Source

```ori
@count_items (xs: [int]) -> int = xs.length();
@check_length () -> int = {
    let a = [10, 20, 30]; let b = [40, 50];
    let c = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    a.length() + b.length() + count_items(xs: c) - count_items(xs: b)
}
@check_iteration () -> int = {
    let xs = [1, 2, 3, 4, 5]; let total = 0;
    for x in xs do total += x; total
}
@check_passing () -> int = count_items(xs: [100, 200, 300, 400, 500]);
@main () -> int = { let a = check_length(); let b = check_iteration(); let c = check_passing(); a + b + c }
```

## Phase-by-Phase Analysis

### Lexer

- User source: 863 bytes, 226 tokens, 0 errors
- Prelude: 10,331 bytes, 1,516 tokens, 0 errors
- Clean lex of list bracket tokens `[`, `]`, comma separators, and all integer literals

### Parser

- User module: 5 functions, 0 tests, 0 types, 0 traits, 0 impls, 70 expressions, 0 errors
- Prelude module: 9 functions, 39 traits, 46 expressions, 0 errors
- List literals correctly parsed via "list literal" parse context (observed for `[10, 20, 30]`, `[40, 50]`, `[1..10]`, `[100..500]`, `[1, 2, 3, 4, 5]`)
- `for` loop parsed via "for loop" context
- Method calls (`.length()`) parsed via "method call" context

### Canonicalization

- User module: 70 source expressions lowered to 78 canon nodes, 5 roots, 6 constants, 0 decision trees
- Prelude module: 46 source expressions lowered to 46 canon nodes, 9 roots, 6 constants, 4 decision trees
- No errors

### Type Checking

- Registration, signature collection, and body checking all completed successfully for both prelude (9 functions) and user (5 functions)
- Import resolution: hash-first misses for generic builtins (`len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`), hash-first hits for `compare`, `min`, `max`
- No type errors -- `[int]` correctly inferred for all list literals, `xs.length()` resolved as list method returning `int`

### ARC Pipeline

Five functions declared:
- `count_items` -- 1 param (the `[int]` list), fastcc, Direct return
- `check_length` -- 0 params, fastcc, Direct return
- `check_iteration` -- 0 params, fastcc, Direct return
- `check_passing` -- 0 params, fastcc, Direct return
- `main` -- 0 params, C ABI, Direct return

Nounwind analysis: 1 pass, 0 nounwind functions, 0 mono-propagated. All 5 functions are **not** nounwind. This is correct -- `check_length` calls `count_items` which receives a list by pointer (potential unwind from overflow-checked arithmetic on the call path), and `check_length` itself has overflow-checked arithmetic.

Type registration: 6 prelude types registered (Ordering, CancellationReason, FormatType, Alignment, Sign, FormatSpec + TraceEntry). No user types -- this journey uses only built-in `[int]`.

### LLVM IR -- Deep Scrutiny

#### List Representation

Lists are represented as `{ i64, i64, ptr }` -- a triple of `(length, capacity, data_pointer)`. This is the standard Ori list ABI: length and capacity as i64, with a heap-allocated data buffer obtained from `ori_list_alloc_data`.

#### `_ori_count_items` (22 bytes native)

```llvm
define fastcc i64 @_ori_count_items(ptr %0) {
  ; Load all 3 fields of the list struct via GEP
  %param.load.f0 = load i64, ptr %param.load.f0.ptr  ; length
  %param.load.f1 = load i64, ptr %param.load.f1.ptr  ; capacity
  %param.load.f2 = load ptr, ptr %param.load.f2.ptr  ; data ptr
  ; Reconstruct aggregate, then extract length
  %list.len = extractvalue { i64, i64, ptr } %param.load.s2, 0
  ret i64 %list.len
}
```

**Observation**: The function loads all 3 fields (length, capacity, data_ptr) into an aggregate, then extracts only field 0 (length). Fields 1 and 2 are dead loads. This is the same pattern as J4's dead struct field loads -- DCE at `-O1+` will eliminate them. At `-O0` the native code confirms all 3 loads occur (3 `mov` instructions) but only the length is returned.

**ARC correctness**: No `ori_buffer_rc_dec` in this function -- correct, because the function borrows the list via pointer and does not consume it. The caller retains ownership.

#### `_ori_check_length` (446 bytes native)

This is the most ARC-intensive function. Three list allocations:

1. `let a = [10, 20, 30]` -- `ori_list_alloc_data(3, 8)`, stores 10/20/30 via GEP, constructs `{ 3, 3, ptr }`
2. `let b = [40, 50]` -- `ori_list_alloc_data(2, 8)`, stores 40/50, constructs `{ 2, 2, ptr }`
3. `let c = [1..10]` -- `ori_list_alloc_data(10, 8)`, stores 1-10 individually, constructs `{ 10, 10, ptr }`

**ARC operations (6 total)**:

| Operation | Target | Why |
|-----------|--------|-----|
| `ori_buffer_rc_dec(a)` | list `a` | After extracting `a.length()`, `a` is no longer needed |
| `ori_list_rc_inc(b)` | list `b` | `b` is used twice: once for `b.length()` and once as argument to `count_items(xs: b)` |
| `ori_buffer_rc_dec(b)` | list `b` | After extracting `b.length()`, one reference consumed |
| `ori_buffer_rc_dec(c)` | list `c` | After passing to `count_items(xs: c)`, `c` no longer needed |
| `ori_buffer_rc_dec(b)` | list `b` | After passing to `count_items(xs: b)`, second reference consumed |
| (landing pad) `ori_buffer_rc_dec(b)` | list `b` | Cleanup on unwind from `count_items(xs: c)` call |

**ARC correctness analysis**:
- List `a`: allocated (rc=1), used for `.length()` extraction, then dec'd (rc=0, freed). Correct.
- List `b`: allocated (rc=1), inc'd (rc=2) because used in two contexts (`.length()` extraction and `count_items` call). Dec'd after `.length()` (rc=1), dec'd after `count_items` (rc=0, freed). Correct.
- List `c`: allocated (rc=1), passed to `count_items` (borrowed, not consumed by callee), dec'd after call returns (rc=0, freed). Correct.
- Landing pad: if `count_items(xs: c)` panics via overflow, list `b` (still live at rc=1) is cleaned up. Correct.

**Finding**: List `a` is dec'd immediately after its `.length()` is extracted, but the length extraction is a pure `extractvalue` (no read from the data buffer). The dec could be moved earlier if the compiler knew `.length()` was a pure metadata read. However, the current placement is correct and conservative.

**Exception handling**: `check_length` has `personality ptr @rust_eh_personality` and a landing pad (`bb6`) that cleans up list `b` on unwind. This is correct -- the first `count_items` call is `invoke` (can unwind), and `b` is live across that call. The second `count_items` call uses plain `call` (not `invoke`) because there are no more resources to clean up after it.

#### `_ori_check_iteration` (514 bytes native)

```llvm
; Allocate list [1, 2, 3, 4, 5]
%list.data = call ptr @ori_list_alloc_data(i64 5, i64 8)
; Store elements 1-5
; Construct { 5, 5, ptr }

; RC inc for iteration (list used both in loop and for cleanup)
call void @ori_list_rc_inc(ptr %rc_inc.data, i64 %rc_inc.cap)

; Create iterator from list
%list.iter = call ptr @ori_iter_from_list(ptr %list.data5, i64 %list.len, i64 %list.cap, i64 8, ptr null)

; Loop: bb1 -> bb2 (body) -> bb1 | bb1 -> bb4 -> bb3 (exit)
bb1:
  %v12 = phi i64 [ 0, %bb0 ], [ %add.val, %add.ok ]   ; accumulator (total)
  %iter_next.has = call i8 @ori_iter_next(ptr %list.iter, ptr %iter_next.scratch, i64 8)
  ; Check if Some (tag != 0) or None (tag == 0)
  br i1 %ne, label %bb2, label %bb4

bb2:  ; loop body
  %proj.1 = extractvalue { i64, i64 } %iter_next.1, 1  ; x = element value
  %add = @llvm.sadd.with.overflow.i64(i64 %v12, i64 %proj.1)  ; total += x
  br label %bb1  ; back to loop

bb3:  ; exit
  call void @ori_buffer_rc_dec(...)   ; release the list's extra RC
  call void @ori_iter_drop(ptr %list.iter)  ; drop iterator
  ret i64 %v15  ; return total
```

**Iterator protocol**: The list iterator uses the opaque C API:
1. `ori_iter_from_list(data, len, cap, elem_size, elem_dropper)` -- creates iterator state on heap, increments RC on the list buffer
2. `ori_iter_next(iter, scratch, elem_size)` -> `i8` (0=None, 1=Some) -- writes element to scratch buffer
3. `ori_iter_drop(iter)` -- frees iterator state, decrements RC on the list buffer

**ARC correctness**:
- List allocated at rc=1
- `ori_list_rc_inc` bumps to rc=2 (one for the iterator's reference, one for the list variable)
- Loop runs, iterator consumes elements without modifying RC
- On exit: `ori_buffer_rc_dec` drops the list variable's reference (rc=1), `ori_iter_drop` drops the iterator's reference (rc=0, freed)
- Correct: no leak, no double-free

**Phi node usage**: `%v12` phi merges the initial `0` with the loop-updated `%add.val`. The list struct `%v11` phi is a pass-through (same value both branches) -- this keeps the list live for cleanup.

**Finding**: The `%v11` phi for the list struct is `[ %list.2, %bb0 ], [ %v11, %add.ok ]` -- it never changes. This is a loop-invariant value that could be hoisted out of the phi. LLVM's LICM pass will handle this at `-O1+`, but it is slightly wasteful at `-O0`.

#### `_ori_check_passing` (156 bytes native)

```llvm
; Allocate [100, 200, 300, 400, 500]
%list.data = call ptr @ori_list_alloc_data(i64 5, i64 8)
; Store elements
; Store struct to stack for by-pointer call
store { i64, i64, ptr } %list.2, ptr %ref_arg
%call = call fastcc i64 @_ori_count_items(ptr %ref_arg)
; Drop with unique optimization
call void @ori_buffer_drop_unique(ptr ..., i64 ..., i64 ..., i64 8, ptr null)
ret i64 %call
```

**Key finding -- `ori_buffer_drop_unique`**: This function uses `ori_buffer_drop_unique` instead of `ori_buffer_rc_dec`. The ARC pipeline has determined that this list is provably unique (rc=1 at all points, never shared). This is the static uniqueness optimization from `ori_arc` (`cow_mode=1`): skip the atomic RC decrement and directly free the buffer. This is correct and more efficient.

Compare with `check_length` which uses `ori_buffer_rc_dec` -- there, list `b` is shared (inc'd to rc=2), so the dynamic RC path is required.

#### `_ori_main` (114 bytes native)

Straightforward: three sequential `call fastcc` to the sub-functions, two overflow-checked additions, return. No ARC operations -- `main` deals only with `int` return values. C ABI wrapper truncates i64 to i32 as usual.

### Native Code Analysis

| Function | Size (bytes) | Stack frame | ARC ops | Calls |
|----------|-------------|-------------|---------|-------|
| `_ori_count_items` | 22 | 0 (leaf) | 0 | 0 |
| `_ori_check_length` | 446 | 0xB8 (184B) | 6 | `alloc_data` x3, `rc_dec` x4, `rc_inc` x1, `count_items` x2, `panic_cstr` x3 |
| `_ori_check_iteration` | 514 | 0xB8 (184B) | 3 | `alloc_data` x1, `rc_inc` x1, `iter_from_list` x1, `iter_next` x(loop), `rc_dec` x1, `iter_drop` x1, `panic_cstr` x1 |
| `_ori_check_passing` | 156 | 0x38 (56B) | 1 | `alloc_data` x1, `count_items` x1, `drop_unique` x1 |
| `_ori_main` | 114 | 0x28 (40B) | 0 | `check_*` x3, `panic_cstr` x2 |
| `main` (C wrapper) | 8 | 8 (push) | 0 | `_ori_main` x1 |

Total user code: ~1,260 bytes (vs ~307B in J3, ~349B in J4 -- expected growth for list operations).

### Binary Size

| Metric | J10 Value | J4 Value | Notes |
|--------|-----------|----------|-------|
| Binary (debug) | 6.35 MiB | 6.35 MiB | Statically linked ori_rt |
| .text | 899 KiB | ~869 KiB | Slight growth from list/iterator runtime symbols |
| .rodata | 134 KiB | 133 KiB | Additional overflow message strings |
| User .text | ~1,260 bytes | ~349 bytes | 3.6x growth: heap alloc, ARC, iteration |
| Debug info | ~4.5 MiB | ~4.8 MiB | .debug_* sections |

### Pipeline Performance

| Phase | J10 Time | Notes |
|-------|----------|-------|
| Lexer | <1ms | 863B user + 10,331B prelude |
| Parser | <1ms | 226 + 1,516 tokens |
| Canonicalization | <1ms | 78 + 46 canon nodes |
| Type check | <1ms | 5 user + 9 prelude functions |
| LLVM codegen | ~0.30s | Includes ARC pipeline (5 functions), nounwind (1 pass) |
| AOT compile | ~0.30s | Linking with ori_rt |
| Total (first run) | ~0.30s | Cold start |

### Eval Trace Analysis

108 trace lines total. Key observations:
- `check_length`: builds 3 lists (Int literals 10/20/30, 40/50, 1-10), calls `count_items` twice, evaluates `.length()` method calls, performs Add/Sub binary operations
- `check_iteration`: builds list [1,2,3,4,5], evaluates `For` loop with 5 iterations of `Assign(total += x)`, each iteration traces `Binary(Add)` + `Ident(total)` + `Ident(x)`
- `check_passing`: builds list [100,200,300,400,500], calls `count_items`
- `main`: binds all 3 results, final `Binary(Add, Binary(Add, a, b), c)`

The for loop shows 5 iterations (lines 63-87), each with the pattern: `Assign -> Binary(Add) -> Ident(total) -> Ident(x) -> evaluate_binary(Add, int, int)`. This confirms the imperative mutation loop is correctly interpreted.

## Findings

| Finding | Severity | Description |
|---------|----------|-------------|
| Dead list field loads | LOW | `count_items` loads all 3 list fields but only uses length; DCE removes at -O1+ (same pattern as J4 dead struct loads) |
| Loop-invariant phi | LOW | `check_iteration` carries the list struct through a phi that never changes value; LICM handles at -O1+ |
| Overflow message dedup (cont.) | LOW | 6 overflow message constants, some identical (`ovf.msg` and `ovf.msg.1` both "integer overflow on addition") |
| Static uniqueness optimization | POSITIVE | `check_passing` correctly uses `ori_buffer_drop_unique` for provably-unique list, avoiding atomic RC decrement |
| Exception-safe ARC cleanup | POSITIVE | `check_length` landing pad correctly cleans up shared list `b` on unwind from `count_items` call |
| Iterator protocol correctness | POSITIVE | `check_iteration` correctly manages dual RC (list var + iterator ref), drops both on exit, no leak |

## Conclusion

Journey 10 is the first to exercise **heap-allocated ARC-managed collections**. The compiler correctly handles:
- List literal allocation via `ori_list_alloc_data` with element-by-element store
- ARC reference counting: `ori_list_rc_inc` for shared references, `ori_buffer_rc_dec` for dynamic drops, `ori_buffer_drop_unique` for statically-proven unique drops
- Iterator protocol: `ori_iter_from_list` / `ori_iter_next` / `ori_iter_drop` with correct RC lifecycle
- Exception-safe cleanup via landing pads
- List passed by pointer (24B = 3x i64 > 16B threshold)
- `.length()` as pure field extraction from the list struct

The static uniqueness optimization in `check_passing` is a notable win -- the ARC pipeline correctly identifies that the inline list literal `[100, 200, 300, 400, 500]` is never shared and emits `ori_buffer_drop_unique` instead of the heavier `ori_buffer_rc_dec`.

**Status: PASS** -- Both backends produce correct output (33). ARC lifecycle is sound. No memory leaks or double-frees in the generated code.
