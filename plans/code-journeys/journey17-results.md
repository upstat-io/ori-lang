# Journey 17: COW Slice + Combined Operations -- Results

**Date**: 2026-03-02
**Status**: PASS (both paths)
**Eval exit code**: 10 (correct)
**AOT exit code**: 10 (correct)

## Journey Code

```ori
@main () -> int = {
    let $nums = [10, 20, 30, 40, 50];
    let $slice = nums.take(count: 2);
    let slice_sum = 0;
    for x in slice do {
        slice_sum += x;
    };
    let $slice_len = slice.length();

    let $greeting = "hello world";
    let $sub = greeting.substring(start: 0, end: 3);
    let $sub_len = sub.length();

    let chain = [1, 2, 3];
    let chain = chain.push(4);
    let chain = chain.push(5);
    let $chain_len = chain.length();

    slice_len + sub_len + chain_len
}
```

**Expected**: `slice_len=2`, `sub_len=3`, `chain_len=5`, result `2+3+5=10`.

---

## Phase-by-Phase Analysis

### 1. Lexer

- Source: 831 bytes, 172 tokens, 0 errors
- Prelude: 10,331 bytes, 1,516 tokens, 0 errors (CONFIRMED M1)
- **Clean pass.** List literal `[10, 20, 30, 40, 50]`, string literal `"hello world"`, method calls `.take()`, `.substring()`, `.push()`, `.length()` all tokenized correctly.

### 2. Parser

- User module: 1 function, 45 expressions, 0 errors, 0 warnings
- Prelude: 9 functions, 39 traits, 46 expressions, 0 errors, 0 warnings
- Method calls parsed correctly: `nums.take(count: 2)`, `greeting.substring(start: 0, end: 3)`, `chain.push(4)`, `chain.push(5)`, `.length()` (4 calls)
- List literal `[10, 20, 30, 40, 50]` and `[1, 2, 3]` parsed as "list literal" contexts
- `for x in slice do { ... }` parsed as "for loop" context
- **Clean pass.**

### 3. Type Checker

- Registration: 9 prelude functions + 1 user function, 0 impls, 0 errors
- Body checking complete with 0 errors
- Prelude import resolution: hash-first miss for generic builtins (`len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`), hash-first hit for value builtins (`compare`, `min`, `max`)
- Types inferred correctly: `[int]` for lists, `str` for strings, `int` for lengths
- `.take(count:)` returns `[int]`, `.substring(start:, end:)` returns `str`, `.push()` returns `[int]`, `.length()` returns `int`
- **Clean pass.**

### 4. Canonicalizer

