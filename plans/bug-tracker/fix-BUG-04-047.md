---
bug: "BUG-04-047"
title: "AIMS emit_terminator_rc misses RcInc for duplicate terminator uses (latent double-free)"
severity: "high"
status: in-progress
goal: "emit_terminator_rc emits exactly (occurrences + live_at_exit - 1) RcInc ops per distinct RC-managed variable used in a terminator, closing the latent double-free for Jump { args: [v, v] } and analogous duplicated-use shapes in Invoke / InvokeIndirect."
success_criteria:
  - "Unit test `test_emit_terminator_rc_jump_dup_arg_emits_one_inc` passes: constructs `Jump { args: [v, v] }` with RC-typed block params and asserts exactly one `RcInc { var: v, count: 1 }` is emitted by `emit_terminator_rc`."
  - "Unit test `test_emit_terminator_rc_jump_single_arg_not_live_emits_zero_incs` (negative pin) passes: `Jump { args: [v] }` with `is_live_at_exit == false` emits zero `RcInc`."
  - "Local `debug_assert_eq!` inside `emit_terminator_rc` asserts emitted per-var RcInc count equals `(k + live).saturating_sub(1)` for every owned-at-entry RC-managed var, across all terminator shapes — fires on any future regression that under- or over-emits."
  - "`timeout 150 ./test-all.sh` green — 16,000+ tests, no regressions."
  - "`cargo test -p ori_arc` green — all AIMS tests pass including the new forward_walk tests."
subsystem: "compiler/ori_arc/src/aims/emit_rc/forward_walk.rs"
found: "2026-04-08"
source: "continue-roadmap"
third_party_review:
  status: findings
  updated: 2026-04-08
---

# Fix: BUG-04-047 — AIMS emit_terminator_rc misses RcInc for duplicate terminator uses

**Status:** In Progress
**Severity:** high
**Goal:** Close the latent double-free surfaced by BUG-04-047 by making `emit_terminator_rc` emit the correct number of `RcInc` operations for every RC-managed variable used in a terminator, including the previously-uncovered case where the same variable appears multiple times in `terminator.used_vars()` (e.g., `Jump { args: [v, v] }`).

**Success Criteria:**
- [ ] Unit test `test_emit_terminator_rc_jump_dup_arg_emits_one_inc` passes (semantic pin for the primary repro shape).
- [ ] Unit test `test_emit_terminator_rc_jump_single_arg_not_live_emits_zero_incs` passes (negative pin — rejects over-emission).
- [ ] Local `debug_assert_eq!` in `emit_terminator_rc` pins the RC-balance formula per owned-at-entry RC-managed var.
- [ ] Matrix tests cover all duplication shapes: Jump (2, 3 dups), interleaved `[v, w, v]`, body+terminator interleaving, Invoke duplicates, InvokeIndirect `closure == args[i]` duplicate, scalar negative pin.
- [ ] `timeout 150 ./test-all.sh` green.
- [ ] `cargo test -p ori_arc` green.
- [ ] `/tpr-review` clean.
- [ ] `/impl-hygiene-review` clean.

**Context:** During `plans/repr-opt/section-07-enum-repr.md` §07.R TPR-07-022 design research, a `/tp-help` round 2 surfaced that `emit_terminator_rc` at `forward_walk.rs:63-86` tracks a `uses_so_far` counter for terminator uses but never consults the counter against the total — the gate at line 77 only checks `is_live_at_exit(var)`. That gate is correct for single-use terminators but silently emits **zero** `RcInc` when the same RC-managed var appears multiple times in `terminator.used_vars()` and is dead at exit. The current lineage-based dedup in `take_project` masks the bug by collapsing duplicate-arg merge params into one lineage class, so no current Ori source repro exists — but TPR-07-022's `let_alias_rep` dedup fix will remove that masking and unmask the double-free. BUG-04-047 must land **before or together with** the TPR-07-022 fix.

---

## 1. Root Cause Analysis

