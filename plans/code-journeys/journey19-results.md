# Journey 19: COW Comprehensive Stress Test -- Results

**Date**: 2026-03-02
**Status**: PASS (both paths)
**Eval exit code**: 28 (correct)
**AOT exit code**: 28 (correct)

## Journey Code

```ori
@main () -> int = {
    // List COW: build, share, mutate copy, verify original
    let $base = [1, 2, 3, 4, 5];
    let extended = base;
    let extended = extended.push(6);
    let $list_result = base.length() + extended.length();
    // base.length() = 5, extended.length() = 6
    // list_result = 11

    // String COW: short (SSO) + long (heap), share + concat
    let $word = "hey";
    let $sentence = word + " there";
    let $str_result = word.length() + sentence.length();
    // 3 + 9 = 12

    // Map COW: create, insert, length
    let $scores = {"x": 1, "y": 2};
    let $scores2 = scores.insert(key: "z", value: 3);
    let $map_result = scores.length() + scores2.length();
    // 2 + 3 = 5

    list_result + str_result + map_result
}
```

**Expected**: `list_result(11) + str_result(12) + map_result(5) = 28`

---

## Phase-by-Phase Analysis

### 1. Lexer

- Source: 1,028 bytes, 168 tokens, 0 errors
- Prelude: 10,331 bytes, 1,516 tokens, 0 errors (CONFIRMED M1, 16th journey)
- All three collection literal syntaxes tokenized cleanly: `[1,2,3,4,5]`, `"hey"`, `{"x": 1, "y": 2}`
- Named arguments `key:`, `value:` tokenized correctly
- **Clean pass.**

### 2. Parser

- User module: 1 function, 44 expressions, 0 errors, 0 warnings
- Prelude: 9 functions, 39 traits, 46 expressions, 4 decision trees, 0 errors
- 44 expressions is the highest single-function count in any journey -- reflects the three distinct collection sections
- List literal `[1,2,3,4,5]` parsed correctly (5 integer elements)
- Map literal `{"x": 1, "y": 2}` parsed as "map literal" context with 2 entries
- Method calls: `.push(6)`, `.length()`, `.insert(key:, value:)` all correctly parsed
- String concatenation `word + " there"` parsed as binary add
- **Clean pass.**

### 3. Type Checker

- Registration: 9 prelude functions + 1 user function, 0 impls, 0 tests
- Signature collection + body checking: complete with 0 errors
- Prelude import resolution: hash-first miss for generics (`len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`), hash-first hit for value builtins (`compare`, `min`, `max`)
- Correctly infers types across all three collection sections:
  - `base: [int]`, `extended: [int]` (after `.push()`)
  - `word: str`, `sentence: str` (after `+`)
  - `scores: {str: int}`, `scores2: {str: int}` (after `.insert()`)
  - `list_result: int`, `str_result: int`, `map_result: int`
- Immutable `$base`, `$word`, `$scores`, `$list_result`, `$str_result`, `$map_result`, `$scores2`, `$sentence` correctly distinguished from mutable `extended`
- **Clean pass.**

### 4. Canonicalizer

- User module: 44 source expressions -> 54 canon nodes (22.7% expansion)
- 6 constants, 0 decision trees
- Prelude: 46 expressions -> 46 canon nodes, 4 decision trees
- **Expansion ratio**: 22.7% is at the high end of the L1 range (0-25%), reflecting the complexity of three collection types with method calls and sharing operations in a single function. (CONFIRMED L1)

### 5. Eval Trace

69 trace lines covering full execution across all three collection phases:

**List phase** (CanId 0-18):
1. `CanId(5)`: `List(CanRange(0..5))` -- builds [1,2,3,4,5]
2. `CanId(6)`: `Let(Immutable)` -- binds `$base`
3. `CanId(8)`: `Let(Mutable)` -- binds `extended = base` (creates shared reference)
4. `CanId(11)`: `MethodCall(push)` on `extended` with arg `6` -- COW push (clones because shared)
5. `CanId(12)`: `Let(Mutable)` -- rebinds `extended` to push result
6. `CanId(14)`: `MethodCall(length)` on `base` -> 5
7. `CanId(16)`: `MethodCall(length)` on `extended` -> 6
8. `CanId(17)`: `Binary(Add)` `int + int` -> 11

**String phase** (CanId 19-30):
1. `CanId(19)`: `Str("hey")` -- SSO string (3 bytes, fits inline)
2. `CanId(20)`: `Let(Immutable)` -- binds `$word`
3. `CanId(21)`: Loads `word`, `CanId(22)`: Loads `" there"`
4. `CanId(23)`: `Binary(Add)` `str + str` -> `"hey there"` (9 bytes, heap-allocated)
5. `CanId(24)`: `Let(Immutable)` -- binds `$sentence`
6. `CanId(26)`: `MethodCall(length)` on `word` -> 3
7. `CanId(28)`: `MethodCall(length)` on `sentence` -> 9
8. `CanId(29)`: `Binary(Add)` `int + int` -> 12

