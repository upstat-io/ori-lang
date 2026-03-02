# Journey 15: COW Map Operations — Results

**Date**: 2026-03-02
**Status**: PASS (both paths)
**Eval exit code**: 63 (correct)
**AOT exit code**: 63 (correct)

## Journey Code

```ori
@main () -> int = {
    let m = {"a": 10, "b": 20};
    let m = m.insert(key: "c", value: 30);
    let $size = m.length();

    let total = 0;
    for entry in m do {
        total += entry.1;
    };

    total + size
}
```

**Expected**: `size=3`, `total=10+20+30=60`, result `60+3=63`.

---

## Phase-by-Phase Analysis

### 1. Lexer

- Source: 481 bytes, 91 tokens, 0 errors
- Prelude: 10,331 bytes, 1,516 tokens, 0 errors (CONFIRMED M1)
- **Clean pass.** Map literal `{"a": 10, "b": 20}` tokenized correctly: `{`, string, `:`, integer, `,` ...

### 2. Parser

- User module: 1 function, 25 expressions, 0 errors
- Prelude: 9 functions, 39 traits, 46 expressions, 0 errors
- Map literal correctly parsed as "map literal" context (line 13 of trace)
- Method call `.insert(key:, value:)` and `.length()` correctly recognized
- `for entry in m do { ... }` parsed as "for loop" context
- **Clean pass.**

### 3. Type Checker

- Registration: 9 prelude functions + 1 user function, 0 impls
- Body checking complete with 0 errors
- Prelude import resolution: hash-first miss for generic builtins (`len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`), hash-first hit for value builtins (`compare`, `min`, `max`)
- Map types inferred correctly: `{str: int}` with `insert` returning same map type
- Tuple field access `entry.1` on `(str, int)` iteration entries resolved correctly
- **Clean pass.**

### 4. Canonicalizer

- User module: 25 source expressions -> 29 canon nodes (16% expansion)
- 6 constants, 0 decision trees
- Prelude: 46 expressions -> 46 canon nodes, 4 decision trees
- **Expansion ratio**: 16% is consistent with prior journeys (J7=11.1%, J10=11.4%, J12=11.4%) (CONFIRMED L1)

### 5. Eval Trace

- 52 trace lines covering full execution
- **Map construction** (CanId 4): `Map(CanMapEntryRange(0..2))` -- builds 2-entry map from string keys and int values
- **Insert** (CanId 9): `MethodCall(CanId(6), Name(insert), CanRange(0..2))` -- calls insert with key="c", value=30
- **Length** (CanId 12): `MethodCall(CanId(11), Name(length), CanRange(0..0))` -- returns 3
- **For loop** (CanId 24): iterates 3 times, each iteration:
  - Loads `entry` (CanId 19), accesses field `.1` (CanId 20), adds to `total` (CanId 21/22)
  - Three Add operations: `0+10=10`, `10+20=30`, `30+30=60`
- **Final** (CanId 27): `total(60) + size(3) = 63`
- **Clean pass.** All 3 iterations correctly executed.

### 6. ARC Trace

- `ori_llvm::codegen::type_registration`: Registers 7 user types (Ordering, enums, structs)
- `ori_llvm::codegen::function_compiler`: Declares `_ori_main` with Direct return passing, C calling convention
- Nounwind analysis: 1 pass, 0 nounwind functions, 0 mono propagated
- Entry point wrapper: `main()` generated with `has_args=false`, `returns_int=true`
- **No ARC-specific warnings or errors.**

### 7. LLVM Warnings

- No warnings emitted (file contains only compilation timing)
- **Clean.**

### 8. Build Output

- Compiled in 0.26s (LLVM IR dump), 0.28s (warnings), 0.26s (ARC trace)
- No errors or warnings in build_stdout/build_stderr
- **Clean build.**

### 9. Binary Analysis

- **Size**: 6,776,320 bytes (6.46 MB) -- consistent with prior journeys
- **Text section**: 965,689 bytes (943 KB)
- **Rodata**: 136,130 bytes
- **`_ori_main` size**: 0x545 = 1,349 bytes -- moderate for map+iterator+loop

---

## LLVM Deep Scrutiny (9 Categories)

### S1. Correctness

**Verdict**: CORRECT -- both paths produce 63.

The LLVM IR correctly implements:
1. Map literal construction via `ori_map_literal_alloc` + 2x `ori_map_literal_put`
2. COW insert via `ori_map_insert_cow` with `cow_mode=1` (unique owner hint)
3. Length extraction via `extractvalue` field 0 from map struct
4. Map iteration via `ori_iter_from_map` + `ori_iter_next` loop
5. Tuple field `.1` access via `extractvalue` on `{ {i64,i64,ptr}, i64 }` entry
6. Final addition of `total + size`

