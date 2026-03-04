# Journey 3 Results: "I am recursive"

**Date**: 2026-03-03
**Status**: PASS (Eval=61, AOT=61)

## Source

```ori
@fib (n: int) -> int =
    if n <= 1 then n
    else fib(n: n - 1) + fib(n: n - 2);

@gcd (a: int, b: int) -> int =
    if b == 0 then a
    else gcd(a: b, b: a % b);

@main () -> int = {
    let f = fib(n: 10);        // = 55
    let g = gcd(a: 48, b: 18); // = 6
    f + g                      // = 61
}
```

**Features exercised**: recursion, `if/then/else` expressions, `<=` comparison, `==` equality, `%` modulo, `+`/`-` arithmetic, named arguments, block expression with `let` bindings, `() -> int` main entry point.

## Phase Trace Summary

### Lexer
- Source: 452 bytes, 123 tokens, 0 errors.
- Prelude: 10,331 bytes, 1,516 tokens, 0 errors.

### Parser
- User module: 3 functions, 0 tests, 0 types, 0 traits, 0 impls, 0 imports, 38 expressions, 0 errors, 0 warnings.
- Prelude module: 9 functions, 0 tests, 39 traits, 46 expressions, 0 errors, 0 warnings.
- Parse contexts entered correctly: `function definition`, `if expression`, `function call`, `expression` (block body).

### Canonicalization
- User module: 38 source expressions -> 40 canon nodes, 3 roots (fib, gcd, main), 6 constants, 0 decision trees.
- Prelude module: 46 source expressions -> 46 canon nodes, 9 roots, 4 decision trees.

### Type Checker
- Prelude: 9 functions registered, signatures collected, bodies checked. Hash-first import lookups: hits for `compare`, `min`, `max`; AST fallbacks for generic builtins (`len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`).
- User module: 3 functions registered, signatures collected, bodies checked. 0 errors.

### Interpreter (Eval)
- Exit code: 61 (correct).
- Eval trace: 2,263 lines showing deep recursive call trees for `fib(10)`.
- Recursive call depth visible in trace nesting: `eval_call` spans nesting up to ~10 levels deep for fib(10).
- Operations traced: `LtEq`, `Sub`, `Add` for fib; `Eq`, `Rem` for gcd (implied by `%` desugaring).

### ARC Pipeline / LLVM Codegen
- Type registration: 6 prelude types registered (Ordering, FormatSpec, TraceEntry, FormatType, Alignment, Sign -- all enums/structs from prelude).
- Function declarations: `_ori_fib` (1 param, Fast CC, Direct return), `_ori_gcd` (2 params, Fast CC, Direct return), `_ori_main` (0 params, C CC, Direct return).
- Nounwind analysis: 1 pass, 0 nounwind functions, 0 mono-propagated. All 3 functions may throw (overflow panics for fib, potential division-related panic for gcd).
- Entry point wrapper: C `main()` generated, `has_args=false`, `returns_int=true`, `has_panic=false`.
- Build time: 0.27s (first run), 0.25s (LLVM IR dump run). 0.42s compilation on cold run.

## LLVM Deep Scrutiny (9 Categories)

### 1. IR Structure & Control Flow

**fib**: Clean SSA with 8 basic blocks. Entry `bb0` performs `icmp sle i64 %0, 1`, branching to `bb1` (base case: return n) or `bb2` (recursive case). `bb2` computes `n-1` with overflow check, calls `_ori_fib` recursively, then `bb4` computes `n-2` with overflow check, calls `_ori_fib` again, then `bb6` adds results with overflow check. All three arithmetic operations use `llvm.ssub.with.overflow.i64` / `llvm.sadd.with.overflow.i64` intrinsics. Phi node in `bb3` merges base case value with recursive result. **Correct**.

**gcd**: 5 basic blocks. Entry `bb0` performs `icmp eq i64 %1, 0`, branching to `bb1` (base: return a) or `bb2` (recursive: `srem` + tail call). `bb2` uses `srem i64 %0, %1` for modulo, then calls `_ori_gcd(b, a%b)`. Phi node in `bb3` merges both paths. **Correct**.

**main**: 4 basic blocks. `bb0` calls `_ori_fib(10)`, `bb1` calls `_ori_gcd(48, 18)`, `bb3` adds with overflow check, `add.ok` returns result. Constants correctly: `10` for fib, `48` and `18` for gcd. **Correct**.

**Entry wrapper**: `main()` calls `_ori_main()`, truncates i64 to i32 for exit code. **Correct**.

**Verdict**: PASS. All control flow graphs are structurally sound with correct branching for if/then/else recursion patterns.

### 2. Type Safety & Calling Convention