**Map phase** (CanId 31-47):
1. `CanId(35)`: `Map(CanMapEntryRange(0..2))` -- builds {"x": 1, "y": 2}
2. `CanId(36)`: `Let(Immutable)` -- binds `$scores`
3. `CanId(40)`: `MethodCall(insert, key="z", value=3)` on `scores` -- COW insert
4. `CanId(41)`: `Let(Immutable)` -- binds `$scores2`
5. `CanId(43)`: `MethodCall(length)` on `scores` -> 2
6. `CanId(45)`: `MethodCall(length)` on `scores2` -> 3
7. `CanId(46)`: `Binary(Add)` `int + int` -> 5

**Final sum** (CanId 48-52):
1. `CanId(48)`: Loads `list_result` (11)
2. `CanId(49)`: Loads `str_result` (12)
3. `CanId(50)`: `Binary(Add)` `11 + 12 = 23`
4. `CanId(51)`: Loads `map_result` (5)
5. `CanId(52)`: `Binary(Add)` `23 + 5 = 28`

**All binary Add operations confirmed**: `int + int` for all arithmetic, `str + str` for concatenation. 5 addition operations total (3 section sums + 2 cross-section additions).

**Clean pass.** All three collection types evaluated correctly with no interference.

### 6. ARC Trace

- `ori_llvm::codegen::type_registration`: Registers 7 user types (Ordering, enums, PanicInfo, TraceEntry, FormatSpec, Sign, FormatType)
- `ori_llvm::codegen::function_compiler`: Declares `_ori_main` with `Direct` return passing, `C` calling convention, 0 parameters
- Nounwind analysis: 1 pass, 0 nounwind functions, 0 mono propagated -- function calls `ori_list_push_cow`, `ori_str_concat`, `ori_map_insert_cow` (all may throw)
- Entry point wrapper: `main()` generated with `has_args=false`, `returns_int=true`
- **No ARC-specific warnings or errors.**

### 7. LLVM Warnings

- No warnings emitted. File contains only compilation timing line.
- **Clean.**

### 8. Build Output

- build_stdout: Empty (clean)
- build_stderr: Empty (clean)
- Compiled in 0.26s
- **Clean build.**

### 9. Binary Analysis

- **Size**: 6,730,472 bytes (6.42 MB) -- consistent with prior journeys
- **Text section**: 951,113 bytes (929 KB)
- **Rodata**: 136,044 bytes
- **`_ori_main`**: starts at `0x1eb00`, ends at `0x1f273` = 1,907 bytes -- the largest `_ori_main` of any journey, reflecting three collection types in one function
- **Stack frame**: `sub $0x268, %rsp` = 616 bytes -- significant, handling list triple, push output, 3 string allocas, 2 string concat operands, map key/val temps, insert output, plus all the spills
- **Helper functions**:
  - `_ori_drop$3` at `0x1f280` (18 bytes): `ori_rc_free(ptr, 24, 8)` -- string/map data drop
  - `_ori_elem_inc$3` at `0x1f2a0` (63 bytes): SSO-aware RC inc for str map keys
  - `_ori_elem_dec$3` at `0x1f2e0` (70 bytes): SSO-aware RC dec for str map keys
  - `main` wrapper at `0x1f330` (9 bytes): calls `_ori_main`, truncates to i32

---

## LLVM Deep Scrutiny (9 Categories)

### S1. Correctness

**Verdict**: CORRECT -- both paths produce 28.

The LLVM IR correctly implements all three collection phases:

1. **List COW**: `ori_list_alloc_data(5, 8)` allocates, elements stored via GEP, `ori_list_rc_inc` for sharing, `ori_list_push_cow` with `cow_mode=0` for dynamic COW check, length via `extractvalue` field 0
2. **String COW**: `ori_str_from_raw` for literals "hey" and " there", `ori_rc_inc` for SSO-aware sharing of `word`, `ori_str_concat` for concatenation, `ori_str_len` for length extraction
3. **Map COW**: `ori_map_literal_alloc(2, 24, 8)` + 2x `ori_map_literal_put` for literal, `ori_rc_inc` for sharing, `ori_map_insert_cow` with `cow_mode=0` for dynamic COW, length via `extractvalue` field 0

All three lengths are correctly extracted, summed, and the final `add i64 %add121, %add120` produces 28 which is returned as the exit code.

### S2. ARC / Reference Counting

**CRITICAL FOCUS: Cross-collection ARC correctness**

This journey is unique in exercising all three ARC-managed collection types (`[int]`, `str`, `{str: int}`) in a single function. The key concern is whether RC operations for one type interfere with another.

