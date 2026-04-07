---
section: "02"
title: "Effect-Aware Coalescing Barriers"
status: not-started
reviewed: false
goal: "Coalescing barriers use MemoryContract to selectively flush only variables whose callee params are Owned+!Dead, reducing unnecessary barrier flushes by 40%+"
inspired_by:
  - "LLVM DependencyAnalysis.cpp CanAlterRefCount/CanDecrementRefCount (llvm-project/llvm/lib/Transforms/ObjCARC/DependencyAnalysis.cpp)"
  - "Lean4 Borrow.lean static borrow parameter tracking (lean4/src/Lean/Compiler/IR/Borrow.lean:58-90)"
  - "Codex analysis: 'Ori's coalesce/mod.rs flushes all pending RC at any Apply — coarser than necessary'"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Define Callee RC Observation Query"
    status: not-started
  - id: "02.2"
    title: "Implement Selective Barrier in coalesce_block_rc"
    status: not-started
  - id: "02.3"
    title: "Thread MemoryContract into Coalescing"
    status: not-started
  - id: "02.4"
    title: "Matrix Testing and Measurement"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Effect-Aware Coalescing Barriers

**Status:** Not Started
**Goal:** Replace the conservative `flush_all()` barrier at every function call with selective barriers that consult `MemoryContract`. Only flush RC for variables whose callee parameters are `Owned` + non-`Dead` (i.e., the callee might actually observe the refcount). For `Borrowed` parameters and pure functions, no barrier is needed. Measure improvement using Section 01 statistics.

**Context:** Today, `coalesce_block_rc()` in `coalesce/mod.rs:71-74` calls `flush_all()` at every `Apply`/`ApplyIndirect` instruction — every pending RC operation for every variable is flushed. This is correct but conservative: a callee with all `Borrowed` parameters can never observe the refcount of its arguments, so there's no need to flush. Codex identified this as the single highest-value optimization from the Clang study. LLVM's `DependencyAnalysis.cpp` asks "can this specific call alter this specific pointer's refcount?" using alias analysis. Ori can do even better because it has typed IR and `MemoryContract` with per-parameter access/consumption information.

**Reference implementations:**
- **LLVM** `DependencyAnalysis.cpp:35-64`: `CanAlterRefCount()` checks if an instruction can modify a pointer's refcount. Uses alias analysis (`onlyReadsMemory`, `onlyAccessesArgPointees`).
- **Lean4** `Borrow.lean:58-90`: `InitParamMap` marks function parameters as borrow vs owned at definition time. Borrowed params don't need RC ops at call boundaries.

**Depends on:** Section 01 (uses statistics to measure improvement).

---

## 02.1 Define Callee RC Observation Query

**File(s):** `compiler/ori_arc/src/aims/emit_rc/coalesce/mod.rs` (new helper)

Define the core query: "can this callee observe the refcount of a given argument variable?"

- [ ] Add `callee_may_observe_rc()` function:
  ```rust
  /// Determine whether a callee might observe the refcount of argument `arg_idx`.
  ///
  /// Returns `true` (conservative) if the callee might inspect the refcount
  /// via uniqueness checks, COW, or ownership transfer. Returns `false` if
  /// the parameter is borrowed and the callee has no effects that could
  /// observe refcounts.
  ///
  /// Falls back to `true` (conservative) for:
  /// - Unknown callees (no contract found)
  /// - ApplyIndirect (closure calls — contract may not be available)
  /// - FFI callees
  fn callee_may_observe_rc(
      contract: Option<&MemoryContract>,
      arg_idx: usize,
  ) -> bool {
      let Some(contract) = contract else { return true };
      let Some(param) = contract.params.get(arg_idx) else { return true };
      
      // Borrowed params that are Dead or Linear can never observe RC.
      // The callee receives a reference but doesn't inspect its refcount.
      if param.access == AccessClass::Borrowed {
          return false;
      }
      
      // Owned params that are Dead — callee takes ownership but doesn't use.
      // Still needs correct RC at transfer point.
      // Owned + Linear — callee uses exactly once, may observe.
      // Owned + Affine/Unrestricted — callee may copy/share, definitely observes.
      true
  }
  ```

- [ ] Unit tests for `callee_may_observe_rc()`:
  - `Borrowed + Dead` → false
  - `Borrowed + Linear` → false
  - `Borrowed + Unrestricted` → false (borrowed can never observe RC regardless of consumption)
  - `Owned + Dead` → true (ownership transfer requires correct RC)
  - `Owned + Linear` → true
  - `Owned + Unrestricted` → true
  - `None` contract → true (conservative)
  - arg_idx out of bounds → true (conservative)

