# Journey 18: COW String Sharing + SSO Boundary

**Code**:
```ori
@main () -> int = {
    let $short = "hello";
    let $long = "this is a longer string!!!!!!!";

    let $short_len = short.length();
    let $long_len = long.length();

    // Test string sharing: concat creates new, originals unchanged
    let $orig_short = short;
    let $concat = short + "!!!";
    let $orig_short_len = orig_short.length();
    let $concat_len = concat.length();

    short_len + long_len + orig_short_len + concat_len
}
```
**Source**: 745 bytes, **Expected Result**: 48 (= 5 + 30 + 5 + 8)
**Actual**: Eval = 48 (correct), **AOT = 48 (correct)**

Both execution paths produce the correct result. This journey tests string SSO (Small String Optimization), heap allocation for long strings, string sharing via variable copy, and concatenation creating new strings without mutating originals.

---

## Transformation Timeline

### Stage 1-2: Lexer
```
745 bytes -> 104 tokens (0 errors)
Prelude: 10,331 bytes -> 1516 tokens (0 errors)
```
- 7.2 bytes/token ratio (high due to comments and long string literal)
- CONFIRMED M1: Prelude still 10,331 bytes

### Stage 3: Parser
```
104 tokens -> 1 function, 0 types, 22 expressions, 0 errors
Prelude: 1516 tokens -> 9 functions, 39 traits, 46 expressions
```
- Single `@main` function
- 22 expressions: 3 string literals, 3 `.length()` method calls, 1 binary `+` (str concat), 1 identifier copy, 3 binary `+` (int adds), and let bindings
- `short + "!!!"` parsed as binary Add on two string expressions
- `.length()` parsed as method calls (0 args)

### Stage 4: Type Checker
```
registration: 9 functions (prelude), 1 function (user), 0 tests, 0 impls
signatures: collected for all functions
body checking: complete (prelude + user)
```
- Hash-first miss on generic prelude functions (len, is_empty, is_some, is_none, is_ok, is_err) -- AST fallback
- Hash-first hit on non-generic prelude functions (compare, min, max)
- `.length()` resolved as str method, `+` resolved as str concat (then int add for the final sums)
- All `$` bindings correctly typed as immutable

### Stage 5: Canonicalizer
```
canon lower_module started (functions=1, source_exprs=22)
canon lower_module complete (canon_nodes=30, roots=1, constants=6, decision_trees=0)
```
- 36% canon expansion (22 -> 30) -- immutable let binding desugaring and concat operation expansion
- 6 constants (string literals interned via Name)
- 0 decision trees (no match expressions)

### Stage 6a: Eval Path
```
35 eval_can calls (from trace)
```
Trace shows clear execution flow:
1. `CanId(0)` -- Str("hello") -> `short`
2. `CanId(2)` -- Str("this is a longer string!!!!!!!") -> `long`
3. `CanId(5)` -- MethodCall(short, length, []) -> 5 (`short_len`)
4. `CanId(8)` -- MethodCall(long, length, []) -> 30 (`long_len`)
5. `CanId(10)` -- Ident(short) -> copies `short` to `orig_short`
6. `CanId(14)` -- Binary(Add, short, "!!!") -> "hello!!!" (`concat`)
   - `evaluate_binary op=Add left_type="str" right_type="str"` -- string concatenation
7. `CanId(17)` -- MethodCall(orig_short, length, []) -> 5 (`orig_short_len`)
8. `CanId(20)` -- MethodCall(concat, length, []) -> 8 (`concat_len`)
9. `CanId(28)` -- final: 5 + 30 + 5 + 8 = 48
   - Three `evaluate_binary op=Add left_type="int" right_type="int"` calls in left-associative chain

String sharing verification: `orig_short` (CanId 10) reads `short` via Ident node. After `concat = short + "!!!"` (CanId 14), `orig_short.length()` at CanId 17 still returns 5. This confirms the interpreter preserves originals during concatenation -- value semantics correct.

### Stage 6b: LLVM Path

#### ARC Trace
```
nounwind analysis: 1 function, 1 pass, 0 nounwind (main NOT marked nounwind)
Type registration: Ordering, PanicInfo, TraceEntry, FormatType, Alignment, Sign enums/structs
Function declaration: _ori_main, C calling convention, Direct return passing
```
- `_ori_main` not marked nounwind (correct -- calls runtime functions that may throw)
- C calling convention (entry point, not fastcc)

#### Type Representation
```llvm
; str = { len: i64, cap: i64, data: *mut u8 }  (heap)
; str = { bytes[23], flags }                     (SSO)
; Both represented as: { i64, i64, ptr } -- 24 bytes
; SSO discriminator: high bit of byte 23 (MSB of 3rd field when viewed as ptr)
```