- All Ori `int` correctly represented as `i64`.
- `fib`: `fastcc i64 @_ori_fib(i64)` -- 1 param, fast calling convention.
- `gcd`: `fastcc i64 @_ori_gcd(i64, i64)` -- 2 params, fast calling convention.
- `main`: `i64 @_ori_main()` -- C calling convention (correct for entry point).
- C wrapper: `i32 @main()` calls `@_ori_main()` and truncates with `trunc i64 ... to i32`.
- Recursive calls pass parameters correctly: `_ori_fib(i64 %sub.val)`, `_ori_gcd(i64 %1, i64 %rem)`.

**Verdict**: PASS. Type widths, calling conventions, and parameter passing are all correct.

### 3. Overflow & Arithmetic Safety

- **Subtraction overflow**: `fib` uses `@llvm.ssub.with.overflow.i64` for both `n-1` and `n-2`, branching to `ori_panic_cstr` on overflow. Two distinct panic blocks (`sub.ovf_panic`, `sub.ovf_panic5`) with overflow message globals (`@ovf.msg`, `@ovf.msg.1`).
- **Addition overflow**: `fib` uses `@llvm.sadd.with.overflow.i64` for `fib(n-1) + fib(n-2)`, with panic on overflow (`@ovf.msg.2`). `main` also uses `@llvm.sadd.with.overflow.i64` for `f + g` (`@ovf.msg.3`).
- **Modulo**: `gcd` uses bare `srem i64 %0, %1`. This is safe because the `b == 0` check happens before the `srem`, so division-by-zero is structurally impossible at the `srem` instruction.
- **Comparison**: `icmp sle` (signed less-than-or-equal) for `<=`, `icmp eq` for `==`. Both correct for signed i64.
- `ori_panic_cstr` is marked `#1 = { cold }` -- correct cold-path annotation.

**Observation**: 4 overflow message globals exist (`ovf.msg` through `ovf.msg.3`), but only 2 distinct strings ("integer overflow on subtraction" x2, "integer overflow on addition" x2). The duplicates could theoretically be deduplicated by LLVM's constant merging pass, but this is cosmetic, not a bug.

**Verdict**: PASS. All arithmetic is overflow-checked. Modulo is guarded by prior zero check. No safety gaps.

### 4. Memory Management (ARC/RC)

This program uses only `int` values (i64 scalars). No heap allocations, no RC operations, no strings, no collections.

- ARC trace shows type registration only (prelude types), no RC increment/decrement.
- No `ori_rc_inc` / `ori_rc_dec` / `ori_buffer_rc_dec` calls in the LLVM IR.
- No `alloca` beyond the stack frame (confirmed by disassembly: `sub $0x38,%rsp` for fib, `sub $0x28,%rsp` for gcd).

**Verdict**: PASS. No memory management needed; none generated. Clean.

### 5. Function Symbols & Linkage

- `_ori_fib`: 0x1a090, size 0xb7 (183 bytes), T (global text).
- `_ori_gcd`: 0x1a150, size 0x54 (84 bytes), T (global text).
- `_ori_main`: 0x1a1b0, size 0x52 (82 bytes), T (global text).
- `main`: 0x1a210, wraps `_ori_main`.
- `ori_panic_cstr`: 0x1af70, size 0x28a, T (global text, from `ori_rt`).
- No duplicate symbols. No unexpected exports.

**Verdict**: PASS. Symbol table is clean. All user functions properly exported.

### 6. Recursion Correctness

**fib (tree recursion)**:
- Two recursive calls per non-base invocation: `_ori_fib(n-1)` then `_ori_fib(n-2)`.
- Calls are sequential (not parallel): first call at 0x1a0fb, result stored at `(%rsp)`, second call at 0x1a117, result stored at `0x8(%rsp)`, then added at 0x1a0e7.
- Stack frame: 0x38 (56 bytes) per call. For fib(10), max depth ~10, so ~560 bytes stack. Well within limits.
- No tail-call optimization (expected -- tree recursion is not tail-recursive).

**gcd (linear recursion)**:
- Single recursive call: `_ori_gcd(b, a%b)`.
- The recursive call IS in tail position in the source, but the generated code does NOT apply TCO. The `call` at 0x1a182 is followed by `mov %rax,0x8(%rsp)` + `jmp` to the return path. Stack frame: 0x28 (40 bytes) per call.
- For gcd(48, 18): call chain is gcd(48,18) -> gcd(18,12) -> gcd(12,6) -> gcd(6,0) = 4 calls. Max stack ~160 bytes. Safe.

**Note**: gcd is tail-recursive in the source (`else gcd(a: b, b: a % b)` is the last expression). The compiler does not currently apply tail-call optimization (TCO), which means deeply recursive gcd calls could theoretically blow the stack. This is a known limitation, not a J3-specific bug. For the test case (depth 4), this is irrelevant.

**Verdict**: PASS. Recursion semantics are correct. TCO not applied to gcd (documented limitation).

### 7. Binary & Section Analysis