- User module: 45 source expressions -> 56 canon nodes (24.4% expansion)
- 6 constants, 0 decision trees
- Prelude: 46 expressions -> 46 canon nodes, 4 decision trees
- **Expansion ratio**: 24.4% -- higher than average (consistent with complex method chains; similar to J9's 25% for bool+string). The for loop and compound assignment contribute extra nodes. (CONFIRMED L1)

### 5. Eval Trace

- 75 trace lines covering full execution
- **List creation** (CanId 5): `List(CanRange(0..5))` builds `[10, 20, 30, 40, 50]`
- **take** (CanId 9): `MethodCall(CanId(7), Name(take), CanRange(5..6))` with count=2, returns `[10, 20]`
- **for loop** (CanId 20): iterates over slice, 2 iterations:
  - `slice_sum += 10` (Add, left=0, right=10), `slice_sum += 20` (Add, left=10, right=20)
- **slice.length()** (CanId 22): `MethodCall(CanId(21), Name(length))` returns 2
- **String creation** (CanId 24): `Str(Name("hello world"))`
- **substring** (CanId 29): `MethodCall(CanId(26), Name(substring), CanRange(7..9))` with start=0, end=3, returns "hel"
- **sub.length()** (CanId 32): `MethodCall(CanId(31), Name(length))` returns 3
- **chain list** (CanId 37): `List(CanRange(9..12))` builds `[1, 2, 3]`
- **chain.push(4)** (CanId 41): `MethodCall(CanId(39), Name(push), CanRange(12..13))` returns `[1, 2, 3, 4]`
- **chain.push(5)** (CanId 45): `MethodCall(CanId(43), Name(push), CanRange(13..14))` returns `[1, 2, 3, 4, 5]`
- **chain.length()** (CanId 48): returns 5
- **Final** (CanId 54): `slice_len(2) + sub_len(3) + chain_len(5) = 10`
- **Clean pass.** All three collection operations (slice, substring, chained push) produce correct results.

### 6. ARC Trace

- `ori_llvm::codegen::type_registration`: Registers 6 user types (enums for Ordering, FormatType, Alignment, Sign, plus structs for FormatSpec, TraceEntry)
- `ori_llvm::codegen::function_compiler`: Declares `_ori_main` with Direct return passing, C calling convention, 0 parameters
- Nounwind analysis: 1 pass, 0 nounwind functions, 0 mono propagated
- Entry point wrapper: `main()` generated with `has_args=false`, `returns_int=true`, `has_panic=false`
- **No ARC-specific warnings or errors.**

### 7. LLVM Warnings

- No warnings emitted (file contains only compilation timing: "Compiled in 0.27s")
- **Clean.**

### 8. Build Output

- Build: compiled in 0.27-0.28s across trace runs
- No errors or warnings in build_stdout/build_stderr (empty files)
- **Clean build.**

### 9. Binary Analysis

- **Size**: 6,764,360 bytes (6.45 MB) -- consistent with prior journeys
- **Text section**: 959,513 bytes (937 KB)
- **Rodata**: 136,206 bytes
- **`_ori_main` size**: 0x48f = 1,167 bytes -- reasonable for 3 collection types + loop + string ops
- **Stack frame**: 0x1a8 = 424 bytes (from `sub $0x1a8, %rsp`)
- **`_ori_drop$3`**: 18 bytes -- minimal string drop function

---

## LLVM Deep Scrutiny (9 Categories)

### S1. Correctness

**Verdict**: CORRECT -- both paths produce 10.

The LLVM IR correctly implements:
1. **List creation**: `ori_list_alloc_data(5, 8)` + 5 element stores for `[10,20,30,40,50]`
2. **Seamless slice (take)**: `ori_list_slice_take(data, len, cap, 2, 8, out)` -- zero-copy view of first 2 elements
3. **For-loop over slice**: `ori_iter_from_list` + `ori_iter_next` loop with phi-based accumulator
4. **Slice length**: `extractvalue { i64, i64, ptr } %take.val.s2, 0` -- O(1) field extraction
5. **String creation**: `ori_str_from_raw(@str, 11)` for "hello world"
6. **String substring**: `ori_str_substring(sret, self_ptr, 0, 3)` -- produces "hel"
7. **String SSO-aware RC**: High-bit flag check to skip RC for SSO strings
8. **String length**: `ori_str_len(self_ptr)` -- function call (not extractvalue)
9. **Chained list pushes**: Two `ori_list_push_cow` calls with COW semantics
10. **Final addition**: `add i64 %list.len9, %str.len` + `add i64 %add, %list.len49`

### S2. ARC / Reference Counting

**List slice ARC lifecycle**:

The slice sharing protocol follows this sequence:
1. `ori_list_alloc_data(5, 8)` -- creates backing buffer (RC=1)
2. `ori_list_slice_take(data, len, cap, 2, ...)` -- internally calls `ori_rc_inc(original_data)` (RC=2), returns `{len=2, cap=SLICE_FLAG|0, data=original}`
3. `ori_buffer_rc_dec(original_data, ...)` at bb1:52 -- drops the `nums` binding (RC=1)
4. `ori_list_rc_inc(slice_data, slice_cap)` at bb1:55 -- increments for what? (RC=2)
5. `ori_iter_from_list(slice_data, ...)` at bb1:59 -- creates iterator from the slice
6. `ori_iter_drop(list.iter)` at bb5:86 -- drops the iterator
7. `ori_buffer_rc_dec(slice_data, ...)` at bb7:97 -- drops the `slice` binding (RC -> 0, freed)

**Issue M20 (MEDIUM)**: **Redundant RC inc on slice data after original is dropped.** At bb1, the code first decrements the original list's RC (line 52), then immediately increments the slice's RC (line 55). Since the slice shares the same backing buffer, this is equivalent to: RC goes 2->1 (dec), then 1->2 (inc). The net effect is no change. The inc at line 55 appears to be emitted for the iterator's "borrow" of the slice, but the iterator already receives the data pointer and the subsequent `ori_buffer_rc_dec` at bb7 will handle the final decrement. This is the same pattern as M15 (redundant RC inc/dec pair across operations).

**String SSO-aware RC (two-pass)**:

The IR emits two identical SSO check sequences:
1. **First check** (bb9, lines 127-134): For `greeting` (the original string). Checks high-bit (`and i64 %p2i, -9223372036854775808`) and null pointer. If SSO or null, skips `ori_rc_dec`; else calls `ori_rc_dec(data, @_ori_drop$3)`.
2. **Second check** (bb11, lines 142-149): For `sub` (the substring result). Same SSO/null check pattern.

Both are correct: "hello world" is 11 bytes -- above SSO threshold (typically 15 or 23 bytes), so it will be heap-allocated and the RC operations will execute. The substring "hel" is 3 bytes -- likely SSO, so the RC dec will be skipped. This is architecturally sound.

**Chained push ARC**:
- First push (line 227): `cow_mode=0` (dynamic RC check) -- the `[1,2,3]` list was just allocated with RC=1
- Second push (line 161): `cow_mode=1` (static unique) -- the result of the first push is known to be uniquely owned
- Final `ori_buffer_rc_dec` (line 191): drops the final chain list after extracting its length

**Issue M21 (MEDIUM)**: **First push uses cow_mode=0 for freshly allocated list.** The `[1,2,3]` list is created via `ori_list_alloc_data(3, 8)` immediately before the first `ori_list_push_cow` call. Since it was just allocated, its RC is guaranteed to be 1 (unique owner). The push should use `cow_mode=1` (static unique) to skip the runtime uniqueness check. Instead it uses `cow_mode=0` (dynamic), which will call `ori_rc_is_unique(data)` unnecessarily. The second push correctly uses `cow_mode=1` because the ARC analysis recognizes the previous push's output as unique. The analysis fails to recognize a fresh `ori_list_alloc_data` result as unique.

**Issue**: **Missing noalias on first push data pointer.** Line 227 passes `ptr %list.data30` (no `noalias`), while line 161 passes `ptr noalias %list.data34`. Since the first push's data is freshly allocated and not aliased, it should also have `noalias`. This is correlated with the cow_mode=0 issue -- when the analysis cannot prove uniqueness, it also cannot assert noalias.

### S3. Alignment

**CONFIRMED M5**: `align 4` on i64 loads from struct fields throughout the IR:
- Line 38: `%take.val.f0 = load i64, ptr %take.val.f0.ptr, align 4`
- Line 41: `%take.val.f1 = load i64, ptr %take.val.f1.ptr, align 4`
- Line 71: `%iter_next.elem = load i64, ptr %iter_next.scratch, align 4`
- Line 100: `%str.val.f0 = load i64, ptr %str.val.f0.ptr, align 4`
- Lines 111, 114: Substring result field loads, all `align 4`
- Lines 163, 166, 229, 232: Push output field loads, all `align 4`

All should be `align 8` for i64 values. 16+ instances in this IR alone.

### S4. Dead Code / Unreachable Blocks

**CONFIRMED M3**: Unnecessary `br label` blocks:
- bb0 -> bb1 (line 46): Could be a single block
- bb5 -> bb7 via bb6 (lines 83-91): bb6 exists only to branch to bb5
- Additional trivial branches: bb7->bb9, bb9->bb11 (via rc_dec conditional), bb13->bb15, bb15->bb17

**CONFIRMED M11**: Orphaned landing pads with no predecessors:
- `bb2` (lines 62-65): `landingpad` cleanup, no invoke targets it
- `bb8` (lines 121-124): `landingpad` cleanup, unreachable
- `bb10` (lines 136-139): `landingpad` cleanup, unreachable
- `bb12` (lines 151-154): `landingpad` cleanup, unreachable
- `bb14` (lines 173-176): `landingpad` cleanup, unreachable
- `bb16` (lines 182-185): `landingpad` cleanup, unreachable
- `bb18` (lines 196-199): `landingpad` cleanup, unreachable

**7 orphaned landing pads** -- the most in any journey so far. Each collection operation (slice, string, push) appears to generate its own landing pad pair, even though no `invoke` instructions are emitted.

### S5. Loop / Iterator Codegen

**For-loop over slice**:
1. `ori_iter_from_list(data, len, cap, 8, null)` creates iterator from the slice (bb1:59)
2. Loop at bb3: `ori_iter_next(iter, scratch_buf, 8)` returns `i8` (has_more flag)
3. `zext i8` to `i64` for tag check (line 70) -- CONFIRMED M13 (Option-like `{tag, value}` construction)
4. Element extracted via `load i64, ptr %iter_next.scratch, align 4` (line 71)
5. Accumulate: `add i64 %v15, %proj.1` (line 80) -- correct SSA phi-based accumulation
6. Loop exit: `ori_iter_drop(list.iter)` at bb5:86

**SSA phi nodes**: bb3 has `%v15 = phi i64 [ 0, %bb1 ], [ %add55, %bb4 ]` for the accumulator. Correct.

**CONFIRMED L7**: Dead phi at loop exit -- `bb5` has:
- `%v16 = phi i64 [ 0, %bb6 ]` -- dead, never used
- `%v17 = phi i64 [ %v15, %bb6 ]` -- dead, never used (the `slice_sum` value is computed but not part of the return expression, since only `slice_len` is used)

Actually, `slice_sum` is computed in the for loop but only `slice_len` contributes to the return value. The loop body computes `slice_sum` but the value is discarded. This is a source-level dead computation, not a codegen bug -- the for loop has side effects only on the mutable `slice_sum` variable which is never read after the loop. The compiler could potentially warn about unused mutable variables.

### S6. String Handling

**String literal**: `@str = private unnamed_addr constant [12 x i8] c"hello world\00"` -- correctly null-terminated, 11 bytes + null. Created via `ori_str_from_raw(sret, @str, 11)`.

**String substring**: `ori_str_substring(sret, self_ptr, 0, 3)` -- takes the string struct by pointer (stored to alloca `%substring.self` first), returns via sret. The runtime function `ori_str_substring` (symbol at 0x23480, 1,683 bytes) handles SSO/heap distinction and likely produces a zero-copy slice when the backing string is heap-allocated.

**String length**: `ori_str_len(self_ptr)` is a function call, NOT an `extractvalue`. This is different from list length which uses `extractvalue` on the `{i64, i64, ptr}` struct directly. For strings, the SSO encoding means length can be stored differently (SSO length in the high bits of the pointer field), so a function call is correct and necessary.

**SSO RC check**: The `rc_dec` path for strings correctly checks the high bit of the data pointer (`and i64 %p2i, -9223372036854775808`) to detect SSO strings, and also checks for null. Both SSO and null skip `ori_rc_dec`. This is correct -- "hello world" (11 bytes) will be heap-allocated in the Ori SSO scheme (threshold likely < 11), so RC operations will execute for it. "hel" (3 bytes) is definitely SSO and will skip RC.

### S7. COW-Specific Codegen

**Seamless list slice (take)**:

The `ori_list_slice_take` function (symbol size: 0x8d = 141 bytes) delegates to `ori_list_slice` which creates a zero-copy view. The slice shares the original buffer's data pointer. The return value is `{len=2, cap=SLICE_FLAG|byte_offset, data=original+offset}`. The SLICE_FLAG in the capacity field marks this as a seamless slice rather than an owned buffer.

Key observation: The slice's capacity encodes a slice flag, which means:
- `ori_rc_is_unique(data)` would read garbage (data points into interior of another allocation)
- `ori_list_push_cow` correctly checks `is_slice_cap(cap)` before the uniqueness fast path
- `ori_list_rc_inc` at bb1:55 takes `(data, cap)` and internally determines whether to increment via the original buffer header or handle slice semantics

**String substring (seamless)**:

`ori_str_substring` (symbol size: 0x693 = 1,683 bytes -- relatively large) handles the full substring semantics. The runtime source shows it produces an SSO or heap-allocated result depending on the substring length. For a 3-byte substring ("hel"), this will almost certainly produce an SSO result -- no heap allocation, no RC needed.

**Chained push COW**:

Two `ori_list_push_cow` calls with different cow_modes:
1. `push_cow(data, 3, 3, &4, 8, 8, null, cow_mode=0, out)` -- dynamic RC check
2. `push_cow(data, 4, ?, &5, 8, 8, null, cow_mode=1, out)` -- static unique

The second push correctly uses `cow_mode=1` because the ARC pipeline determined the first push's output is uniquely owned (single reference from the `let chain = ...` rebinding). The first push could use `cow_mode=1` too (see M21).

**Mixed collection types**: The codegen correctly handles three different collection types in the same function:
- `[int]` lists with slice semantics (slice_take + iterator)
- `str` with SSO-aware RC and substring
- `[int]` lists with COW push semantics

Each type gets its own RC protocol: lists use `ori_buffer_rc_dec` with element size/count, strings use SSO-gated `ori_rc_dec`, and the iterator gets `ori_iter_drop`. No cross-contamination between types.

### S8. Calling Convention / ABI

- `_ori_main` uses C calling convention (not fastcc) -- CONFIRMED M10
- No `nounwind` attribute on `_ori_main` -- CONFIRMED M10
- Runtime function declarations:
  - `ori_list_slice_take`: `nounwind` -- correct (pure memory operation)
  - `ori_buffer_rc_dec`: `nounwind memory(inaccessiblemem: readwrite)` -- correct
  - `ori_list_rc_inc`: `nounwind memory(inaccessiblemem: readwrite)` -- correct
  - `ori_rc_dec`: `nounwind memory(inaccessiblemem: readwrite)` -- correct
  - `ori_rc_free`: `nounwind` -- correct
  - `ori_str_from_raw`: `noalias sret` on return -- correct
  - `ori_str_substring`: `noalias sret` on return -- correct
  - `ori_list_push_cow`: `noalias` on output `ptr` only (not on data for cow_mode=0 call) -- see M21
  - `ori_iter_from_list`, `ori_iter_next`, `ori_iter_drop`: no attributes -- CONFIRMED H3 (missing nounwind)
  - `ori_str_len`: no attributes -- should at least have `nounwind`

**Sret pattern**: Both `ori_str_from_raw` and `ori_str_substring` use sret for their 24-byte struct returns, then per-field GEP+load+insertvalue to materialize the value. This is the correct pattern for JIT compatibility (avoids FastISel aggregate load bug).

### S9. Code Quality / Optimization Opportunities

**CONFIRMED M5**: `align 4` on i64 loads (16+ instances)

**CONFIRMED M3**: Multiple unnecessary `br label` blocks (at least 4 trivial fall-through branches)

**CONFIRMED M11**: 7 orphaned landing pads -- worst count in any journey. Each section of code (slice, string, push) contributes its own pair.

**CONFIRMED M13**: Iterator next returns `{i64 tag, i64 elem}` via separate alloca + zext + insertvalue, then immediately destructures. The `zext i8 to i64` for the tag (line 70) is wasteful -- a direct `i8` comparison would suffice.

**CONFIRMED L7**: 2 dead phi values at loop exit (`%v16`, `%v17`)

**NEW M21**: First `ori_list_push_cow` call uses `cow_mode=0` (dynamic uniqueness check) for a freshly-allocated list that is provably unique. The ARC analysis recognizes `push_cow` output as unique (second push correctly gets `cow_mode=1`) but does not recognize `ori_list_alloc_data` output as unique.

**Observation -- slice_sum is dead**: The for loop computes `slice_sum = 30` but this value is never read (only `slice_len` is used in the return expression). The compiler generates the full loop body + accumulator without noticing the accumulated value is dead. A dead-variable analysis could skip the loop body entirely (the loop over the slice is pure -- no side effects beyond the accumulator).

---

## Disassembly Analysis

**`_ori_main`**: 1,167 bytes (0x48f). Stack frame: 0x1a8 = 424 bytes.

Key operations in native code:
1. `ori_list_alloc_data` + 5 movq stores for `[10,20,30,40,50]`
2. `ori_list_slice_take` for zero-copy take(2)
3. `ori_buffer_rc_dec` to drop original list reference
4. `ori_list_rc_inc` to inc slice (redundant -- M20)
5. `ori_iter_from_list` + loop with `ori_iter_next` + `add` for slice iteration
6. `ori_iter_drop` at loop exit
7. `ori_buffer_rc_dec` for slice cleanup
8. `ori_str_from_raw` for "hello world"
9. `ori_str_substring` for "hel"
10. SSO check: `movabs $0x8000000000000000` + `and` + `setne` + null check + `or` -- 2 passes (original + substring)
11. `ori_str_len` for substring length
12. `ori_list_alloc_data` + 3 movq stores for `[1,2,3]`
13. Two `ori_list_push_cow` calls
14. `ori_buffer_rc_dec` for final chain cleanup
15. Two `add` instructions for final `slice_len + sub_len + chain_len`
16. `ret`

**`_ori_drop$3`**: 18 bytes. Calls `ori_rc_free(ptr, 24, 8)` -- correct for string layout cleanup.

**`main`**: 8 bytes. Just `push %rax; call _ori_main; pop %rcx; ret`. Clean.

---

## COW Focus Area Analysis

### 1. List take/slice -- seamless zero-copy?

**YES.** The codegen calls `ori_list_slice_take` which produces a `{len, SLICE_FLAG|offset, data_ptr}` tuple. No element copying occurs -- the slice's data pointer points into the original buffer. The runtime source (`compiler/ori_rt/src/list/slice.rs`) confirms:
- `ori_list_slice_take` delegates to `ori_list_slice(data, len, cap, 0, n.min(len), elem_size, out_ptr)`
- The slice result uses `make_slice_cap(total_byte_offset)` to encode the slice flag in the capacity field
- `ori_rc_inc(original_data)` increments the backing buffer's RC

### 2. String substring -- seamless slice?

**PARTIAL.** `ori_str_substring` is called as a function (symbol size 1,683 bytes, suggesting non-trivial logic). For the 3-byte result "hel", the runtime likely produces an SSO (Small String Optimization) result -- the string is stored inline in the struct's fields, not on the heap. This is even better than a seamless slice: zero allocation, zero RC. For longer substrings that exceed the SSO threshold, the runtime would need to produce a heap-backed result. Whether it uses seamless slicing (sharing the original string's buffer) or copies is not visible from the IR alone -- that is a runtime implementation detail.

