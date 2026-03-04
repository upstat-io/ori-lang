# Journey 11 Results: "I am a derived trait"

**Date**: 2026-03-03
**Status**: PASS -- both eval and AOT produce correct result (exit code 33)
**Previously**: CRITICAL C3 -- payload sum type `$eq` not generated. Now FIXED.

## Source

```ori
// Journey 11: "I am a derived trait"
// Features: #[derive(Eq)], struct equality, sum type equality, == and !=
// Expected: check_struct_eq() + check_sum_eq() + check_nested() = 7 + 11 + 15 = 33

#[derive(Eq)]
type Point = { x: int, y: int }

#[derive(Eq)]
type Color = Red | Green | Blue;

#[derive(Eq)]
type Shape = Circle(radius: int) | Rect(w: int, h: int);

@check_struct_eq () -> int = {
    let p1 = Point { x: 10, y: 20 };
    let p2 = Point { x: 10, y: 20 };
    let p3 = Point { x: 10, y: 30 };
    let same = if p1 == p2 then 3 else 0;
    let diff = if p1 != p3 then 4 else 0;
    same + diff
    // = 3 + 4 = 7
}

@check_sum_eq () -> int = {
    let c1 = Red;
    let c2 = Red;
    let c3 = Blue;
    let unit_same = if c1 == c2 then 5 else 0;
    let unit_diff = if c1 != c3 then 6 else 0;
    unit_same + unit_diff
    // = 5 + 6 = 11
}

@check_nested () -> int = {
    let s1 = Circle(radius: 10);
    let s2 = Circle(radius: 10);
    let s3 = Rect(w: 5, h: 8);
    let payload_same = if s1 == s2 then 7 else 0;
    let payload_diff = if s1 != s3 then 8 else 0;
    payload_same + payload_diff
    // = 7 + 8 = 15
}

@main () -> int = {
    let a = check_struct_eq();   // = 7
    let b = check_sum_eq();      // = 11
    let c = check_nested();      // = 15
    a + b + c                    // = 33
}
```

**Features exercised**: `#[derive(Eq)]`, struct equality (`==`/`!=`), unit-variant sum type equality, payload sum type equality (record variants), derived `$eq` codegen for structs and enums, `if/then/else`, named arguments, let bindings, integer addition.

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 33        | 33       | (none) | (none) | PASS   |
| AOT     | 33        | 33       | (none) | compile msg only | PASS   |

## Pipeline Trace Summary

### Lexer
- Source: 1319 bytes, 344 tokens, 0 errors
- Prelude: 10331 bytes, 1516 tokens, 0 errors
- Clean pass, no issues.

### Parser
- User module: 4 functions, 3 types, 85 expressions, 0 errors, 0 warnings
- Correctly parsed: `#[derive(Eq)]` attribute on all three types, struct definition `Point`, unit-variant sum `Color`, record-variant sum `Shape`, struct literals, variant construction, equality/inequality operators
- Prelude: 9 functions, 46 expressions, 4 decision trees

### Canonicalization
- User module: 4 functions, 100 canon nodes, 6 constants, 0 decision trees
- Prelude: 9 functions, 46 canon nodes, 4 decision trees
- Note: No decision trees in user module -- `==`/`!=` is lowered as binary ops, not match/decision tree. The derived `$eq` methods use internal dispatch.

### Type Checker
- Registration, signature collection, body checking: all complete, 0 errors
- Two modules checked:
  - Prelude: 9 functions, 0 tests, 0 impls
  - User: 4 functions, 0 tests, 0 impls
- 6 generic prelude imports required AST fallback: `len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`
- 3 non-generic prelude imports hit hash-first: `compare`, `min`, `max`
- The `#[derive(Eq)]` attribute is processed during type registration, generating implicit `impl Eq for Point`, `impl Eq for Color`, and `impl Eq for Shape`. These are NOT counted in the user impls count (they are compiler-generated).

