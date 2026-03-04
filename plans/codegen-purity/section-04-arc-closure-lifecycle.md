---
section: "04"
title: "ARC Closure Lifecycle"
status: not-started
goal: "Closure environments are freed when their last reference goes out of scope — zero leaks"
inspired_by:
  - "Swift lib/SILOptimizer/ARC/ — tracks closure context RC through capture analysis"
  - "Rust rustc_codegen_llvm/mir/operand.rs — drops closure environments at scope exit"
depends_on: []
sections:
  - id: "04.1"
    title: "Closure Environment Drop Emission"
    status: not-started
  - id: "04.2"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: ARC Closure Lifecycle

**Status:** Not Started
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

- [ ] Investigate why `rc_insert/` doesn't emit `RcDec` for closure variables at scope exit (classification already treats function/closure values as RC-managed)
- [ ] Check `rc_insert/` for special-casing that might skip closure variables (e.g., does it only handle struct/list/map types?)
- [ ] Fix liveness tracking in `liveness/` to include closure variables in their live ranges
- [ ] Ensure `rc_insert/` emits `RcDec` for closure variables at scope exit
- [ ] Handle the case where closures are passed to other functions (rc_inc on pass, rc_dec when callee is done)
- [ ] Write test: closure created in a loop — verify no leak growth with `ORI_CHECK_LEAKS=1`
- [ ] Write test: closure passed to another function and used — verify environment freed after last use
- [ ] Add a negative test for over-release (no double `RcDec`) on closure values that are moved then consumed
- [ ] Verify with `diagnostics/rc-stats.sh`: every `ori_rc_alloc` for closures has a matching `ori_rc_dec`

---

## 04.2 Completion Checklist

- [ ] Closure environments get `ori_rc_dec` at end of live range
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks for J5 program
- [ ] `diagnostics/rc-stats.sh` shows balanced RC for closure environments
- [ ] Closures in loops don't accumulate leaked environments
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green

**Exit Criteria:** Running J5's `make_adder` program with `ORI_CHECK_LEAKS=1` reports 0 leaks. `rc-stats.sh` shows every `ori_rc_alloc` for closure environments has a matching `ori_rc_dec`.
