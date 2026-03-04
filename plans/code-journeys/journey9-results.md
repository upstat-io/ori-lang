# Journey 9 Results: "I am a string"

**Date**: 2026-03-03
**Status**: PASS (Eval=13, AOT=13)

## Source

```ori
@bool_to_int (b: bool) -> int = if b then 1 else 0;

@check_logic () -> int = {
    let a = true && true;       // true  -> 1
    let b = true && false;      // false -> 0
    let c = false || true;      // true  -> 1
    let d = false || false;     // false -> 0
    bool_to_int(b: a) + bool_to_int(b: b) + bool_to_int(b: c) + bool_to_int(b: d)
    // = 1 + 0 + 1 + 0 = 2
}

@check_strings () -> int = {
    let s1 = "hello";
    let s2 = "world!";
    let s3 = "";
    s1.length() + s2.length() + s3.length()
    // = 5 + 6 + 0 = 11
}

@main () -> int = {
    let a = check_logic();      // = 2
    let b = check_strings();    // = 11
    a + b                       // = 13
}
```

**Expected**: `check_logic() + check_strings() = 2 + 11 = 13`

## Features Exercised

- Boolean operators (`&&`, `||`) with short-circuit constant folding
- Boolean-to-int conversion via `if/then/else`
- String literals (including empty string `""`)
- `.length()` method on strings
- ARC lifecycle for string values (first journey with ARC-managed types)
- SSO (Small String Optimization) detection in RC decrement paths
- Named argument function calls
- Block expressions with multiple `let` bindings

## Phase Results

### Lexer
- Source: 859 bytes, 177 tokens, 0 errors
- Prelude: 10,331 bytes, 1,516 tokens, 0 errors

### Parser
- User module: 4 functions, 0 tests, 0 types, 0 traits, 0 impls, 0 imports, 52 expressions, 0 errors, 0 warnings
- Prelude module: 9 functions, 39 traits, 46 expressions, 0 errors, 0 warnings
- Parse contexts entered correctly: `function definition`, `if expression`, `expression` (block body), `function call`, `method call`
- String literals parsed as `TokenKind::String` at positions 108, 114, 120 (spans 598-605, 620-628, 643-645)

### Canonicalization
- User module: 52 source expressions -> 65 canon nodes, 4 roots (`bool_to_int`, `check_logic`, `check_strings`, `main`), 6 constants, 0 decision trees
- Prelude module: 46 source expressions -> 46 canon nodes, 9 roots, 6 constants, 4 decision trees

### Type Checker
- Prelude: 9 functions registered, signatures collected, bodies checked. Hash-first hits for `compare`, `min`, `max`; AST fallbacks for generic builtins (`len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`).
- User module: 4 functions registered, signatures collected, bodies checked. 0 errors.
- String `.length()` method call correctly resolved on `str` type.
- Boolean `&&`/`||` constant-folded at canon level (appears as `Constant(ConstantId(1))` = true, `Constant(ConstantId(2))` = false).

### Evaluator (Interpreter)
- Exit code: 13 (CORRECT)
- No stdout, no stderr
- Eval trace (69 lines) shows correct execution order:
  1. Block entry for `@main` (CanId 64)
  2. `let a = check_logic()` -- calls `check_logic`, enters block (CanId 39)
  3. `let a = true` (Constant 1), `let b = false` (Constant 2), `let c = true` (Constant 1), `let d = false` (Constant 2) -- boolean operators constant-folded
  4. Four calls to `bool_to_int`: `bool_to_int(true)` -> `If(true) -> 1`, `bool_to_int(false)` -> `If(false) -> 0`, etc.
  5. Three `Add` operations: `1+0=1`, `1+1=2`, `2+0=2`
  6. `let b = check_strings()` -- calls `check_strings`, enters block (CanId 54)
  7. `let s1 = "hello"` (Str), `let s2 = "world!"` (Str), `let s3 = ""` (Str)
  8. Three `MethodCall` operations for `.length()`: `s1.length()=5`, `s2.length()=6`, `s3.length()=0`
  9. Two `Add` operations: `5+6=11`, `11+0=11`
  10. Final `Add`: `2+11=13`