### ARC Pipeline
- Type registration: 10 types total (3 user + 7 prelude: Ordering, CancellationReason, PanicInfo, TraceEntry, FormatSpec, NurseryErrorMode, FormatType)
- User types registered:
  - `%ori.Point = type { i64, i64 }` -- struct, 2 int fields, boxed category
  - `%ori.Color = type { i64 }` -- enum, 3 unit variants (Red=0, Green=1, Blue=2), discriminant-only
  - `%ori.Shape = type { i64, [2 x i64] }` -- enum, 2 record variants (Circle: 1 field, Rect: 2 fields), tag + payload
- Derived methods compiled:
  - `Point` -- 1 derive (Eq), 2 fields (struct `$eq`)
  - `Color` -- 1 derive (Eq), 3 variants (enum `$eq`, tag-only)
  - `Shape` -- 1 derive (Eq), 2 variants (enum `$eq`, per-variant field comparison)
- 4 user functions + 3 derived `$eq` methods = 7 total functions emitted
- Nounwind analysis: 2 fixed-point passes, 4 nounwind (all 4 user functions). The `$eq` methods are not counted in the nounwind set (they do not appear in the user function list; they are trait method implementations emitted separately by `derive_codegen`).
- Entry point wrapper: `main()` -> `_ori_main()` with `trunc i64 to i32`

### Evaluator
- Full execution traced through 107 lines of CAN evaluation:
  - `check_struct_eq()`: constructs 3 Point structs, evaluates `p1 == p2` (Eq on struct, true -> 3), `p1 != p3` (NotEq on struct, true -> 4), returns 7
  - `check_sum_eq()`: constructs 3 Color variants (Red, Red, Blue), evaluates `c1 == c2` (Eq on variant, true -> 5), `c1 != c3` (NotEq on variant, true -> 6), returns 11
  - `check_nested()`: constructs 3 Shape variants (Circle(10), Circle(10), Rect(5,8)), evaluates `s1 == s2` (Eq on variant, true -> 7), `s1 != s3` (NotEq on variant, true -> 8), returns 15
  - Final: `a + b + c` = `7 + 11 + 15` = 33
- Binary operator trace confirms correct dispatch: `Eq` with `left_type="struct"`, `Eq` with `left_type="variant"`, `NotEq` with `left_type="variant"` -- all operating on the derived `$eq` method.

## LLVM Deep Scrutiny (9 Categories)

### 1. Attributes & Calling Convention

| Function | fastcc | nounwind | Status |
|----------|--------|----------|--------|
| `_ori_main` | No (C ABI) | Yes | OK |
| `_ori_check_struct_eq` | Yes | Yes | OK |
| `_ori_check_sum_eq` | Yes | Yes | OK |
| `_ori_check_nested` | Yes | Yes | OK |
| `_ori_Point$eq` | Yes | No `nounwind` | See below |
| `_ori_Color$eq` | Yes | No `nounwind` | See below |
| `_ori_Shape$eq` | Yes | No `nounwind` | See below |
| `main` (wrapper) | No (C ABI) | No | OK |
| `ori_panic_cstr` | No (extern) | No | OK -- `cold` attr present |

**Derived `$eq` methods lack `nounwind`**: The three `$eq` functions do not have the `nounwind` attribute, even though they perform only `extractvalue`, `icmp`, `load`, `store`, `getelementptr`, `switch`, and `br` -- none of which can unwind. This is because derived methods are emitted by `derive_codegen` outside the standard nounwind analysis pipeline (which only processes user functions). The nounwind analysis logs confirm: "nounwind analysis complete, nounwind_count=4" -- exactly the 4 user functions, not including the 3 derived methods.

**Severity**: LOW -- missing `nounwind` on leaf functions that cannot unwind. Does not affect correctness. Could inhibit some LLVM optimizations that rely on knowing a callee cannot unwind (e.g., call folding, EH edge elimination). The fix would be to either include derived methods in the nounwind fixed-point analysis, or unconditionally mark pure comparison methods as `nounwind`.

### 2. Derived `$eq` Codegen -- Struct (Point)

