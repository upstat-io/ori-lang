# Journey 12 Results: "I am an option"

**Date**: 2026-03-03
**Status**: PASS -- both eval and AOT produce correct result (exit code 33)
**Historical**: Previously CRITICAL (C4) -- Option match tag inversion caused silent miscompilation. Fixed in commit `77fe984c`.

## Source

```ori
// Journey 12: "I am an option"
// Features: Option<T>, Some/None, match on Option, ? propagation
// Expected: check_some() + check_none() + check_chain() + check_prop() = 20 + 5 + 8 + 0 = 33

@safe_div (a: int, b: int) -> Option<int> =
    if b == 0 then None else Some(a / b);

@unwrap_or (opt: Option<int>, default: int) -> int =
    match opt { Some(v) -> v, None -> default }

@check_some () -> int = {
    let a = safe_div(a: 100, b: 5);
    unwrap_or(opt: a, default: 0)
    // = 20
}

@check_none () -> int = {
    let b = safe_div(a: 100, b: 0);
    unwrap_or(opt: b, default: 5)
    // = 5
}

@check_chain () -> int = {
    let x = unwrap_or(opt: safe_div(a: 80, b: 10), default: 0);
    let y = unwrap_or(opt: safe_div(a: 50, b: 0), default: 0);
    x + y
    // = 8 + 0 = 8
}

@try_div (a: int, b: int, c: int) -> Option<int> = {
    let x = safe_div(a: a, b: b)?;
    safe_div(a: x, b: c)
}

@check_prop () -> int = {
    let ok = unwrap_or(opt: try_div(a: 1000, b: 10, c: 5), default: -1);
    let fail_first = unwrap_or(opt: try_div(a: 1000, b: 0, c: 5), default: -10);
    let fail_second = unwrap_or(opt: try_div(a: 1000, b: 10, c: 0), default: -10);
    ok + fail_first + fail_second
    // = 20 + (-10) + (-10) = 0
}

@main () -> int = {
    let a = check_some();     // = 20
    let b = check_none();     // = 5
    let c = check_chain();    // = 8
    let d = check_prop();     // = 0
    a + b + c + d             // = 33
}
```

**Features exercised**: `Option<T>` generic type, `Some`/`None` constructors, `match` on `Option`, `?` propagation operator on `Option`, function composition (nested calls), named arguments, let bindings, negative integer defaults, integer addition.

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 33        | 33       | (none) | (none) | PASS   |
| AOT     | 33        | 33       | (none) | compile: 0.15s | PASS   |

## Pipeline Trace Summary

### Lexer
- Source: 1,446 bytes, 426 tokens, 0 errors
- Prelude: 10,331 bytes, 1,516 tokens, 0 errors
- Clean pass, no issues.

### Parser
- 8 functions (`safe_div`, `unwrap_or`, `check_some`, `check_none`, `check_chain`, `try_div`, `check_prop`, `main`), 0 type definitions, 0 errors, 0 warnings
- Correctly parsed `match` expression with variant patterns (`Some(v)`, `None`)
- Correctly parsed `?` postfix operator in `try_div`
- Parse contexts include: 8x "function definition", 1x "match expression", 5x "expression" (block bodies), 15x "function call"
- Primary nodes include: `if` keyword, `Some`/`None` constructors, `match` keyword, integer literals (100, 5, 0, 80, 10, 50, 1000, -1, -10), identifiers

### Type Checker
- Prelude: registration, signatures, body checking -- all complete for 9 functions, 0 tests, 0 impls
- User module: registration, signatures, body checking -- all complete for 8 functions, 0 tests, 0 impls
- Import resolution: hash-first miss (AST fallback) for 6 generic builtins (`len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`); hash-first hit for `compare`, `min`, `max`
- No type errors. `Option<int>` correctly instantiated as generic. `?` operator correctly typed for `Option` return context.