The 24-byte `{ i64, i64, ptr }` LLVM type is a union -- the same memory layout is used for both SSO (inline storage in 23 bytes + 1 flag byte) and heap (len + cap + data_ptr). The SSO/heap discrimination happens at runtime by checking the high bit of the third field (ptr field):
- SSO: high bit = 1 (flags byte has `0x80`)
- Heap: high bit = 0 (user-space pointers have MSB clear on x86_64)

#### String Literal Constants
```llvm
@str = private unnamed_addr constant [6 x i8] c"hello\00", align 1
@str.1 = private unnamed_addr constant [31 x i8] c"this is a longer string!!!!!!!\00", align 1
@str.2 = private unnamed_addr constant [4 x i8] c"!!!\00", align 1
```
- Three string literals in `.rodata`, each NUL-terminated
- "hello" = 6 bytes ([5 + NUL]), "this is a longer string!!!!!!!" = 31 bytes ([30 + NUL]), "!!!" = 4 bytes ([3 + NUL])
- `unnamed_addr` -- address not significant, allows merging
- `align 1` -- byte alignment (correct for string data)

#### Generated LLVM IR (annotated key sections)

```llvm
define i64 @_ori_main() personality ptr @rust_eh_personality {
bb0:
  ; --- Allocas for all string operations ---
  %str_len.self108 = alloca { i64, i64, ptr }, align 8     ; for concat.length()
  %str_len.self87 = alloca { i64, i64, ptr }, align 8      ; for orig_short.length()
  %ori_str_concat.sret = alloca { i64, i64, ptr }, align 8 ; concat result
  %str_op.rhs = alloca { i64, i64, ptr }, align 8          ; concat RHS ("!!!")
  %str_op.lhs = alloca { i64, i64, ptr }, align 8          ; concat LHS (short)
  %str.val.sret59 = alloca { i64, i64, ptr }, align 8      ; "!!!" from_raw result
  %str_len.self29 = alloca { i64, i64, ptr }, align 8      ; for long.length()
  %str_len.self = alloca { i64, i64, ptr }, align 8        ; for short.length()
  %str.val.sret1 = alloca { i64, i64, ptr }, align 8       ; "long" from_raw result
  %str.val.sret = alloca { i64, i64, ptr }, align 8        ; "hello" from_raw result

  ; --- Create "hello" (5 bytes -> SSO) ---
  call void @ori_str_from_raw(ptr %str.val.sret, ptr @str, i64 5)
  ; ... alloca+store+load roundtrip to extract { i64, i64, ptr } ...   <-- M7
  %str.val.s2 = insertvalue ... ; SSA value for short

  ; --- Create "this is a longer string!!!!!!!" (30 bytes -> HEAP) ---
  call void @ori_str_from_raw(ptr %str.val.sret1, ptr @str.1, i64 30)
  ; ... alloca+store+load roundtrip ...                                <-- M7
  %str.val.s210 = insertvalue ... ; SSA value for long

  ; --- RC inc for short (preparing for .length() call?) ---
  %rc_inc.fat_data = extractvalue { i64, i64, ptr } %str.val.s2, 2
  ; SSO check: is high bit set?
  %rc_inc.sso_flag = and i64 %rc_inc.p2i, -9223372036854775808       ; 0x8000000000000000
  %rc_inc.is_sso = icmp ne i64 %rc_inc.sso_flag, 0
  ; Null check:
  %rc_inc.null = icmp eq i64 %rc_inc.null.p2i, 0
  ; Skip RC if SSO or null:
  %rc_inc.skip_rc = or i1 %rc_inc.is_sso, %rc_inc.null
  br i1 %rc_inc.skip_rc, label %rc_inc.sso_skip, label %rc_inc.heap

rc_inc.heap:
  call void @ori_rc_inc(ptr %rc_inc.fat_data)  ; RC++
  br label %rc_inc.sso_skip

rc_inc.sso_skip:
  ; --- short.length() ---
  store { i64, i64, ptr } %str.val.s2, ptr %str_len.self, align 8
  %str.len = call i64 @ori_str_len(ptr %str_len.self)
  br label %bb1

bb1:
  ; --- RC dec for short (after .length() consumed it?) ---
  ; ... SSO/null check ...
  br i1 %rc_dec.skip_rc28, label %rc_dec.sso_skip22, label %rc_dec.heap21

rc_dec.heap21:
  call void @ori_rc_dec(ptr %rc_dec.fat_data20, ptr @"_ori_drop$3")  ; RC-- str
  br label %rc_dec.sso_skip22

rc_dec.sso_skip22:
  ; --- long.length() ---
  store { i64, i64, ptr } %str.val.s210, ptr %str_len.self29, align 8
  %str.len30 = call i64 @ori_str_len(ptr %str_len.self29)
  br label %bb3

bb3:
  ; --- RC dec for long (after .length() consumed it?) ---
  ; ... SSO/null check ...
  br label %rc_dec.sso_skip43

rc_dec.sso_skip43:
  ; --- RC inc for short (preparing for orig_short copy + concat) ---
  ; ... SSO/null check ...
  br i1 %rc_inc.skip_rc58, label %rc_inc.sso_skip52, label %rc_inc.heap51

rc_inc.heap51:
  call void @ori_rc_inc(ptr %rc_inc.fat_data50)  ; RC++ for short sharing
  br label %rc_inc.sso_skip52

rc_inc.sso_skip52:
  ; --- Create "!!!" (3 bytes -> SSO) ---
  call void @ori_str_from_raw(ptr %str.val.sret59, ptr @str.2, i64 3)
  ; ... alloca+store+load roundtrip ...                                <-- M7
  %str.val.s268 = insertvalue ... ; SSA value for "!!!"

  ; --- Concat: short + "!!!" ---
  store { i64, i64, ptr } %str.val.s2, ptr %str_op.lhs, align 8
  store { i64, i64, ptr } %str.val.s268, ptr %str_op.rhs, align 8
  call void @ori_str_concat(ptr %ori_str_concat.sret, ptr %str_op.lhs, ptr %str_op.rhs)
  ; ... alloca+store+load roundtrip for concat result ...              <-- M7
  %ori_str_concat.s2 = insertvalue ... ; SSA value for concat result

  ; --- RC dec for "!!!" (consumed by concat) ---
  ; ... SSO/null check ...
  ; --- RC dec for short (consumed by concat) ---
  ; ... SSO/null check ...

rc_dec.sso_skip80:
  ; --- orig_short.length() (short value still alive via inc from rc_inc.heap51) ---
  store { i64, i64, ptr } %str.val.s2, ptr %str_len.self87, align 8
  %str.len88 = call i64 @ori_str_len(ptr %str_len.self87)
  br label %bb5

bb5:
  ; --- RC dec for orig_short (short) ---
  ; ... SSO/null check ...

rc_dec.sso_skip101:
  ; --- concat.length() ---
  store { i64, i64, ptr } %ori_str_concat.s2, ptr %str_len.self108, align 8
  %str.len109 = call i64 @ori_str_len(ptr %str_len.self108)
  br label %bb7

bb7:
  ; --- RC dec for concat ---
  ; ... SSO/null check ...

rc_dec.sso_skip113:
  ; --- Final addition: short_len + long_len + orig_short_len + concat_len ---
  %add = add i64 %str.len, %str.len30
  %add120 = add i64 %add, %str.len88
  %add121 = add i64 %add120, %str.len109
  ret i64 %add121
}
```