- [ ] **Subsection close-out (02.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-02.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 02.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 02.2 Implement Selective Barrier in coalesce_block_rc

**File(s):** `compiler/ori_arc/src/aims/emit_rc/coalesce/mod.rs`

Replace `flush_all()` with `flush_selective()` that only flushes variables the callee can observe.

- [ ] Add `flush_selective()` function.

  **Import requirement:** `coalesce/mod.rs` currently imports only `rustc_hash::FxHashMap`. Add `use rustc_hash::FxHashSet;` for the flushed-vars tracking set.

  **Import requirement:** `MemoryContract` lives in `crate::aims::contract`. Add `use crate::aims::contract::MemoryContract;`. The `Name` type comes from `ori_ir::Name`.

  ```rust
  /// Flush pending RC operations only for variables that the callee may observe.
  ///
  /// For each argument to the call, check if the callee can observe its refcount
  /// via `callee_may_observe_rc()`. Only flush pending ops for observed args.
  /// Non-argument variables with pending ops are NOT flushed — they survive
  /// across the call barrier.
  fn flush_selective(
      instr: &ArcInstr,
      contracts: &FxHashMap<Name, MemoryContract>,
      pending: &mut FxHashMap<ArcVarId, PendingRc>,
      out: &mut Vec<ArcInstr>,
      stats: &mut CoalesceStats,
  ) {
      let (func_name, args) = match instr {
          ArcInstr::Apply { func, args, .. } => (Some(*func), args.as_slice()),
          ArcInstr::ApplyIndirect { args, .. } => (None, args.as_slice()),
          _ => unreachable!("flush_selective called on non-call"),
      };
      
      let contract = func_name.and_then(|n| contracts.get(&n));
      
      // For each argument, check if callee observes its RC.
      let mut flushed_vars = FxHashSet::default();
      for (idx, &arg_var) in args.iter().enumerate() {
          if callee_may_observe_rc(contract, idx) {
              if let Some(entry) = pending.remove(&arg_var) {
                  flush_entry(out, arg_var, &entry);
                  flushed_vars.insert(arg_var);
              }
          }
      }
      
      // Also flush the destination variable if it has pending ops
      // (the call defines it — must be correct before definition).
      if let Some(dst) = instr.defined_var() {
          if let Some(entry) = pending.remove(&dst) {
              flush_entry(out, dst, &entry);
          }
      }
      
      // Track how many variables survived the barrier.
      if flushed_vars.is_empty() && contract.is_some() {
          stats.selective_barrier_skips += 1;
      }
  }
  ```

  **Architecture note:** `flush_selective()` uses the same `flush_entry()` and `pending` map as the existing `flush_all()` and `flush_touched()` — it integrates naturally into the current coalescing architecture.

- [ ] Update the main loop in `coalesce_block_rc()`:
  ```rust
  // Replace the is_call branch at ~line 71-74:
  if is_call {
      // Extract callee name for contract lookup.
      let callee_name = match &instr {
          ArcInstr::Apply { func, .. } => Some(*func),
          _ => None, // ApplyIndirect — no static callee name
      };
      let contract = callee_name.and_then(|n| contracts.get(&n));
      
      if contract.is_none() {
          // No contract for this callee (unknown, FFI, indirect) — conservative.
          flush_all(&mut pending, &mut new_body);
          stats.flush_all_count += 1;
      } else {
          flush_selective(&instr, contracts, &mut pending, &mut new_body, &mut stats);
          stats.selective_barrier_count += 1;
      }
  }
  ```

  **Note:** The `contracts` map is populated for all analyzed functions, so `contracts.is_empty()` would be wrong — it would be `true` only if zero functions were analyzed (never in practice). The correct fallback is per-callee: no contract for THIS specific callee.

- [ ] Add `selective_barrier_count` and `selective_barrier_skips` to `CoalesceStats`.

- [ ] **Subsection close-out (02.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-02.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 02.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 02.3 Thread MemoryContract into Coalescing

**File(s):** `compiler/ori_arc/src/aims/emit_rc/coalesce/mod.rs`, `compiler/ori_arc/src/aims/realize/mod.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline.rs`

After Section 01, the coalescing pass takes `&mut Vec<ArcInstr>` and returns `CoalesceStats`. It needs an additional `contracts` parameter to make selective barrier decisions. The call chain is: `run_aims_pipeline()` -> `realize_rc_reuse()` (has `contracts` param) -> `emit_rc_unified()` -> `coalesce_block_rc()` (per block). Threading contracts through requires updating `emit_rc_unified()` as well.

- [ ] Update `coalesce_block_rc()` signature:
  ```rust
  pub(crate) fn coalesce_block_rc(
      body: &mut Vec<ArcInstr>,
      contracts: &FxHashMap<Name, MemoryContract>,
  ) -> CoalesceStats
  ```

- [ ] Update all call sites (3 locations, 2 production + tests):
  1. `realize/emit_unified.rs:108` — the `for block in &mut func.blocks` loop calls `coalesce_block_rc(&mut block.body)`. Update `emit_rc_unified()` to accept and forward `contracts`. Note: `emit_rc_unified()` does NOT currently receive `contracts` — it needs a new parameter threaded from `realize_rc_reuse()` which already has `contracts: &FxHashMap<Name, MemoryContract>`.
  2. `aims/emit_rc/mod.rs:59` — the re-export `pub(crate) use coalesce::coalesce_block_rc;`. Update the function's public signature.
  3. `aims/emit_rc/coalesce/tests.rs` — 6 test call sites currently pass only `&mut body`. Pass `&FxHashMap::default()` for backward-compatible conservative behavior.

- [ ] Thread `contracts` through `realize_rc_reuse()` → `emit_rc_unified()` → `coalesce_block_rc()`:
  - `realize_rc_reuse()` in `realize/mod.rs` already receives `contracts: &FxHashMap<Name, MemoryContract>` (line ~22 of its signature)
  - `emit_rc_unified()` in `realize/emit_unified.rs` is called by `realize_rc_reuse()` — add `contracts` parameter
  - `coalesce_block_rc()` called from `emit_rc_unified()` at line ~108 — forward `contracts`

- [ ] Verify backward compatibility: passing empty contracts map → identical behavior to current `flush_all()`. Write an explicit test that constructs a block with an Apply instruction, passes `&FxHashMap::default()`, and asserts `flush_all_count == 1` (conservative fallback).

**Semantic pin:** Test that with empty contracts, behavior is identical to pre-change (every call triggers `flush_all`). Test that with contracts where all params are `Borrowed`, `flush_all_count` drops to 0 and `selective_barrier_skips` equals the call count.

**Negative pin:** Test that with an all-`Owned` contract, the owned arg's pending RC IS flushed via `flush_selective()` -- the selective path must NOT skip owned arguments. Verify by asserting the flushed variable appears in emitted output and `selective_barrier_skips == 0`. A second negative pin: construct a program where skipping the barrier for an owned parameter would cause use-after-free (e.g., callee takes ownership and drops), compile and run with `ORI_CHECK_LEAKS=1`, and verify zero leaks -- proves selective barriers don't over-optimize.

- [ ] **Subsection close-out (02.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-02.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 02.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 02.4 Matrix Testing and Measurement

**File(s):** `compiler/ori_arc/src/aims/emit_rc/coalesce/tests.rs`

Comprehensive testing across type × call pattern × contract combinations.

- [ ] **Write failing test matrix BEFORE implementation** (TDD: tests must fail before 02.2 changes are applied):
  - **Type dimension**: int (scalar), str (heap), [int] (collection), Option\<str\> (enum+heap), closures, structs with heap fields
  - **Call pattern dimension**: single call, chained calls, call in loop, nested calls, call with mixed owned/borrowed params
  - **Contract dimension**: all-borrowed contract, all-owned contract, mixed contract, no contract (FFI/unknown)

- [ ] **Barrier reduction tests:**
  - Block with 1 Apply, all-Borrowed contract → `flush_all_count == 0`, `selective_barrier_count == 1`
  - Block with 3 Applies, all-Borrowed contracts → `flush_all_count == 0`, `selective_barrier_count == 3`
  - Block with 1 Apply, no contract → `flush_all_count == 1` (conservative fallback)
  - Block with 1 Apply, mixed contract (2 Borrowed + 1 Owned) → only owned arg flushed

- [ ] **Correctness tests:**
  - Verify RC operations are still correct after selective barriers (no use-after-free, no leaks)
  - `ORI_CHECK_LEAKS=1` on test programs with selective barriers enabled
  - Dual-exec parity: interpreter and LLVM produce identical results

- [ ] **Measurement:**
  - Run `ORI_LOG=ori_arc=info ori build tests/spec/` and compare `barrier_flush_all_count` before vs after
  - Target: 40%+ reduction in `barrier_flush_all_count` on typical programs

- [ ] **Verify tests pass in debug and release**

- [ ] **TPR checkpoint** — `/tpr-review` covering 02.1–02.4 implementation work

- [ ] **Subsection close-out (02.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-02.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 02.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] **Fix hygiene:** Remove stale plan annotation from `coalesce/tests.rs:1` — module doc says "Tests for static RC coalescing (Section 07.2)" referencing a different plan's section number. Replace with a neutral description: "Tests for static RC coalescing."
- [ ] `callee_may_observe_rc()` correctly classifies Borrowed params as non-observing
- [ ] `flush_selective()` only flushes variables for observed arguments
- [ ] `coalesce_block_rc()` accepts `&FxHashMap<Name, MemoryContract>`
- [ ] Empty contracts → identical behavior to pre-change (conservative fallback verified)
- [ ] All-Borrowed calls → zero `flush_all` barriers
- [ ] Section 01 statistics show measurable `barrier_flush_all_count` reduction
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on all test programs
- [ ] Dual-exec parity maintained (interpreter == LLVM)
- [ ] `./test-all.sh` green (debug + release)
- [ ] No spurious warnings in normal compilation
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 02` returns 0 annotations
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues
- [ ] `/impl-hygiene-review` passed — hygiene review clean
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `ORI_LOG=ori_arc=info ori build` shows `barrier_flush_all` count reduced by 40%+ compared to baseline (measured on `tests/spec/` suite). All existing tests pass. `ORI_CHECK_LEAKS=1` reports zero leaks. Dual-exec parity maintained across all test programs.