### Canonicalization
- User module: 117 canon nodes, 8 roots, 0 method roots, 6 constants, 1 decision tree
- Prelude: 46 canon nodes, 9 roots, 6 constants, 4 decision trees
- The 1 user decision tree corresponds to the `match opt { Some(v) -> v, None -> default }` in `unwrap_or`.
- The `?` operator in `try_div` is desugared before canonicalization (into a match on Option with early return for None).
- The 8 roots are the 8 user functions.

### ARC Pipeline
- Type registration: 6 user types registered:
  - Ordering (Idx::ORDERING): enum with 3 unit variants (Less, Equal, Greater)
  - PanicInfo (Idx 117): struct, boxed (6 fields)
  - TraceEntry (Idx 125): struct, boxed (4 fields: function, location, line, column)
  - FormatType (Idx 194): enum with 8 unit variants
  - Sign (Idx 192): enum with 3 unit variants
  - Alignment (Idx 190): enum with 3 unit variants
  - Note: `Option<int>` is NOT a registered user type -- it uses the built-in `{ i64, i64 }` representation.
- 8 user functions declared, all FastCC with direct return except `main` (C ABI):
  - `safe_div`: 2 params, returns `{ i64, i64 }` (Option<int>)
  - `unwrap_or`: 2 params (`{ i64, i64 }` + `i64`), returns `i64`
  - `check_some`, `check_none`: 0 params, returns `i64`
  - `check_chain`: 0 params, returns `i64`
  - `try_div`: 3 params, returns `{ i64, i64 }` (Option<int>)
  - `check_prop`: 0 params, returns `i64`
  - `main`: 0 params, C ABI, returns `i64`
- Nounwind analysis: 2 fixed-point passes, all 8 functions marked nounwind, 0 mono-propagated
- Entry point wrapper: `main()` -> `_ori_main()` with `returns_int=true`, `has_args=false`, `has_panic=false`

### Evaluator
- Traced full execution of `@main`:
  - `check_some()`: calls `safe_div(100, 5)` -> `b != 0`, takes else branch -> `Some(20)`. Then `unwrap_or(Some(20), 0)` -> match routes to `Some(v)`, binds `v = 20`, returns 20.
  - `check_none()`: calls `safe_div(100, 0)` -> `b == 0`, takes then branch -> `None`. Then `unwrap_or(None, 5)` -> match routes to `None` arm, returns default 5.
  - `check_chain()`: `safe_div(80, 10)` -> `Some(8)`, `unwrap_or(Some(8), 0)` -> 8. `safe_div(50, 0)` -> `None`, `unwrap_or(None, 0)` -> 0. Sum: 8 + 0 = 8.
  - `check_prop()`: `try_div(1000, 10, 5)` -> `safe_div(1000, 10)` = `Some(100)`, `?` unwraps to 100, `safe_div(100, 5)` = `Some(20)`. `unwrap_or(Some(20), -1)` = 20. `try_div(1000, 0, 5)` -> `safe_div(1000, 0)` = `None`, `?` propagates `None` immediately. `unwrap_or(None, -10)` = -10. `try_div(1000, 10, 0)` -> `safe_div(1000, 10)` = `Some(100)`, `?` unwraps to 100, `safe_div(100, 0)` = `None`. `unwrap_or(None, -10)` = -10. Sum: 20 + (-10) + (-10) = 0.
  - Final: 20 + 5 + 8 + 0 = 33.
- All operations correct, no errors.

## LLVM Deep Scrutiny (9 Categories)

### 1. Option<int> Representation

```llvm
; Option<int> is represented as:
{ i64, i64 }
; Field 0: discriminant tag (0 = Some, 1 = None)
; Field 1: payload (int value for Some, undefined for None)
```

**Analysis**: `Option<int>` uses a flat two-word struct rather than a named type. This is the compiler's built-in representation for `Option<T>` when `T` fits in a single machine word. The tag occupies the first i64, the payload the second. This is 16 bytes total -- 8 bytes of overhead from the tag.