- Binary size: 6,561,408 bytes (6.3 MB, debug build -- expected for statically-linked `ori_rt`).
- Text section: 889,873 bytes (869 KB).
- Rodata: 136,536 bytes (133 KB) -- includes overflow panic strings.
- Debug info: ~4.1 MB (`.debug_info` + `.debug_line` + `.debug_str` + `.debug_ranges` + `.debug_aranges`).
- No unexpected sections. Standard ELF layout.

**Verdict**: PASS. Binary structure is normal for a debug build.

### 8. Disassembly Quality

**fib disassembly** (0x1a090, 183 bytes):
- Clean register usage: `%rdi` for parameter `n`, `%rax` for return value.
- Overflow check pattern: `seto %al` + `jo` (jump on overflow) to panic.
- Stack slots used for saving values across recursive calls: `0x30(%rsp)` = n, `0x20(%rsp)` = n-1, `0x18(%rsp)` = n-2, `0x10(%rsp)` = result, `(%rsp)` = fib(n-1), `0x8(%rsp)` = fib(n-2).
- Alignment padding: `nopw 0x0(%rax,%rax,1)` at end. Standard.

**gcd disassembly** (0x1a150, 84 bytes):
- `%rdi` = a, `%rsi` = b. `cqto` + `idiv %rdi` for signed division (srem). Remainder in `%rdx`, passed as `%rsi` for next call.
- Wait -- there is a subtle issue in the disassembly. At 0x1a17a: `cqto` sign-extends `%rax` into `%rdx:%rax`, then `idiv %rdi` divides `%rdx:%rax` by `%rdi`. But at 0x1a170, `%rdi` was loaded from `0x18(%rsp)` (which is `b`), and `%rax` was loaded from `0x20(%rsp)` (which is `a`). So the division is `a / b` with remainder `a % b` in `%rdx`. Then `%rdx` is moved to `%rsi` (second arg = `a % b`) and `%rdi` already holds `b` (first arg). This matches `gcd(a: b, b: a % b)`. **Correct**.

**main disassembly** (0x1a1b0, 82 bytes):
- Immediate operands: `$0xa` (10) for fib, `$0x30` (48) and `$0x12` (18) for gcd. **Correct**.
- Overflow check on final addition. **Correct**.

**C main wrapper** (0x1a210):
- `push %rax` (align stack), `call _ori_main`, `pop %rcx` (restore), `ret`. Minimal. **Correct**.
- Note: The truncation (`trunc i64 to i32` in IR) is implicit in x86-64 since `%eax` is the low 32 bits of `%rax` and `ret` from `main()` uses `%eax` as the exit code per System V ABI.

**Verdict**: PASS. Disassembly matches IR semantics. Register allocation and stack layout are correct.

### 9. Warnings & Diagnostics

- Build stdout: empty (clean).
- Build stderr: empty (clean).
- LLVM warnings: `Compiling plans/code-journeys/journey3.ori (first run)...` + `Compiled in 0.25s` -- informational only, no warnings.
- AOT stderr: `Compiling ... (first run)...` + `Compiled in 0.42s` -- informational only.
- No linker warnings. No undefined symbol warnings. No LLVM verification errors.

**Verdict**: PASS. Zero warnings, zero diagnostics.

## Observations & Notes

1. **No TCO for gcd**: The `gcd` function is tail-recursive but not optimized to a loop. This is a known compiler limitation. For production use with very large inputs, this could stack overflow. Not a J3 bug.

2. **Duplicate overflow message strings**: `@ovf.msg` and `@ovf.msg.1` contain identical content ("integer overflow on subtraction\00"), as do `@ovf.msg.2` and `@ovf.msg.3` ("integer overflow on addition\00"). LLVM's constant merge pass may deduplicate these, but the compiler could also emit them as shared globals. Cosmetic optimization opportunity.

3. **srem without explicit zero guard in IR**: The `srem` instruction in `_ori_gcd` has no explicit division-by-zero check in the IR. This is safe because the `icmp eq i64 %1, 0` branch guarantees `b != 0` at the `srem` site. However, if the function were called directly with `b=0` and the branch were somehow skipped (impossible in well-formed IR), `srem` by zero is undefined behavior in LLVM. The current code is correct.

4. **Eval trace size**: 3.5 MB / 2,263 lines for fib(10). The exponential blowup of tree recursion (fib(10) makes 177 calls) is clearly visible. This confirms the interpreter handles deep recursion correctly.

5. **fastcc on recursive functions**: Both `fib` and `gcd` use `fastcc`, which allows LLVM more freedom in register allocation and tail-call optimization. Despite this, LLVM did not convert `gcd` to a loop -- likely because the current IR structure (with separate basic blocks for the recursive call result) prevents the optimization. A future improvement could emit the tail call with `musttail` to force TCO.

## Verdict

**PASS** -- All 9 categories clean. Both backends produce the correct result (61). No bugs, no safety issues, no regressions.