### 3. Chained push -- uniqueness fast path?

**PARTIAL.** The second push (`chain.push(5)`) correctly uses `cow_mode=1` (static unique), which skips the runtime `ori_rc_is_unique` check entirely -- pure fast path. However, the first push (`chain.push(4)`) uses `cow_mode=0` (dynamic check), which will call `ori_rc_is_unique(data)` at runtime. Since the list was just allocated, this check will succeed (RC=1), so the result is correct but suboptimal. See M21.

### 4. ARC for slice sharing -- correct RC protocol?

**CORRECT but redundant.** The slice correctly increments the backing buffer's RC (inside `ori_list_slice_take`). The codegen then decrements the original `nums` binding's RC and increments the slice's RC again -- this extra inc/dec pair (M20) is a no-op. The final cleanup correctly decrements the slice's RC after use, eventually freeing the backing buffer. No leaks, no use-after-free.

### 5. Mixed collection types -- correct type discrimination?

**YES.** Three distinct collection types in one function:
- **Lists** (`[int]`): `ori_list_alloc_data` / `ori_buffer_rc_dec` / `ori_list_rc_inc` / `ori_list_push_cow`
- **Strings** (`str`): `ori_str_from_raw` / `ori_str_substring` / `ori_str_len` / SSO-gated `ori_rc_dec` with `_ori_drop$3`
- **Iterators**: `ori_iter_from_list` / `ori_iter_next` / `ori_iter_drop`