### S2. ARC / Reference Counting

**Map ARC lifecycle**:
- Map struct: `{ i64 len, i64 cap, ptr data }` -- same layout as list/set
- After `ori_map_insert_cow` returns the new map, the code does:
  1. `ori_rc_inc(data_ptr)` at bb1 line 82
  2. `ori_map_buffer_rc_dec(data_ptr, cap, len, ...)` at bb3 line 95 -- drops the OLD map reference
  3. `ori_rc_inc(data_ptr)` at bb3 line 97 -- for the iterator's ownership transfer

**Issue M15 (MEDIUM)**: **Redundant RC inc/dec pair around map consumption**. After insert returns `insert.val.s2`, the code does `rc_inc` (bb1:82), then immediately `rc_dec` (bb3:95) on the same data pointer with the same map dimensions, then does `rc_inc` again (bb3:97) for the iterator. The first `rc_inc`/`rc_dec` pair is a no-op that could be eliminated. This appears to be the ARC pipeline emitting separate RC operations for the rebinding `let m = ...` and the subsequent consumption by `.length()` and `for..in`, without noticing the intermediate operations cancel.

**String keys ARC**: The `_ori_elem_inc$3` / `_ori_elem_dec$3` functions correctly handle SSO (small string optimization) and null checks via the high-bit flag test before calling `ori_rc_inc`/`ori_rc_dec`. This is correct -- short strings like "a", "b", "c" will be SSO and skip RC operations entirely.

**val_inc is null**: In the `ori_map_insert_cow` call (line 68), `val_inc` is `null` because `int` values are scalars (no RC needed). Correct.

### S3. Alignment

**CONFIRMED M5**: `align 4` on i64 loads from struct fields throughout the IR:
- Line 22: `%str.val.f0 = load i64, ptr %str.val.f0.ptr, align 4`
- Line 25: `%str.val.f1 = load i64, ptr %str.val.f1.ptr, align 4`
- Line 42: `%map.cap = load i64, ptr %map.out_cap, align 4`
- Line 46: `store i64 10, ptr %map.val_tmp, align 4`
- Lines 120-130: Iterator element loads all use `align 4`

These should all be `align 8` for i64 values. LLVM may auto-correct in optimization, but it is technically incorrect metadata.

### S4. Dead Code / Unreachable Blocks

**CONFIRMED M3**: Unnecessary `br label` at bb0->bb1 (line 78) and bb8->bb7 (line 157).

**CONFIRMED M11**: Orphaned landing pads:
- `bb2` (line 86-89): `landingpad ... cleanup` with **no predecessors** -- dead code
- `bb4` (line 104-111): `landingpad ... cleanup` with **no predecessors** -- dead code, contains map RC dec cleanup that can never execute

Both landing pads are artifacts of the exception handling setup but have no invoke instructions targeting them.

### S5. Loop / Iterator Codegen

**For..in map iteration** follows the same pattern as list iteration (J10):
1. `ori_iter_from_map(data, cap, len, key_size, val_size, owns_data=true, key_dec, val_dec)` creates iterator
2. Loop: `ori_iter_next(iter, scratch_buf, 32)` returns `i8` (has_more)
3. Entry is loaded from scratch as `{ {i64,i64,ptr}, i64 }` -- tuple of (str key, int value)
4. Tag check: `icmp ne i64 %proj.0, 0` (Some vs None)
5. On Some: extract field `.1` via `extractvalue { {i64,i64,ptr}, i64 } %proj.1, 1` -- gets the int value
6. Accumulate: `add i64 %v18, %proj.136`
7. Loop exit: `ori_iter_drop(iter)` + final `ori_map_buffer_rc_dec` for map cleanup

**SSA phi nodes**: Correctly maintain `%v18` (total accumulator) and `%v19` (map struct, for cleanup). The map struct phi `%v19` is loop-invariant (always the same value) but needed for post-loop cleanup.

**Issue L7 (CONFIRMED)**: `%v20 = phi i64 [ 0, %bb8 ]` in bb7 (line 145) is a dead phi -- the value is never used. This was previously seen in J7.

### S6. String Handling

**String constants**: Three global string constants:
- `@str = private unnamed_addr constant [2 x i8] c"a\00"` (key "a")
- `@str.1 = private unnamed_addr constant [2 x i8] c"b\00"` (key "b")
- `@str.2 = private unnamed_addr constant [2 x i8] c"c\00"` (insert key "c")