```llvm
define fastcc i1 @"_ori_Point$eq"(%ori.Point %0, %ori.Point %1) {
entry:
  %self.x = extractvalue %ori.Point %0, 0
  %other.x = extractvalue %ori.Point %1, 0
  %eq.x = icmp eq i64 %self.x, %other.x
  br i1 %eq.x, label %eq.check.1, label %eq.false

eq.true:
  ret i1 true

eq.false:
  ret i1 false

eq.check.1:
  %self.y = extractvalue %ori.Point %0, 1
  %other.y = extractvalue %ori.Point %1, 1
  %eq.y = icmp eq i64 %self.y, %other.y
  br i1 %eq.y, label %eq.true, label %eq.false
}
```

**Analysis**: Short-circuit lexicographic field comparison. Compares `x` first; if unequal, returns `false` immediately without comparing `y`. This is the optimal pattern -- no unnecessary field extraction when early fields differ. The struct is passed by value (2 x i64 = 16 bytes, fits in two registers per x86-64 SysV ABI).

**Native code** (`_ori_Point$eq`, 40 bytes):
```asm
mov    %rcx,-0x10(%rsp)    ; spill other.y
mov    %rsi,-0x8(%rsp)     ; spill other.x
cmp    %rdx,%rdi           ; compare self.x vs other.x
je     eq.check.1          ; if equal, check y
jmp    eq.false
eq.true:
  mov    $0x1,%al           ; return true
  ret
eq.false:
  xor    %eax,%eax          ; return false
  ret
eq.check.1:
  mov    -0x8(%rsp),%rax    ; load other.y
  mov    -0x10(%rsp),%rcx   ; load self.y
  cmp    %rcx,%rax          ; compare y fields
  je     eq.true
  jmp    eq.false
```

The two-register struct is passed as `(self.x=%rdi, self.y=%rsi, other.x=%rdx, other.y=%rcx)` via fastcc. The spills of `%rsi` and `%rcx` are unnecessary in debug mode -- the values could stay in registers. At -O1+ these would be eliminated.

**Verdict**: Correct and efficient codegen. Short-circuit behavior is sound.

**Severity**: None (correctness). LOW (debug-mode register spills -- expected).

### 3. Derived `$eq` Codegen -- Unit-Variant Sum (Color)

```llvm
define fastcc i1 @"_ori_Color$eq"(%ori.Color %0, %ori.Color %1) {
entry:
  %eq.tag.self = extractvalue %ori.Color %0, 0
  %eq.tag.other = extractvalue %ori.Color %1, 0
  %eq.tags = icmp eq i64 %eq.tag.self, %eq.tag.other
  br i1 %eq.tags, label %eq.true, label %eq.false

eq.true:
  ret i1 true

eq.false:
  ret i1 false
}
```

**Analysis**: For a unit-variant-only enum (no payloads), equality is a single tag comparison. This is optimal -- extract discriminants, compare, done. No payload checking needed. The `%ori.Color = type { i64 }` is just a tag wrapper.

**Native code** (`_ori_Color$eq`, 11 bytes):
```asm
cmp    %rsi,%rdi    ; compare tags
jne    eq.false
mov    $0x1,%al     ; true
ret
eq.false:
xor    %eax,%eax    ; false
ret
```

Extremely lean -- 3-4 instructions. The single-field enum is passed as a bare i64 (unwrapped by fastcc), so it is just a register-to-register comparison.

**Verdict**: Optimal. Cannot be improved.

**Severity**: None.

### 4. Derived `$eq` Codegen -- Payload Sum (Shape)