Each type's RC protocol is independent. No cross-contamination. String RC uses SSO checks (high-bit flag on pointer), list RC uses `ori_buffer_rc_dec` (element-aware), iterator has its own `ori_iter_drop`. The codegen correctly selects the appropriate protocol for each type.

---

## Findings Summary

| ID | Severity | Description | Status |
|----|----------|-------------|--------|
| M1 | MEDIUM | Prelude 10,331 bytes overhead | CONFIRMED (15/15) |
| M3 | MEDIUM | Unnecessary `br label` after function calls (4+ instances) | CONFIRMED (15/15) |
| M5 | MEDIUM | `align 4` on i64 loads -- should be `align 8` (16+ instances) | CONFIRMED |
| M10 | MEDIUM | `_ori_main` missing `nounwind` attribute | CONFIRMED |
| M11 | MEDIUM | Orphaned landing pads -- **7 in this journey** (worst yet) | CONFIRMED |
| M13 | MEDIUM | Iterator next uses Option-like `{tag, value}` with zext | CONFIRMED |
| M20 | MEDIUM | Redundant RC inc on slice data after original RC dec | NEW |
| M21 | MEDIUM | First push_cow uses cow_mode=0 for freshly-allocated list | NEW |
| H3 | HIGH | Missing nounwind on `ori_iter_from_list`, `ori_iter_next`, `ori_iter_drop`, `ori_str_len` | CONFIRMED |
| L1 | LOW | Canon expansion 24.4% | CONFIRMED |
| L7 | LOW | Dead phi values at loop exit (2 instances) | CONFIRMED |