### AOT Compilation
- Build: 0.28s compile time, 0 errors
- Binary: 6,634,920 bytes (6.3 MB debug)
- Exit code: 13 (CORRECT)
- No stdout, no stderr (return-code-only journey)

## LLVM Deep Scrutiny (9 Categories)

### 1. IR Structure & Control Flow

**`bool_to_int`**: Minimal 4-block diamond. Entry `bb0` branches on `i1 %0`. `bb1` (true) and `bb2` (false) each branch to `bb3`, which uses a phi node: `%v4 = phi i64 [0, %bb2], [1, %bb1]`. Returns `%v4`. **Correct**.

**`check_logic`**: 8 blocks. Entry `bb0` calls `bool_to_int(true)` -- the `&&`/`||` operators have been constant-folded at the canon level, so the IR directly passes boolean constants:
- `bool_to_int(i1 true)` for `true && true`
- `bool_to_int(i1 false)` for `true && false`
- `bool_to_int(i1 true)` for `false || true`
- `bool_to_int(i1 false)` for `false || false`

Three additions chain the results with overflow checks. Block layout sequences calls between overflow check-and-continue blocks (`add.ok` -> next call -> `add.ok6` -> next call -> `add.ok12` -> return). **Correct**.

**`check_strings`**: The most complex function -- 14 blocks. Entry `bb0` constructs three strings via `ori_str_from_raw`, loads each into an `{ i64, i64, ptr }` LLVM struct (the OriStr representation). Then calls `ori_str_len` on each string, interleaved with RC decrement operations. Control flow:

```
bb0: construct s1, s2, s3; call ori_str_len(s1) -> bb1
bb1: RC-- s1 (SSO check) -> rc_dec.sso_skip
rc_dec.sso_skip: call ori_str_len(s2) -> bb3
bb3: RC-- s2 (SSO check) -> rc_dec.sso_skip25
rc_dec.sso_skip25: add(len1, len2) with overflow check -> add.ok
add.ok: call ori_str_len(s3) -> bb5
bb5: RC-- s3 (SSO check) -> rc_dec.sso_skip36
rc_dec.sso_skip36: add(prev, len3) with overflow check -> add.ok46
add.ok46: return
```

**Assessment**: CORRECT. The control flow properly sequences string construction, length measurement, RC cleanup, and arithmetic. Each string is decremented after its last use (after `ori_str_len` returns).

**`main`**: 4 blocks. `bb0` calls `check_logic()`, `bb1` calls `check_strings()`, `bb3` adds with overflow check, `add.ok` returns. Standard pattern. **Correct**.

**Entry wrapper**: `main()` calls `_ori_main()`, truncates i64 to i32. **Correct**.

**Verdict**: PASS. All control flow graphs are structurally sound, with correct interleaving of ARC operations and string method calls.

### 2. Type Safety & Calling Convention

- `bool_to_int`: `fastcc i64 @_ori_bool_to_int(i1 %0)` -- bool is `i1`, return is `i64`. Correct.
- `check_logic`: `fastcc i64 @_ori_check_logic()` -- no params, returns `i64`. Correct.
- `check_strings`: `i64 @_ori_check_strings()` -- NOTE: **not** marked `fastcc`. This is because `check_strings` calls runtime functions (`ori_str_from_raw`, `ori_str_len`, `ori_rc_dec`) that use C calling convention, so the nounwind analysis determined it may unwind. Missing `fastcc` is a minor codegen inefficiency but not incorrect.
- `main`: `i64 @_ori_main()` -- C calling convention (entry point). Correct.
- String representation: `{ i64, i64, ptr }` -- the OriStr fat struct (length, capacity, data pointer). Correct.
- `ori_str_from_raw`: `void @ori_str_from_raw(ptr noalias sret({i64, i64, ptr}), ptr, i64)` -- takes raw C string pointer and length, returns OriStr via sret. Correct.
- `ori_str_len`: `i64 @ori_str_len(ptr)` -- takes pointer to OriStr, returns length as i64. Correct.

