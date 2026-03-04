# Journey 8 Results: "I am generic"

**Date**: 2026-03-03
**Status**: PASS -- both eval and AOT produce correct result (exit code 57)

## Source

```ori
// Journey 8: "I am generic"
// Features: generic functions, generic structs, type inference, monomorphization
// Expected: identity(42) + first(10, 20) + get_value(Box { value: 5 }) = 42 + 10 + 5 = 57
type Box<T> = { value: T }
@identity<T> (x: T) -> T = x;
@first<A, B> (a: A, b: B) -> A = a;
@get_value<T> (b: Box<T>) -> T = b.value;
@main () -> int = {
    let a = identity(x: 42);
    let b = first(a: 10, b: 20);
    let c = get_value(b: Box { value: 5 });
    a + b + c
}
```

**Features exercised**: generic type parameters (single and multi), generic struct types, monomorphization, type inference at call sites, struct field access through generics, named arguments, let bindings, integer addition.

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 57        | 57       | (none) | (none) | PASS   |
| AOT     | 57        | 57       | (none) | compile msg only | PASS   |

## Pipeline Trace Summary

### Lexer
- Source: 585 bytes, 139 tokens, 0 errors
- Prelude: 10331 bytes, 1516 tokens, 0 errors
- Clean pass, no issues.

### Parser
- User module: 4 functions, 1 type, 22 expressions, 0 errors, 0 warnings
- Correctly parsed: `type Box<T>` generic struct definition, generic parameter lists `<T>`, `<A, B>`, struct literal `Box { value: 5 }`, field access `b.value`
- Prelude: 9 functions, 39 traits, 46 expressions, 0 errors

### Canonicalization
- User module: 4 functions, 24 canon nodes, 6 constants, 0 decision trees
- Prelude: 9 functions, 46 canon nodes, 4 decision trees
- Clean pass.

### Type Checker
- Registration, signature collection, body checking: all complete, 0 errors
- 6 generic prelude imports required AST fallback (hash-first miss): `len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`
- 3 non-generic prelude imports hit hash-first: `compare`, `min`, `max`
- **Monomorphization** -- 3 generic instances recorded:
  1. `identity` with `[int]` -- single type param, trivial passthrough
  2. `first` with `[int, int]` -- two type params, returns first
  3. `get_value` with `[int]` -- type param through generic struct `Box<int>`
- **Applied -> Struct resolution**: `Box<int>` (applied generic) registered as concrete struct `Idx(228)` for monomorphized field access
- User module: 4 functions, 0 tests, 0 impls -- checked successfully

### ARC Pipeline
- Type registration: Ordering, PanicInfo, TraceEntry, FormatSpec-related enums/structs (prelude types), plus user's `Box<int>` (`%ori.Box = type { i64 }`)
- 4 user functions declared with monomorphized names:
  - `main` -> `_ori_main` (C ABI, 0 params)
  - `first$m$int_int` -> `_ori_first$24m$24int_int` (fastcc, 2 params)
  - `get_value$m$int` -> `_ori_get_value$24m$24int` (fastcc, 1 param)
  - `identity$m$int` -> `_ori_identity$24m$24int` (fastcc, 1 param)
- Nounwind analysis: 3 fixed-point passes, 7 total nounwind (4 user + 3 mono-propagated), all user functions marked nounwind
- Entry point wrapper: `main()` -> `_ori_main()` with `trunc i64 to i32`

### Evaluator
- Traced full execution:
  - `identity(x: 42)`: resolves ident -> calls body -> evaluates `Ident(x)` -> returns `Int(42)`
  - `first(a: 10, b: 20)`: resolves ident -> calls body -> evaluates `Ident(a)` -> returns `Int(10)`
  - `get_value(b: Box { value: 5 })`: constructs `Struct(Box, {value: 5})` -> calls body -> evaluates `Field(b, value)` -> returns `Int(5)`
  - `Add(42, 10)` = 52, `Add(52, 5)` = 57
- All operations correct, no errors.

## LLVM Deep Scrutiny (9 Categories)

### 1. Attributes & Calling Convention

| Function | fastcc | nounwind | Status |
|----------|--------|----------|--------|
| `_ori_main` | No (C ABI) | Yes | OK |
| `_ori_identity$24m$24int` | Yes | Yes | OK |
| `_ori_first$24m$24int_int` | Yes | Yes | OK |
| `_ori_get_value$24m$24int` | Yes | Yes | OK |
| `main` (wrapper) | No (C ABI) | No | OK |
| `ori_panic_cstr` | No (extern) | No | OK -- `cold` attr present |

**Verdict**: All correct. Monomorphized internal functions use `fastcc`, entry point uses C ABI. `nounwind` on all user functions. `ori_panic_cstr` correctly marked `cold`.

**Severity**: None.

### 2. Monomorphization & Name Mangling