**Tag values**: `Some = 0`, `None = 1`. This convention places the "present" variant at tag 0 and the "absent" variant at tag 1. This is correct post-C4 fix. Before the fix, these were inverted, causing `Some` to be treated as `None` and vice versa in decision tree matching.

**None construction**: `{ i64 1, i64 0 }` -- tag=1, payload=0 (the 0 is undefined/unused but zeroinitializer is clean).

**Some construction**: `{ i64 0, i64 <value> }` -- tag=0, payload=value. Visible in `safe_div`:
```llvm
%variant.f0 = insertvalue { i64, i64 } zeroinitializer, i64 %div, 1
```
This starts from zeroinitializer (tag=0=Some, payload=0), then inserts the division result into field 1. Efficient single-instruction construction.

**Verdict**: Clean and correct. The two-word flat representation avoids any heap allocation for `Option<int>`.

**Severity**: None.

### 2. Attributes & Calling Convention

| Function | fastcc | nounwind | Status |
|----------|--------|----------|--------|
| `_ori_safe_div` | Yes | Yes | OK |
| `_ori_unwrap_or` | Yes | Yes | OK |
| `_ori_check_some` | Yes | Yes | OK |
| `_ori_check_none` | Yes | Yes | OK |
| `_ori_check_chain` | Yes | Yes | OK |
| `_ori_try_div` | Yes | Yes | OK |
| `_ori_check_prop` | Yes | Yes | OK |
| `_ori_main` | No (C ABI) | Yes | OK |
| `main` (wrapper) | No (C ABI) | No | OK (known L-2) |
| `ori_panic_cstr` | No | No | OK -- `cold` present |

**Verdict**: All 8 user functions correctly marked `fastcc` + `nounwind`. Entry point uses C ABI. Panic helper marked `cold`. Consistent with all previous journeys.

**Severity**: None (existing L-2 about wrapper `nounwind` still applies).

### 3. `@safe_div` -- Option Construction via If/Else

```llvm
define fastcc { i64, i64 } @_ori_safe_div(i64 %0, i64 %1) #0 {
bb0:
  %eq = icmp eq i64 %1, 0
  br i1 %eq, label %bb1, label %bb2

bb1:                              ; b == 0 -> None
  br label %bb3

bb2:                              ; b != 0 -> Some(a / b)
  %div = sdiv i64 %0, %1
  %variant.f0 = insertvalue { i64, i64 } zeroinitializer, i64 %div, 1
  br label %bb3

bb3:                              ; merge
  %v10 = phi { i64, i64 } [ %variant.f0, %bb2 ], [ { i64 1, i64 0 }, %bb1 ]
  ret { i64, i64 } %v10
}
```

**Analysis**: The if/else compiles to a branch on `b == 0`. The None path produces `{ i64 1, i64 0 }` (tag=1, inline constant). The Some path constructs via `insertvalue` from zeroinitializer (tag=0 implicit) with the division result at index 1. The merge block uses a `phi` to select between the two paths.

**Correctness verification**:
- `safe_div(100, 5)`: `%eq` = false (5 != 0), goes to bb2. `%div` = 20. `%variant.f0` = `{0, 20}`. Returns `{0, 20}` = `Some(20)`. Correct.
- `safe_div(100, 0)`: `%eq` = true (0 == 0), goes to bb1. Returns `{1, 0}` = `None`. Correct.

**Quality**: Clean codegen. The `insertvalue` from zeroinitializer is an efficient way to build `Some(value)` -- tag 0 comes free from the zero base, and only the payload field needs an insert. The phi merge is well-formed.

**Issue (LOW)**: bb1 is an empty block containing only `br label %bb3`. It could be eliminated by having bb0 branch directly to bb3 from the `true` path. This is the recurring M-1 pattern (unnecessary block boundary from if/else lowering).

