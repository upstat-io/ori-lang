# Code Journey Summary

All 7 journeys completed. All 10 findings resolved (1 fixed during journey runs, 9 fixed by `plans/codegen-journey-fixes/`).

## Journey Results

| # | Theme | Features | Eval | AOT | Key Finding |
|---|-------|----------|------|-----|-------------|
| 1 | Bare arithmetic | `let`, `+`, `*`, `-` | 33 | 33 | Constant-folded to `ret i64 33` |
| 2 | Function calls | functions, `if`/`else`, `bool` | 11 | 11 | **CRITICAL**: fastcc lost on invoke→call downgrade (fixed) |
| 3 | Generics | `<T>`, type inference, monomorphization | 17 | 17 | Nounwind misses monomorphized callees |
| 4 | Closures | lambdas, HOF, function pointers | 16 | 16 | Nounwind unsound for indirect calls |
| 5 | Structs | struct types, field access | 7 | 7 | Clean value-type IR |
| 6 | Pattern matching | `match`, guards, wildcards | 3 | 3 | Textbook switch+phi compilation |
| 7 | Sum types | enum variants, variant patterns | 37 | 37 | Tag switch with phi merges |

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

**2. Nounwind analysis unsound for indirect calls through closures** — FIXED
- **First seen**: Journey 4
- **Status**: Fixed in `plans/codegen-journey-fixes/` §01.1 (2026-02-26)
- **Description**: `_ori_apply` was marked `nounwind` but calls through a function pointer (`%closure.fn_ptr`). If the closure target could panic, unwinding would cross a `call` in a `nounwind` function — undefined behavior.
- **Fix**: `is_arc_function_nounwind()` now uses explicit `match` with `ApplyIndirect { .. } => false` arm. Any function containing an indirect call is conservatively marked may-unwind.

**3. Nounwind analysis doesn't cover monomorphized callees** — FIXED
- **First seen**: Journey 3
- **Status**: Fixed in `plans/codegen-journey-fixes/` §01.2 (2026-02-26)
- **Description**: `_ori_main` called `identity<int>` and `first<int,int>` (both nounwind) but used `invoke` with landing pads because monomorphized functions were compiled AFTER callers.
- **Fix**: Two-pass architecture — prepare all functions to ARC IR, compute complete nounwind set via fixed-point iteration, then emit LLVM IR. Mono dispatch propagation bridges original→mangled name domains.

### MEDIUM

**4. 98 eager runtime declarations** — FIXED
- **First seen**: Journey 1
- **Status**: Fixed in `plans/codegen-journey-fixes/` §02.1 (2026-02-26)
- **Description**: Every runtime function was declared in the LLVM module even when zero were called. Journey 1's `@main` returned `33` and still got 98 `declare` statements.
- **Fix**: Data-driven `RT_FUNCTIONS` table + `IrBuilder::runtime_fn()` lazy cache. Declarations emitted on first use only. Journey 1 now produces 0 `declare` statements.

**5. Dead unreachable blocks in nounwind functions** — FIXED
- **First seen**: Journey 2
- **Status**: Fixed in `plans/codegen-journey-fixes/` §02.2 (2026-02-26)
- **Description**: When `invoke` was downgraded to `call` for nounwind callees, former unwind-target blocks became unreachable with just `unreachable` instructions.
- **Fix**: Dead unwind blocks are now skipped entirely during emission. `block_map` uses `Option<BlockId>` — dead blocks map to `None` and are never created.

**6. Trampoline overhead for non-capturing lambdas** — FIXED
- **First seen**: Journey 4
- **Status**: Fixed in `plans/codegen-journey-fixes/` §03.1 (2026-02-26)
- **Description**: Non-capturing lambdas still required a trampoline function `_ori_partial_N` that just forwarded the call.
- **Fix**: Non-capturing lambdas are declared with `ccc` + phantom `ptr %_env`. The `fn_ptr` in the closure pair points directly to the lambda function — no trampoline generated.

**7. Redundant unconditional branches in match arms** — FIXED
- **First seen**: Journey 6
- **Status**: Fixed in `plans/codegen-journey-fixes/` §02.3 (2026-02-26)
- **Description**: Match arm bodies each got their own basic block containing only `br label %merge`, creating unnecessary branch overhead for simple matches.
- **Fix**: Added `ArcInstr::Select` to ARC IR. Simple matches (≤8 edges, no guards, no bindings) compile to branchless `icmp eq` + `select` chains. Guarded/complex matches retain the phi/branch structure.

### LOW

**8. Opaque struct type names** — FIXED
- **First seen**: Journey 5
- **Status**: Fixed in `plans/codegen-journey-fixes/` §04 (2026-02-26)
- **Description**: LLVM struct types used auto-generated names like `%ori.3` instead of `%Point`.
- **Fix**: `type_info/mod.rs` now uses the struct's user-visible name for LLVM named struct types (e.g., `%ori.Point`).

