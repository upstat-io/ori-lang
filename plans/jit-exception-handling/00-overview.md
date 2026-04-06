---
plan: "jit-exception-handling"
title: "JIT Exception Handling: Remaining Bug Fixes & Verification"
status: in-progress
supersedes: []
references:
  - "compiler/ori_rt/src/io/mod.rs"
  - "compiler/ori_rt/src/io/jit_recovery.rs"
  - "compiler/ori_rt/src/eh_personality.c"
  - "compiler/ori_llvm/src/codegen/function_compiler/impls.rs"
  - "compiler/ori_llvm/src/evaluator/mod.rs"
  - "compiler/ori_arc/src/lower/constructs.rs"
---

# JIT Exception Handling: Remaining Bug Fixes & Verification

## Mission

Fix remaining LLVM codegen bugs exposed by the unified `invoke/landingpad` exception handling architecture (now fully implemented). The core architectural work (Sections 01-03) is complete: `ori_panic` uses `_Unwind_RaiseException`, JIT test wrappers use two-layer `invoke`/`landingpad`, `InvokeIndirect` terminators exist in ARC IR and LLVM emission, short-circuit `&&`/`||` is branch-based, and the evaluator uses direct calls with `did_panic()`. The remaining work is fixing LLVM codegen bugs exposed by the expanded test coverage (Section 04) and running full verification (Section 05).

## Architecture (Implemented)

```
CURRENT STATE (implemented):
  ori_panic() → _Unwind_RaiseException → ori_eh_personality
                                           ↓
                               LLVM landing pads fire in order:
                               1. catch(expr:) landing pad [innermost]
                               2. RC cleanup landing pads [each frame]
                               3. test wrapper catch-all landing pad [outermost]
                                    ↓
                               ori_catch_cleanup() + did_panic() → test result
```

## What Is Already Done

**Section 01 (Runtime Panic Path) — COMPLETE:**
- `ori_panic` and `ori_panic_cstr` go directly to `aot_raise_exception` (no longjmp)
- `jit_recovery` module is `pub(crate)` with all needed items visible
- AOT `main()` wrapper uses `invoke`/`landingpad` on Itanium (in `entry_point.rs`)
- `ori_run_main` Itanium path still uses `catch_unwind` but is a safety-net only — the LLVM-generated `main()` wrapper handles unwinding via landingpads directly
- Stale longjmp comments cleaned up in `eh_personality.c`, `io/mod.rs`, `jit_recovery.rs` (01.R complete)

**Section 02 (ARC IR InvokeIndirect) — COMPLETE:**
- `InvokeIndirect` variant exists in `ArcTerminator` with all match sites updated across `ori_arc`, `ori_llvm`, `ori_repr`, `oric`
- `terminate_invoke_indirect` and `emit_invoke_indirect` exist in the builder
- Indirect calls inside `catch(expr:)` use `InvokeIndirect` (via `catch_unwind_target` check in `lower/calls/mod.rs`)
- Short-circuit `&&`/`||` use branch-based lowering (`lower_short_circuit_and`/`lower_short_circuit_or`)

**Section 03 (LLVM Emission & Wrappers) — COMPLETE:**
- Two-layer test wrappers with `invoke`/`landingpad catch-all` in `impls.rs`
- `InvokeIndirect` terminator emission in `terminators.rs`
- Void-return `Apply` defines dst as unit constant (BUG-04-024 fixed) in `apply.rs`
- Evaluator uses direct call + `did_panic()` (no `jit_run_protected`)

## Remaining Work

**Section 04 (Exposed Bug Fixes) — COMPLETE:**
- ~~04.1: Division by zero~~ — **FIXED**: added `checked_div`/`checked_rem` to `checked_ops.rs`
- ~~04.2: COW nested collections double-free~~ — **FIXED**: `ori_map_get` shallow byte-copy without `RcInc` for RC-managed value types. Fix: conditional `RcInc` in `emit_map_get` on the Some path.
- ~~04.3: Tuple/struct for-yield type confusion~~ — **FIXED**: override ARC pool_type_store_size with LLVM struct store size via `for_yield_elem_size_types` pre-scan
- ~~04.4a: Negative range iteration~~ — **FIXED**: recognize i64::MAX sentinel for descending unbounded ranges
- ~~04.4b: Coalesce ARC leak~~ — **FIXED by 04.5**: `propagate_borrowed_closure` unanimity for merge block params
- ~~04.4c: Coalesce None path~~ — **FIXED**: added `merge_mutable_vars` to `lower_coalesce`
- ~~**04.5**: AIMS borrowed-def propagation~~ — **FIXED**: `propagate_borrowed_closure` unanimity rule for Jump param propagation to merge blocks. Root cause of 04.4b.
- ~~**04.6**: Panic handler exception propagation~~ — **FIXED**: 3 sub-issues: (1) main wrapper `invoke` for no-args `@main`, (2) `extern "C-unwind"` for `dispatch_panic`/`aot_raise_exception`, (3) PanicInfo field index remapping via `ReprPlan`.

**Section 04B (Polymorphic Lambda Monomorphization) — IN PROGRESS:**
- Scheme unwrapping in ARC lowering, BoundVar→concrete substitution, and nested capture resolution have landed
- TPR-04B-013 reopened: list-concat lambda crash (BUG-04-030 Root Cause F) still reproduces on current tree
- In-tree LLVM verification and TPR-04B-013 both blocked by BUG-04-030