**Severity**: LOW (existing M-1).

### 4. `@unwrap_or` -- Match on Option with Decision Tree

```llvm
define fastcc i64 @_ori_unwrap_or({ i64, i64 } %0, i64 %1) #0 {
bb0:
  %proj.0 = extractvalue { i64, i64 } %0, 0
  switch i64 %proj.0, label %bb4 [
    i64 0, label %bb2
    i64 1, label %bb3
  ]

bb1:                              ; merge
  %v3 = phi i64 [ %1, %bb3 ], [ %proj.1, %bb2 ]
  ret i64 %v3

bb2:                              ; Some(v) arm
  %proj.1 = extractvalue { i64, i64 } %0, 1
  br label %bb1

bb3:                              ; None arm
  br label %bb1

bb4:                              ; unreachable default
  unreachable
}
```

**Analysis**: The match compiles to a `switch` on the discriminant. Tag 0 (Some) goes to bb2 which extracts the payload via `extractvalue`. Tag 1 (None) goes to bb3 which uses the `default` parameter. The merge block phi selects between the two.

**Correctness verification**:
- `unwrap_or(Some(20), 0)`: tag=0, switch to bb2. `%proj.1 = extractvalue {0, 20}, 1` = 20. Returns 20. Correct.
- `unwrap_or(None, 5)`: tag=1, switch to bb3. Returns `%1` = 5. Correct.

**Quality**: This is good codegen. The `extractvalue` for payload extraction is clean -- no alloca+store+GEP+load pattern. This is a significant improvement over J6's record-variant matching. `Option<int>` uses `{ i64, i64 }` (flat struct), so `extractvalue` works directly without needing to go through an array payload.

The `unreachable` default arm is correct -- the match is exhaustive over `Some` and `None`.

**Issue (LOW)**: bb3 (None arm) is an empty block with just `br label %bb1`. Same M-1 pattern.

**Severity**: LOW (existing M-1).

### 5. `@try_div` -- `?` Propagation Codegen

```llvm
define fastcc { i64, i64 } @_ori_try_div(i64 %0, i64 %1, i64 %2) #0 {
bb0:
  %call = call fastcc { i64, i64 } @_ori_safe_div(i64 %0, i64 %1)
  br label %bb1

bb1:
  %proj.0 = extractvalue { i64, i64 } %call, 0
  %eq = icmp eq i64 %proj.0, 0
  br i1 %eq, label %bb3, label %bb4

bb3:                              ; Some path: unwrap and continue
  %proj.1 = extractvalue { i64, i64 } %call, 1
  br label %bb5

bb4:                              ; None path: propagate None
  ret { i64, i64 } { i64 1, i64 0 }

bb5:
  %v11 = phi i64 [ %proj.1, %bb3 ]
  %call1 = call fastcc { i64, i64 } @_ori_safe_div(i64 %v11, i64 %2)
  br label %bb6

bb6:
  ret { i64, i64 } %call1
}
```

**Analysis**: The `?` operator desugars into a discriminant check + early return. The codegen:
1. Calls `safe_div(a, b)`, getting an `Option<int>` result
2. Extracts the tag (`extractvalue ... 0`)
3. Compares tag to 0 (Some): `icmp eq i64 %proj.0, 0`
4. If Some (tag=0): extract payload, continue to second `safe_div` call
5. If None (tag!=0): immediately return `{ i64 1, i64 0 }` (None)

**Correctness verification**:
- `try_div(1000, 10, 5)`: `safe_div(1000, 10)` = `{0, 100}`. Tag=0, Some. Unwrap to 100. `safe_div(100, 5)` = `{0, 20}`. Returns `{0, 20}` = `Some(20)`. Correct.
- `try_div(1000, 0, 5)`: `safe_div(1000, 0)` = `{1, 0}`. Tag=1, None. Early return `{1, 0}` = `None`. Correct. The second `safe_div` is never called.
- `try_div(1000, 10, 0)`: `safe_div(1000, 10)` = `{0, 100}`. Tag=0, Some. Unwrap to 100. `safe_div(100, 0)` = `{1, 0}`. Returns `{1, 0}` = `None`. Correct.