#### Runtime Function Declarations
```llvm
declare void @ori_str_from_raw(ptr noalias sret({ i64, i64, ptr }), ptr, i64)
declare void @ori_rc_inc(ptr) #1          ; nounwind memory(inaccessiblemem: readwrite)
declare void @ori_rc_dec(ptr, ptr) #1     ; nounwind memory(inaccessiblemem: readwrite)
declare i64 @ori_str_len(ptr)             ; no attrs
declare void @ori_str_concat(ptr noalias sret({ i64, i64, ptr }), ptr, ptr)
declare void @ori_rc_free(ptr, i64, i64)  ; nounwind
```

---

## LLVM Deep Scrutiny

### 1. Instruction Purity

**Actual IR instruction count** (all blocks, excluding allocas/declarations): ~120 instructions

The dominant instruction sources:
- 3x `ori_str_from_raw` calls + alloca/store/load roundtrip + insertvalue chains: ~36 instructions
- 8x SSO/null guard sequences (2 inc + 6 dec): each is 7 instructions (extractvalue, ptrtoint, and, icmp_ne, ptrtoint, icmp_eq, or) = ~56 instructions
- 4x `ori_str_len` calls + store to alloca: ~8 instructions
- 1x `ori_str_concat` + alloca/store: ~6 instructions
- 3x `add i64` + ret: ~4 instructions
- Branching overhead (br/label): ~10 instructions

**Optimal IR** for this program:
```llvm
define i64 @_ori_main() nounwind {
  %short = call { i64, i64, ptr } @ori_str_from_raw(ptr @str, i64 5)       ; SSO inline
  %long = call { i64, i64, ptr } @ori_str_from_raw(ptr @str.1, i64 30)     ; heap alloc
  %short_len = call i64 @ori_str_len(ptr <short>)                           ; 5
  %long_len = call i64 @ori_str_len(ptr <long>)                             ; 30
  ; orig_short = short (SSO: just copy 24 bytes, no RC needed)
  ; concat = short + "!!!"
  %rhs = call { i64, i64, ptr } @ori_str_from_raw(ptr @str.2, i64 3)       ; SSO inline
  %concat = call { i64, i64, ptr } @ori_str_concat(ptr <short>, ptr <rhs>)
  %orig_short_len = call i64 @ori_str_len(ptr <short>)                      ; 5
  %concat_len = call i64 @ori_str_len(ptr <concat>)                         ; 8
  ; cleanup: only long (heap) and concat (8 bytes -> SSO, no cleanup!) need RC dec
  call void @ori_rc_dec(ptr <long.data>, ...)  ; only if heap
  %r1 = add i64 %short_len, %long_len
  %r2 = add i64 %r1, %orig_short_len
  %r3 = add i64 %r2, %concat_len
  ret i64 %r3
}
```
Optimal: ~15 instructions + ~4 calls. Actual: ~120 instructions + ~14 calls. **Ratio: ~5.0x** (HIGH).