**Verdict**: PASS. All types match their intended semantics. The missing `fastcc` on `check_strings` is an observation (see Category 8).

### 3. Overflow & Arithmetic Safety

- `check_logic`: Three `@llvm.sadd.with.overflow.i64` calls for the three `+` operators between `bool_to_int` results. Each branches to `ori_panic_cstr` on overflow. Panic message constants: `@ovf.msg`, `@ovf.msg.1`, `@ovf.msg.2`.
- `check_strings`: Two `@llvm.sadd.with.overflow.i64` calls for `len1+len2` and `(len1+len2)+len3`. Panic: `@ovf.msg.5`, `@ovf.msg.6`.
- `main`: One `@llvm.sadd.with.overflow.i64` for `a+b`. Panic: `@ovf.msg.7`.
- All overflow messages contain `"integer overflow on addition\00"` (29 bytes each).
- `ori_panic_cstr` marked `cold` (`#2`). Correct.

**Observation**: 7 overflow message globals exist, all containing identical text. This is the previously-noted dedup opportunity.

**Verdict**: PASS. All arithmetic is overflow-checked. No safety gaps.

### 4. Memory Management (ARC/RC) -- CRITICAL FOCUS

This is the first journey with ARC-managed types (strings). The ARC analysis is the key focus.

**String Construction**: Three strings are constructed via `ori_str_from_raw`:

```llvm
call void @ori_str_from_raw(ptr %str.val.sret, ptr @str, i64 5)       ; "hello"
call void @ori_str_from_raw(ptr %str.val.sret1, ptr @str.3, i64 6)    ; "world!"
call void @ori_str_from_raw(ptr %str.val.sret11, ptr @str.4, i64 0)   ; ""
```

Each string is constructed into a stack alloca, then loaded field-by-field via GEP+load+insertvalue into an `{ i64, i64, ptr }` aggregate. The alloca+load pattern avoids returning the 24-byte struct by value across the C ABI boundary (sret convention).

**String Representation**: The OriStr struct `{ i64, i64, ptr }` contains:
- Field 0 (`i64`): length
- Field 1 (`i64`): capacity
- Field 2 (`ptr`): data pointer (heap allocation or SSO sentinel)

**RC Decrement with SSO Detection**: Each string is RC-decremented after its last use. The decrement logic is inlined with SSO (Small String Optimization) detection:

```llvm
; Extract data pointer from the OriStr
%rc_dec.fat_data = extractvalue { i64, i64, ptr } %str.val.s2, 2

; Check SSO flag: high bit of pointer
%rc_dec.p2i = ptrtoint ptr %rc_dec.fat_data to i64
%rc_dec.sso_flag = and i64 %rc_dec.p2i, -9223372036854775808  ; 0x8000000000000000
%rc_dec.is_sso = icmp ne i64 %rc_dec.sso_flag, 0

; Check null pointer
%rc_dec.null.p2i = ptrtoint ptr %rc_dec.fat_data to i64
%rc_dec.null = icmp eq i64 %rc_dec.null.p2i, 0

; Skip RC if SSO or null
%rc_dec.skip_rc = or i1 %rc_dec.is_sso, %rc_dec.null
br i1 %rc_dec.skip_rc, label %rc_dec.sso_skip, label %rc_dec.heap
```

**Assessment**: CORRECT. The SSO detection pattern is:
1. Extract the data pointer from field 2 of the OriStr
2. Check if the high bit (bit 63) is set -- this is the SSO flag indicating the string is stored inline
3. Check if the pointer is null (empty string or uninitialized)
4. If either SSO or null, skip the RC decrement (no heap allocation to manage)
5. Otherwise, call `ori_rc_dec(ptr, ptr @"_ori_drop$3")` to decrement the reference count