**Quality**: The `?` propagation codegen is clean and correct. The early return for `None` avoids any unnecessary computation. The `icmp eq + br i1` pattern is the right approach for a two-variant check (more efficient than a `switch` for only 2 cases). The None literal `{ i64 1, i64 0 }` is an inline constant return.

**Issue (LOW)**: The `bb0 -> bb1` boundary and the `bb5 -> bb6` boundary are redundant (single-predecessor, unconditional branch). The phi at bb5 has only one incoming edge.

**Severity**: LOW (existing M-1).

### 6. Overflow Checking

Six overflow-checked additions across the program:
- `_ori_check_chain`: 1 addition (x + y)
- `_ori_check_prop`: 2 additions (ok + fail_first, sum + fail_second)
- `_ori_main`: 3 additions (a + b, sum + c, sum + d)

All use `@llvm.sadd.with.overflow.i64` with dedicated panic paths. Each panic path calls `ori_panic_cstr` with an overflow message, followed by `unreachable`.

**Duplicate overflow message strings**:
```llvm
@ovf.msg   = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
@ovf.msg.1 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
@ovf.msg.2 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
@ovf.msg.3 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
@ovf.msg.4 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
@ovf.msg.5 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
```

Six identical 29-byte strings for six overflow check sites. LLVM's linker will merge `unnamed_addr` constants at link time, but the IR is unnecessarily verbose. A single shared constant would suffice.

**Severity**: LOW (existing finding, same as J4/J7 -- string dedup).

### 7. ARC Purity

Zero ARC operations in the generated IR. This is correct -- `Option<int>` is a flat `{ i64, i64 }` value type containing only primitives. No heap allocation, no reference counting needed.

The `?` propagation returns `Option<int>` by value -- no RC increment/decrement on the option itself. The `safe_div` results are passed and returned as SSA values.

**Verdict**: PERFECT -- No RC on scalar-only option types.

### 8. Native Code Quality (Disassembly)

**`_ori_safe_div`** (75 bytes, 16 instructions):
```asm
; save args to stack
mov    %rsi,-0x10(%rsp)
mov    %rdi,-0x8(%rsp)
cmp    $0x0,%rsi              ; b == 0?
jne    <Some path>
; None path:
mov    $0x1,%ecx              ; tag = 1
xor    %eax,%eax              ; payload = 0
jmp    <merge>
; Some path:
mov    -0x10(%rsp),%rcx       ; reload b
mov    -0x8(%rsp),%rax        ; reload a
cqto                          ; sign-extend for division
idiv   %rcx                   ; a / b
xor    %ecx,%ecx              ; tag = 0
; merge:
mov    %rcx,-0x20(%rsp)       ; store tag
mov    %rax,-0x18(%rsp)       ; store payload
mov    -0x20(%rsp),%rax       ; return tag in rax
mov    -0x18(%rsp),%rdx       ; return payload in rdx
ret
```

The `{ i64, i64 }` return convention uses `%rax` for field 0 (tag) and `%rdx` for field 1 (payload). This is the System V AMD64 two-register struct return -- efficient, no memory needed for the return value itself. The stack spills at the end (store+reload) come from the phi node lowering (M-1 pattern).

**`_ori_unwrap_or`** (50 bytes, ~12 instructions):
```asm
mov    %rdx,-0x10(%rsp)       ; save default
mov    %rsi,-0x8(%rsp)        ; save payload
test   %rdi,%rdi              ; tag == 0?
je     <Some>                 ; if Some, jump
; None path:
jmp    <load default>
; merge:
mov    -0x18(%rsp),%rax
ret
; Some path:
mov    -0x8(%rsp),%rax        ; load payload
mov    %rax,-0x18(%rsp)
jmp    <merge>
; None path:
mov    -0x10(%rsp),%rax       ; load default
mov    %rax,-0x18(%rsp)
jmp    <merge>
```