**Analysis of RC operations by type**:

**List RC operations** (lines 42-95):
| Location | Operation | Target | Purpose |
|----------|-----------|--------|---------|
| bb0:44 | `ori_list_rc_inc(data, cap)` | `%list.2` | Sharing: `extended = base` (RC 1->2) |
| bb0:49 | `ori_list_push_cow(...)` | `%list.2` fields | COW push consumes one ref |
| bb1:62 | `extractvalue` len | `%list.2` | Get `base.length()` = 5 |
| bb3:78 | `ori_buffer_rc_dec(data, len, cap, 8, null)` | `%list.2` | Drop `base` reference |
| bb3:79 | `extractvalue` len | `%push.val.s2` | Get `extended.length()` = 6 |
| bb5:95 | `ori_buffer_rc_dec(data, len, cap, 8, null)` | `%push.val.s2` | Drop `extended` reference |

**List RC balance**: The `ori_list_rc_inc` at bb0:44 increments for sharing. The `ori_list_push_cow` decrements the old buffer's RC internally (slow path). After push, `base` has RC=1 (one reference remaining). At bb3:78, `ori_buffer_rc_dec` drops `base` (RC 1->0, freed). The `push.val.s2` (extended) starts at RC=1 from push's allocation. At bb5:95, `ori_buffer_rc_dec` drops `extended` (RC 1->0, freed). **Balanced.**

Note: `ori_buffer_rc_dec` with `elem_dec=null` is correct for `[int]` -- int elements have no inner RC.

**String RC operations** (lines 97-283):
| Location | Operation | Target | Purpose |
|----------|-----------|--------|---------|
| bb5:107 | `ori_rc_inc` (SSO-guarded) | `%str.val.s2` ("hey") | Sharing: `word` used in concat |
| rc_inc.sso_skip:220 | `ori_str_concat(...)` | consumes both operands | Creates "hey there" |
| rc_dec.sso_skip:230-251 | `ori_rc_dec` (SSO-guarded) | `%str.val.s228` (" there") | Drop temp string |
| rc_dec.sso_skip:244-251 | `ori_rc_dec` (SSO-guarded) | `%str.val.s2` ("hey") | Drop original after concat |
| rc_dec.sso_skip31:258-260 | `ori_str_len(self)` | `%str.val.s2` ("hey") | Get `word.length()` = 3 |
| bb7:122-129 | `ori_rc_dec` (SSO-guarded) | `%str.val.s2` ("hey") | Drop `word` after length |
| rc_dec.sso_skip50:274-276 | `ori_str_len(self)` | `%ori_str_concat.s2` ("hey there") | Get `sentence.length()` = 9 |
| bb9:144-151 | `ori_rc_dec` (SSO-guarded) | `%ori_str_concat.s2` ("hey there") | Drop `sentence` after length |

**String SSO guard pattern**: Every string RC inc/dec is guarded by the SSO check:
```llvm
%rc_inc.p2i = ptrtoint ptr %data to i64
%rc_inc.sso_flag = and i64 %rc_inc.p2i, -9223372036854775808  ; 0x8000000000000000
%rc_inc.is_sso = icmp ne i64 %rc_inc.sso_flag, 0
%rc_inc.null = icmp eq i64 %rc_inc.p2i, 0
%rc_inc.skip_rc = or i1 %is_sso, %null
br i1 %skip_rc, label %skip, label %heap
```
This is correct: SSO strings have the high bit set in the data pointer field, and null pointers are skipped. For "hey" (3 bytes), this will be SSO-encoded and skip RC operations entirely. For "hey there" (9 bytes), this may be heap-allocated and require RC.

**String RC balance**: `ori_rc_inc` at bb5:107 increments `word` for use in concat. After concat, `word` is decremented at rc_dec.sso_skip (line 244-251). Then `word` is used for `.length()` at rc_dec.sso_skip31:258 but the string is materialized on the stack for the `ori_str_len` call. After that, `word` is decremented again at bb7 (line 122-129).

**Issue M20 (MEDIUM)**: **Double RC dec on `word` string ("hey")**. The IR decrements `word`'s RC at rc_dec.sso_skip (line 244-251), then reads `word.length()`, then decrements again at bb7 (line 122-129). If "hey" were a heap-allocated string (not SSO), the first decrement could free it, and the subsequent `ori_str_len` call would be a use-after-free. However, "hey" (3 bytes) IS SSO, so both decrements are skipped (SSO guard catches them). **This is a latent use-after-free bug that is masked by SSO for short strings.** A longer string (>23 bytes) in the same position would trigger the bug.

