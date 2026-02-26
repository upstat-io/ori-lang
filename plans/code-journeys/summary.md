# Code Journey Summary

Journeys 1–6 completed, Journey 7 verified correct (results pending).

## Journey Results

| # | Theme | Features | Eval | AOT | Key Finding |
|---|-------|----------|------|-----|-------------|
| 1 | Bare arithmetic | `let`, `+`, `*`, `-` | 33 | 33 | Constant-folded to `ret i64 33` |
| 2 | Function calls | functions, `if`/`else`, `bool` | 11 | 11 | **CRITICAL**: fastcc lost on invoke→call downgrade (fixed) |
| 3 | Generics | `<T>`, type inference, monomorphization | 17 | 17 | Nounwind misses monomorphized callees |
| 4 | Closures | lambdas, HOF, function pointers | 16 | 16 | Nounwind unsound for indirect calls |
| 5 | Structs | struct types, field access | 7 | 7 | Clean value-type IR |
| 6 | Pattern matching | `match`, guards, wildcards | 3 | 3 | Textbook switch+phi compilation |
| 7 | Sum types | enum variants, variant patterns | 37 | 37 | (results pending) |

---

## Deduplicated Findings by Severity

### CRITICAL

**1. Calling convention lost on nounwind call downgrade** — FIXED
- **First seen**: Journey 2
- **Status**: Fixed in `compiler/ori_llvm/src/codegen/ir_builder/calls.rs`
- **Description**: When `invoke fastcc` was downgraded to `call` for nounwind functions, the `fastcc` calling convention was silently dropped. `IrBuilder::call()` did not explicitly set the calling convention, unlike `invoke()`. Result: `fastcc`-defined functions called with C convention, producing wrong return values.
- **Fix**: Added `call_val.set_call_convention(func.get_call_conventions())` to `call()` and `call_tail()`.
- **Regression from**: Nounwind analysis added in the previous session.

### HIGH

**2. Nounwind analysis unsound for indirect calls through closures**
- **First seen**: Journey 4
- **Confirmed in**: —
- **Description**: `_ori_apply` is marked `nounwind` but calls through a function pointer (`%closure.fn_ptr`). If the closure target could panic (e.g., division by zero), unwinding would cross a `call` in a `nounwind` function — undefined behavior. The analysis (`is_arc_function_nounwind()`) only checks direct `ArcTerminator::Invoke` callees and `ArcInstr::Apply` names against `ori_panic*`. Indirect calls through closure pointers are `Apply` instructions that don't match the prefix check, so they're silently treated as nounwind.
- **Impact**: Any higher-order function receiving a panicking closure silently produces UB.
- **Suggested fix**: Any function containing an indirect call (closure/function-pointer invocation) should be conservatively marked as potentially throwing.

**3. Nounwind analysis doesn't cover monomorphized callees**
- **First seen**: Journey 3
- **Confirmed in**: —
- **Description**: `_ori_main` calls `identity<int>` and `first<int,int>`, both trivially nounwind, but `_ori_main` uses `invoke` with landing pads because monomorphized functions are compiled AFTER their callers. At `_ori_main`'s compilation time, the monomorphized names aren't in `nounwind_functions`.
- **Impact**: All generic function calls pay unnecessary `invoke` + landing pad overhead.
- **Suggested fix**: Two-pass approach (compile all, analyze nounwind, re-emit callers), or topological ordering so callees compile before callers.

### MEDIUM

**4. 98 eager runtime declarations**
- **First seen**: Journey 1
- **Confirmed in**: Journeys 2, 3, 4, 5, 6
- **Description**: Every runtime function is declared in the LLVM module even when zero are called. Journey 1's `@main` returns a constant `33` and still gets 98 `declare` statements.
- **Impact**: Unnecessary IR bloat and compile-time overhead. LLVM's linker strips unused declarations, so no runtime impact.
- **Suggested fix**: Lazy declaration — only emit `declare` when a runtime function is first referenced.

**5. Dead unreachable blocks in nounwind functions**
- **First seen**: Journey 2
- **Confirmed in**: Journeys 4, 5, 6
- **Description**: When `invoke` is downgraded to `call` for nounwind callees, the former unwind-target blocks become unreachable and emit just `unreachable`. One dead block per nounwind call site. LLVM DCE removes these, but they're unnecessary IR clutter.
- **Impact**: Minor — cleaned up by LLVM passes. Increases raw IR size.
- **Suggested fix**: Skip emitting blocks that have no predecessors, or track which blocks were unwind-only and omit them when the invoke is downgraded.

**6. Trampoline overhead for non-capturing lambdas**
- **First seen**: Journey 4
- **Description**: A non-capturing lambda like `(x: int) -> int = x + 1` still requires: (a) a `{ ptr, ptr }` closure pair with null `env_ptr`, (b) a trampoline function `_ori_partial_0` that just forwards the call. Non-capturing lambdas could be passed as bare function pointers, avoiding the closure allocation and trampoline indirection.
- **Impact**: Extra function call + null pointer in every non-capturing lambda invocation.