Parameters arrive as `%rdi` (tag), `%rsi` (payload), `%rdx` (default) -- the `{ i64, i64 }` struct is passed as two register args in fastcc. The `test %rdi,%rdi` + `je` for tag==0 (Some) is efficient. The redundant jumps and stack round-trips are from M-1 block boundaries.

Ideal codegen would be:
```asm
test   %rdi,%rdi
cmove  %rsi,%rdx              ; if Some, use payload; else use default
mov    %rdx,%rax
ret
```
4 instructions ideal vs ~12 actual. Overhead: ~3x. This is debug build quality.

**`_ori_try_div`** (101 bytes, ~22 instructions):
```asm
sub    $0x38,%rsp
mov    %rdx,0x20(%rsp)        ; save c
call   _ori_safe_div           ; safe_div(a, b)
mov    %rax,0x28(%rsp)        ; save tag
mov    %rdx,0x30(%rsp)        ; save payload
mov    0x28(%rsp),%rax
cmp    $0x0,%rax              ; tag == 0 (Some)?
jne    <None early return>
; Some path:
mov    0x30(%rsp),%rax        ; unwrap payload = x
jmp    <continue>
; None early return:
xor    %eax,%eax
mov    %eax,%edx              ; payload = 0
mov    $0x1,%eax              ; tag = 1
add    $0x38,%rsp
ret                           ; early return None
; continue:
mov    0x20(%rsp),%rsi        ; c
mov    ...,%rdi               ; x
call   _ori_safe_div           ; safe_div(x, c)
; return result directly
add    $0x38,%rsp
ret
```

The `?` propagation compiles to a clean early return. The None path returns immediately with `{1, 0}` without calling the second `safe_div`. The Some path falls through to the second call. Stack frame is 56 bytes (0x38) for saved values.

**`_ori_main`** (162 bytes, ~38 instructions):
- Clean sequential call pattern: 4 calls to `check_some`, `check_none`, `check_chain`, `check_prop`
- Results saved on stack between calls
- 3 overflow-checked additions (`add` + `seto` + `jo`)
- Stack frame: 56 bytes (0x38)

**Severity**: LOW for all functions (debug build quality, M-1 overhead dominates).

### 9. Binary Size & Sections

| Metric | Value |
|--------|-------|
| Binary size | 6,561,616 bytes (6.26 MiB) |
| .text | 890,449 bytes (870 KiB) |
| .rodata | 136,504 bytes (133 KiB) |
| User code | ~449 bytes (safe_div: 75, unwrap_or: 50, check_some: 62, check_none: 62, check_chain: 132, try_div: 101, check_prop: 254, main: 162, wrapper: ~8) |
| Debug info | ~4.8 MiB (.debug_*) |

User code is ~449 bytes out of ~890 KB .text (0.050%). Consistent with previous journeys -- runtime dominates.

**Severity**: None -- expected for debug builds with statically-linked runtime.

## C4 Fix Verification

The C4 bug (commit `77fe984c`) was an Option match tag inversion in the decision tree:
- **Before fix**: `Some` was mapped to tag 1 and `None` to tag 0 during decision tree compilation, but construction used `Some=0, None=1`. This caused `match opt { Some(v) -> v, None -> default }` to return `default` when given `Some` and extract garbage when given `None`.
- **After fix**: Tags are consistent: `Some=0`, `None=1` in both construction and matching.

**Verified in LLVM IR**:
- Construction in `safe_div`: `zeroinitializer` (tag=0) for Some, `{ i64 1, i64 0 }` (tag=1) for None.
- Matching in `unwrap_or`: `switch ... i64 0, label %bb2` (Some arm), `i64 1, label %bb3` (None arm).
- Propagation in `try_div`: `icmp eq i64 %proj.0, 0` (check for Some=0), early return `{ i64 1, i64 0 }` (None=1).