The constant `-9223372036854775808` is `0x8000_0000_0000_0000` (i64 sign bit), which matches the SSO flag convention documented in `ori_rt`.

**RC Decrement Call**: When the pointer is heap-allocated (non-SSO, non-null):

```llvm
rc_dec.heap:
  call void @ori_rc_dec(ptr %rc_dec.fat_data, ptr @"_ori_drop$3")  ; RC-- str
  br label %rc_dec.sso_skip
```

The destructor `@"_ori_drop$3"` is defined in the module:

```llvm
define void @"_ori_drop$3"(ptr %0) #3 {
entry:
  call void @ori_rc_free(ptr %0, i64 24, i64 8)
  ret void
}
```

This calls `ori_rc_free` with size=24 (the OriStr struct size) and alignment=8. The destructor is marked `cold nounwind` (`#3`) -- correct since drop functions are on deallocation paths.

**RC Balance Analysis**:
- 3 strings constructed (`ori_str_from_raw` x3)
- 3 strings decremented (one RC-- per string after its last use)
- Balance: +3 / -3 = 0. No leaks, no double-frees.

**String Lifetime Ordering**:
- `s1` ("hello"): constructed in `bb0`, used for `ori_str_len` in `bb0`/`bb1`, RC-- in `bb1`
- `s2` ("world!"): constructed in `bb0`, used for `ori_str_len` in `rc_dec.sso_skip`, RC-- in `bb3`
- `s3` (""): constructed in `bb0`, used for `ori_str_len` in `add.ok`, RC-- in `bb5`

Each string is alive only while needed and dropped immediately after. **Correct**.

**SSO Effectiveness**: For the test strings:
- "hello" (5 bytes): fits in SSO (threshold is 23 bytes) -- SSO flag will be set, no heap alloc, RC-- skipped
- "world!" (6 bytes): fits in SSO -- same behavior
- "" (0 bytes): null or SSO -- RC-- skipped

This means for this specific journey, no `ori_rc_dec` calls actually execute at runtime. All three strings are SSO or null. The RC codegen is present and correct, but the fast path (SSO skip) handles all cases. This is the ideal outcome for short strings.

**Verdict**: PASS. ARC lifecycle is complete and correct. RC balance is zero. SSO detection is properly inlined. The destructor is well-formed. For these short strings, no heap allocation or RC operations occur at runtime.

### 5. Function Symbols & Linkage

From the disassembly:
- `_ori_bool_to_int`: 0x1b090, ~36 bytes, T (global text)
- `_ori_check_logic`: 0x1b0c0, ~182 bytes, T (global text)
- `_ori_check_strings`: 0x1b180, ~650 bytes, T (global text)
- `_ori_main`: 0x1b410, ~72 bytes, T (global text)
- `_ori_drop$3`: 0x1b460, ~18 bytes (string destructor)
- `main`: 0x1b480, C wrapper

Runtime symbols referenced:
- `ori_str_from_raw`: 0x22dd0 (string construction)
- `ori_str_len`: 0x2b360 (string length)
- `ori_rc_dec`: 0x29970 (RC decrement)
- `ori_rc_free`: 0x1f980 (RC free)
- `ori_panic_cstr`: 0x1c1e0 (panic handler)

**Verdict**: PASS. All symbols present and correctly linked. No duplicates, no missing references.

### 6. Constant Data

String literal constants in rodata:

```llvm
@str = private unnamed_addr constant [6 x i8] c"hello\00", align 1
@str.3 = private unnamed_addr constant [7 x i8] c"world!\00", align 1
@str.4 = private unnamed_addr constant [1 x i8] zeroinitializer, align 1
```

**Assessment**: CORRECT.
- "hello" = 5 chars + null = 6 bytes. Correct.
- "world!" = 6 chars + null = 7 bytes. Correct.
- "" = 1 byte zero (null terminator only, using `zeroinitializer`). Correct.
- All marked `private unnamed_addr` -- not exported, address not significant. Correct for string literals.