### New Findings

**M20 (MEDIUM)**: **Redundant RC inc on slice data after original list is dropped.** After `ori_list_slice_take` returns (which internally increments RC to 2), the codegen emits `ori_buffer_rc_dec` on the original binding (RC 2->1), then `ori_list_rc_inc` on the slice (RC 1->2). The inc and the previous dec cancel out. The net effect is correct (RC stays at 2 during the transition), but two unnecessary runtime calls are emitted. Source: ARC pipeline emitting separate RC operations for the `$nums` drop and the `$slice` consumption without recognizing the canceling pair.

**M21 (MEDIUM)**: **First `ori_list_push_cow` uses `cow_mode=0` (dynamic) for a provably unique list.** The `[1,2,3]` list is created by `ori_list_alloc_data` immediately before the push. Since it has never been shared, its RC is 1 (unique). The ARC analysis should recognize `ori_list_alloc_data` output as unique and emit `cow_mode=1`. The second push correctly gets `cow_mode=1` because it recognizes `push_cow` output as unique. The analysis handles "push output is unique" but not "alloc output is unique". Source: `compiler/ori_arc/src/borrow/mod.rs` (ownership analysis) or `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_cow.rs` (cow_mode emission).

### What Works Well

- **Seamless zero-copy list slicing**: `ori_list_slice_take` produces a view, not a copy. SLICE_FLAG in cap correctly marks the result as non-owning.
- **SSO-aware string RC**: High-bit check correctly skips RC for small strings. "hel" (3 bytes) is SSO, "hello world" (11 bytes) is heap -- both handled correctly.
- **Chained push COW**: Second push uses `cow_mode=1` (static unique fast path). No unnecessary RC checks or copies for the second mutation.
- **Mixed collection type discrimination**: Three different collection types in one function, each with correct RC protocols. No cross-contamination.
- **String substring via runtime function**: Correctly delegates to `ori_str_substring` which handles SSO/heap distinction.
- **Iterator loop compilation**: Correct phi-based SSA for accumulator, proper iterator lifecycle (create -> loop -> drop).
- **Slice iteration**: `ori_iter_from_list` correctly handles slice data (interior pointer with SLICE_FLAG cap).
- **Post-use cleanup**: Every collection gets its RC decremented after use -- no leaks.
- **Single drop function**: Only one `_ori_drop$3` generated (for strings) -- correct deduplication (no lists own RC-managed elements in this journey).
- **Value-returning main**: Clean `trunc i64 to i32` for process exit code.