**7. Redundant unconditional branches in match arms**
- **First seen**: Journey 6
- **Description**: Match arm bodies each get their own basic block containing only `br label %merge`. LLVM's SimplifyCFG folds these at `-O1`+, but at `-O0` they remain as unnecessary blocks. The codegen could emit phi predecessors directly from switch/branch targets.
- **Impact**: Minor IR bloat at `-O0`. No effect at optimization levels.

### LOW

**8. Opaque struct type names**
- **First seen**: Journey 5
- **Description**: LLVM struct types use auto-generated names like `%ori.3` instead of `%Point`. Cosmetic only — doesn't affect codegen or runtime behavior, but hurts IR readability when debugging.

**9. Trampoline missing nounwind attribute**
- **First seen**: Journey 4
- **Description**: `_ori_partial_0` (the closure trampoline) lacks `nounwind` even though it only calls a nounwind lambda. Cosmetic for most cases but could affect optimization if the trampoline were called from another nounwind function.

**10. `cargo run` silently strips LLVM feature**
- **First seen**: Journey 1
- **Description**: Running `cargo run -- run file.ori` rebuilds `oric` WITHOUT `--features llvm`, overwriting the LLVM-enabled binary at `target/debug/ori`. The symlink at `~/.local/bin/ori` then points to a non-LLVM binary. Any `cargo run` invocation silently breaks AOT until next `cargo bl`.
- **Impact**: Developer experience trap — easy to accidentally lose LLVM support.

---

## Findings by Compiler Phase

### Codegen: Nounwind Analysis (3 findings)
- **#1** [FIXED] fastcc lost on call downgrade
- **#2** Unsound for indirect calls (closures)
- **#3** Misses monomorphized callees (compilation order)

### Codegen: IR Emission (4 findings)
- **#4** 98 eager runtime declarations
- **#5** Dead unreachable blocks
- **#7** Redundant unconditional branches in match
- **#8** Opaque struct type names

### Codegen: Closure Pipeline (2 findings)
- **#6** Trampoline overhead for non-capturing lambdas
- **#9** Trampoline missing nounwind

### Developer Tooling (1 finding)
- **#10** `cargo run` overwrites LLVM binary

---

## What Works Well

1. **Constant folding** — Pure arithmetic folds to a single `ret` instruction (Journey 1)
2. **Struct compilation** — Value-type structs use `extractvalue`, constant literals, no unnecessary allocas (Journey 5)
3. **Pattern matching** — Compiles to textbook `switch` + `phi` + `icmp` (Journey 6)
4. **Generic monomorphization** — Correct specialization with mangled names (Journey 3)
5. **Closure representation** — Fat pointer `{ ptr, ptr }` with trampoline bridging works correctly (Journey 4)
6. **Guard variable elision** — Match guard variables alias the scrutinee, zero overhead (Journey 6)
7. **Let binding elimination** — Constant let bindings folded into usage sites (Journeys 1, 4)

---

## Recommended Fix Priority

| Priority | Finding | Effort | Impact |
|----------|---------|--------|--------|
| 1 | **#2** Nounwind unsound for indirect calls | Low | Prevents UB with panicking closures |
| 2 | **#3** Nounwind misses monomorphized callees | Medium | Eliminates unnecessary landing pads for generics |
| 3 | **#4** Eager runtime declarations | Medium | Reduces IR size and compile time |
| 4 | **#5** Dead unreachable blocks | Low | Cleaner IR output |
| 5 | **#6** Non-capturing lambda trampolines | Medium | Eliminates indirection for common case |
| 6 | **#10** `cargo run` strips LLVM | Low | Developer experience |
| 7 | **#7** Redundant match branches | Low | Marginally cleaner `-O0` IR |
| 8 | **#8** Opaque struct names | Low | IR readability |
| 9 | **#9** Trampoline missing nounwind | Low | Cosmetic correctness |

---

## Coverage Map

Features tested and working on both paths:
- Arithmetic, let bindings, constant folding
- Function calls, multiple functions, if/else, booleans
- Generics with type inference, monomorphization
- Closures, lambdas, higher-order functions, trampolines
- Struct types, field access, value-type ABI
- Pattern matching: literals, guards, wildcards, phi merges
- Sum types (enums), variant construction, variant pattern matching

Features not yet tested:
- Iterators, `.map()`, `.filter()`, `.collect()`
- Result/Option, `?` propagation
- Collections: lists, maps, sets
- ARC-heavy code: shared references, RC lifecycle
- Strings, formatting, interpolation
- Loops, ranges, `for` expressions
- Derived traits (Eq, Printable, etc.)
- Modules, imports