Let me verify more carefully. Looking at the control flow:
- After concat, at rc_dec.sso_skip (line 243): This decrements `%str.val.s228` (the " there" literal), NOT `word`.
- Then at rc_dec.sso_skip31 (line 257): This is after decrementing the sharing copy of `word` (`%str.val.s2`). But wait -- the rc_inc at bb5:107 incremented `word` for sharing. After concat consumes one reference, the dec at line 244 drops the " there" temporary, and the dec at line 250 drops the sharing copy of `word`. So after rc_dec.sso_skip31, `word` still has its original RC=1 (or is SSO). Then `ori_str_len` reads from it. Then at bb7:122, `word` is decremented again (final drop). The net is: +1 (inc for sharing) -1 (after concat) -1 (after length) = -1 net. Combined with the initial creation (implicit RC=1), the total is 1-1=0. **Actually correct.** The inc at bb5:107 establishes a second reference, one dec returns it after concat, another dec is the final cleanup. I retract the M20 concern.

Let me re-trace more carefully:
1. `ori_str_from_raw("hey")` creates `word` at RC=1 (or SSO)
2. `ori_rc_inc(word.data)` at bb5:107 -> RC=2 (or skipped for SSO)
3. `ori_str_concat(word, " there")` consumes logical references. The concat function does NOT consume ownership -- it takes pointer-to-struct args
4. `ori_rc_dec(" there".data)` at rc_dec.sso_skip -> drops temp
5. `ori_rc_dec(word.data)` at rc_dec.sso_skip31 -> RC 2->1 (drops the sharing copy)
6. `ori_str_len(word)` reads from `word` which is still alive at RC=1 -- SAFE
7. `ori_rc_dec(word.data)` at bb7 -> RC 1->0, freed -- final drop
8. `ori_str_len(sentence)` reads from `sentence` (concat result) -- separate RC chain
9. `ori_rc_dec(sentence.data)` at bb9 -> final drop of sentence

**String RC balance**: CORRECT. Every string is properly cleaned up. No use-after-free.

**Map RC operations** (lines 305-344, bb11-bb15):
| Location | Operation | Target | Purpose |
|----------|-----------|--------|---------|
| rc_dec.sso_skip62:284 | `ori_str_from_raw("x")`, `ori_str_from_raw("y")` | map keys | Key creation |
| rc_dec.sso_skip62:305 | `ori_map_literal_alloc(2, 24, 8)` | map data | Allocate hash table |
| rc_dec.sso_skip62:311,314 | `ori_map_literal_put` x2 | map entries | Insert "x":1, "y":2 |
| rc_dec.sso_skip62:318 | `ori_rc_inc(map.data)` | `%map.2` | Sharing: `scores` used in insert |
| rc_dec.sso_skip62:334 | `ori_map_insert_cow(...)` | `%map.2` fields | COW insert "z":3 |
| bb11:159 | `extractvalue` len | `%map.2` | Get `scores.length()` = 2 |
| bb13:175 | `ori_map_buffer_rc_dec(data, cap, len, 24, 8, @elem_dec, null)` | `%map.2` | Drop `scores` |
| bb13:176 | `extractvalue` len | `%insert.val.s2` | Get `scores2.length()` = 3 |
| bb15:192 | `ori_map_buffer_rc_dec(data, cap, len, 24, 8, @elem_dec, null)` | `%insert.val.s2` | Drop `scores2` |

**Map RC balance**: `ori_rc_inc` at line 318 increments for sharing before insert. `ori_map_insert_cow` with `cow_mode=0` checks uniqueness -- finds RC=2 (shared), takes slow path: copies map, inserts "z":3 into copy, decrements old map RC (back to 1). At bb13:175, `ori_map_buffer_rc_dec` drops `scores` (RC 1->0, freed). At bb15:192, `ori_map_buffer_rc_dec` drops `scores2` (RC 1->0, freed). **Balanced.**

**Map element cleanup**: `ori_map_buffer_rc_dec` is called with `elem_dec=@_ori_elem_dec$3` -- the SSO-aware string key decrementer. This means when the map buffer is freed, each key string's RC is decremented. Value (`int`) has no cleanup (`val_dec=null`). Correct.

**Cross-collection interference analysis**: NO interference detected. Each collection type uses its own set of RC functions:
- Lists: `ori_list_rc_inc` / `ori_buffer_rc_dec` with `elem_dec=null` (scalar elements)
- Strings: `ori_rc_inc` / `ori_rc_dec` with SSO guards (no elem callbacks)
- Maps: `ori_rc_inc` / `ori_map_buffer_rc_dec` with `elem_dec=@_ori_elem_dec$3` (string key cleanup)

The RC operations for different types are completely independent -- no shared state, no cross-type callbacks. This confirms that the ARC system correctly discriminates between collection types.

### S3. Alignment

**CONFIRMED M5**: `align 4` on i64 operations throughout. Instances in this IR:

- Line 32: `store i64 1, ptr %list.elem_ptr, align 4` (should be `align 8`)
- Line 34: `store i64 2, ptr %list.elem_ptr1, align 4`
- Line 48: `store i64 6, ptr %push.elem, align 4`
- Lines 51, 54, 57: Push output field loads `align 4`
- Lines 99, 102: String struct field loads `align 4`
- Lines 210, 213: String struct field loads `align 4`
- Lines 306: `%map.cap = load i64, ptr %map.out_cap, align 4`
- Lines 310: `store i64 1, ptr %map.val_tmp, align 4`
- Lines 336, 339: Insert output field loads `align 4`

18+ instances. All should be `align 8` for i64 values. LLVM's optimizer auto-corrects in most cases, but the metadata is incorrect.

### S4. Dead Code / Unreachable Blocks

**CONFIRMED M3**: Unnecessary `br label` at:
- bb0 -> bb1 (line 59): Unconditional branch to sequentially-next block
- bb1 -> bb3 (line 63): Same pattern
- bb3 -> bb5 (line 80): Same pattern

**CONFIRMED M11**: Orphaned landing pads with no predecessors:
- `bb2` (lines 65-72): List cleanup for `%list.2` -- no `invoke` targets it
- `bb4` (lines 82-89): List cleanup for `%push.val.s2` -- no `invoke` targets it
- `bb6` (lines 116-119): Empty cleanup -- just `resume`
- `bb8` (lines 131-141): String cleanup for `%ori_str_concat.s2` -- no `invoke` targets it
- `bb10` (lines 153-156): Empty cleanup -- just `resume`
- `bb12` (lines 162-169): Map cleanup for `%map.2` with `ori_map_buffer_rc_dec` -- no `invoke` targets it
- `bb14` (lines 179-186): Map cleanup for `%insert.val.s2` with `ori_map_buffer_rc_dec` -- no `invoke` targets it
- `bb16` (lines 198-201): Empty cleanup -- just `resume`

**8 orphaned landing pads** -- the highest count of any journey. This is because the function has 3 collection types, each needing cleanup at multiple lifecycle points. All use `call` (not `invoke`), so none of these landing pads can ever execute.

### S5. Loop / Iterator Codegen

No `for..in` loops in this journey. All collection operations are method calls (`.push()`, `.length()`, `.insert()`). The loop-like iteration pattern is not exercised here -- that was covered in J13 (list iteration), J15 (map iteration), and J16 (dual list iteration).

### S6. String Handling

**String constants**: 5 global string constants:
- `@str = [4 x i8] c"hey\00"` (3 bytes + null)
- `@str.1 = [7 x i8] c" there\00"` (6 bytes + null)
- `@str.2 = [2 x i8] c"x\00"` (1 byte + null, map key)
- `@str.3 = [2 x i8] c"y\00"` (1 byte + null, map key)
- `@str.4 = [2 x i8] c"z\00"` (1 byte + null, insert key)

All are converted via `ori_str_from_raw(sret, ptr, len)` into `{i64, i64, ptr}` Ori strings using the per-field GEP+load+insertvalue pattern (correct for JIT FastISel compatibility).

**SSO behavior analysis**:
- "hey" (3 bytes): SSO-eligible. The data pointer will have the high bit set, so all RC operations on it are no-ops (SSO guard skips).
- " there" (6 bytes): SSO-eligible. Same SSO guard applies.
- "hey there" (9 bytes): May be SSO-eligible (SSO capacity is typically 23 bytes). If so, the concat result is also SSO.
- "x", "y", "z" (1 byte each): SSO-eligible. All map key strings avoid RC overhead entirely.

**SSO-aware string concat**: `ori_str_concat(sret, lhs_ptr, rhs_ptr)` takes both operands by pointer, creates the concatenated result. The caller is responsible for RC management of the operands before and after the call.

**String `.length()` codegen**: `ori_str_len(ptr)` is called with the string struct stored in a stack alloca. This requires materializing the full 24-byte struct on the stack for each length call. Unlike list `.length()` which compiles to `extractvalue` (zero-cost field extraction), string length calls a runtime function because the length may need to be extracted differently for SSO vs heap strings. This is a design difference, not a bug.

### S7. Map-Specific Codegen

**Map literal construction** (lines 284-316):
1. `ori_map_literal_alloc(count=2, key_size=24, val_size=8, &out_cap)` allocates hash table
2. Two calls to `ori_map_literal_put(data, cap, key_tmp, val_tmp, 24, 8, @ori_str_hash)` for "x":1 and "y":2
3. Map struct built: `{i64 2, i64 cap, ptr data}`