Each is converted via `ori_str_from_raw(sret, ptr, len=1)` into `{i64, i64, ptr}` Ori strings. The per-field GEP+load+insertvalue pattern is used (correct for large structs in JIT per FastISel rules).

**Hash and equality thunks**: `ori_str_hash` and `ori_str_eq` function pointers are passed to map operations for key comparisons. Both are declared as external functions and passed by address.

### S7. Map-Specific Codegen

**Map literal construction** (lines 40-52):
1. `ori_map_literal_alloc(count=2, key_size=24, val_size=8, &out_cap)` -- allocates hash table
2. Two calls to `ori_map_literal_put(data, cap, key_tmp, val_tmp, key_size=24, val_size=8, hash_fn)` -- inserts "a":10 and "b":20
3. Result built via `insertvalue`: `{i64 2, i64 cap, ptr data}`

Key size 24 = `sizeof({i64, i64, ptr})` = Ori `str` layout. Value size 8 = `sizeof(i64)` = `int`. Both correct.

**COW insert** (line 68):
```
ori_map_insert_cow(data, len, cap, key_ptr, val_ptr, key_size=24, val_size=8,
                   key_eq=@ori_str_eq, key_hash=@ori_str_hash,
                   key_inc=@_ori_elem_inc$3, val_inc=null, cow_mode=1, sret)
```
- `cow_mode=1` indicates the caller believes the map is uniquely owned (can mutate in-place)
- `key_inc` is the SSO-aware string RC incrementer
- `val_inc=null` because int values need no RC
- Returns new map struct via sret pointer `%insert.out`

This is architecturally sound -- the COW semantics delegate to the runtime for the uniqueness check.

**Map layout**: `{ i64 len, i64 cap, ptr data }` -- same as list and set. The data pointer points to the hash table buffer managed by the runtime. The 3-field struct is passed/returned by value (24 bytes, fits in registers on x86-64).

### S8. Calling Convention / ABI

- `_ori_main` uses C calling convention (not fastcc) -- consistent with M10
- No `nounwind` attribute on `_ori_main` -- CONFIRMED M10
- Runtime function declarations have appropriate attributes:
  - `ori_rc_inc`: `nounwind memory(inaccessiblemem: readwrite)` -- correct
  - `ori_rc_dec`: `nounwind memory(inaccessiblemem: readwrite)` -- correct
  - `ori_rc_free`: `nounwind` -- correct
  - `_ori_elem_inc$3` / `_ori_elem_dec$3` / `_ori_drop$3`: `cold nounwind` -- correct
- `ori_map_insert_cow` uses `noalias` on data pointer and sret -- correct for COW semantics

**Sret pattern**: Map insert result is returned via `ptr noalias %insert.out` (sret), then loaded field-by-field via GEP+load+insertvalue. This avoids the FastISel aggregate load bug.

### S9. Code Quality / Optimization Opportunities

**CONFIRMED M5**: `align 4` on i64 loads (18+ instances in this IR)

**CONFIRMED M3**: Redundant `br label` blocks (bb0->bb1, bb8->bb7)

**CONFIRMED M11**: 2 orphaned landing pads (bb2, bb4)

**CONFIRMED L7**: Dead phi value `%v20` at loop exit

**NEW M15**: Redundant RC inc/dec pair on map data pointer (bb1:82 inc, bb3:95 dec, bb3:97 inc). The net effect is a single inc, but three runtime calls are emitted. The RC elimination pass should detect that the inc at bb1 and the dec at bb3 form a canceling pair when the data pointer is the same.

**Observation**: The `_ori_elem_inc$3` function loads all 3 fields of the string struct from the pointer, builds the struct value, then extracts field 2 (the data pointer) to check SSO/null. The load of fields 0 and 1 (lines 180-184) is dead -- only field 2 (the ptr) is used. This is a codegen inefficiency in the element inc/dec thunks. LLVM's optimizer should eliminate the dead loads, but the IR is unnecessarily verbose.

---

## Disassembly Analysis

**`_ori_main`**: 1,349 bytes (0x545). Stack frame: 0x220 = 544 bytes.

Key operations in native code:
1. Three `ori_str_from_raw` calls for string constants
2. `ori_map_literal_alloc` + two `ori_map_literal_put` calls for map construction
3. `ori_map_insert_cow` for the insert operation
4. Two `ori_rc_inc` calls (redundant pair + iterator ownership)
5. `ori_map_buffer_rc_dec` for old map cleanup
6. `ori_iter_from_map` to create iterator
7. Loop: `ori_iter_next` + accumulate + branch
8. `ori_map_buffer_rc_dec` + `ori_iter_drop` for final cleanup
9. `add %rcx, %rax` for final `total + size`