The length passed to `ori_str_from_raw` matches: 5, 6, 0 respectively (excluding the null terminator).

**Verdict**: PASS.

### 7. Binary & Section Analysis

- Binary size: 6,634,920 bytes (6.3 MB debug build)
- `.text`: 906,481 bytes (885 KB) -- 37 KB larger than J4 (869 KB), consistent with string runtime being pulled in
- `.rodata`: 136,799 bytes (134 KB)
- Debug info: ~4.8 MB (`.debug_info` 1.66 MB + `.debug_line` 652 KB + `.debug_str` 1.81 MB + `.debug_ranges` 575 KB + `.debug_aranges` 56 KB)
- User code footprint: `check_strings` is ~650 bytes (largest user function due to inline SSO checks)
- No unexpected sections. Standard ELF layout.

**Verdict**: PASS. Binary size is normal for a debug build with string runtime.

### 8. Nounwind Analysis

From the ARC trace:
- 4 functions prepared (nounwind analysis pass)
- Fixed-point: 2 passes, 2 nounwind count, 0 mono-propagated
- `_ori_bool_to_int` marked nounwind -- correct (pure computation, no calls that can throw)
- `_ori_check_logic` marked nounwind -- correct (calls only `bool_to_int` which is nounwind, plus overflow panics which are `unreachable`)
- `_ori_check_strings`: NOT marked nounwind -- the function calls `ori_str_from_raw`, `ori_str_len`, and `ori_rc_dec`, which are external C functions that could theoretically unwind
- `_ori_main`: NOT marked nounwind -- calls `check_strings` which is not nounwind

**Observation**: `check_strings` lacks both `nounwind` and `fastcc`. The `nounwind` omission is conservative-correct (runtime functions are external), but since `ori_str_from_raw`, `ori_str_len`, and `ori_rc_dec` are all declared `nounwind` in the module, the nounwind analysis could propagate this. Currently, the runtime function `ori_str_from_raw` is declared without `nounwind`, while `ori_str_len` and `ori_rc_dec` are declared with `nounwind` (attributes `#0` and `#4`). The missing `nounwind` on `ori_str_from_raw` is the bottleneck.

**Verdict**: PASS. The analysis is conservative-correct. Minor optimization opportunity: adding `nounwind` to `ori_str_from_raw`'s declaration would allow `check_strings` and `_ori_main` to be marked nounwind and fastcc.

### 9. Disassembly Quality

**`_ori_bool_to_int`** (36 bytes, 0x1b090-0x1b0b3):
- Tests `%dil` bit 0, branches to true/false paths
- True: `mov $1, %eax`, store to stack. False: `xor %eax, %eax`, store to stack.
- Loads result from stack, returns.
- Extra stack round-trip in debug mode (expected at -O0).

**`_ori_check_logic`** (182 bytes, 0x1b0c0-0x1b174):
- 56-byte stack frame (`sub $0x38, %rsp`)
- Four `call` to `_ori_bool_to_int` with immediate args (`$1` = true, `$0` = false)
- Three `add` + `seto` + `jo` sequences for overflow checking
- Results stored on stack between calls.
- Clean, expected debug-mode code.

**`_ori_check_strings`** (650 bytes, 0x1b180-0x1b40a):
- Large stack frame: `sub $0x108, %rsp` (264 bytes) -- accommodates 3 OriStr stack allocas (72 bytes), 3 str_len.self copies (72 bytes), plus spills
- Three `call ori_str_from_raw` with `lea` for sret pointer, `lea` for string constant pointer, `mov` for length
- Per-field GEP+load after each `ori_str_from_raw` to materialize the OriStr aggregate
- Three `call ori_str_len` with pointer to stack OriStr copy
- SSO check pattern: `movabs $0x8000000000000000, %rdx` + `and` + `cmp $0x0` + `setne` + null check + `or` + `test + jne`
- `call ori_rc_dec` with `lea _ori_drop$3` as second argument
- Overflow checks on both additions