**COW insert** (line 334):
```
ori_map_insert_cow(data, len, cap, key_ptr, val_ptr,
    key_size=24, val_size=8,
    key_eq=@ori_str_eq, key_hash=@ori_str_hash,
    key_inc=@_ori_elem_inc$3, val_inc=null,
    cow_mode=0, sret)
```
- `cow_mode=0`: Dynamic uniqueness check (runtime calls `ori_rc_is_unique`)
- `key_inc=@_ori_elem_inc$3`: SSO-aware string key RC incrementer (needed to clone keys during COW copy)
- `val_inc=null`: Int values have no RC (scalar)
- `key_eq=@ori_str_eq` and `key_hash=@ori_str_hash`: Function pointers for hash table operations

**Element inc/dec thunks** (`_ori_elem_inc$3` at line 395, `_ori_elem_dec$3` at line 425):

Both load the string struct fields from the pointer argument, extract the data pointer (field 2), check the SSO high-bit and null conditions, and conditionally call `ori_rc_inc` or `ori_rc_dec`. The loads of fields 0 and 1 are dead code (only field 2 is used for the SSO check) -- the optimizer should eliminate them, but the IR is verbose.

**Map length codegen**: `extractvalue { i64, i64, ptr } %map.2, 0` -- zero-cost field extraction, same as list length. No runtime function call needed (unlike string length).

### S8. Calling Convention / ABI

- `_ori_main` uses C calling convention (not fastcc) -- CONFIRMED M10
- No `nounwind` attribute on `_ori_main` -- CONFIRMED M10
- Nounwind analysis: 0 nounwind functions (correct -- `_ori_main` calls `ori_list_push_cow`, `ori_str_concat`, `ori_map_insert_cow`, all of which may throw)
- Runtime function attributes:
  - `ori_rc_inc`: `nounwind memory(inaccessiblemem: readwrite)` -- correct
  - `ori_rc_dec`: `nounwind memory(inaccessiblemem: readwrite)` -- correct
  - `ori_list_rc_inc`: `nounwind memory(inaccessiblemem: readwrite)` -- correct
  - `ori_buffer_rc_dec`: `nounwind memory(inaccessiblemem: readwrite)` -- correct
  - `_ori_drop$3` / `_ori_elem_inc$3` / `_ori_elem_dec$3`: `cold nounwind` -- correct
- `ori_map_insert_cow` uses `noalias` on sret pointer -- correct for COW
- `ori_list_push_cow` uses `noalias` on sret pointer -- correct

**Entry point wrapper** (lines 455-460):
```llvm
define i32 @main() {
    %ori_main_result = call i64 @_ori_main()
    %exit_code = trunc i64 %ori_main_result to i32
    ret i32 %exit_code
}
```
Correctly truncates i64 to i32 for process exit code. `28 & 0xFF = 28` (fits in range).

### S9. Code Quality / Optimization Opportunities

**CONFIRMED M5**: `align 4` on i64 loads (18+ instances)

**CONFIRMED M3**: 3+ redundant `br label` blocks

**CONFIRMED M11**: 8 orphaned landing pads -- highest count yet (previous max was ~3)

**CONFIRMED M7**: Verbose `alloca+store+GEP+load+insertvalue` pattern for string struct construction (6 instances in this IR for the 5 string constants + 1 concat result). Each requires 6 IR instructions to materialize a 3-field struct.

**NEW M17 (CONFIRMED from J14)**: **Redundant SSO check sequences**. Each string RC operation (inc or dec) repeats the full 5-instruction SSO+null check pattern:
```llvm
%p2i = ptrtoint ptr %data to i64
%sso_flag = and i64 %p2i, -9223372036854775808
%is_sso = icmp ne i64 %sso_flag, 0
%null = icmp eq i64 %p2i, 0
%skip_rc = or i1 %is_sso, %null
```
With 6 string RC operations in this function, this pattern is emitted 6 times. The SSO/null status of a string does not change during its lifetime, so multiple checks on the same string value are redundant. A CSE-like optimization in the ARC pipeline could hoist the check result.

**Observation**: The `_ori_drop$3` function uses `ori_rc_free(ptr, 24, 8)` for both string drops and map buffer drops (since both use the same ARC header layout). This is a universal drop function parameterized by size and alignment, not type-discriminated. It works because all ARC-managed heap objects have the same header format.

**Observation**: `_ori_elem_inc$3` and `_ori_elem_dec$3` load all 3 fields of the string struct from the pointer but only use field 2 (the data pointer). Fields 0 and 1 are dead loads. LLVM should optimize these away, but the IR is 12 instructions per thunk that could be 6.

---

## Disassembly Analysis

**`_ori_main`**: 1,907 bytes at `0x1eb00`-`0x1f273`. Stack frame: `sub $0x268 = 616` bytes.

This is the largest `_ori_main` of any journey, reflecting three collection types:

**List section** (0x1eb00-0x1ec45):
- `ori_list_alloc_data(5, 8)` at 0x1eb2c
- 5x `movq $N, offset(%rdi)` for element stores (direct memory writes, efficient)
- `ori_list_rc_inc` at 0x1eb7d
- `ori_list_push_cow` at 0x1ebcf (stack manipulation for 10-arg call)
- 2x `ori_buffer_rc_dec` at 0x1ec21 and 0x1ec45

**String section** (0x1ec62-0x1efd2):
- `ori_str_from_raw("hey", 3)` at 0x1ec75
- SSO check at 0x1eca4 (`movabs $0x8000000000000000`)
- `ori_str_from_raw(" there", 6)` at 0x1ee08
- `ori_str_concat` at 0x1ee7d
- 4x SSO-guarded `ori_rc_dec` sequences with `_ori_drop$3` function pointer
- 2x `ori_str_len` calls at 0x1ef59 and 0x1efae

**Map section** (0x1eff0-0x1f26e):
- 2x `ori_str_from_raw` for keys "x", "y"
- `ori_map_literal_alloc(2, 24, 8)` at 0x1f086
- 2x `ori_map_literal_put` with `ori_str_hash` function pointer
- `ori_rc_inc` for map sharing at 0x1f19e
- `ori_str_from_raw("z")` for insert key
- `ori_map_insert_cow` at 0x1f244 with full 13-arg call (stack-heavy)
- 2x `ori_map_buffer_rc_dec` with `_ori_elem_dec$3` function pointer

**Final sum** (0x1edc6-0x1edda):
```asm
add    %rsi,%rcx     ; map_result = scores.length() + scores2.length()
add    %rdx,%rax     ; list_result + str_result (already computed)
add    %rcx,%rax     ; final = (list_result + str_result) + map_result
```

**Helper functions**:
- `_ori_drop$3` (18 bytes): Minimal -- just `ori_rc_free(ptr, 24, 8)` + ret
- `_ori_elem_inc$3` (63 bytes): Load field 2, SSO/null check, conditional `ori_rc_inc`
- `_ori_elem_dec$3` (70 bytes): Load field 2, SSO/null check, conditional `ori_rc_dec` with `_ori_drop$3`
- `main` (9 bytes): `push %rax; call _ori_main; pop %rcx; ret`

---

## Cross-Collection Type Discrimination Analysis

**CRITICAL FOCUS**: This journey validates that the compiler correctly discriminates between collection types in drop functions and cleanup paths.

**Drop function dispatch**:
| Collection | Drop mechanism | Element cleanup | Correct? |
|-----------|---------------|-----------------|----------|
| `[int]` | `ori_buffer_rc_dec(ptr, len, cap, elem_size=8, elem_dec=null)` | None (scalar int) | YES |
| `str` | `ori_rc_dec(ptr, drop_fn=@_ori_drop$3)` with SSO guard | N/A (atomic type) | YES |
| `{str: int}` | `ori_map_buffer_rc_dec(ptr, cap, len, key_size=24, val_size=8, elem_dec=@_ori_elem_dec$3, val_dec=null)` | Key: SSO-aware str dec; Val: none (scalar) | YES |

**Key observation**: The compiler uses DIFFERENT drop functions for each collection type:
1. **Lists**: `ori_buffer_rc_dec` -- generic buffer RC decrement with optional per-element cleanup
2. **Strings**: `ori_rc_dec` with SSO guard -- string-specific RC with SSO awareness
3. **Maps**: `ori_map_buffer_rc_dec` -- map-specific buffer RC with both key and value element cleanup callbacks

This type discrimination is correct and ensures no cross-type interference. A string's RC dec will never accidentally call a list's element cleanup, and vice versa.

**Cleanup order**: Cleanup proceeds in lexical block order (list section first, string section next, map section last). Within each section, the pattern is: original dropped first, then derived copy. This is the correct stack-unwind order (LIFO for bindings).

---

## Findings Summary

| ID | Severity | Description | Status |
|----|----------|-------------|--------|
| M1 | MEDIUM | Prelude 10,331 bytes overhead | CONFIRMED (16/16) |
| M3 | MEDIUM | Unnecessary `br label` after calls | CONFIRMED (16/16) |
| M5 | MEDIUM | `align 4` on i64 loads (18+ instances) | CONFIRMED |
| M7 | MEDIUM | Verbose alloca+store+load for struct construction (6 string instances) | CONFIRMED |
| M10 | MEDIUM | `_ori_main` missing `nounwind` attribute | CONFIRMED |
| M11 | MEDIUM | Orphaned landing pads -- 8 in this journey (highest count) | CONFIRMED |
| M17 | MEDIUM | Redundant SSO check sequences on same string value (6 checks) | CONFIRMED (from J14) |
| L1 | LOW | Canon expansion 22.7% (high end of 0-25% range) | CONFIRMED |
| L2 | LOW | 4 prelude decision trees | CONFIRMED |