Generic functions are monomorphized with a `$m$` separator followed by concrete type names joined with `_`:

| Generic Signature | Monomorphized Symbol | Correctness |
|-------------------|---------------------|-------------|
| `identity<T>` with `T=int` | `_ori_identity$24m$24int` | Correct |
| `first<A, B>` with `A=int, B=int` | `_ori_first$24m$24int_int` | Correct |
| `get_value<T>` with `T=int` | `_ori_get_value$24m$24int` | Correct |

The `$24` in the symbol names is URL-encoding of `$` (dollar sign). The mangling scheme `name$m$types` produces unique symbols per instantiation. Multi-type-param functions use `_` to separate type names.

**Verdict**: Monomorphization produces correct, unique, deterministic symbol names. Each generic function is instantiated exactly once for its concrete type arguments.

**Severity**: None.

### 3. Generic Struct Lowering

The generic `Box<T>` struct instantiated as `Box<int>` is lowered to:

```llvm
%ori.Box = type { i64 }
```

This is correct -- `Box<int>` has a single `i64` field (`value: int`). The struct is passed by value (fits in a register), not boxed on the heap.

In `_ori_get_value$24m$24int`:
```llvm
define fastcc i64 @"_ori_get_value$24m$24int"(%ori.Box %0) #0 {
bb0:
  %proj.0 = extractvalue %ori.Box %0, 0
  ret i64 %proj.0
}
```

Field access `b.value` correctly lowers to `extractvalue %ori.Box %0, 0` (field index 0). The struct is passed directly as a value (single-field struct fits in one i64 register), not via pointer.

In `_ori_main`:
```llvm
%call2 = call fastcc i64 @"_ori_get_value$24m$24int"(%ori.Box { i64 5 })
```

The struct literal `Box { value: 5 }` is passed as a constant aggregate `%ori.Box { i64 5 }` directly -- no heap allocation, no RC management needed. This is optimal for a single-field value-type struct.

**Verdict**: Generic struct lowering is correct and efficient. No unnecessary indirection.

**Severity**: None.

### 4. Control Flow & Block Structure

**`_ori_identity$24m$24int`**: Single block, `ret i64 %0`. Minimal -- correct.

**`_ori_first$24m$24int_int`**: Single block, `ret i64 %0`. Ignores second parameter. Correct and minimal.

**`_ori_get_value$24m$24int`**: Single block, `extractvalue` + `ret`. Correct and minimal.

**`_ori_main`**: 6 blocks (bb0 -> bb1 -> bb3 -> bb5 -> add.ok -> add.ok6), plus 2 panic blocks. Three calls in sequence, two overflow-checked additions, final `ret`.

**Verdict**: All control flow is correct. Monomorphized functions are trivially simple (single block, no branches). No unreachable blocks, no dead code.

**Severity**: None.

### 5. Overflow Checking

**Additions in `_ori_main`**:
- `%add = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)` -- checks `a + b` (42 + 10)
- `%add3 = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call2)` -- checks `(a + b) + c` (52 + 5)
- Both branch to `ori_panic_cstr` on overflow, followed by `unreachable`

**Verdict**: All integer additions have overflow checks. Consistent with Ori spec (overflow panics). No unary operations in this journey, so the negation issue from J2 is not exercised.

**Severity**: None.

### 6. Redundant Blocks & Unnecessary Branches

**`_ori_main`** has sequential blocks with unconditional branches:
```llvm
bb0:
  %call = call fastcc i64 @"_ori_identity$24m$24int"(i64 42)
  br label %bb1
bb1:
  %call1 = call fastcc i64 @"_ori_first$24m$24int_int"(i64 10, i64 20)
  br label %bb3
bb3:
  %call2 = call fastcc i64 @"_ori_get_value$24m$24int"(%ori.Box { i64 5 })
  br label %bb5
```

Three unconditional branches between sequential blocks. These could be merged into a single entry block. This is the same sequential block merging pattern observed in J2.

**Severity**: LOW -- same finding as J2 (sequential block merging). 3 unnecessary `br` instructions. Not a correctness issue.

### 7. Duplicate Overflow Messages

```llvm
@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
@ovf.msg.1 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
```

Two identical string constants for two addition overflow checks. Same finding as J2.

**Severity**: LOW -- 29 bytes wasted in .rodata. Known issue from J2.

### 8. Native Code Quality (Disassembly)

**`_ori_identity$24m$24int`** (4 bytes, 2 instructions):
```asm
mov    %rdi,%rax   ; copy arg to return register
ret
```
Optimal. Single `mov` + `ret`. Cannot be improved.

**`_ori_first$24m$24int_int`** (4 bytes, 2 instructions):
```asm
mov    %rdi,%rax   ; copy first arg to return register
ret
```
Optimal. Ignores second argument (in `%rsi`). Cannot be improved.

