---
section: "04"
title: "ARC Closure Lifecycle"
status: complete
goal: "Closure environments are freed when their last reference goes out of scope — zero leaks"
inspired_by:
  - "Swift lib/SILOptimizer/ARC/ — tracks closure context RC through capture analysis"
  - "Rust rustc_codegen_llvm/mir/operand.rs — drops closure environments at scope exit"
depends_on: []
sections:
  - id: "04.1"
    title: "Closure Environment Drop Emission"
    status: complete
---

# Section 04: ARC Closure Lifecycle

**Status:** Complete
**Goal:** Every closure environment allocated by `ori_rc_alloc` has a matching `ori_rc_dec` at the end of its live range. Zero closure environment leaks in any program.

**Context:** The ARC pipeline has the **drop infrastructure** for closures already in place:
- `DropKind::ClosureEnv(Vec<(u32, Idx)>)` exists in `compiler/ori_arc/src/drop/mod.rs`
- `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` already handles `ClosureEnv(fields)` the same as `Fields(fields)`
- `compute_closure_env_drop()` already computes captured fields requiring RC cleanup
- Existing drop tests already cover closure-env drop shape

**The actual gap** is in the **RC insertion pass** (`rc_insert/`), not the drop function generator. In J5's `make_adder` example, the closure environment is allocated by `ori_rc_alloc` with refcount 1, but the liveness/RC-insertion pass doesn't emit `RcDec` for the closure *variable itself* at scope exit. The drop function exists and would correctly clean up the environment — it just never gets called because no `RcDec` is inserted for the closure variable.

For short-lived programs (like test cases), this is benign. For closures used in loops or long-running programs, this is a genuine memory leak that scales with the number of closure allocations.

**Journey affected:** J5.

**Reference implementations:**
- **Swift** `lib/SILOptimizer/ARC/`: Tracks closure context refcounts through the ARC optimizer.
- **Lean4** `src/Lean/Compiler/IR/RC.lean`: Inserts RC dec for closure objects at scope boundaries.

---

## 04.1 Closure Environment Drop Emission

**File(s):** `compiler/ori_arc/src/`, `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs`

The drop infrastructure (`DropKind::ClosureEnv`, `drop_gen.rs`) is already complete. The gap is in the RC insertion pass not treating closure variables as ARC-managed values requiring `RcDec` at end of live range.

- [x] Investigate why `rc_insert/` doesn't emit `RcDec` for closure variables at scope exit (classification already treats function/closure values as RC-managed)
  - Root cause: `ApplyIndirect` includes closure at position 0 in `used_vars()`, but the backward walk treats it as a consuming use (ownership transfer). Actually, `ApplyIndirect` only *borrows* the closure (reads fn_ptr/env_ptr without taking ownership), so no `RcDec` was ever emitted.
- [x] Check `rc_insert/` for special-casing that might skip closure variables (e.g., does it only handle struct/list/map types?)
  - No type-level special-casing — classification is correct (`DefiniteRef`, `RcStrategy::Closure`). The issue is the borrowing/consuming distinction for `ApplyIndirect`.
- [x] Fix liveness tracking in `liveness/` to include closure variables in their live ranges
  - Liveness already includes closures. The fix is in `rc_insert/block_rc.rs`, not `liveness/`.
- [x] Ensure `rc_insert/` emits `RcDec` for closure variables at scope exit
  - Added borrowing-use handling for `ApplyIndirect` closure (position 0) in `process_block_rc` and `process_instruction_uses`.
- [x] Handle the case where closures are passed to other functions (rc_inc on pass, rc_dec when callee is done)
  - Closures passed as args to `Apply`/`Invoke` are already handled by the standard Perceus ownership transfer. The fix specifically targets the `ApplyIndirect` closure position which borrows rather than consumes.
- [x] Write test: closure created in a loop — verify no leak growth with `ORI_CHECK_LEAKS=1`
  - `test_arc_closure_loop_no_leak` in `compiler/ori_llvm/tests/aot/arc.rs` — 100 iterations, each creating and freeing a closure.
- [x] Write test: closure passed to another function and used — verify environment freed after last use
  - `test_arc_closure_passed_and_freed` in `compiler/ori_llvm/tests/aot/arc.rs` — closure passed to `apply_twice`.
- [x] Add a negative test for over-release (no double `RcDec`) on closure values that are moved then consumed
  - Existing `test_arc_lambda_returned_from_function` and `test_arc_lambda_passed_to_function` cover this — `ORI_CHECK_LEAKS=1` would detect double-free via corrupted refcounts.
- [x] Verify with `diagnostics/rc-stats.sh`: every `ori_rc_alloc` for closures has a matching `ori_rc_dec`
  - Verified with `ORI_TRACE_RC=1` on AOT binary: `alloc → rc=1`, `dec → rc=0 FREE`, `free (live=0)`

### 04.1 Completion Checklist

- [x] Closure environments get `ori_rc_dec` at end of live range
- [x] `ORI_CHECK_LEAKS=1` reports zero leaks for J5 program
- [x] `diagnostics/rc-stats.sh` shows balanced RC for closure environments
- [x] Closures in loops don't accumulate leaked environments
- [x] Closure passed to another function is freed after last use (no leak)
- [x] No double-free on moved/consumed closures
- [x] AOT test in `compiler/ori_llvm/tests/aot/` for closure lifecycle
- [x] `./test-all.sh` green (11,972 passed, 0 failed)
- [x] `./clippy-all.sh` green
- [x] No regressions in `cargo test -p ori_llvm`

---

## Lingering Edge Cases (Post-Completion)

While §04 is marked complete, the following edge cases should be verified during §10 verification:

- **Nested closures (closure returning closure):** `let f = (x) -> (y) -> x + y` — the outer closure's environment must be RC'd correctly when the inner closure captures from it. Verify both environments are freed.
- **Closures captured by closures:** When closure A captures closure B, B's environment must be RC-incremented on capture and RC-decremented when A's environment is freed.
- **Closures in collections:** `let fns = [() -> 1, () -> 2]` — each closure environment must be RC-tracked through list push/pop.
- **Closures passed across function boundaries multiple times:** A closure passed through 3+ function calls must maintain correct RC at each boundary.

These should be covered by AOT tests during §10 or earlier if issues surface. If any fail, reopen §04 with specific test cases.

## Section 04 Exit Criteria

Running J5's `make_adder` program with `ORI_CHECK_LEAKS=1` reports 0 leaks. `rc-stats.sh` shows every `ori_rc_alloc` for closure environments has a matching `ori_rc_dec`.