**9. Trampoline missing nounwind attribute** — FIXED
- **First seen**: Journey 4
- **Status**: Fixed in `plans/codegen-journey-fixes/` §03.2 (2026-02-26)
- **Description**: Closure trampolines lacked `nounwind` even when their target lambda was provably nounwind.
- **Fix**: `generate_closure_wrapper()` now accepts `target_is_nounwind: bool` and sets the nounwind attribute when the target is in `nounwind_functions`.

**10. `cargo run` silently strips LLVM feature** — FIXED
- **First seen**: Journey 1
- **Status**: Fixed in `plans/codegen-journey-fixes/` §05 (2026-02-26)
- **Description**: `cargo run -- run file.ori` rebuilt `oric` without `--features llvm`, overwriting the LLVM-enabled binary.
- **Fix**: LLVM is now a default feature in `oric/Cargo.toml`. `cargo run` includes LLVM automatically. Non-LLVM builds require explicit `--no-default-features`.

---

## Findings by Compiler Phase

### Codegen: Nounwind Analysis (3 findings — all FIXED)
- **#1** [FIXED] fastcc lost on call downgrade
- **#2** [FIXED] Unsound for indirect calls (closures) — §01.1
- **#3** [FIXED] Misses monomorphized callees (compilation order) — §01.2

### Codegen: IR Emission (4 findings — all FIXED)
- **#4** [FIXED] 98 eager runtime declarations — §02.1
- **#5** [FIXED] Dead unreachable blocks — §02.2
- **#7** [FIXED] Redundant unconditional branches in match — §02.3
- **#8** [FIXED] Opaque struct type names — §04

### Codegen: Closure Pipeline (2 findings — all FIXED)
- **#6** [FIXED] Trampoline overhead for non-capturing lambdas — §03.1
- **#9** [FIXED] Trampoline missing nounwind — §03.2

### Developer Tooling (1 finding — FIXED)
- **#10** [FIXED] `cargo run` overwrites LLVM binary — §05

---

## What Works Well

1. **Constant folding** — Pure arithmetic folds to a single `ret` instruction (Journey 1)
2. **Struct compilation** — Value-type structs use `extractvalue`, constant literals, no unnecessary allocas (Journey 5)
3. **Pattern matching** — Simple matches compile to branchless `select` chains; complex matches use textbook `switch` + `phi` (Journey 6)
4. **Generic monomorphization** — Correct specialization with mangled names, two-pass nounwind analysis (Journey 3)
5. **Closure representation** — Non-capturing lambdas pass as bare function pointers; capturing closures use fat pointer `{ ptr, ptr }` (Journey 4)
6. **Guard variable elision** — Match guard variables alias the scrutinee, zero overhead (Journey 6)
7. **Let binding elimination** — Constant let bindings folded into usage sites (Journeys 1, 4)
8. **Lazy declarations** — Only actually-used runtime functions appear in IR (Journey 1: 0 declares)
9. **Named struct types** — IR shows `%ori.Point` not `%ori.3` (Journey 5)
10. **Sound nounwind analysis** — Indirect calls conservatively may-unwind; two-pass ensures complete analysis (Journeys 3, 4)

---

## Fix Priority (all resolved)

| Priority | Finding | Status | Fixed In |
|----------|---------|--------|----------|
| 1 | **#2** Nounwind unsound for indirect calls | FIXED | §01.1 |
| 2 | **#3** Nounwind misses monomorphized callees | FIXED | §01.2 |
| 3 | **#4** Eager runtime declarations | FIXED | §02.1 |
| 4 | **#5** Dead unreachable blocks | FIXED | §02.2 |
| 5 | **#6** Non-capturing lambda trampolines | FIXED | §03.1 |
| 6 | **#10** `cargo run` strips LLVM | FIXED | §05 |
| 7 | **#7** Redundant match branches | FIXED | §02.3 |
| 8 | **#8** Opaque struct names | FIXED | §04 |
| 9 | **#9** Trampoline missing nounwind | FIXED | §03.2 |

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

---

## Resolution

All 10 findings from code journeys 1–7 have been resolved:

- **Finding #1** was fixed during the initial journey runs (calling convention preservation)
- **Findings #2–#10** were fixed by the `plans/codegen-journey-fixes/` implementation plan (6 sections, 2026-02-26)

### Key architectural changes:
1. **Two-pass compilation** (§01.2): ARC IR preparation is now separated from LLVM emission. All functions are prepared first, then `compute_nounwind_set()` runs fixed-point iteration over the full set, then LLVM IR is emitted using the complete nounwind information.
2. **Lazy runtime declarations** (§02.1): Static `RT_FUNCTIONS` table with `runtime_fn()` cache replaces eager `declare_runtime()`.
3. **Branchless select chains** (§02.3): `ArcInstr::Select` added to ARC IR. Simple matches compile to `icmp eq` + `select` chains — no branch misprediction.
4. **Non-capturing lambda fast path** (§03.1): Lambdas with zero captures skip trampoline generation entirely.
5. **LLVM as default feature** (§05): `cargo run` preserves LLVM support.

### Verification:
- 10,573 tests passing, 0 failures
- `dual-exec-verify.sh`: 0 behavioral mismatches
- All 7 journey programs produce identical eval/AOT results