Key overhead sources:
1. **SSO guard sequences**: 56 of 120 instructions (~47%) are SSO/null checks before RC inc/dec. These are the `ptrtoint`+`and`+`icmp`+`or`+`br` sequences that check whether a string is SSO (skip RC) or heap (perform RC). This is architecturally necessary but verbose.
2. **alloca+store+load roundtrip** (M7): Every `ori_str_from_raw` and `ori_str_concat` result goes through sret alloca -> field-by-field GEP+load -> insertvalue chain. ~36 instructions for 4 struct constructions.
3. **Unnecessary RC operations** around `.length()`: 2 RC inc/dec pairs bracket the first two `.length()` calls even though they only extract a scalar (M16).
4. **Dead branches** (M3): Multiple `br label` to immediately following blocks.

**Severity: HIGH** -- instruction overhead ratio of 5.0x is the highest seen in any journey (J13 was 3.6x)

### 2. ARC Purity

RC operations in generated IR (by purpose):

| Operation | Location | Purpose | Necessary? |
|-----------|----------|---------|------------|
| RC inc (short) | bb0 -> rc_inc.heap | Before `short.length()` | **NO** -- `.length()` is scalar extraction (M16) |
| RC dec (short) | bb1 -> rc_dec.heap21 | After `short.length()` consumes ref | **NO** -- paired with unnecessary inc |
| RC dec (long) | bb3 -> rc_dec.heap42 | After `long.length()` consumes ref | **PARTIAL** -- long is still used implicitly? Actually no, `long` is only used once for `.length()`. But this RC dec is for the long string itself; it would be needed at scope exit anyway, so placing it here is correct but early. |
| RC inc (short) | rc_dec.sso_skip43 -> rc_inc.heap51 | Before `orig_short = short` copy + `short + "!!!"` | **YES** -- short is aliased (orig_short keeps a reference) |
| RC dec ("!!!") | rc_inc.sso_skip52 -> rc_dec.heap70 | After concat consumes "!!!" | Correct but "!!!" is SSO, so the guard skips it |
| RC dec (short) | rc_dec.sso_skip71 -> rc_dec.heap79 | After concat consumes short operand | Correct but short is SSO, so the guard skips it |
| RC dec (short/orig_short) | bb5 -> rc_dec.heap100 | After `orig_short.length()` | **YES** -- releases the shared reference |
| RC dec (concat) | bb7 -> rc_dec.heap112 | After `concat.length()` | **YES** -- releases concat string |

**Balance Analysis**: The SSO guard sequences ensure that for SSO strings (like "hello" at 5 bytes and "!!!" at 3 bytes), no actual RC operations execute -- the guards correctly skip them. For the heap-allocated strings:

- "this is a longer string!!!!!!!" (30 bytes, heap): Created by `ori_str_from_raw` with RC=1. One RC dec after `.length()` -> RC goes to 0, freed. **Correct.**
- concat result "hello!!!" (8 bytes): This fits in SSO (8 <= 23), so `ori_str_concat` returns an SSO string. The RC dec guard at bb7 will detect SSO and skip. **Correct -- no leak.**
- "hello" (5 bytes, SSO): All RC inc/dec guards correctly detect SSO and skip. **Correct.**

**Net**: 1 unnecessary inc/dec pair around `short.length()` (M16 CONFIRMED from J13). The pair is harmless because the SSO guard skips the actual RC calls, but it adds ~14 instructions of guard overhead that could be avoided if the ARC analysis knew `.length()` is non-consuming.

The long string's early dec after `.length()` is interesting: it frees the heap string as soon as its length is extracted. Since `long` is `$` (immutable) and never used again after `.length()`, this is actually correct eager cleanup.

**SSO correctness**: The generated code correctly handles the SSO/heap duality:
- The guard checks `(ptr & 0x8000000000000000) != 0 || ptr == null` to decide whether to skip RC
- For SSO strings, the third field (`ptr` slot) contains packed inline bytes with the high bit set in byte 23 (the flags byte), so the ptrtoint+and correctly detects SSO
- For null pointers (empty strings), the null check catches them

**Severity: MEDIUM** -- 1 unnecessary pair (CONFIRMED M16), but SSO guards prevent actual overhead at runtime

### 3. Attribute Audit