```llvm
define fastcc i1 @"_ori_Shape$eq"(ptr %0, ptr %1) {
entry:
  ; Load both Shape values field-by-field via GEP
  %param.0.f0.ptr = getelementptr inbounds nuw %ori.Shape, ptr %0, i32 0, i32 0
  %param.0.f0 = load i64, ptr %param.0.f0.ptr, align 8
  %param.0.s0 = insertvalue %ori.Shape zeroinitializer, i64 %param.0.f0, 0
  %param.0.f1.ptr = getelementptr inbounds nuw %ori.Shape, ptr %0, i32 0, i32 1
  %param.0.f1 = load [2 x i64], ptr %param.0.f1.ptr, align 8
  %param.0.s1 = insertvalue %ori.Shape %param.0.s0, [2 x i64] %param.0.f1, 1
  ; ... same for param.1 ...
  %eq.tag.self = extractvalue %ori.Shape %param.0.s1, 0
  %eq.tag.other = extractvalue %ori.Shape %param.1.s1, 0
  %eq.tags = icmp eq i64 %eq.tag.self, %eq.tag.other
  br i1 %eq.tags, label %eq.tags.match, label %eq.false

eq.tags.match:
  store %ori.Shape %param.0.s1, ptr %eq.self, align 8
  store %ori.Shape %param.1.s1, ptr %eq.other, align 8
  %eq.self.payload = getelementptr inbounds nuw %ori.Shape, ptr %eq.self, i32 0, i32 1
  %eq.other.payload = getelementptr inbounds nuw %ori.Shape, ptr %eq.other, i32 0, i32 1
  switch i64 %eq.tag.self, label %eq.false [
    i64 0, label %eq.v.Circle
    i64 1, label %eq.v.Rect
  ]

eq.v.Circle:
  ; Compare radius field (1 field)
  %eq.v0.self.f0.val = load i64, ptr %eq.v0.self.f0, align 8
  %eq.v0.other.f0.val = load i64, ptr %eq.v0.other.f0, align 8
  %eq.v0.f0 = icmp eq i64 %eq.v0.self.f0.val, %eq.v0.other.f0.val
  br i1 %eq.v0.f0, label %eq.true, label %eq.false

eq.v.Rect:
  ; Compare w field, then h field (short-circuit)
  %eq.v1.f0 = icmp eq i64 %eq.v1.self.f0.val, %eq.v1.other.f0.val
  br i1 %eq.v1.f0, label %eq.v1.f1, label %eq.false
eq.v1.f1:
  %eq.v1.f11 = icmp eq i64 %eq.v1.self.f1.val, %eq.v1.other.f1.val
  br i1 %eq.v1.f11, label %eq.true, label %eq.false
```

**Analysis**: This is the most architecturally significant derived method. The codegen follows this pattern:
1. **Tag check first** -- compare discriminants; if different, return `false` immediately
2. **Switch dispatch** -- if tags match, `switch` on the tag to variant-specific comparison blocks
3. **Per-variant field comparison** -- each variant gets its own block(s) with short-circuit field comparison
4. **Default unreachable** -- `switch` default falls to `eq.false` (conservative, could be `unreachable` for exhaustive enums)

The `Shape` type (24 bytes: tag + 2 x i64 payload) is correctly passed by pointer (`ptr %0, ptr %1`) since it exceeds 16 bytes. The per-field GEP+load pattern is used to reconstitute the aggregate values.

**Previously critical C3 bug -- FIXED**: This is the exact scenario that previously failed. The C3 bug was that `$eq` was not generated for sum types with payloads. The codegen now correctly emits per-variant field comparison with short-circuit behavior.

**Redundant load-store round-trip**: After loading both shapes field-by-field into LLVM SSA values, the codegen stores them back to `%eq.self` and `%eq.other` allocas, then re-loads via GEP for payload access. This is a load -> insertvalue -> store -> GEP -> load round-trip. At -O1+, SROA (Scalar Replacement of Aggregates) will eliminate this. In an ideal codegen, the payload fields could be extracted directly from the GEP'd input pointers without the intermediate alloca.

**Native code** (`_ori_Shape$eq`, 236 bytes):
The native code is larger due to the alloca round-trip and switch dispatch. Key observations:
- Tag comparison is a direct `cmp` on loaded i64 values
- The `switch` compiles to a `test`/`je` for tag 0 (Circle) and `sub $1`/`je` for tag 1 (Rect) -- linear scan, acceptable for 2 variants
- Per-variant field comparisons are direct `cmp` on loaded values
- The `eq.v.Rect` block correctly chains two comparisons (w first, then h) with short-circuit

**Verdict**: Correct. The C3 bug is definitively fixed. Codegen quality is acceptable for debug mode with the known load-store round-trip overhead.

**Severity**: LOW -- alloca round-trip in `Shape$eq` adds unnecessary memory operations in debug mode. SROA eliminates at -O1+.

### 5. `==` and `!=` Desugaring