**Assessment**: The code is significantly larger than previous journeys due to the SSO-check inlining. Each string cleanup generates ~30 bytes of inline SSO check code. This is a deliberate tradeoff: the fast path (SSO skip) avoids a function call to `ori_rc_dec` for short strings, which is the common case.

**`_ori_drop$3`** (18 bytes, 0x1b460-0x1b471):
- `push %rax`, `mov $0x18, %esi` (size=24), `mov $0x8, %edx` (align=8), `call ori_rc_free`, `pop %rax`, `ret`
- Minimal and correct.

**`main`** (8 bytes, 0x1b480-0x1b487):
- `push %rax`, `call _ori_main`, `pop %rcx`, `ret`. Standard C wrapper.

**Verdict**: PASS. Disassembly matches IR semantics. Stack frame sizes are appropriate for the data involved.

## Summary

| Category | Verdict | Notes |
|----------|---------|-------|
| IR structure & control flow | PASS | 14 blocks in check_strings with correct ARC interleaving |
| Type safety & calling convention | PASS | OriStr as `{i64, i64, ptr}`, sret convention for construction |
| Overflow & arithmetic safety | PASS | All 6 additions overflow-checked |
| Memory management (ARC/RC) | PASS | RC balance 0; SSO detection inlined; correct destructor |
| Function symbols & linkage | PASS | All user + runtime symbols present and linked |
| Constant data | PASS | String literals correctly emitted with null terminators |
| Binary & section analysis | PASS | Normal debug build size with string runtime |
| Nounwind analysis | PASS | Conservative-correct; `ori_str_from_raw` missing nounwind is bottleneck |
| Disassembly quality | PASS | SSO-check inlining adds size but avoids runtime calls for short strings |

## Observations

1. **First ARC-managed journey**: This is the first journey involving heap-managed types. The ARC pipeline correctly generates RC increment (none needed -- strings are consumed linearly), RC decrement (3 times, once per string after last use), and SSO fast-path detection. The RC balance is zero: no leaks, no double-frees.

2. **SSO effectiveness**: All three test strings ("hello"=5B, "world!"=6B, ""=0B) fit within the SSO threshold (23 bytes). At runtime, no heap allocation or RC operations execute. The generated RC decrement code is dead code for this test case, but would activate correctly for strings longer than 23 bytes.

3. **Boolean operator constant folding**: The `&&` and `||` operators are constant-folded during canonicalization. The IR passes literal `true`/`false` constants directly to `bool_to_int` rather than generating short-circuit evaluation branches. This confirms that constant propagation works correctly through boolean operators.

4. **Inline SSO check tradeoff**: Each string RC decrement generates ~30 bytes of inline SSO detection code (pointer-to-int, high-bit mask, null check, branch). For a function with 3 strings, this adds ~90 bytes. The tradeoff is avoiding a function call to `ori_rc_dec` for the common case (short strings). This is a reasonable choice -- SSO strings are the majority in typical programs.

5. **Missing `nounwind` on `ori_str_from_raw`**: The `ori_str_from_raw` runtime function is declared without the `nounwind` attribute, while `ori_str_len` and `ori_rc_dec` have it. This prevents `check_strings` from being marked `nounwind` and `fastcc`. Adding `nounwind` to `ori_str_from_raw`'s declaration would be a minor codegen improvement.

6. **Overflow message dedup (continuing pattern)**: 7 overflow message globals, all containing the same 29-byte string. Same observation as J2/J3/J4.

7. **OriStr struct layout**: The `{ i64, i64, ptr }` representation (24 bytes) is confirmed working correctly through the full pipeline: construction via `ori_str_from_raw` (sret), field access via GEP, length query via `ori_str_len`, and cleanup via the inline SSO check + `ori_rc_dec` path.
