# Journey 2 Results: "I am a branch"

**Date**: 2026-03-03
**Status**: PASS -- both eval and AOT produce correct result (exit code 17)

## Source

```ori
@my_abs (n: int) -> int = if n < 0 then -n else n;
@my_max (a: int, b: int) -> int = if a > b then a else b;
@my_sign (n: int) -> int =
    if n > 0 then 1
    else (if n < 0 then -1 else 0);

@main () -> int = {
    let a = my_abs(n: -7);       // = 7
    let b = my_max(a: 3, b: 10); // = 10
    let c = my_sign(n: 0);       // = 0
    a + b + c                    // = 17
}
```

**Features exercised**: if/else, comparison operators (`<`, `>`), unary negation, nested conditionals, multiple function calls, named arguments, let bindings, integer addition.

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 17        | 17       | (none) | (none) | PASS   |
| AOT     | 17        | 17       | (none) | (none) | PASS   |

## Pipeline Trace Summary

### Lexer
- Source: 567 bytes, 140 tokens, 0 errors
- Prelude: 10331 bytes, 1516 tokens, 0 errors
- Clean pass, no issues.

### Parser
- 4 functions, 40 expressions, 0 errors, 0 warnings
- Correctly parsed `if`/`then`/`else` expressions, unary minus, nested parenthesized `if`
- Prelude: 9 functions, 39 traits, 46 expressions, 0 errors

### Canonicalization
- User module: 4 functions, 43 canon nodes, 6 constants, 0 decision trees
- Prelude: 9 functions, 46 canon nodes, 4 decision trees
- Clean pass.

### Type Checker
- Registration, signature collection, body checking: all complete, 0 errors
- 6 generic prelude imports required AST fallback (hash-first miss): `len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`
- 3 non-generic prelude imports hit hash-first: `compare`, `min`, `max`
- User module: 4 functions, 0 tests, 0 impls -- checked successfully

### ARC Pipeline
- Type registration: Ordering, PanicInfo, TraceEntry, FormatSpec-related enums/structs
- 4 user functions declared: `my_abs`, `my_max`, `my_sign`, `main`
- All use `fastcc` calling convention (except `main` which uses C ABI)
- Nounwind analysis: 2 fixed-point passes, all 4 functions marked nounwind
- Entry point wrapper: `main()` -> `_ori_main()` with `trunc i64 to i32`

### Evaluator
- Traced full execution:
  - `my_abs(-7)`: `Lt(-7, 0)` = true, `Neg(-7)` = 7
  - `my_max(3, 10)`: `Gt(3, 10)` = false, returns 10
  - `my_sign(0)`: `Gt(0, 0)` = false, `Lt(0, 0)` = false, returns 0
  - `Add(7, 10)` = 17, `Add(17, 0)` = 17
- All operations correct, no errors.

## LLVM Deep Scrutiny (9 Categories)

### 1. Attributes & Calling Convention

| Function | fastcc | nounwind | noalias | Status |
|----------|--------|----------|---------|--------|
| `_ori_my_abs` | Yes | Yes | N/A (scalar) | OK |
| `_ori_my_max` | Yes | Yes | N/A (scalar) | OK |
| `_ori_my_sign` | Yes | Yes | N/A (scalar) | OK |
| `_ori_main` | No (C ABI) | Yes | N/A (scalar) | OK |
| `main` (wrapper) | No (C ABI) | No | N/A | OK |
| `ori_panic_cstr` | No | No | N/A | OK -- `cold` attr present |

**Verdict**: All correct. Internal functions use `fastcc`, entry point uses C ABI. `nounwind` on all user functions (panic paths end in `unreachable`). `ori_panic_cstr` correctly marked `cold`.

**Severity**: None.

### 2. Control Flow & Branch Structure

**`_ori_my_abs`**: 4 blocks (bb0 -> bb1|bb2 -> bb3). Compare `slt i64 %0, 0`, branch to negate or passthrough, phi merge. Correct diamond pattern.

**`_ori_my_max`**: 4 blocks. Compare `sgt i64 %0, %1`, phi selects between %0 and %1. Correct diamond.

**`_ori_my_sign`**: 7 blocks. Outer if: `sgt i64 %0, 0` -> bb1 (return 1) or bb2 (inner if). Inner if: `slt i64 %0, 0` -> bb4 (-1) or bb5 (0), phi merge at bb6, then br to bb3 for outer phi merge. Correct nested conditional structure.

**`_ori_main`**: Linear call sequence with overflow-checked additions. Two overflow panic paths.