In the user functions, `==` desugars to a direct call to the derived `$eq` method:
```llvm
%eq_trait = call fastcc i1 @"_ori_Point$eq"(%ori.Point { i64 10, i64 20 }, %ori.Point { i64 10, i64 20 })
```

And `!=` desugars to `$eq` + `xor`:
```llvm
%eq_trait1 = call fastcc i1 @"_ori_Point$eq"(%ori.Point { i64 10, i64 20 }, %ori.Point { i64 10, i64 30 })
%neq = xor i1 %eq_trait1, true
```

This is the correct pattern: `a != b` is `!(a == b)`, implemented as `xor i1 %result, true`. No separate `$ne` method is needed.

For `Shape` (by-pointer), the calling convention correctly uses alloca:
```llvm
%ref_arg = alloca %ori.Shape, align 8
store %ori.Shape { i64 0, [2 x i64] [i64 10, i64 0] }, ptr %ref_arg, align 8
; ... same for ref_arg1 ...
%eq_trait = call fastcc i1 @"_ori_Shape$eq"(ptr %ref_arg, ptr %ref_arg1)
```

**Verdict**: Correct desugaring. `==` calls `$eq` directly, `!=` inverts via `xor`. Pointer-passing for >16B types is sound.

**Severity**: None.

### 6. Control Flow & Block Structure

**`_ori_check_struct_eq`**: 7 blocks (bb0, bb1, bb2, bb3, bb4, bb5, bb6, add.ok, add.ovf_panic). Two equality checks, each producing a diamond (true/false -> phi merge), then overflow-checked addition. Clean.

**`_ori_check_sum_eq`**: Same structure as check_struct_eq. Two equality checks with diamond branches. Clean.

**`_ori_check_nested`**: Same pattern but with alloca for Shape values (by-pointer passing). Four allocas for the two `$eq` calls. Clean.

**`_ori_main`**: 6 blocks. Three sequential function calls with unconditional branches between them (same sequential block merging pattern as prior journeys), two overflow-checked additions.

**Verdict**: All control flow is correct. No unreachable blocks, no dead code. The diamond pattern for `if eq then X else 0` is the expected codegen.

**Severity**: LOW -- sequential block merging in `_ori_main` (same finding as J2).

### 7. Overflow Checking

Five overflow-checked additions across the module:
- `check_struct_eq`: `same + diff` (1 check)
- `check_sum_eq`: `unit_same + unit_diff` (1 check)
- `check_nested`: `payload_same + payload_diff` (1 check)
- `main`: `a + b` and `(a + b) + c` (2 checks)

All use `@llvm.sadd.with.overflow.i64` with branch to `ori_panic_cstr` + `unreachable`. Correct per Ori spec.

**Severity**: None.

### 8. Duplicate Overflow Messages

```llvm
@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
@ovf.msg.1 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
@ovf.msg.2 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
@ovf.msg.3 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
@ovf.msg.4 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
```

Five identical overflow message constants -- one per addition. This is 4 x 29 = 116 bytes wasted in `.rodata`. Same finding as J2.

**Severity**: LOW -- same known issue.

### 9. Binary Size & Sections

| Metric | Value |
|--------|-------|
| Binary size | 6,561,584 bytes (6.26 MiB) |
| .text | 890,497 bytes (869 KiB) |
| .rodata | 136,504 bytes (133 KiB) |
| User code | ~630 bytes total |
| Debug info | ~4.8 MiB (.debug_*) |

**User code breakdown** (from symbol table):

| Symbol | Size (bytes) | Description |
|--------|-------------|-------------|
| `_ori_check_struct_eq` | 159 (0x9f) | Struct `==`/`!=` checks |
| `_ori_check_sum_eq` | 141 (0x8d) | Unit sum `==`/`!=` checks |
| `_ori_check_nested` | 262 (0x106) | Payload sum `==`/`!=` checks (alloca overhead) |
| `_ori_main` | 114 (0x72) | Call + add |
| `_ori_Point$eq` | 40 (0x28) | Struct derived Eq |
| `_ori_Color$eq` | 11 (0x0b) | Unit-variant derived Eq |
| `_ori_Shape$eq` | 236 (0xec) | Payload-variant derived Eq |
| `main` (wrapper) | 8 | C entry point |
| **Total** | **971** | |