- **Symptom**: Latent — no source-level repro in the current tree. Will manifest as glibc-detected double-free in `iterator_drop` AOT pins as soon as TPR-07-022's dedup fix lands and removes the lineage-based masking.
- **Proximate cause**: `compiler/ori_arc/src/aims/emit_rc/forward_walk.rs:63-86`, the main "RcInc for terminator uses" loop:
  ```rust
  for var in terminator.used_vars() {
      if !is_owned_at_entry(...) { continue; }
      *uses_so_far.entry(var).or_insert(0) += 1;  // L75: counter incremented
      if is_live_at_exit(ctx.state_map, ctx.blk, var) {  // L77: gate only checks liveness
          // emit RcInc
      }
  }
  ```
  For `Jump { args: [v, v] }` with `v: RC-managed`, `is_live_at_exit(v) == false` (both uses transfer to target block's params, nothing alive after), the loop visits `v` twice, increments `uses_so_far[v]` to 2 — **and emits zero RcInc**. The target block's two params each own a reference, but only one underlying RC exists → the second `RcDec` double-frees a pointer already freed by the first param.
- **Root cause**: The `uses_so_far` counter is dead state. The loop increments it at L75 but never reads it in the gate condition. The gate is a per-variable binary check (`is_live_at_exit`), not a per-use count check. This is a copy-paste-and-delete fossil — the counter was originally threaded into a `has_future_use`-style check that was never wired up. In contrast, the body walk's `compute_has_future_use` at `compiler/ori_arc/src/aims/realize/walk.rs:278-297` correctly uses the counter:
  ```rust
  let remaining_in_block = total_uses.saturating_sub(uses_so_far);
  let live = is_live_at_exit(ctx.state_map, ctx.blk, var);
  remaining_in_block > 0 || /* terminator pending */ || live
  ```
  Phase C (terminator emission) was never extended to mirror this pattern.
- **Blast radius**:
  - Primary: `Jump { args: [v, v, ...] }` with any RC-managed duplicated var — latent double-free.
  - Secondary: `Invoke { args: [v, v, ...] }` and `InvokeIndirect { closure, args }` where `closure == args[i]` or `args[i] == args[j]` — same latent pattern.
  - Tertiary: cross-phase — `uses_so_far` is produced by `walk_body_unified` at `walk.rs:139` and threaded through `BodyWalkResult.uses_so_far` → `emit_unified.rs:242` → `emit_terminator_rc(..., uses_so_far, ...)`. The whole chain carries state that is written-only. This is a `LEAK:scattered-knowledge` per `impl-hygiene.md` — body walk is the canonical home for body use counts; terminator phase has no legitimate need for them.
- **Affected files**:
  - `compiler/ori_arc/src/aims/emit_rc/forward_walk.rs` — replace the gate with terminator-local occurrence counting + aggregated RcInc emission + debug_assert invariant.
  - `compiler/ori_arc/src/aims/realize/walk.rs` — remove `uses_so_far` from `BodyWalkResult` (no longer consumed).
  - `compiler/ori_arc/src/aims/realize/emit_unified.rs` — drop the `uses_so_far` destructure and the `uses_so_far` argument to `emit_terminator_rc`.
  - `compiler/ori_arc/src/aims/emit_rc/forward_walk/tests.rs` — **new file** — unit tests for `emit_terminator_rc` covering the matrix in § 2.
  - `compiler/ori_arc/src/aims/emit_rc/forward_walk.rs` — add `#[cfg(test)] mod tests;` declaration at bottom.

**Reference implementations** (prior art in the Ori tree — no cross-language reference needed since the pattern already exists in-tree):
- **Ori body walk** `compiler/ori_arc/src/aims/realize/walk.rs:278-297` — `compute_has_future_use` is the canonical per-use "does this var need an Inc" predicate. The bug is that Phase C (terminator) was never extended to mirror it; this fix extends the SAME semantic into Phase C via a simpler aggregated formulation.
- **Ori body walk** `walk.rs:204-268` — `emit_pre_instr_incs_unified` demonstrates the ownership-agnostic Inc-per-non-last-use pattern. Phase C must stay consistent with this semantic (Inc side is ownership-agnostic; Dec side is ownership-aware).

---

## 1.5 Fix Consensus (via /tp-help)

Independent dual-source design review of the proposed fix approach, run BEFORE tests or implementation to catch wrong-approach errors before locking them in. See `.claude/skills/fix-bug/SKILL.md` § Phase 1.75.

- **Proposed approach (pre-consensus)**: Mirror `compute_has_future_use` per-use pattern inside `emit_terminator_rc` — keep the `uses_so_far` parameter, increment per-iteration, emit one `RcInc { count: 1 }` per iteration where `remaining_later > 0 || is_live_at_exit`. Also add a verify-pass invariant in `compiler/ori_arc/src/verify/mod.rs` checking `rc_inc_count(var) >= occurrences(var) - 1` per terminator.
- **tp-help run scratch dir**: `/tmp/ori-tpr-NQScKIzW`

### Round 1

- **Codex summary**: Diagnosis confirmed. `uses_so_far` is dead state; the pattern IS latent. BUT Codex pushed back on the per-use `has_future_use` approach as **non-minimal** and a `LEAK:algorithmic-duplication` (duplicating `compute_has_future_use`'s control-flow skeleton in a second file). Recommended the simpler aggregated formulation: count terminator occurrences locally, emit ONE `RcInc { count: k - 1 + live }` per distinct var, remove `uses_so_far` from the function signature entirely. Also flagged the proposed verify-pass lower-bound invariant as **too weak** — it would false-pass when the same var already got a body-phase `RcInc` earlier in the block. Recommended instead a **local `debug_assert!`** inside `emit_terminator_rc` asserting the per-var emitted count equals `(k + live).saturating_sub(1)` — sharper because it's an equality, not a lower bound. Codex also flagged that the test matrix was missing **live-at-exit cells** and the **InvokeIndirect closure-tail duplicate cell** (`closure == args[i]`). Finally, Codex flagged a pre-existing DRIFT adjacent to the fix: the hand-rolled Invoke/InvokeIndirect owned-position check at `forward_walk.rs:42-58` disagrees with the canonical `ArcTerminator::is_owned_position` — but said **do NOT fold it into this fix**, file it as a separate bug.
- **Gemini summary**: Diagnosis confirmed. Diverged from Codex sharply by claiming the fix **MUST** include per-position ownership gating via `terminator.is_owned_position(pos)`, and that this in turn **REQUIRES fixing `is_owned_position` to return `true` for Jump args and Return value** (currently returns `false` for everything except Invoke/InvokeIndirect). Gemini claimed a separate adjacent **LEAK**: `Branch { cond: v }` + `live_at_exit` currently emits a leaking RcInc because Branch only reads `v`. Gemini also noted an orthogonal DRIFT between `ApplyIndirect` arg ordering (`[closure, ...args]`) and `InvokeIndirect` ordering (`[...args, closure]`) as a "maintenance hazard" but said to note it, not fix it here.
- **Agreement points**:
  1. Root cause diagnosis is correct — `uses_so_far` is dead state.
  2. The test matrix needs live-at-exit cells and InvokeIndirect closure-tail coverage.
  3. `forward_walk.rs` stays flat; tests live in sibling `forward_walk/tests.rs`.
  4. The aggregated formula is `(occurrences + live_at_exit).saturating_sub(1)` (Codex expressed as `k - 1 + live`; Gemini as saturating_sub — equivalent).
  5. The adjacent DRIFT between `ApplyIndirect` / `InvokeIndirect` arg ordering is a separate concern, not to fix here.
- **Disagreement points**:
  1. **Per-position ownership gate**: Codex says NO (existing body-walk Inc semantics is ownership-agnostic; stay consistent). Gemini says YES (emission is currently over-eager for Branch cond + live_at_exit).
  2. **Fix `is_owned_position` for Jump/Return as prerequisite**: Gemini says YES (mandatory). Codex says not needed for this bug.
  3. **`uses_so_far` parameter**: Codex says REMOVE it from the function signature (clean up dead state at the API boundary). Gemini's proposed algorithm KEEPS it.
  4. **Per-use loop vs aggregated emission**: Codex recommends aggregated (pre-count + emit one `RcInc { count }` per var). Gemini recommends per-use (mirroring body walk).
  5. **Verify invariant home**: my original proposal was `verify/mod.rs`; Codex says local `debug_assert!` is sharper.
- **Independent code verification**: I verified each load-bearing claim against the actual source tree to apply trust-but-verify per the `feedback_reviewer_grounding_and_trust.md` memory rule:
  - **Gemini's claim that `is_owned_position` must be fixed for Jump/Return is a confabulation.** Grepped for all callers of the terminator variant of `is_owned_position`:
    ```
    compiler/ori_arc/src/ir/tests.rs: 20+ call sites (all tests)
    compiler/ori_arc/src/aims/realize/walk_dec.rs:145 — instr.is_owned_position (ArcInstr, NOT ArcTerminator)
    compiler/ori_arc/src/aims/realize/walk.rs:223 — instr.is_owned_position (ArcInstr, NOT ArcTerminator)
    ```
    Production RC emission code calls `ArcInstr::is_owned_position` (the instruction variant, on body instructions). The `ArcTerminator::is_owned_position` method is only exercised from `ir/tests.rs`. Gemini's premise — that "fix `is_owned_position` for Jump/Return is a prerequisite" — depends on a consumer that does not exist. Rejected.
  - **Codex's claim that `uses_so_far` is dead post-`emit_terminator_rc`**: verified via grep. `uses_so_far` is:
    1. Created at `walk.rs:60`,
    2. Mutated inside body walk at `walk.rs:242`,
    3. Returned in `BodyWalkResult.uses_so_far` at `walk.rs:139`,
    4. Destructured at `emit_unified.rs:242`,
    5. Passed by value into `emit_terminator_rc` at `emit_unified.rs:255`,
    6. Mutated at `forward_walk.rs:75`,
    7. Never read after mutation.
    Confirmed — the parameter is dead state. Codex is right.
  - **Codex's DRIFT claim about `forward_walk.rs:42-45` vs `terminator.rs:104-109`**: verified:
    ```rust
    // forward_walk.rs:42-45 (hand-rolled):
    let is_owned = arg_ownership.get(pos).is_some_and(|o| *o == ArgOwnership::Owned);
    // ↑ empty ownership → false (NOT owned)

    // terminator.rs:104-108 (canonical ArcTerminator::is_owned_position for Invoke):
    pos < args.len()
        && arg_ownership.get(pos).is_none_or(|o| *o == ArgOwnership::Owned)
    // ↑ empty ownership → true (Owned)
    ```
    Confirmed DRIFT. Direct-Invoke calls with empty `arg_ownership` get DIFFERENT answers from the two checks. Real latent bug. Will file via `/add-bug` per the "do NOT fold in" guidance.
  - **Gemini's DRIFT claim about `ApplyIndirect [closure, ...args]` vs `InvokeIndirect [...args, closure]`**: verified in `ir/instr.rs` and `ir/terminator.rs`. Confirmed. Real structural inconsistency. Will file via `/add-bug`.
  - **Gemini's claim that `Branch { cond: v } + live_at_exit` leaks under current code**: I did not find a failing test or leak-check run demonstrating this. The existing 14,967+ test suite passes including many Branch-heavy patterns. Gemini's reasoning assumed the Inc side should be ownership-aware, but the body walk at `walk.rs:204-268` via `decide.rs:178-196` emits Inc unconditionally for `Normal` semantics when `has_future_use` — ownership-agnostic. The terminator emission has been consistent with this model. The RC math is balanced via downstream Dec emission paths (edge cleanup, last-use dec at successor). Gemini's proposed semantic change would risk unbalancing the pipeline. I cannot confirm the leak without empirical evidence; staying with the existing pattern.
- **Outcome**: **Persuaded divergence → adopt Codex's simpler aggregated approach.** Gemini's proposal is a broader refactor based on a confabulated dependency and would over-correct the existing Inc-side semantics. Codex's proposal is minimal, stays consistent with body-walk model, and removes the `uses_so_far` dead state across the API boundary.

### Final agreed approach

1. **`forward_walk.rs` — replace the buggy loop (lines 63-86) with aggregated counting**:
   - Snapshot `let term_phase_start = new_body.len();` before the inc loop so the debug_assert can scope to only terminator-phase emissions.
   - Walk `terminator.used_vars()` once, building `term_counts: FxHashMap<ArcVarId, usize>` for each owned-at-entry RC-managed var.
   - For each `(var, k)` pair, compute `count = (k + is_live_at_exit(var) as usize).saturating_sub(1)` and emit `RcInc { var, count: count as u32, strategy }` if `count > 0`.
   - Add `debug_assert_eq!` that recomputes actual per-var inc totals from `new_body[term_phase_start..]` and compares to the formula — catches any under/over-emission regression.
2. **`forward_walk.rs` — drop `uses_so_far` parameter**: `emit_terminator_rc` no longer needs it. The Invoke/InvokeIndirect project-borrowed block at lines 31-61 also does not need it (only reads args + ownership + state_map).
3. **`walk.rs` — drop `uses_so_far` from `BodyWalkResult`**: the field is no longer consumed anywhere. Inside `walk_body_unified`, `uses_so_far` remains as a local accumulator used by `compute_has_future_use` — just no longer returned.
4. **`emit_unified.rs:242-255` — update the destructure + call site**: remove `uses_so_far` from the `BodyWalkResult { .. }` pattern and drop it from the `emit_terminator_rc` argument list.
5. **`forward_walk.rs` — add `#[cfg(test)] mod tests;` at bottom**.
6. **`forward_walk/tests.rs` — new file** with the full matrix in § 2.
7. **Adjacent bugs filed via `/add-bug`** (NOT folded into this fix per both reviewers' guidance):
   - One bug for the hand-rolled Invoke/InvokeIndirect owned-position check at `forward_walk.rs:42-45` disagreeing with canonical `ArcTerminator::is_owned_position`.
   - One bug for the `ApplyIndirect [closure, ...args]` vs `InvokeIndirect [...args, closure]` structural inconsistency.

---

## 2. TDD — Test Matrix

All tests live in `compiler/ori_arc/src/aims/emit_rc/forward_walk/tests.rs` (new file). Each test constructs a minimal `ArcFunction` with a single block, the appropriate terminator shape, calls `emit_terminator_rc`, and inspects the emitted `new_body` for `RcInc` counts.

### Exact failing case (semantic pin)
- [ ] `test_emit_terminator_rc_jump_dup_arg_emits_one_inc` — `Jump { target: bb1, args: [v, v] }` where `v: RC-managed`, two RC-typed params at bb1, `is_live_at_exit(v) == false`. Asserts exactly ONE `RcInc { var: v, count: 1 }` in emitted body. Without the fix: asserts 0 incs → test fails → proves fix addresses the bug.

### Edge cases
- [ ] `test_emit_terminator_rc_jump_single_arg_not_live_emits_zero_incs` — `Jump { args: [v] }`, `live=false`. Zero incs. **Negative pin**: rejects over-emission.
- [ ] `test_emit_terminator_rc_jump_single_arg_live_emits_one_inc` — `Jump { args: [v] }`, `live=true`. One inc (the single-use live-at-exit case).
- [ ] `test_emit_terminator_rc_jump_triple_dup_emits_two_incs` — `Jump { args: [v, v, v] }`, `live=false`. Exactly 2 incs: formula `(3 + 0) - 1 = 2`.
- [ ] `test_emit_terminator_rc_jump_triple_dup_live_emits_three_incs` — `Jump { args: [v, v, v] }`, `live=true`. 3 incs: `(3 + 1) - 1 = 3`.
- [ ] `test_emit_terminator_rc_jump_interleaved_args_counts_per_var` — `Jump { args: [v, w, v] }`, both `v` and `w` RC-managed, `live=false`. Asserts ONE inc for `v` (two occurrences → 1), zero for `w` (one occurrence → 0).
- [ ] `test_emit_terminator_rc_return_single_not_live_emits_zero` — `Return { value: v }`, `live=false`. Zero incs.
- [ ] `test_emit_terminator_rc_return_single_live_emits_one` — `Return { value: v }`, `live=true`. One inc.

### Cross-terminator coverage
- [ ] `test_emit_terminator_rc_invoke_dup_owned_emits_one_inc` — `Invoke { args: [v, v], arg_ownership: [Owned, Owned] }`, `live=false`. ONE inc (same as Jump).
- [ ] `test_emit_terminator_rc_invoke_triple_dup_mixed_ownership` — `Invoke { args: [v, v, v], arg_ownership: [Owned, Borrowed, Owned] }`, `live=false`. TWO incs (formula is ownership-agnostic on the Inc side, matching body-walk semantics and the existing single-use case).
- [ ] `test_emit_terminator_rc_invoke_indirect_closure_equals_arg` — `InvokeIndirect { closure: v, args: [v], arg_ownership: [Owned] }`, `live=false`. `used_vars()` returns `[v, v]` (args first, closure last). ONE inc. **Gap cell flagged by Codex** — ensures closure-tail duplicate is covered.
- [ ] `test_emit_terminator_rc_branch_single_cond_not_live_emits_zero` — `Branch { cond: v, then: bb1, else: bb2 }`, `v: RC-managed`, `live=false`. Zero incs (single use, not live). Also asserts the existing RcDec for Branch scrutinee fires (lines 88-106 of the file).
- [ ] `test_emit_terminator_rc_branch_single_cond_live_emits_one` — `Branch { cond: v }`, `live=true`. ONE inc (single use, live). Asserts existing RcDec does NOT fire when live.
- [ ] `test_emit_terminator_rc_switch_scrutinee_not_live_emits_zero` — `Switch { scrutinee: v, cases, default }`, `v: RC-managed`, `live=false`. Zero incs. Existing RcDec fires.

### Scalar / non-RC negative pin
- [ ] `test_emit_terminator_rc_scalar_jump_dup_emits_zero_incs` — `Jump { args: [v, v] }` where `v` is scalar (ValueRepr::Scalar). `is_owned_at_entry` filter rejects → zero incs regardless of duplication. **Negative pin**: rejects spurious RC emission on scalars.

### Cross-phase interaction
- [ ] `test_emit_terminator_rc_after_body_use_dup_term_counts_correctly` — Block body has an `Apply` using `v`, then terminator is `Jump { args: [v, v] }`, `live=false`. Asserts the fix is terminator-local: the body-phase uses do NOT reduce terminator-phase inc count. Expected: body-walk emits its own incs for the body use; terminator emission emits `(2 + 0) - 1 = 1` additional inc for the two-duplicate jump args. This catches a regression where someone might try to re-thread body `uses_so_far` and get the math wrong.

### Invariant negative pin
- [ ] `test_emit_terminator_rc_debug_assert_fires_on_under_emission` — Compile-time only via `#[cfg(debug_assertions)]` section. Manually construct an `ArcFunction` state that, under a hypothetical bug where the formula returns 0 for a 2-duplicate case, would trigger the `debug_assert_eq!`. Proves the assertion is load-bearing. (This test may live as a `#[should_panic(expected = "RC balance violated")]` wrapped in a `#[cfg(debug_assertions)]` guard.)

### Verify tests fail before fix
- [ ] All new tests compile and fail against current `emit_terminator_rc` (except the `_emits_zero_incs` negative pins which should already pass). Confirms each test targets a real bug cell.

---

## 3. Implementation

### Step 1 — Update `forward_walk.rs` (core fix)
- [ ] Change `pub(crate) fn emit_terminator_rc` signature: remove `mut uses_so_far: FxHashMap<ArcVarId, usize>` parameter. Update imports (no longer needs `FxHashMap` via `rustc_hash`).
- [ ] Replace the lines 63-86 loop with aggregated terminator-occurrence counting + aggregated `RcInc` emission + local `debug_assert_eq!`.
- [ ] Add `#[cfg(test)] mod tests;` declaration at the bottom of the file.

Target code shape:

```rust
//! Terminator RC emission for the unified realization pipeline.
//!
//! Contains [`emit_terminator_rc`] (Phase C), which handles terminator uses —
//! emitting `RcInc` for live-at-exit variables and for duplicate owned
//! transfers, plus `RcDec` for Branch/Switch scrutinees that are not
//! ownership-transferred.
//!
//! The legacy body forward walk (Phase B) has been removed — body RC emission
//! is now handled by `realize/walk.rs` via the unified `decide()` surface.

use rustc_hash::FxHashMap;

use crate::ir::{ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, ValueRepr};

use super::helpers::{is_live_at_exit, is_owned_at_entry, BlockCtx};
use super::rc_strategy;

/// Phase C: handle terminator uses and non-transfer `RcDec`.
pub(crate) fn emit_terminator_rc(
    ctx: &BlockCtx<'_>,
    block_idx: usize,
    new_body: &mut Vec<ArcInstr>,
) {
    let terminator = &ctx.func.blocks[block_idx].terminator;

    // Invoke/InvokeIndirect project-borrowed-at-owned-position handling
    // (unchanged — separate concern from duplicate-use; see BUG-XX-YYY
    // for the adjacent DRIFT between this hand-rolled check and
    // `ArcTerminator::is_owned_position`).
    match terminator {
        ArcTerminator::Invoke {
            args,
            arg_ownership,
            ..
        }
        | ArcTerminator::InvokeIndirect {
            args,
            arg_ownership,
            ..
        } => {
            for (pos, &var) in args.iter().enumerate() {
                let is_owned = arg_ownership
                    .get(pos)
                    .is_some_and(|o| *o == ArgOwnership::Owned);
                if is_owned
                    && ctx.project_borrowed_defs.contains(&var)
                    && ctx.func.var_reprs[var.index()] != ValueRepr::Scalar
                {
                    if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                        new_body.push(ArcInstr::RcInc {
                            var,
                            count: 1,
                            strategy,
                        });
                    }
                }
            }
        }
        _ => {}
    }

    // RcInc for terminator uses (BUG-04-047): aggregate per distinct var.
    //
    // Count how many times each RC-managed var appears in the terminator's
    // `used_vars()` multiset. For `k` occurrences of var `v` with
    // `is_live_at_exit == L`, emit `(k + L as usize).saturating_sub(1)`
    // `RcInc` operations (as a single `RcInc { count }` for efficiency).
    //
    // Rationale: the baseline refcount provides one "free" transfer. Each
    // additional owned transfer of the same underlying reference needs its
    // own `RcInc`. If `v` is also live after this block (used in a
    // successor), one more `RcInc` hands the reference to the successor's
    // liveness chain. This is Phase C's dual of the body walk's per-use
    // `has_future_use` semantics in
    // `realize/walk.rs::compute_has_future_use` — expressed here as a
    // terminator-local aggregated count because terminators are a single
    // emission point, not an instruction stream.
    //
    // Prior to the fix, the gate at this site only checked `is_live_at_exit`,
    // missing the case where `v` appears multiple times in the terminator's
    // arg list and is dead at exit (e.g. `Jump { args: [v, v] }` after the
    // TPR-07-022 `let_alias_rep` dedup fix removes the lineage masking).
    // That miss produced a latent double-free at the target block's
    // per-param `RcDec`.
    let term_phase_start = new_body.len();
    let mut term_counts: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    for var in terminator.used_vars() {
        if !is_owned_at_entry(
            ctx.state_map,
            ctx.blk,
            var,
            ctx.defined_in_block,
            ctx.borrowed_defs,
            ctx.all_borrowed_defs,
        ) {
            continue;
        }
        *term_counts.entry(var).or_insert(0) += 1;
    }
    for (&var, &k) in &term_counts {
        let live = is_live_at_exit(ctx.state_map, ctx.blk, var) as usize;
        let count = (k + live).saturating_sub(1);
        if count > 0 {
            if let Some(strategy) = rc_strategy(ctx.func, var, ctx.pool) {
                new_body.push(ArcInstr::RcInc {
                    var,
                    count: count as u32,
                    strategy,
                });
            }
        }
    }

    // Invariant: emitted terminator-phase RcInc count per var must equal
    // the aggregated formula. Catches any future regression that under- or
    // over-emits at this site. Debug-only — zero release overhead.
    #[cfg(debug_assertions)]
    {
        let mut actual: FxHashMap<ArcVarId, u64> = FxHashMap::default();
        for instr in &new_body[term_phase_start..] {
            if let ArcInstr::RcInc { var, count, .. } = instr {
                *actual.entry(*var).or_insert(0) += u64::from(*count);
            }
        }
        for (&var, &k) in &term_counts {
            let live = is_live_at_exit(ctx.state_map, ctx.blk, var) as usize;
            let expected = (k + live).saturating_sub(1) as u64;
            let got = actual.get(&var).copied().unwrap_or(0);
            debug_assert_eq!(
                got, expected,
                "emit_terminator_rc: RC balance violated for var={var:?} at \
                 block {}: expected {expected} RcInc(s), got {got} \
                 (occurrences={k}, live_at_exit={live})",
                ctx.blk.raw()
            );
        }
    }

    // RcDec for Branch/Switch scrutinee — read but not ownership-transferred.
    // Return/Jump/Invoke transfer ownership; Resume/Unreachable have nothing.
    match &ctx.func.blocks[block_idx].terminator {
        ArcTerminator::Branch { cond, .. }
        | ArcTerminator::Switch {
            scrutinee: cond, ..
        } => {
            if !ctx.state_map.is_excluded(*cond) && !is_live_at_exit(ctx.state_map, ctx.blk, *cond)
            {
                if let Some(strategy) = rc_strategy(ctx.func, *cond, ctx.pool) {
                    new_body.push(ArcInstr::RcDec {
                        var: *cond,
                        strategy,
                    });
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
```

### Step 2 — Update `walk.rs` to drop `uses_so_far` from `BodyWalkResult`
- [ ] `BodyWalkResult` struct definition at `walk.rs:25-39`: remove the `pub uses_so_far: FxHashMap<ArcVarId, usize>,` field and its doc comment.
- [ ] `walk_body_unified` return expression at `walk.rs:138-144`: remove `uses_so_far,` from the struct literal. The local `uses_so_far` variable stays (it's still needed internally by `emit_pre_instr_incs_unified` via `compute_has_future_use`).

### Step 3 — Update `emit_unified.rs` caller
- [ ] `emit_unified.rs:241-253` — `walk::BodyWalkResult { ... }` destructure: remove `uses_so_far,` line.
- [ ] `emit_unified.rs:255` — `emit_terminator_rc(&ctx, block_idx, uses_so_far, &mut new_body);` → `emit_terminator_rc(&ctx, block_idx, &mut new_body);`.

### Step 4 — Create `forward_walk/tests.rs`
- [ ] Create directory `compiler/ori_arc/src/aims/emit_rc/forward_walk/` and file `tests.rs` inside.
- [ ] Implement all matrix tests from § 2. Each test builds a minimal `ArcFunction` using existing `ArcFunction::new` + manual block insertion (see `compiler/ori_arc/src/aims/realize/tests.rs` for reference patterns), constructs a `BlockCtx` with stub `AimsStateMap` + empty auxiliary sets, invokes `emit_terminator_rc`, and asserts on the emitted `new_body`.

### Step 5 — File adjacent bugs via `/add-bug`
- [ ] File BUG for forward_walk.rs:42-45 hand-rolled `is_owned` check disagreeing with `ArcTerminator::is_owned_position` (empty ownership default differs).
- [ ] File BUG for `ApplyIndirect [closure, ...args]` vs `InvokeIndirect [...args, closure]` structural inconsistency.

### Step 6 — Run verification
- [ ] `timeout 150 cargo test -p ori_arc` — ori_arc unit tests pass.
- [ ] `timeout 150 ./test-all.sh` — full suite green.
- [ ] `timeout 150 ./clippy-all.sh` — no new lints.

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix (no test modifications needed)
- [ ] Matrix completeness verified — every cell in type x pattern x feature grid has a test
- [ ] Debug AND release builds pass (`cargo b && cargo b --release`)
- [ ] Interpreter and LLVM produce identical results for all new tests (dual-execution parity — N/A for this unit-level fix since no source-level repro exists; matrix is compiler-unit-test only)
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on affected test programs (N/A — unit level)
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p ori_arc` green
- [ ] `/commit-push` — commit all changes before review
- [ ] Bug entry in `plans/bug-tracker/section-04-codegen-llvm.md` updated: `- [x]` with resolution details
- [ ] Fix section frontmatter `status` updated to `complete`
- [ ] Bug-tracker `00-overview.md` Quick Reference open bug count updated
- [ ] Adjacent BUG filed via `/add-bug` for `forward_walk.rs:42-45` is_owned DRIFT
- [ ] Adjacent BUG filed via `/add-bug` for ApplyIndirect/InvokeIndirect used_vars() ordering inconsistency
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues
- [ ] `/impl-hygiene-review` passed — MUST run AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` retrospective completed — MANDATORY at fix close, after both reviews are clean

**Exit Criteria:** `cargo test -p ori_arc -- aims::emit_rc::forward_walk::tests` passes all matrix tests (expanded from 15 → 18 after TPR round 1). `timeout 150 ./test-all.sh` reports 16,000+ tests passing with zero regressions. The `debug_assert_eq!` inside `emit_terminator_rc` never fires on existing test inputs. Manual grep for `uses_so_far` in `compiler/ori_arc/src/aims/` shows it has been removed from `BodyWalkResult`, from `emit_terminator_rc`'s signature, and from the `emit_unified.rs` call site (the local accumulator inside `walk_body_unified` is the only remaining reference). The adjacent bugs for forward_walk.rs:42-45 DRIFT and ApplyIndirect/InvokeIndirect ordering have both been filed in the bug tracker with `/add-bug`. Both dual-source TPR reviewers (codex + gemini) return clean after fix rounds.

---

## 5.R Third Party Review Findings

### Round 1 (2026-04-08, run `/tmp/ori-tpr-XwczknLn`)

- [x] `[TPR-04-001-codex][medium]` `compiler/ori_arc/src/aims/emit_rc/forward_walk/tests.rs:67` — Close GAP in the RC-strategy matrix for `emit_terminator_rc`.
  Evidence: The committed fixture hardcodes every test var to `list<str>` with `ValueRepr::RcPointer` at `forward_walk/tests.rs:67-72`, and none of the 15 tests mutates `var_types` or `var_reprs` to cover any other RC strategy. That leaves `FatPointer` (str), `AggregateFields` (tuple/struct), `InlineEnum` (Option/Result), and `Iterator` unexercised even though `emit_terminator_duplicate_use_incs` dispatches through `rc_strategy`, `RcStrategy` has materially different variants in `compiler/ori_arc/src/ir/repr.rs:128-207`, `emit_rc_inc_inline_enum` does real work in `compiler/ori_llvm/src/codegen/arc_emitter/rc_ops.rs:297-303` (tag-switch with per-variant RC field inc), and `emit_rc_inc_iterator` is a **no-op** at `rc_ops.rs:336-342` because iterators are move-only with no refcount header.
  Impact: The GAP proves the new count algebra only on the heap-pointer path. A strategy-specific regression (e.g., the formula emits `count: 2` for an iterator var whose LLVM emitter silently drops the inc, leaving two target-block params sharing a single move-only handle and double-freeing it) would ship green.
  Resolved: Fixed 2026-04-08 in commit `894bba57`. Introduced `VarKind` enum and `TerminatorFixture::new_with_kind` constructor, added 5 new tests (`jump_with_fat_pointer_dup_arg_emits_one_rc_inc`, `jump_with_inline_enum_dup_arg_emits_one_rc_inc`, `jump_with_aggregate_fields_dup_arg_emits_one_rc_inc`, `jump_with_closure_dup_arg_emits_one_rc_inc`, `jump_with_iterator_dup_arg_pins_algebra_not_semantic_validity`). Each test builds a minimal fixture with the target strategy's type and verifies the `(k + live).saturating_sub(1)` formula holds. Iterator test is explicitly pinned as "algebra only, not semantic validity" per the no-op LLVM emission. Matrix grew 15 → 20 tests for the strategy axis. Basis: direct_file_inspection. Confidence: high. (Codex-only finding — no gemini counterpart.)

- [x] `[TPR-04-002-codex][medium]` `compiler/ori_arc/src/aims/emit_rc/forward_walk/tests.rs:157` — Add the planned cross-phase GAP regression around `uses_so_far` removal.
  Evidence: Every committed test drives `emit_terminator_rc` directly through `TerminatorFixture::run()` at `forward_walk/tests.rs:156-157`, and every test case builds the fixture with `Vec::new()` for the block body. The plan's own TDD matrix explicitly required `test_emit_terminator_rc_after_body_use_dup_term_counts_correctly` at `plans/bug-tracker/fix-BUG-04-047.md:181` ("Block body has an `Apply` using `v`, then terminator is `Jump { args: [v, v] }`, `live=false`. Asserts the fix is terminator-local: the body-phase uses do NOT reduce terminator-phase inc count."), but that body-plus-terminator case was not added.
  Impact: Leaves the one regression vector unique to this change unpinned: a future change that accidentally re-couples terminator duplicate counting to body-use accounting would still pass the current suite. That weakens the phase-boundary guarantee that justified removing `BodyWalkResult.uses_so_far` from the realized output.
  Resolved: Fixed 2026-04-08 in commit `894bba57`. Added `jump_with_duplicated_arg_after_body_use_counts_terminator_locally` test: the block body contains an `Apply` using `v(0)` as a borrowed arg, then the terminator is `Jump { args: [v(0), v(0)] }` with `live=false`. The test verifies that the terminator-phase inc count is exactly 1 regardless of body use, pinning the phase-boundary guarantee. Because `emit_terminator_rc` is invoked directly by the fixture (no body walk runs), any re-coupling to body accounting would have to come through `emit_terminator_rc`'s signature — which no longer accepts `uses_so_far`. Basis: direct_file_inspection. Confidence: high. (Codex-only finding — no gemini counterpart.)

- [x] `[TPR-04-003-codex][low]` `plans/bug-tracker/fix-BUG-04-047.md:5` — Resolve DRIFT between BUG-04-047's fix section and the tracker state.
  Evidence: The section tracker at `plans/bug-tracker/section-04-codegen-llvm.md` marked `[x] BUG-04-047` resolved, but `plans/bug-tracker/fix-BUG-04-047.md` still said `status: in-progress` at line 5, repeats `**Status:** In Progress` at line 23, and leaves the completion checklist unchecked at lines 28-35.
  Impact: Breaks the bug-tracker SSOT discipline from `impl-hygiene.md`: follow-up reviews and automation cannot tell whether the fix is actually closed or still missing required work.
  Resolved: Fixed 2026-04-08 in commit `894bba57`. Reverted the premature `[x]` on BUG-04-047 in `section-04-codegen-llvm.md` back to `[ ]` with an explicit "In progress via `plans/bug-tracker/fix-BUG-04-047.md`" annotation and a note that the entry stays `[ ]` until Phase 5 (TPR + hygiene reviews) exits cleanly. Both tracker and fix section are now consistently "in-progress" during the TPR review loop; both will flip to complete together at Phase 5 exit (after round 3 verifies zero findings). Basis: direct_file_inspection. Confidence: high. (Codex-only finding — no gemini counterpart.)

- [x] `[TPR-04-001-gemini][medium]` `compiler/ori_arc/src/aims/emit_rc/forward_walk.rs:172` — Missing RcDec for borrowed Invoke and InvokeIndirect arguments.
  Resolved: Rejected after independent code verification on 2026-04-08. Gemini claimed that `emit_branch_switch_cond_dec` only handles `Branch`/`Switch` terminators and therefore borrowed `Invoke`/`InvokeIndirect` args at last-use-dead-at-exit receive no `RcDec`, leaking. This is **confabulated**: `compiler/ori_arc/src/aims/emit_rc/edge_cleanup.rs:277-438` (`collect_invoke_edge_decs`) is the canonical home for terminator borrowed-arg `RcDec` emission. Specifically, Category 2 at lines 412-437: "borrowed Invoke/InvokeIndirect args absent from exit_states — caller must still RcDec. Emit on both edges." The `forward_walk.rs::emit_branch_switch_cond_dec` handles only `Branch`/`Switch` scrutinees because `Invoke`/`InvokeIndirect` have dual successors (normal + unwind) and their per-edge dec placement is architecturally different — it lives in `edge_cleanup.rs` as edge-specific emission, not in `forward_walk.rs` as terminator-phase emission. Gemini failed to grep for the parallel emission pass before flagging the claim. No action needed on `forward_walk.rs`; the responsibility is correctly placed elsewhere.

- [x] `[TPR-04-002-gemini][low]` `compiler/ori_arc/src/aims/emit_rc/forward_walk.rs:136` — Robust invariant enforcement via `assert_terminator_rc_balance` (positive observation).
  Resolved: Not a finding on 2026-04-08. This entry is a positive observation reframed as a finding — gemini is commending the `assert_terminator_rc_balance` helper as "significantly improving maintainability" without flagging any defect. Per `feedback_reviewer_grounding_and_trust.md`, gemini's lower trust tier includes susceptibility to "positive confirmations reframed as findings." The helper is correct code and needs no modification.

### Round 1 — Transport Issue

### Round 2 (2026-04-09, run `/tmp/ori-tpr-WakF0smm`)

Round 2 verified all 3 codex round 1 findings were correctly fixed (RC-strategy matrix expanded, cross-phase regression test added, status DRIFT reverted). Gemini returned clean with **zero findings** and — critically — correctly verified the `edge_cleanup.rs` canonical home before concluding (proving the round 1 confabulation was a learnable mistake). Gemini also correctly emitted the sentinels this round, validating the Step 0.5 tooling fix. Codex surfaced one new low-severity DRIFT finding:

- [x] `[TPR-04-001-codex-r2][low]` `.gemini/skills/review-work/SKILL.md:215-220` — Sync the minimal envelope template with the canonical verification shape.
  Evidence: The new Step 0.5 sentinel block (added in commit `894bba57`) is correct and prominent, and the common-mistakes list is strong enough to fix the round-1 `missing_begin_sentinel` failure. But the same file still carried a second "authoritative" envelope template at lines 158-221 whose sample `verification` object used `fresh_verification_count`, `direct_file_inspection_count`, `git_history_count`, and `inference_count` at lines 215-219. That contradicted the canonical envelope contract in `.claude/skills/dual-tpr/findings-schema.json:93-99` which standardizes on `tests_rerun`, `diagnostics_run`, and `verification_gaps` as arrays of strings. Repo-wide grep confirmed no dual-TPR parser or invariant code consumes the count keys — they were pure DRIFT. Gemini itself emitted the stale form in its round 2 envelope (`fresh_verification_count: 0, direct_file_inspection_count: 13, ...`), demonstrating the impact: the mid-file template was actively misleading the reviewer even though its envelope parsed.
  Impact: SSOT violation per `impl-hygiene.md`. Two exemplars of the envelope in a single skill file with divergent `verification` shapes. Low severity because the counts are silently dropped by the parser (no findings lost), but it creates future tooling/documentation drift and wastes reviewer attention on fields that aren't consumed.
  Resolved: Fixed 2026-04-09. Replaced the stale count-based `verification` sample at lines 215-220 with the canonical array-based shape from `findings-schema.json:93-99`, and added a "Canonical `verification` shape" explainer paragraph immediately below the template pointing at the schema as the authoritative source and explicitly banning the `*_count` keys. Repo-wide grep confirms zero remaining references to any of the four stale count keys in `.gemini/` or `.claude/`. Basis: direct_file_inspection. Confidence: high. (Codex-only finding — no gemini counterpart.)

### Round 1 — Transport Issue (resolved in round 2)

- [x] `[TPR-GEMINI-ENVELOPE-FORMAT][tooling]` `.gemini/skills/review-work/SKILL.md:73-84` — gemini emitted the envelope JSON without the required `<!-- BEGIN-ORI-DUAL-TPR-V1 -->` / `<!-- END-ORI-DUAL-TPR-V1 -->` sentinels, causing `parse-gemini.py` to fail with `missing_begin_sentinel`.
  Evidence: `/tmp/ori-tpr-XwczknLn/gemini.parse-error` reports `missing_begin_sentinel | BEGIN sentinel not found in assistant text`. Manual extraction of gemini's assistant text from `gemini.jsonl` shows gemini emitted the full valid envelope JSON wrapped only in a ```` ```json ```` fenced code block, without the HTML-comment sentinels that `.gemini/skills/review-work/SKILL.md:75-84` instructs it to produce. The SKILL.md rule is under a "Envelope output requirement" heading that isn't prominent enough to survive gemini's thinking-phase output composition.
  Impact: Every `/tpr-review` and `/review-work` invocation hits a 22-minute transport delay followed by a deterministic parse failure on the gemini side, making dual-source reviews unreliable. After BUG-08-006's deterministic-failure classification, the wrapper correctly refuses to retry, but the envelope is still unparseable and the loop cannot merge findings without manual recovery.
  Resolved: Fixed 2026-04-08 in commit `894bba57`. Added a new "Step 0.5: Envelope Sentinels (MANDATORY — load-bearing at output time)" section immediately after Step 0 in `.gemini/skills/review-work/SKILL.md`, with 4 common-mistake examples (bare fenced JSON, sentinels inside fence, only one sentinel, wrong sentinel text) and a pre-submit validation checklist. Verified in round 2 (run `/tmp/ori-tpr-WakF0smm`): gemini correctly emitted both sentinels on the first attempt, round 2 parsed cleanly in 437s (down from 1346s in round 1), and gemini also self-corrected its round 1 Invoke-dec confabulation by running a fresh grep for `collect_invoke_edge_decs` in `edge_cleanup.rs` before reaching its round 2 conclusion.
  Basis: direct_file_inspection (manual envelope extraction from gemini.jsonl) + fresh_verification (round 2 empirical validation). Confidence: high.