| Function | Expected Attrs | Actual Attrs | Status |
|----------|---------------|--------------|--------|
| `_ori_main` | personality (has EH) | personality ptr @rust_eh_personality | OK |
| `_ori_main` | NOT nounwind | (none) | OK -- calls may-throw functions |
| `ori_str_from_raw` | nounwind, noalias sret | noalias sret | **MISSING nounwind** |
| `ori_str_len` | nounwind, readonly | (none) | **MISSING nounwind, readonly** |
| `ori_str_concat` | nounwind, noalias sret | noalias sret | **MISSING nounwind** |
| `ori_rc_inc` | nounwind mem(inaccessible:rw) | nounwind memory(inaccessiblemem: readwrite) | OK |
| `ori_rc_dec` | nounwind mem(inaccessible:rw) | nounwind memory(inaccessiblemem: readwrite) | OK |
| `ori_rc_free` | nounwind | nounwind | OK |
| `_ori_drop$3` | cold nounwind | cold nounwind | OK |

**Missing attributes on string runtime functions** (H3 scope expansion):
- `ori_str_from_raw`: Should have `nounwind` (allocation failure = abort, not throw)
- `ori_str_len`: Should have `nounwind` AND `readonly` (pure function: reads SSO flags or heap len field, no side effects)
- `ori_str_concat`: Should have `nounwind` (allocation failure = abort)

The RC functions (`ori_rc_inc`, `ori_rc_dec`, `ori_rc_free`) correctly have `nounwind` and appropriate memory attributes. The drop helper `_ori_drop$3` correctly has `cold nounwind`.

**Impact**: Because `ori_str_from_raw`, `ori_str_len`, and `ori_str_concat` lack `nounwind`, the entire `_ori_main` function requires `personality` and generates landing pads for exception handling. If all called functions were `nounwind`, the personality and landing pads could be eliminated.

**Severity: HIGH** -- CONFIRMED H3 (scope expansion: now includes string functions, not just list/iterator functions)

### 4. Constant Folding Opportunities

1. **String lengths are statically known**: "hello" = 5, "this is a longer string!!!!!!!" = 30, "!!!" = 3, "hello!!!" = 8. The compiler could theoretically fold all `.length()` calls to constants. However, `ori_str_len` is an opaque runtime call, so the compiler cannot see through it. The `ori_str_from_raw` calls produce OriStr values whose internal structure (SSO vs heap) is opaque to LLVM.

2. **Final result**: `5 + 30 + 5 + 8 = 48` could be constant-folded if `.length()` calls were intrinsified. Not feasible with current runtime-opaque design.

3. **SSO elision for known-small strings**: The compiler knows "hello" is 5 bytes and "!!!" is 3 bytes, both well under the SSO threshold (23 bytes). It could skip the SSO/heap guard entirely for these. Similarly, "hello" + "!!!" = 8 bytes, guaranteed SSO. This optimization is not performed -- all strings go through the same SSO guard path.

4. **`ori_str_from_raw` for SSO-guaranteed strings**: Since the compiler knows the string length at compile time, it could emit inline SSO construction (memcpy to stack + set flags byte) instead of calling the runtime function. This would eliminate 3 runtime calls.

**Severity: MEDIUM** -- static string length and SSO status are knowable at compile time but not exploited

### 5. Alignment Audit

| Location | Type | Actual Align | Correct Align | Status |
|----------|------|-------------|---------------|--------|
| alloca { i64, i64, ptr } | struct | align 8 | align 8 | OK |
| store { i64, i64, ptr } | struct | align 8 | align 8 | OK |
| GEP + load i64 (field 0) | i64 | align 4 | align 8 | **WRONG** (M5) |
| GEP + load i64 (field 1) | i64 | align 4 | align 8 | **WRONG** (M5) |
| GEP + load ptr (field 2) | ptr | align 8 | align 8 | OK |

**CONFIRMED M5**: All `getelementptr inbounds + load i64` sequences for fields 0 and 1 of the `{ i64, i64, ptr }` struct use `align 4` instead of `align 8`. Field 2 (ptr) correctly uses `align 8`. This is consistent with the pattern seen in all previous journeys -- the alignment calculation appears to use `min(field_size, 4)` instead of the natural alignment of the type.

There are 8 instances of `align 4` on i64 loads in the IR (4 from `ori_str_from_raw` result extraction, 4 from `ori_str_concat` result extraction).

**Severity: MEDIUM** (CONFIRMED from J4, J6, J7, J9, J10, J12, J13 -- persistent across 10 journeys now)

### 6. Control Flow Analysis