**`_ori_elem_inc$3`**: 63 bytes. SSO check uses `movabs $0x8000000000000000` for high-bit test, then `or` with null check. If neither SSO nor null, calls `ori_rc_inc`.

**`_ori_elem_dec$3`**: 70 bytes. Same SSO/null check pattern, calls `ori_rc_dec` with drop function pointer.

**`_ori_drop$3`**: 18 bytes. Simple `ori_rc_free(ptr, size=24, align=8)`.

---

## Findings Summary

| ID | Severity | Description | Status |
|----|----------|-------------|--------|
| M1 | MEDIUM | Prelude 10,331 bytes overhead | CONFIRMED (13/13) |
| M3 | MEDIUM | Unnecessary `br label` after function calls | CONFIRMED (13/13) |
| M5 | MEDIUM | `align 4` on i64 loads -- should be `align 8` | CONFIRMED |
| M10 | MEDIUM | `_ori_main` missing `nounwind` attribute | CONFIRMED |
| M11 | MEDIUM | Orphaned landing pads (2 in this journey) | CONFIRMED |
| M15 | MEDIUM | Redundant RC inc/dec pair on map data after insert | NEW |
| L1 | LOW | Canon expansion 16% | CONFIRMED |
| L7 | LOW | Dead phi value at loop exit | CONFIRMED |

### New Findings

**M15 (MEDIUM)**: Redundant RC inc/dec pair on map data pointer between insert return and iterator creation. Three RC calls emitted where one would suffice. The ARC pipeline's RC elimination pass does not recognize the canceling pattern across the block boundary (bb1 inc, bb3 dec). Source: `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` (RC emission), `compiler/ori_arc/src/rc_elim/mod.rs` (elimination pass).

### What Works Well

- **Map literal construction**: Clean `alloc` + `put` pattern -- no unnecessary intermediaries
- **COW insert**: Full COW protocol with `cow_mode=1` (unique owner), correct function pointer passing for key eq/hash/inc
- **Map iteration**: Correct `ori_iter_from_map` with ownership transfer (`owns_data=true`), proper `ori_iter_drop` cleanup
- **Tuple entry access**: `entry.1` compiles to `extractvalue` on the `{ {str}, int }` tuple -- zero-cost field access
- **SSO-aware RC**: Element inc/dec thunks correctly skip RC for SSO strings and null pointers
- **String key handling**: Correct 24-byte key size, proper `ori_str_hash` / `ori_str_eq` thunks
- **Val_inc null for scalars**: Correctly passes null for int value type (no RC needed)
- **Post-loop cleanup**: Both map buffer and iterator properly cleaned up via `ori_map_buffer_rc_dec` + `ori_iter_drop`
- **Map struct layout**: `{i64 len, i64 cap, ptr data}` is consistent with list/set -- unified collection representation

### Cross-Reference with Previous Journeys

| Finding | Journey 15 Status | Cross-Ref |
|---------|-------------------|-----------|
| C1-C4 | Not triggered (no closures, no payload Eq, no Option match, no list indexing) | -- |
| H1 | Not triggered (no recursive functions in user code) | -- |
| H2 | Potential -- `_ori_main` calls `ori_iter_next` which may throw, but not marked nounwind | CONFIRMED |
| M1 | 10,331 bytes prelude | CONFIRMED (13/13) |
| M3 | 2 redundant `br label` blocks | CONFIRMED (13/13) |
| M5 | 18+ instances of `align 4` on i64 | CONFIRMED |
| M10 | `_ori_main` missing `nounwind` | CONFIRMED |
| M11 | 2 orphaned landing pads | CONFIRMED |
| M13 | Iterator element loaded via Option-like construct -- same pattern as J10 | CONFIRMED |
| L1 | 16% canon expansion | CONFIRMED |
| L7 | Dead phi `%v20` | CONFIRMED |

---

## Responsible Source Files

| Component | File |
|-----------|------|
| Map literal construction | `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs` (CtorKind::MapLiteral) |
| Map insert (COW) | `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/map_builtins.rs` (emit_map_insert) |
| Map iteration | `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/map_builtins.rs` (emit_map_iter) |
| Map length | `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/map_builtins.rs` (emit_map_length) |
| Element inc/dec thunks | `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` |
| RC emission | `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` |
| RC elimination | `compiler/ori_arc/src/rc_elim/mod.rs` |
| For-loop iteration | `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs` |
| Runtime map functions | `compiler/ori_rt/src/map/` |
| Runtime iterator | `compiler/ori_rt/src/iter/` |