**Section 05 (Verification) — IN PROGRESS:**
- Pre-verification checks complete (01.R, 04.H, annotations, release build)
- Test matrix: 6/8 categories pass in debug+release (integer, bitwise, COW, struct layout, coalesce, range). 2/8 have known LCFails (catch: BUG-04-030; short-circuit: BUG-04-031/BUG-04-032).
- Dual-execution parity: 6/7 files verified (93/93 tests). operators_logical.ori blocked by BUG-04-031.
- Regression: 16,533 passed, 0 failed, 2656 LCFail. clippy clean.
- TPR in progress.

## Section Dependency Graph

```
  §01 Runtime   ─── COMPLETE
       ↓
  §02 ARC IR    ─── COMPLETE
       ↓
  §03 LLVM      ─── COMPLETE
       ↓
  §04 Exposed bug fixes (8 bugs):  ← COMPLETE
       ↓
  §04B Polymorphic lambda monomorphization  ← IN PROGRESS
       (Root Cause A addressed; list-concat crash + in-tree verification blocked by BUG-04-030)
       ↓
  §05 Verification: test matrix, dual-exec parity, TPR  ← IN PROGRESS
       ↓
  §06 LCFail Resolution (BUG-04-030/031/032/033)  ← IN PROGRESS (TPR review)
       Root causes D → A → B → E → F → 031/032 → 033 → C — all resolved
       Result: 2656 → 2475 LCFails (-7%), CRASH eliminated
```

## Known Bugs (Remaining)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| LLVM sdiv/srem no zero check | Checked arithmetic only covers add/sub/mul/neg overflow; `sdiv`/`srem` emit UB on zero divisor | §04.1 (`checked_ops.rs` + `strategy.rs`) | **Fixed** (2026-04-03) |
| COW double-free nested map/list | RC codegen missing inner collection RC inc during outer COW copy | §04.2 (`emit_map_get` conditional RcInc) | **Fixed** (2026-04-03) |
| Tuple/struct for-yield type confusion crash | RC inc on misaligned pointer (`0x74736574` = "test" in ASCII) -- string data treated as RC pointer | §04.3 (`for_yield_elem_size_types` pre-scan) | **Fixed** (2026-04-03) |
| Negative range iteration | `i64::MAX` sentinel for unbounded end fails with negative step: `0 > i64::MAX` is immediately false | §04.4a (`next_range` sentinel detection) | **Fixed** (2026-04-03) |
| Coalesce ARC leak | Over-conservative borrowed-def marking on merge block params | §04.4b (fixed via §04.5 `propagate_borrowed_closure` unanimity) | **Fixed** (2026-04-03) |
| Coalesce None path | Missing `merge_mutable_vars` in `lower_coalesce` | §04.4c (`lower/expr/short_circuit.rs`) | **Fixed** (2026-04-03) |

## Live Test Results

**Pre-fix snapshot (2026-04-02):**

| Test File | Result | Details |
|-----------|--------|---------|
| `integer_safety.ori` | 26 passed, 4 failed | div/mod by zero tests fail (succeed instead of panicking) |
| `cow/nested.ori` | FATAL crash | `ori_rc_dec called on already-freed allocation` (double-free) |
| `cow/sharing.ori` | FATAL crash | `ori_rc_dec called on already-freed allocation` (double-free) |
| `struct_layout.ori` | FATAL crash | `ori_rc_inc called with misaligned pointer 0x74736574` (type confusion) |
| `test_coalesce_copy.ori` | 15 passed, 2 failed | `test_none_evaluates_default` assertion + `test_list_coalesce` ARC leak |
| `infinite_range.ori` | 13 passed, 1 failed | `test_neg_step_iter` produces `[]` instead of `[0, -1, -2, -3, -4]` |

**Post-fix verification (2026-04-04, §05):**

| Test File | Result | Details |
|-----------|--------|---------|
| `integer_safety.ori` | 30 passed, 0 failed | All tests pass (debug + release) |
| `cow/nested.ori` | 7 passed, 0 failed | Leak check clean |
| `cow/sharing.ori` | 9 passed, 0 failed | Leak check clean |
| `struct_layout.ori` | 16 passed, 0 failed | No crash, no FastISel divergence |
| `test_coalesce_copy.ori` | 17 passed, 0 failed | All tests pass |
| `infinite_range.ori` | 14 passed, 0 failed | Negative step works correctly |

## Legacy: ori_run_main catch_unwind

The Itanium path in `ori_run_main` (lib.rs:430-443) still uses `std::panic::catch_unwind`. This is a working safety net, not a bug -- the LLVM-generated `main()` wrapper in `entry_point.rs` handles Itanium unwinding via `invoke`/`landingpad` directly, so `ori_run_main` is rarely called on Itanium. Replacing `catch_unwind` with an Itanium `ori_try_call` would require either: (a) compiling a C++ file for Itanium (currently only MSVC uses C++), or (b) using the raw `_Unwind_ForcedUnwind` API. This is out of scope for this plan -- the code path functions correctly and is exercised only as a fallback.

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Runtime Panic Path | `section-01-runtime.md` | Complete |
| 02 | ARC IR InvokeIndirect | `section-02-arc-ir.md` | Complete |
| 03 | LLVM Emission & Wrappers | `section-03-llvm-emission.md` | Complete |
| 04 | Exposed Bug Fixes | `section-04-exposed-bugs.md` | Complete |
| 04B | Polymorphic Lambda Monomorphization | `section-04b-lambda-mono.md` | In Progress (blocked by BUG-04-030) |
| 05 | Verification | `section-05-verification.md` | In Progress |
| 06 | LCFail Resolution | `section-06-lcfail-resolution.md` | In Progress |