| Block | Predecessors | Purpose | Status |
|-------|-------------|---------|--------|
| bb0 | (entry) | String creation, first SSO guard | OK |
| bb1 | rc_inc.sso_skip | RC dec for short after .length() | OK |
| bb2 | **(none)** | Landing pad (cleanup short) | **M11**: orphaned |
| bb3 | rc_dec.sso_skip22 | RC dec for long after .length() | OK |
| bb4 | **(none)** | Landing pad (cleanup short+long) | **M11**: orphaned |
| bb5 | rc_dec.sso_skip80 | RC dec for orig_short | OK |
| bb6 | **(none)** | Landing pad (cleanup concat) | **M11**: orphaned |
| bb7 | rc_dec.sso_skip101 | RC dec for concat | OK |
| bb8 | **(none)** | Landing pad (dead) | **M11**: orphaned |
| rc_inc.heap / rc_inc.sso_skip | bb0 | SSO-guarded RC inc for short | OK |
| rc_dec.heap* / rc_dec.sso_skip* | various | SSO-guarded RC dec for various strings | OK |
| rc_inc.heap51 / rc_inc.sso_skip52 | rc_dec.sso_skip43 | SSO-guarded RC inc for sharing | OK |
| rc_dec.sso_skip113 | bb7 or rc_dec.heap112 | Final: compute result + return | OK |

**Orphaned landing pads** (CONFIRMED M11): 4 landing pad blocks (bb2, bb4, bb6, bb8) with `No predecessors!` in the IR comments. These contain cleanup code (RC dec sequences) and `resume` instructions but are unreachable. They are emitted as part of the EH infrastructure for `invoke` instructions that were never actually generated (the calls use `call` not `invoke`).

**Dead branches** (CONFIRMED M3): Multiple instances of unnecessary `br label` to the immediately following block. The SSO guard pattern generates diamond-shaped CFG (`br i1 %skip, skip_label, heap_label / heap_label: call rc_inc; br skip_label / skip_label: ...`) which adds at least one extra branch per guard.

**Notable positive**: The SSO guard generates correct diamond CFG -- the `rc_inc.heap`/`rc_dec.heap` blocks are only reached when the string is actually heap-allocated, and the `sso_skip` label correctly merges both paths.

**Block count**: 22 named blocks for what is conceptually a straight-line program. The SSO guards and orphaned landing pads account for most of this complexity.

**Severity: MEDIUM (M3, M11)**

### 7. Binary Analysis

```
Binary size: 6,665,632 bytes (6.4 MB)
.text section: 928,377 bytes (907 KB)
_ori_main function: 1094 bytes (0x1db00 to 0x1df45)
_ori_drop$3: 18 bytes (0x1df50 to 0x1df61)
main (C entry): 8 bytes (0x1df70 to 0x1df77)
```

- Function size: 1094 bytes for ~120 IR instructions = ~9.1 bytes/IR instruction (high -- SSO guards compile to verbose x86)
- Stack frame: `sub $0x168, %rsp` = 360 bytes -- large for a string manipulation program

The 360-byte stack houses:
- 10 x alloca `{ i64, i64, ptr }` (24 bytes each) = 240 bytes for string temporaries
- Plus spill slots for intermediate SSA values across the SSO guard branches

**Runtime symbols linked** (string-specific):
- `ori_str_from_raw` (0x1f4b0): 214 bytes
- `ori_str_len` (0x21340): 166 bytes
- `ori_str_concat` (0x20770): ~500 bytes (estimate from symbol table gap)
- `ori_rc_inc` (0x1e880): 405 bytes
- `ori_rc_dec` (0x1e510): 496 bytes
- `ori_rc_free` (0x1e700): 384 bytes
- `_ori_drop$3` (0x1df50): 18 bytes

Total runtime code for string operations: ~2183 bytes.

**Assembly pattern analysis**: The SSO guard in x86 assembly compiles to a consistent pattern:
```asm
movabs $0x8000000000000000, %rdx    ; 10-byte immediate load (movabs)
mov    %rcx, %rax
and    %rdx, %rax                    ; check high bit
cmp    $0x0, %rax
setne  %al                           ; is_sso flag
cmp    $0x0, %rcx
sete   %cl                           ; is_null flag
or     %cl, %al                      ; skip = is_sso || is_null
test   $0x1, %al
jne    <skip_label>
```
This is ~30 bytes per SSO guard. With 8 guards in `_ori_main`, that is ~240 bytes of guard code -- about 22% of the function.

**Observation**: The `movabs $0x8000000000000000` (10-byte instruction) is repeated for every guard. On x86_64, this could be hoisted to a register once at function entry and reused. The `cmp $0x0, %rax` after `and` is also redundant -- `and` already sets the zero flag. These are LLVM backend optimization opportunities, not Ori codegen issues.

### 8. SSO-Specific Analysis

**SSO correctness verification** from the IR:

1. **"hello" (5 bytes)**: `ori_str_from_raw(ptr, @str, i64 5)` creates an SSO string because `5 <= 23`. The third field of the resulting `{ i64, i64, ptr }` will have its MSB set (SSO flag). When the SSO guard checks this field, `(ptr & 0x8000000000000000) != 0` is true, so RC operations are skipped. **Correct.**