**`_ori_get_value$24m$24int`** (4 bytes, 2 instructions):
```asm
mov    %rdi,%rax   ; extract Box.value (first field = first register)
ret
```
Optimal. The single-field struct `Box<int>` is passed in `%rdi` (same as a bare `i64`), so field extraction is a register move. Cannot be improved.

**`_ori_main`** (134 bytes, ~30 instructions):
- Stack frame: `sub $0x28, %rsp` (40 bytes)
- Three function calls with constant arguments, results spilled to stack
- Two overflow-checked additions using `add` + `seto` + `jo`
- Panic paths use `lea` to load overflow message + call to `ori_panic_cstr`

The native code for `_ori_main` follows the expected unoptimized pattern (stack spills between calls). The monomorphized callees are extremely lean -- each is just 4 bytes.

**`main` (wrapper)** (8 bytes, 3 instructions):
```asm
push   %rax         ; align stack
call   _ori_main
pop    %rcx          ; restore stack (implicit trunc to i32 via eax)
ret
```
Clean wrapper. The `trunc i64 to i32` is implicit -- x86-64 ABI returns i32 in the low 32 bits of %rax, and `_ori_main` returns i64 in %rax, so the C runtime naturally reads the low 32 bits.

**Verdict**: Monomorphized leaf functions produce optimal native code. Main function has expected debug-mode overhead.

**Severity**: None for leaf functions (optimal). LOW for main function (debug-mode stack spills, same as prior journeys).

### 9. Binary Size & Sections

| Metric | Value |
|--------|-------|
| Binary size | 6,561,488 bytes (6.26 MiB) |
| .text | 889,681 bytes (869 KiB) |
| .rodata | 136,504 bytes (133 KiB) |
| User code | ~150 bytes (identity: 4B + first: 4B + get_value: 4B + main: 134B + wrapper: 8B) |
| Debug info | ~4.8 MiB (.debug_*) |

User code footprint is approximately 150 bytes -- smaller than J2 (307 bytes) because the monomorphized generic functions are trivially simple (single `mov`+`ret`). The binary size is identical to previous journeys -- runtime-dominated.

**Symbol table**: 4 Ori symbols exported:
- `_ori_main` (134 bytes)
- `_ori_identity$24m$24int` (4 bytes)
- `_ori_first$24m$24int_int` (4 bytes)
- `_ori_get_value$24m$24int` (4 bytes)

**Severity**: None -- consistent with prior journeys.

## Findings Summary

| # | Category | Severity | Description | New? |
|---|----------|----------|-------------|------|
| 1 | Sequential block merging | LOW | 3 unconditional `br` between sequential blocks in `_ori_main` | No (J2) |
| 2 | Overflow message dedup | LOW | Two identical `ovf.msg` constants not merged | No (J2) |

No new findings in this journey. All previously identified issues (sequential block merging, overflow message dedup) are consistent with J2.

## Generics-Specific Observations

### Monomorphization Quality

The monomorphization pipeline is working correctly and efficiently:

1. **Type inference**: All three generic call sites correctly infer their type arguments from the concrete argument types without explicit annotation.
2. **One instance per unique type combination**: `identity<int>`, `first<int, int>`, `get_value<int>` -- each gets exactly one instantiation.
3. **Applied -> Struct resolution**: The `Box<T>` generic applied as `Box<int>` is correctly resolved to a concrete struct type `%ori.Box = type { i64 }` in LLVM IR.
4. **No generic code duplication**: No polymorphic dispatch, no runtime type information, no vtables. Pure compile-time specialization.
5. **Optimal leaf code**: Monomorphized functions produce identical native code to what hand-written non-generic equivalents would produce.

### Struct Passing Convention

The `Box<int>` struct (1 field, 8 bytes) is passed by value in a register (`%rdi`). This is correct per the x86-64 System V ABI -- structs up to 16 bytes that contain only integer/pointer fields are passed in registers. The `extractvalue` instruction correctly projects the field without memory access.

### Nounwind Analysis

The fixed-point nounwind analysis correctly identifies that all 4 user functions (plus 3 mono-propagated) are nounwind -- none of them can throw. The 3 fixed-point passes (vs 2 in J2) reflect the slightly more complex call graph with monomorphized callees.

## Cross-Journey Observations

| Feature | First Tested | Journey 8 Status |
|---------|-------------|-----------------|
| Generic functions (single type param) | J8 | Working |
| Generic functions (multi type param) | J8 | Working |
| Generic struct types | J8 | Working |
| Monomorphization | J8 | Working |
| Type inference at call sites | J8 | Working |
| Struct field access through generics | J8 | Working |
| Applied -> Struct type resolution | J8 | Working |
| Monomorphized symbol mangling | J8 | Working |
| Value-passing of small structs | J8 | Working |

## Actionable Items

No new actionable items. All findings are pre-existing from earlier journeys (J2).