Notable: `_ori_Color$eq` at 11 bytes is remarkably lean for a derived method. `_ori_Shape$eq` at 236 bytes is the largest due to the switch dispatch and alloca round-trip, but this is reasonable for a 2-variant enum with payload fields.

Binary size is consistent with prior journeys -- runtime-dominated, user code is negligible.

**Severity**: None.

## Findings Summary

| # | Category | Severity | Description | New? |
|---|----------|----------|-------------|------|
| 1 | Missing `nounwind` on derived methods | LOW | `$eq` methods lack `nounwind` -- emitted outside nounwind analysis | **Yes** (J11) |
| 2 | Alloca round-trip in `Shape$eq` | LOW | Load-insertvalue-store-GEP-load pattern; eliminated by SROA at -O1+ | **Yes** (J11) |
| 3 | Sequential block merging | LOW | Unconditional `br` between sequential blocks in `_ori_main` | No (J2) |
| 4 | Overflow message dedup | LOW | 5 identical `ovf.msg` constants not merged | No (J2) |

## Derive-Specific Observations

### Derived Eq Codegen Quality

The derived `$eq` codegen demonstrates three distinct patterns:

1. **Struct**: Lexicographic short-circuit field comparison using `extractvalue` + `icmp`. Optimal -- no unnecessary work when early fields differ.

2. **Unit-variant enum**: Single tag comparison. Optimal -- 11 bytes of native code for 3-variant enum.

3. **Payload-variant enum**: Tag check -> switch dispatch -> per-variant field comparison with short-circuit. Architecturally correct. The per-variant blocks ensure that only the fields relevant to the matched variant are compared (Circle compares 1 field; Rect compares 2 fields).

### C3 Bug Resolution

The previously-critical C3 bug (payload sum type `$eq` not generated) is definitively fixed. Evidence:
- `_ori_Shape$eq` is present in both LLVM IR and symbol table
- It correctly handles both Circle (1 field) and Rect (2 fields) variants
- The `switch` dispatch on tag value routes to variant-specific comparison blocks
- Short-circuit behavior is preserved within each variant (Rect checks `w` before `h`)
- Both `==` and `!=` work correctly in AOT (exit code 33 = 7 + 11 + 15)

### Passing Convention for Derived Methods

| Type | Size | Passing | Convention |
|------|------|---------|------------|
| `Point` (2 x i64) | 16 bytes | By value | Fits in 2 registers (SysV ABI) |
| `Color` (1 x i64) | 8 bytes | By value | Fits in 1 register |
| `Shape` (1 x i64 + [2 x i64]) | 24 bytes | By pointer | Exceeds 16-byte threshold |

This is consistent with the struct passing convention established in J4 and J8.

## Cross-Journey Observations

| Feature | First Tested | Journey 11 Status |
|---------|-------------|------------------|
| `#[derive(Eq)]` on structs | J11 | Working |
| `#[derive(Eq)]` on unit-variant enums | J11 | Working |
| `#[derive(Eq)]` on payload-variant enums | J11 | Working (C3 FIX confirmed) |
| `==` operator via derived trait | J11 | Working (calls `$eq`) |
| `!=` operator via derived trait | J11 | Working (`$eq` + `xor i1 true`) |
| Short-circuit field comparison | J11 | Working |
| Switch dispatch for variant-specific `$eq` | J11 | Working |
| Derived method codegen (`derive_codegen`) | J11 | Working |

## Actionable Items

| # | Action | Priority | Description |
|---|--------|----------|-------------|
| 1 | Include derived methods in nounwind analysis | LOW | Derived `$eq`/`$compare`/`$hash` methods should participate in the fixed-point nounwind analysis or be unconditionally marked `nounwind` when they cannot unwind |
| 2 | Eliminate alloca round-trip in enum `$eq` | LOW | Payload-variant `$eq` could GEP directly into the input pointers instead of load->store->GEP. This would reduce debug-mode overhead for enum equality |