2. **"this is a longer string!!!!!!!" (30 bytes)**: `ori_str_from_raw(ptr, @str.1, i64 30)` creates a heap string because `30 > 23`. The third field is a valid heap pointer (MSB clear). RC operations will proceed. **Correct.**

3. **"!!!" (3 bytes)**: `ori_str_from_raw(ptr, @str.2, i64 3)` creates an SSO string. RC operations skipped. **Correct.**

4. **"hello!!!" (8 bytes, concat result)**: `ori_str_concat` of "hello" (SSO) + "!!!" (SSO) = 8 bytes. Since `8 <= 23`, the result is SSO. The RC dec guard at bb7 for the concat result will detect SSO and skip. **Correct -- no leak possible.**

**SSO sharing semantics**: When `orig_short = short` copies the SSO string, the entire 24-byte struct is copied by value (memcpy semantics at the LLVM level -- `insertvalue` chain). No RC inc is needed because SSO strings have no heap allocation. The generated code does emit an RC inc guard, but the SSO check skips it at runtime. This is correct but wasteful -- the compiler could statically prove that "hello" is SSO and skip the guard entirely.

**Heap string lifecycle**: The only heap string is "this is a longer string!!!!!!!" (30 bytes):
- Created: `ori_str_from_raw` with `i64 30` -> heap allocation with RC=1
- Used: `.length()` -> RC dec at bb3 (rc_dec.heap42) decrements to 0, freed
- No aliasing: `long` is `$` (immutable), used exactly once, then freed immediately after `.length()`
- **Correct: single-owner heap string created and freed within the function**

### 9. String Concat Analysis

The concat operation (`short + "!!!"`) generates:
```llvm
store { i64, i64, ptr } %str.val.s2, ptr %str_op.lhs, align 8     ; copy short to alloca
store { i64, i64, ptr } %str.val.s268, ptr %str_op.rhs, align 8   ; copy "!!!" to alloca
call void @ori_str_concat(ptr %ori_str_concat.sret, ptr %str_op.lhs, ptr %str_op.rhs)
```