**Verdict**: All control flow is correct. No unreachable blocks, no dead code.

**Severity**: None.

### 3. Phi Nodes & Value Flow

| Function | Phi Nodes | Correctness |
|----------|-----------|-------------|
| `_ori_my_abs` | `%v7 = phi [%0, bb2], [%neg, bb1]` | Correct: n if n>=0, -n if n<0 |
| `_ori_my_max` | `%v7 = phi [%1, bb2], [%0, bb1]` | Correct: b if a<=b, a if a>b |
| `_ori_my_sign` (inner) | `%v10 = phi [0, bb5], [-1, bb4]` | Correct: 0 if n>=0, -1 if n<0 |
| `_ori_my_sign` (outer) | `%v11 = phi [%v10, bb6], [1, bb1]` | Correct: 1 if n>0, inner result otherwise |

**Verdict**: All phi nodes correct. Values and predecessor labels match.

**Severity**: None.

### 4. Overflow Checking

**Additions in `_ori_main`**:
- `%add = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)` -- checks 7+10
- `%add3 = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call2)` -- checks 17+0
- Both branch to `ori_panic_cstr` on overflow, followed by `unreachable`

**Unary negation in `_ori_my_abs`**: Uses `sub i64 0, %0` (no overflow check).

**Issue (MEDIUM)**: Unary negation of `INT_MIN` (`-9223372036854775808`) would silently wrap to `INT_MIN` since `sub i64 0, INT_MIN` overflows without checking. Per Ori spec, integer overflow panics. This is consistent with other languages -- negation of INT_MIN is undefined/overflow. Should use `@llvm.ssub.with.overflow.i64(i64 0, i64 %0)` to trap.

**Severity**: MEDIUM -- edge case, not triggered by this journey's inputs, but a semantic correctness issue for unary negation in general.

### 5. Redundant Blocks & Unnecessary Branches

**`_ori_my_abs`**: bb1 and bb2 both just `br label %bb3` -- could be eliminated with a `select` instruction:
```llvm
; Ideal:
%lt = icmp slt i64 %0, 0
%neg = sub i64 0, %0
%result = select i1 %lt, i64 %neg, i64 %0
ret i64 %result
```
Current: 4 blocks, 7 instructions. Ideal: 1 block, 4 instructions.

**`_ori_my_max`**: Same pattern -- diamond with trivial phi could be a `select`:
```llvm
%gt = icmp sgt i64 %0, %1
%result = select i1 %gt, i64 %0, i64 %1
ret i64 %result
```
Current: 4 blocks, 6 instructions. Ideal: 1 block, 3 instructions.

**`_ori_my_sign`**: bb4 and bb5 are trivial bridges to bb6 -- could be inner `select`. bb1 is a trivial bridge to bb3 -- could be outer `select`. The entire function could be:
```llvm
; Ideal:
%gt = icmp sgt i64 %0, 0
%lt = icmp slt i64 %0, 0
%inner = select i1 %lt, i64 -1, i64 0
%result = select i1 %gt, i64 1, i64 %inner
ret i64 %result
```
Current: 7 blocks, 11 instructions. Ideal: 1 block, 5 instructions.

**`_ori_main`**: bb0->bb1, bb1->bb3, bb3->bb5 have unconditional branches between them that serve no purpose. Could be merged:
```llvm
; The sequence:
;   %call = call ... @_ori_my_abs(i64 -7)
;   br label %bb1
; bb1:
;   %call1 = call ... @_ori_my_max(i64 3, i64 10)
;   br label %bb3
; ...
; Could just be one block with all three calls.
```
3 unnecessary `br` instructions and 3 unnecessary block labels.

**Severity**: MEDIUM -- overhead ratio for small functions is significant (~2x instruction count for abs/max). LLVM opt will clean these up, but the unoptimized IR is notably verbose.

### 6. Constant Propagation Opportunities

All calls in `_ori_main` use constant arguments:
- `@_ori_my_abs(i64 -7)` -- result is always 7
- `@_ori_my_max(i64 3, i64 10)` -- result is always 10
- `@_ori_my_sign(i64 0)` -- result is always 0

A constant-folding pass could replace the entire main with `ret i64 17`. This is acceptable at the unoptimized IR level -- LLVM `-O1` or higher will inline and fold.

**Severity**: LOW -- expected at `-O0`. Not a compiler bug.

### 7. Duplicate Overflow Messages

```llvm
@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
@ovf.msg.1 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
```