### New Findings

No new unique findings. All issues observed are confirmations of previously identified patterns. This is significant: three collection types in one function exposed no new bugs and no new categories of codegen issue. The type discrimination, ARC dispatch, and cleanup ordering are all correct.

### What Works Well

- **Cross-collection ARC isolation**: List, string, and map RC operations are completely independent -- no shared state, no cross-type callbacks, no interference
- **Type-specific drop functions**: `ori_buffer_rc_dec` (lists), `ori_rc_dec` with SSO guard (strings), `ori_map_buffer_rc_dec` with elem callbacks (maps) -- each collection type has its correct cleanup mechanism
- **SSO-aware element thunks**: `_ori_elem_inc$3` and `_ori_elem_dec$3` correctly handle short string keys in maps without unnecessary RC operations
- **COW semantics for all three types**: `ori_list_push_cow`, `ori_str_concat`, `ori_map_insert_cow` all preserve value semantics -- originals are never mutated
- **Zero-cost length for lists and maps**: `extractvalue` field 0 extraction, no runtime call needed
- **Correct argument passing**: Function pointers (`ori_str_eq`, `ori_str_hash`, `_ori_elem_inc$3`) correctly threaded through map operations
- **Cleanup ordering**: Lexical block order (list -> string -> map), LIFO within sections
- **5 global string constants**: All correctly null-terminated and sized
- **Mixed scalar/RC element types**: Lists have `elem_dec=null` (int elements), maps have `key_dec=_ori_elem_dec$3` (str keys) + `val_dec=null` (int values) -- correct per-element type discrimination
- **Single `_ori_drop$3` for all heap types**: Universal drop function parameterized by size/align, shared between strings and map buffers -- efficient code reuse

### Cross-Reference with Previous Journeys

| Finding | J19 Status | Prior Journey |
|---------|-----------|---------------|
| C1-C4 | Not triggered | J5, J10-J12 |
| H1 | Not triggered (no recursion) | J3 |
| H2 | Potential -- runtime calls may throw | J10, J13 |
| M1 | 10,331 bytes prelude (16th consecutive) | All journeys |
| M3 | 3+ redundant branches (16th consecutive) | All journeys |
| M5 | 18+ align-4 instances | J4, J6, J13-J16 |
| M7 | 6 verbose string struct constructions | J6, J13 |
| M10 | Missing nounwind on `_ori_main` | J8-J16 |
| M11 | 8 orphaned landing pads (new high) | J9-J16 |
| M15 | RC inc/dec pair on map data (same pattern as J15) | J15 |
| M16 | Length extraction requires no unnecessary RC pair for lists/maps (extractvalue) | J13 -- string length does require `ori_str_len` call |
| M17 | 6 redundant SSO checks | J14 |
| L1 | 22.7% canon expansion | All journeys |
| L2 | 4 prelude decision trees | All journeys |

---

## Responsible Source Files

| Component | File |
|-----------|------|
| List allocation + push | `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs` |
| String concat + length | `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/str_builtins.rs` |
| Map literal + insert | `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/map_builtins.rs` |
| RC inc/dec emission | `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` |
| SSO guard emission | `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` |
| Drop function generation | `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` |
| Element inc/dec thunks | `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` |
| Nounwind analysis | `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs` |
| Runtime: list COW | `compiler/ori_rt/src/list/` |
| Runtime: string ops | `compiler/ori_rt/src/string/` |
| Runtime: map ops | `compiler/ori_rt/src/map/` |
| Runtime: RC management | `compiler/ori_rt/src/rc/` |

---

## Summary

Journey 19 is the comprehensive COW stress test combining all three collection types (list, string, map) in a single function. It is the final COW journey in the series (J13-J19). Both eval and AOT paths produce the correct result of 28.

**The critical validation**: mixing all three COW collection types in one function introduces no cross-collection ARC interference. Each type uses its own RC function family, its own element cleanup callbacks, and its own drop function. The type discrimination is correct and complete.

**No new findings discovered.** All 9 observed patterns are confirmations of previously identified issues (M1, M3, M5, M7, M10, M11, M15, M17, L1, L2). The absence of new findings in this comprehensive stress test is a positive signal -- the COW codegen architecture is sound.

**COW journey series summary** (J13-J19):
- J13: COW list push + iteration -- PASS, established COW codegen patterns
- J14: COW string concat + substring -- PASS, identified SSO guard redundancy (M17)
- J15: COW map literal + insert + iteration -- PASS, identified map RC pair redundancy (M15/M19)
- J16: COW sharing semantics (list value preservation) -- PASS, validated COW invariant
- J19: All three types combined -- PASS, validated cross-collection isolation

All 5 COW journeys pass on both eval and AOT. No critical or high-severity issues in the COW codegen path.