All three sites are consistent. The fix resolved 114 previously-failing spec tests.

## Findings Summary

| # | Category | Severity | Description | Cross-ref |
|---|----------|----------|-------------|-----------|
| 1 | Redundant blocks | LOW | Empty blocks with `br label` at let-binding boundaries, if/else merges, and `?` propagation continuation | M-1 (J1) |
| 2 | String dedup | LOW | Six identical `ovf.msg` constants (29 bytes each) not merged at IR level | J4, J7 |
| 3 | Phi simplification | LOW | Several single-predecessor phi nodes that could be eliminated (e.g., `try_div` bb5, `unwrap_or` bb1 None edge) | M-1 (J1) |

No new findings. All issues are pre-existing patterns seen in earlier journeys.

## Cross-Journey Observations

- **New features working**: `Option<T>` built-in type, `Some`/`None` constructors, `match` on `Option`, `?` propagation operator with early return for `None`
- **Option representation**: `{ i64, i64 }` flat struct with tag in field 0, payload in field 1. No named LLVM type -- built-in handling. 16 bytes total, passed/returned in two registers (`%rax`/`%rdx` or `%rdi`/`%rsi`).
- **`?` codegen quality**: Clean early-return pattern. None path returns immediately without executing subsequent code. No unnecessary stack manipulation beyond what's needed for call-save.
- **Payload extraction improvement**: `unwrap_or` uses `extractvalue { i64, i64 } %0, 1` directly -- no alloca+store+GEP+load pattern. This is better than J6's record-variant matching. The difference is that `Option<int>` is a flat struct (not `{i64, [N x i64]}` union layout), so `extractvalue` works directly.
- **Tag consistency (C4 FIXED)**: `Some=0, None=1` is now consistent across construction (`safe_div`), matching (`unwrap_or`), and propagation (`try_div`). The C4 fix in `77fe984c` corrected the inversion in 3 locations (flatten.rs, emit.rs, eval decision tree).
- **Consistent patterns**: Same `fastcc` + `nounwind` hygiene. Same M-1 block boundary pattern. Same overflow checking. Same binary size profile. No regression from J1-J8.
- **Nounwind propagation**: All 8 user functions (including `try_div` with `?` operator) correctly marked nounwind in 2 fixed-point passes. The `?` operator does not introduce unwind behavior (it's a value-level branch, not exception-based).

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Correctness | 30% | 10/10 | Both backends produce 33. Option construction, matching, and `?` propagation all correct. C4 fully resolved. |
| Instruction Purity | 20% | 8/10 | `extractvalue` used for payload (no alloca overhead). `?` codegen is clean early-return. Minor redundant blocks. |
| ARC Purity | 15% | 10/10 | Zero RC ops on scalar-only Option<int> |
| Attributes | 15% | 8/10 | All 8 user functions nounwind+fastcc. Missing noreturn on panic (M-2). Missing nounwind on wrapper (L-2). |
| Type Layout | 10% | 9/10 | `{ i64, i64 }` is correct and compact for Option<int>. Could be optimized to single i64 with sentinel for non-nullable types (future). |
| Block Layout | 10% | 7/10 | Switch for match is correct. Branch for `?` is clean. But redundant blocks persist (M-1). |

**Overall Score: 8.8 / 10**

The codegen demonstrates solid Option support with correct type representation, clean `?` propagation via early return, and proper decision tree matching (post-C4 fix). The payload extraction path is notably cleaner than J6's record-variant matching thanks to the flat `{ i64, i64 }` representation. The main quality gap remains the recurring M-1 redundant block pattern at let-binding and if/else boundaries, which is a codegen-wide pattern rather than Option-specific.