Two identical string constants for the two addition overflow checks. These should be deduplicated -- one global suffices for both panic sites.

**Severity**: LOW -- 29 bytes wasted in .rodata. Trivial but indicates the string dedup pass is not merging identical overflow messages.

### 8. Native Code Quality (Disassembly)

**`_ori_my_abs`** (47 bytes, 14 instructions):
- Spills argument to stack (`mov %rdi, -0x8(%rsp)`) then reloads -- unnecessary for a single register value
- Could be: `test %rdi, %rdi` / `jge .done` / `neg %rax` / `ret`
- Ideal: ~5 instructions. Actual: 14 instructions. Overhead: ~2.8x

**`_ori_my_max`** (46 bytes, 13 instructions):
- Same stack spill pattern for both arguments
- Could be: `cmp %rsi, %rdi` / `cmovle %rsi, %rdi` / `mov %rdi, %rax` / `ret`
- Ideal: ~4 instructions. Actual: 13 instructions. Overhead: ~3.25x

**`_ori_my_sign`** (78 bytes, 22 instructions):
- Multiple stack spills and reloads
- Ideal: ~7 instructions with two compares and conditional moves
- Overhead: ~3.1x

**`_ori_main`** (136 bytes, 34 instructions):
- Clean call sequence, overflow checks inline
- Stack frame of 0x28 (40 bytes) for 3 local i64 values + scratch = reasonable
- The `xor %eax, %eax` / `mov %eax, %edi` sequence for zero arg is slightly odd (could be `xor %edi, %edi`) but functionally correct

**Note**: All native code overhead is from unoptimized compilation (`-O0` equivalent). The stack spills are a consequence of the LLVM IR phi pattern not being lowered to cmov/select. At `-O2`, LLVM would produce near-ideal code.

**Severity**: LOW -- expected at debug build quality. Not a compiler issue.

### 9. Binary Size & Sections

| Metric | Value |
|--------|-------|
| Binary size | 6,561,448 bytes (6.26 MiB) |
| .text | 889,809 bytes (868 KiB) |
| .rodata | 136,504 bytes (133 KiB) |
| User code | ~307 bytes (abs+max+sign+main+wrapper) |
| Debug info | ~4.8 MiB (.debug_*) |

User code is 307 bytes out of 889,809 bytes of .text (0.035%). The rest is `ori_rt` (runtime). This is consistent with Journey 1 expectations -- the runtime dominates for trivial programs.

**Severity**: None -- expected for debug builds with statically-linked runtime.

## Findings Summary

| # | Category | Severity | Description |
|---|----------|----------|-------------|
| 1 | Overflow | MEDIUM | Unary negation (`sub i64 0, %0`) lacks overflow check; `-INT_MIN` wraps silently |
| 2 | Redundant blocks | MEDIUM | Trivial if/else diamonds emit 4 blocks + phi instead of `select` instruction |
| 3 | Redundant branches | LOW | `_ori_main` has 3 unnecessary unconditional `br` between sequential blocks |
| 4 | String dedup | LOW | Two identical `ovf.msg` constants not merged |
| 5 | Native overhead | LOW | Debug-mode stack spills; ~3x instruction overhead vs ideal (expected at -O0) |
| 6 | Constant folding | LOW | Constant arguments not folded at IR level (expected, LLVM handles at -O1+) |

## Cross-Journey Observations (vs Journey 1)

- **New features working**: if/else, comparisons (`<`, `>`), unary negation, nested conditionals, multiple function calls
- **Consistent patterns**: Same `fastcc` + `nounwind` attribute hygiene as J1 would have
- **Overflow checking**: Additions checked (good), but negation not checked (new finding for J2)
- **Block structure**: Diamond if/else pattern is functional but not optimized to `select`
- **Binary size**: Same ~6.26 MiB (runtime-dominated), confirming minimal user code footprint

## Actionable Items

1. **Negation overflow check** (MEDIUM): Add `@llvm.ssub.with.overflow.i64(i64 0, i64 %0)` for unary minus codegen, or use the nsw flag with a separate check. This affects all programs using unary negation.

2. **Select optimization** (MEDIUM): For simple if/else expressions where both branches are trivial (no side effects, single value), emit `select` instead of branch+phi diamond. This is a significant win for scalar code.

3. **Block merging in sequences** (LOW): Sequential let bindings each get their own block with unconditional branch to next. These could be merged into a single block.

4. **Overflow message dedup** (LOW): Share a single `@ovf.msg` global for all same-message panic sites within a module.