The `ori_str_concat` function:
- Takes two `OriStr*` parameters (pointers to the alloca'd string structs)
- Returns result via sret (pointer to alloca'd output struct)
- Handles all combinations: SSO+SSO, SSO+heap, heap+SSO, heap+heap
- For SSO+SSO where result <= 23 bytes: copies inline bytes, no heap allocation
- For results > 23 bytes: allocates new heap buffer with RC=1

**Before concat, RC inc for short**: The code increments short's RC before the concat because short is used both as a concat operand AND preserved as `orig_short`. The SSO guard correctly skips the actual RC call. After concat, RC dec for both operands (short and "!!!") -- both SSO, both skipped.

**Value preservation verification**: After `concat = short + "!!!"`, the code computes `orig_short.length()` using the original `%str.val.s2` SSA value (which is the "hello" string). This is the same SSA value used to create `short` -- confirming that concatenation does NOT modify the original. The SSA representation inherently guarantees this (immutable SSA values cannot be mutated), which is the correct compilation of Ori's value semantics.

---

## Issues Found

### HIGH

**H3 CONFIRMED (scope expansion): Missing nounwind on string runtime functions**
- `ori_str_from_raw`: Missing `nounwind` (creates SSO or heap string; heap OOM = abort)
- `ori_str_len`: Missing `nounwind` AND `readonly` (pure query, no side effects)
- `ori_str_concat`: Missing `nounwind` (creates result string; heap OOM = abort)
- **Impact**: Forces `_ori_main` to use `personality` and emit 4 orphaned landing pads. With `nounwind` on these, the entire function could be `nounwind`, eliminating EH overhead.
- **Source**: `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`
- **Cross-ref**: H3 from J13 (list/iterator functions), now expanded to string functions

**Instruction overhead ratio 5.0x** (120 actual vs ~24 optimal)
- Primary contributors: SSO guard sequences (47%), alloca+store+load roundtrips (M7, 30%), unnecessary RC pairs (M16), dead branches (M3)
- This is the highest overhead ratio seen in any journey (previous high was J13 at 3.6x)
- The SSO guard overhead is architecturally necessary for correctness but could be reduced by static analysis (see constant folding section)

### MEDIUM

**M5 CONFIRMED: `align 4` on i64 struct field loads**
- 8 instances of `align 4` on `load i64` via GEP into `{ i64, i64, ptr }` struct
- Should be `align 8` for natural i64 alignment
- Persistent across 10 journeys now (J4, J6, J7, J9, J10, J11, J12, J13, J18)

**M7 CONFIRMED: alloca+store+load roundtrip for sret struct results**
- 4 instances: 3x `ori_str_from_raw` + 1x `ori_str_concat` results
- Each goes through: alloca -> sret call -> GEP+load field 0 -> GEP+load field 1 -> GEP+load field 2 -> insertvalue chain
- Could use direct SSA return if functions returned by register (24 bytes = 3 registers, fits in x86_64 ABI for 2-register return but not 3)
- The sret pattern is correct for C ABI (24-byte struct > 16-byte register return threshold on System V ABI), but the field-by-field reload into insertvalue is verbose

**M3 CONFIRMED: Dead `br label` after calls**
- Multiple instances throughout the function
- SSO guard diamond pattern inherently adds branches

**M11 CONFIRMED: Orphaned landing pads**
- 4 instances (bb2, bb4, bb6, bb8) with no predecessors
- Each contains RC cleanup code and `resume` instructions
- Unreachable because `call` (not `invoke`) is used for runtime functions

**M16 CONFIRMED: Unnecessary RC inc/dec pair around `.length()` scalar extraction**
- First `.length()` call (for `short`) is bracketed by RC inc before and RC dec after
- `.length()` only extracts a scalar value from the string -- no reference is created or consumed
- SSO guard mitigates runtime cost (the guards skip the actual RC calls for SSO strings), but the guard instructions themselves are wasted
- **Cross-ref**: M16 from J13 (identical pattern for list `.length()`)

**M18 CONFIRMED: Redundant SSO check sequences**
- 8 SSO/null guard sequences in a function with only 1 heap string (the 30-byte long string)
- "hello" (5 bytes), "!!!" (3 bytes), and "hello!!!" (8 bytes) are all statically SSO -- guards are unnecessary
- Each guard is ~7 IR instructions / ~30 bytes of x86 -> ~56 wasted IR instructions / ~240 bytes of x86
- The ARC pipeline could tag string values with compile-time-known SSO status and skip guard emission
- **Cross-ref**: M18 from J14 (same pattern, now 8 guard instances vs 19 in J14's larger program)

### LOW

**L10 (NEW): Repeated `movabs $0x8000000000000000` in x86 assembly**
- The SSO flag constant `0x8000000000000000` is loaded via 10-byte `movabs` instruction in every guard
- 8 instances = 80 bytes of repeated constant loads
- Could be hoisted to a register at function entry
- This is an LLVM backend optimization (not Ori codegen), but the high number of guards makes it notable
- **Severity: LOW** -- LLVM -O2 would likely hoist this

---

## Eval vs LLVM Behavioral Comparison

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | **48** | **48** |
| "hello" creation | Correct | Correct (SSO via `ori_str_from_raw`) |
| "long string" creation | Correct | Correct (heap via `ori_str_from_raw`) |
| short.length() -> 5 | Correct | Correct (via `ori_str_len`) |
| long.length() -> 30 | Correct | Correct (via `ori_str_len`) |
| orig_short = short (sharing) | Value copy | SSA value copy (SSO: 24-byte copy, no RC) |
| short + "!!!" -> "hello!!!" | Correct, new string | Correct (via `ori_str_concat`, result is SSO) |
| orig_short.length() -> 5 | Correct | Correct (original preserved) |
| concat.length() -> 8 | Correct | Correct |
| Runtime declarations | N/A | 7 (str_from_raw, str_len, str_concat, rc_inc, rc_dec, rc_free, personality) |
| SSO handling | Transparent (Rust OriStr) | SSO/null guards in generated code |
| RC operations (actual) | N/A | 0 (all SSO strings, 1 heap string freed by dec) |

**No eval-vs-AOT divergence** -- both produce 48. String sharing, SSO, and concatenation all work correctly in both paths.

---

## What Works Exceptionally Well

- **SSO/heap discrimination**: The `ptrtoint + and 0x8000000000000000 + icmp ne` guard correctly distinguishes SSO from heap strings. SSO strings skip all RC operations. This is the fundamental correctness requirement for the OriStr union layout and it works flawlessly.
- **String value semantics**: `orig_short = short` followed by `concat = short + "!!!"` correctly preserves `orig_short` as "hello" (length 5). The SSA representation inherently guarantees this -- the same SSA value is used for both the copy and the original.
- **Heap string lifecycle**: The 30-byte heap string is created with RC=1, used once for `.length()`, and immediately freed. No leak, no dangling reference.
- **Concat creates new string**: `ori_str_concat` produces a new OriStr (SSO for 8-byte result) without modifying either operand. The operand strings are correctly cleaned up afterward.
- **Correct sret calling convention**: 24-byte OriStr structs are returned via hidden sret parameter, matching the System V x86_64 ABI (structs > 16 bytes use sret). This is correct C ABI interop.
- **Drop function**: `_ori_drop$3` correctly calls `ori_rc_free(ptr, 24, 8)` -- size=24 (OriStr heap layout), align=8. Used as the drop callback for `ori_rc_dec`.
- **String constant layout**: `@str = private unnamed_addr constant [6 x i8] c"hello\00"` -- correct NUL-termination, `private` visibility, `unnamed_addr` for potential merging.
- **AOT cache**: Reliable compilation at 0.26-0.28s compile time.