### Cross-Reference with Previous Journeys

| Finding | Journey 17 Status | Cross-Ref |
|---------|-------------------|-----------|
| C1-C4 | Not triggered (no closures, no payload Eq, no Option match, no list indexing) | -- |
| H1 | Not triggered (no recursive functions) | -- |
| H2 | Potential -- `_ori_main` calls non-nounwind iterator and string functions | CONFIRMED |
| H3 | `ori_iter_from_list`, `ori_iter_next`, `ori_iter_drop`, `ori_str_len` missing nounwind | CONFIRMED |
| M1 | 10,331 bytes prelude | CONFIRMED (15/15) |
| M3 | 4+ redundant `br label` blocks | CONFIRMED (15/15) |
| M5 | 16+ instances of `align 4` on i64 | CONFIRMED |
| M10 | `_ori_main` missing `nounwind` | CONFIRMED |
| M11 | 7 orphaned landing pads (worst count -- 3 collection types generate more) | CONFIRMED |
| M13 | Iterator element via Option-like construct | CONFIRMED |
| M15 | Redundant RC inc/dec pattern -- M20 is the same class for slices | CONFIRMED (generalized) |
| L1 | 24.4% canon expansion | CONFIRMED |
| L7 | 2 dead phi values at loop exit | CONFIRMED |

---

## Responsible Source Files

| Component | File |
|-----------|------|
| List slice (take) | `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs` |
| List push (COW) | `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_cow.rs` |
| String substring | `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/str_builtins.rs` |
| String SSO RC | `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` |
| Iterator codegen | `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs` |
| RC emission | `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` |
| COW mode analysis | `compiler/ori_arc/src/borrow/mod.rs` |
| Slice runtime | `compiler/ori_rt/src/list/slice.rs` |
| COW push runtime | `compiler/ori_rt/src/list/cow.rs` |
| String runtime | `compiler/ori_rt/src/string/` |
| Iterator runtime | `compiler/ori_rt/src/iter/` |
