---
section: "12"
title: "FIP Proof Obligations & Enforcement"
status: not-started
goal: "Complete the FIP certification proof obligations (may_deallocate, constant stack) and build a verifier that rejects contract/emission mismatches"
inspired_by:
  - "FP² Theorem 2 (Lorenzen et al., ICFP 2023) — allocation balance"
  - "Koka CheckFBIP (src/Core/CheckFBIP.hs) — FBIP enforcement"
  - "Lean 4 RC.lean — stack-depth tracking in RC insertion"
depends_on: ["09", "10", "11"]
sections:
  - id: "12.1"
    title: "EffectSummary.may_deallocate"
    status: not-started
  - id: "12.2"
    title: "Constant Stack Verification"
    status: not-started
  - id: "12.3"
    title: "FIP Enforcement Verifier"
    status: not-started
  - id: "12.4"
    title: "Stale Documentation Cleanup"
    status: not-started
  - id: "12.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 12: FIP Proof Obligations & Enforcement

**Status:** Not Started

**Goal:** Complete the three FP² proof obligations for FIP certification
(no allocation, no deallocation, constant stack) and build an enforcement
verifier that rejects mismatches between `MemoryContract.fip` and the
emitted IR. After this section, FIP certification is a proven property,
not a metadata label.

**Context:** Section 09.2 Effect Activation added FIP classification to
`extract_contract()`, reading the converged effect summary and token
balance. This produces `FipContract::Certified`, `Conditional`, and
`Bounded(n)` classifications. However, audit reveals three gaps:

1. **`may_deallocate` is missing.** The `EffectSummary` struct has a doc
   comment (contract/mod.rs:300-309) saying `may_deallocate` is "Planned:
   Stage 2", but the field was never added. `extract_contract()` sets
   `is_fbip = !effects.may_allocate` (interprocedural.rs:524) and then
   upgrades FBIP to `FipContract::Certified` (interprocedural.rs:528).
   This FBIP->Certified shortcut is valid (no allocation implies trivially no
   deallocation), but the general case -- functions that allocate AND reuse
   -- cannot be certified without knowing whether unmatched deallocations
   occur. FP² Theorem 2 requires both `may_allocate == false` AND
   `may_deallocate == false` for full FIP.

2. **Constant stack space is not checked.** FP² defines FIP as "no
   allocation, no deallocation, **constant stack space**, provided
   arguments are unique." The stack-space obligation is relevant for
   recursive functions: a self-recursive function with O(n) stack depth
   cannot be FIP even if it is allocation-balanced. The current plan has
   no mechanism to verify this.

3. **No enforcement verifier.** `FipEvidence` (missed reuses, gate
   records) is accumulated during realization (Section 10) and stored in
   `RealizationResult`, but nothing cross-checks it against
   `MemoryContract.fip`. If `extract_contract()` produces
   `FipContract::Certified` but realization emits unmatched allocations,
   this silent inconsistency goes undetected.

**Reference implementations:**
- **Koka** `src/Core/CheckFBIP.hs`: Post-pipeline FBIP enforcement that
  walks the core IR and checks that no unmatched allocations remain.
  Emits warnings for non-FBIP functions. The model for our enforcement
  verifier.
- **FP²** Theorem 2: The formal proof obligation -- FIP requires
  allocation balance, deallocation balance, and bounded stack.
- **Lean 4** `src/Lean/Compiler/IR/RC.lean`: Tracks stack depth during
  RC insertion for tail-call decisions -- relevant for constant-stack
  verification.

**Depends on:** Section 09 (FIP classification in `extract_contract()`),
Section 10 (unified realization producing `FipEvidence`), Section 11
(regression guards).

---

## 12.1 EffectSummary.may_deallocate

**File(s):** `compiler/ori_arc/src/aims/contract/mod.rs`,
`compiler/ori_arc/src/aims/interprocedural.rs`,
`compiler/ori_arc/src/aims/emit_reuse/mod.rs`

FP² Theorem 2 requires both sides of the allocation balance: `may_allocate`
gives FBIP (no fresh allocations). Full FIP additionally requires
`may_deallocate == false` -- no unmatched deallocations (frees without reuse).

**Codebase hygiene:** `interprocedural.rs` is 742 lines (exceeds 500-line
limit). Adding `has_unbounded_stack` detection (12.2) and `extract_contract()`
updates (12.1, 12.2) will push it further. Before starting this section,
extract `extract_contract()` + return-info helpers (lines 478-742, ~265 lines)
into a new `interprocedural/extract.rs` submodule. `interprocedural.rs`
becomes `interprocedural/mod.rs` containing `analyze_program()` +
`analyze_scc_fixpoint()` + demand propagation (~480 lines).

**Codebase hygiene:** `intraprocedural/mod.rs` is 941 lines (exceeds 500-line
limit). Before starting this section, extract the post-convergence passes
(lines 244-841: `populate_borrow_sources`, `populate_sparse_events`,
`populate_var_shapes`, `detect_trmc_candidates`, `populate_context_events`,
`populate_fip_balance`, `count_reusable_constructs`, `count_consumed_params`,
`compute_requires_unique_params`, `record_per_branch_balance`,
`compute_block_fip_balance`, `populate_fip_gate_events`) into a new
`intraprocedural/post_convergence.rs` submodule (~600 lines). Keep
`analyze_function()`, `verify_canonical_fixed_point()`, `widen_to_top()` in
`mod.rs` (~300 lines).

- [ ] **Prerequisite split:** Extract `interprocedural.rs` into `interprocedural/mod.rs` +
  `interprocedural/extract.rs`. Must be done first to keep files under 500 lines
  when adding `may_deallocate` and `has_unbounded_stack`.
- [ ] **Prerequisite split:** Extract `intraprocedural/mod.rs` post-convergence
  passes into `intraprocedural/post_convergence.rs`.
- [ ] Add `may_deallocate: bool` field to `EffectSummary`:
  ```rust
  pub struct EffectSummary {
      pub may_allocate: bool,
      pub alloc_only_on_slow_path: bool,
      /// May the function deallocate on any code path?
      ///
      /// `true` if any consumed value with reusable shape was NOT matched
      /// by a reuse opportunity — the function frees memory without reusing
      /// it. Computed post-emission from `EmitReuseResult.missed_reuses > 0`
      /// or from `FipEvidence.missed_reuses > 0`.
      ///
      /// When `may_allocate == false && may_deallocate == false`, the
      /// function is fully in-place (FIP per FP² Theorem 2).
      pub may_deallocate: bool,
      pub may_share: bool,
      pub may_throw: bool,
  }
  ```

- [ ] Update `#[expect(clippy::struct_excessive_bools)]` reason from "4 independent
  effect flags" to "6 independent effect flags" (adding `may_deallocate` in 12.1
  and `has_unbounded_stack` in 12.2). Update to the final count "6" in one step
  to avoid a partial-count commit.
- [ ] Update `EffectSummary::CONSERVATIVE` to include `may_deallocate: true`
- [ ] Update `EffectSummary::OPTIMISTIC` to include `may_deallocate: false`
- [ ] Update `EffectSummary::join()` to include `may_deallocate: self.may_deallocate || other.may_deallocate`

- [ ] Verify `EffectSummary` derives `Default` correctly with new field:
  `EffectSummary` derives `Default`, so `may_deallocate` defaults to `false`.
  This matches `OPTIMISTIC` (correct for the accumulation pattern: start
  optimistic, OR in effects during analysis). The `accumulate_effect()` method
  in `intraprocedural/state_map.rs` uses `join()` to OR effect flags together,
  so `may_deallocate` will be accumulated correctly during analysis.

- [ ] Verify builtins: `builtins/mod.rs` uses `EffectSummary::default()` for
  all builtin contracts. Since `Default` gives `may_deallocate: false`, this
  is correct (builtins don't deallocate). No change needed beyond verifying.

- [ ] Compute `may_deallocate` during interprocedural analysis:
  A function may deallocate if any code path contains a consumed value
  with reusable shape that is NOT paired with a reuse opportunity. This
  is a post-emission fact -- it requires knowing the reuse plan. Two
  approaches:

  **(a) Two-pass interprocedural** (recommended): First pass computes
  contracts with `may_deallocate = false` (optimistic). After realization
  (which produces `FipEvidence.missed_reuses`), update the contract's
  `may_deallocate` field. This is sound because `may_deallocate` only
  flows into FIP classification (not into other dimensions), so the
  contract update doesn't invalidate converged analysis.

  **(b) Conservative approximation**: Set `may_deallocate = true` if the
  function body contains any `RcDec` that isn't paired with a `Reuse` in
  the same block. Simpler but less precise.

  **Recommended:** Option (a) -- post-emission update from `FipEvidence`.

- [ ] Implement post-emission `may_deallocate` update in `aims_pipeline.rs`:
  After `realize_rc_reuse()` returns `RealizationResult`, update the contract:
  ```rust
  if let Some(contract) = contracts.get_mut(&func.name) {
      contract.effects.may_deallocate = result.fip_evidence.missed_reuses > 0;
  }
  ```
  This requires `contracts` to be `&mut FxHashMap` in `run_aims_pipeline()` and
  `AimsPipelineConfig`. Alternatively, collect updates and apply in a second pass
  in `run_aims_pipeline_all()`. The second approach avoids changing the config
  struct but requires iterating functions twice.

  **Tradeoff:** Changing `AimsPipelineConfig.contracts` from `&FxHashMap` to
  `&mut FxHashMap` touches a widely-used config struct. The current
  `run_aims_pipeline_all()` creates `config` with `contracts: &contracts`
  (immutable borrow). Changing to `&mut` requires `contracts` to be `mut` in
  `run_aims_pipeline_all()` and every call site. **Recommended: second-pass
  approach** -- collect `Vec<(Name, bool)>` during the per-function loop, apply
  to `contracts` after the loop completes. This avoids mutating the config
  struct entirely.

- [ ] Update `extract_contract()` in `interprocedural.rs` to use
  `may_deallocate` for FIP classification:
  - Current: `if is_fbip { FipContract::Certified }` (line 528)
  - New: `if !effects.may_allocate && !effects.may_deallocate { FipContract::Certified }`
  - The FBIP shortcut (`!may_allocate -> Certified`) remains valid as a
    fast path: if the function never allocates, it trivially never
    deallocates (nothing to free).

- [ ] Sync points for `may_deallocate`:
  - `contract/mod.rs` -- struct definition, `all_borrowed()` constructor,
    `CONSERVATIVE`/`OPTIMISTIC` constants, `join()`
  - `intraprocedural/state_map.rs` -- `EffectSummary` stored in `AimsStateMap`;
    `accumulate_effect()` uses `join()` so no code change needed, but verify
    that the `Default`-derived initial value (`false`) is correct
  - `interprocedural.rs` -- `extract_contract()`
  - `builtins/mod.rs` -- builtin effect summaries (uses `Default`, correct as-is)
  - `pipeline/aims_pipeline.rs` -- post-realization `may_deallocate` update
  - `verify/fip.rs` -- enforcement verifier (Section 12.3)
  - `contract/tests.rs` -- join tests, `CONSERVATIVE`/`OPTIMISTIC` field tests

---

## 12.2 Constant Stack Verification

**File(s):** `compiler/ori_arc/src/aims/interprocedural.rs`,
`compiler/ori_arc/src/aims/contract/mod.rs`

FP² requires constant stack space for FIP functions. A self-recursive
function with O(n) stack depth cannot be FIP even if it is allocation-
balanced. This matters for tree traversals that reuse allocations but
recurse to depth proportional to tree height.

- [ ] Define what "constant stack" means in AIMS context:
  - A function has constant stack if it does not call itself recursively
    (directly or via mutual recursion) without a tail-call optimization.
  - Functions with self-recursive tail calls that are rewritten to loops
    by the tail-call pass (pipeline step 8) have constant stack.
  - Functions with non-tail self-recursion have O(n) stack growth.
  - Functions that only call non-recursive callees have constant stack.

- [ ] Add `has_unbounded_stack: bool` field to `EffectSummary`:
  ```rust
  /// Does this function have unbounded stack growth?
  ///
  /// `true` if the function contains non-tail-recursive calls to itself
  /// or to mutual-recursion partners. Functions where all recursive calls
  /// are in tail position (rewritten to loops by the tail-call pass) are
  /// considered constant-stack.
  ///
  /// Unlike `may_allocate`/`may_share`/`may_throw`, this is NOT accumulated
  /// per-block during analysis. It is set once in `extract_contract()` from
  /// SCC membership and syntactic tail-position checks.
  pub has_unbounded_stack: bool,
  ```

  **Sync points for `has_unbounded_stack`:**
  - `contract/mod.rs` -- struct definition, `CONSERVATIVE` (`true`), `OPTIMISTIC`
    (`false`), `join()` (OR -- either side unbounded means joined is unbounded)
  - `contract/mod.rs` -- `#[expect(clippy::struct_excessive_bools)]` reason already
    updated to "6 independent effect flags" in 12.1 (covers both new fields)
  - `contract/mod.rs` -- `all_borrowed()` uses `EffectSummary::OPTIMISTIC` -- correct
  - `intraprocedural/state_map.rs` -- `accumulate_effect()` uses `join()`, which
    will OR `has_unbounded_stack`. But since this field is NOT set during per-block
    analysis (it's set in `extract_contract()`), the accumulated value will be
    `false`. This is harmless -- `extract_contract()` overrides it. Add a comment.
  - `interprocedural.rs` -- `extract_contract()` must set `has_unbounded_stack`
  - `builtins/mod.rs` -- `EffectSummary::default()` gives `false` (correct)
  - `contract/tests.rs` -- join tests

- [ ] Detection approach:
  - During SCC analysis, identify self-recursive SCCs (already done in
    `analyze_scc_fixpoint` in `interprocedural.rs`).
  - For each self-recursive function, check whether ALL recursive calls
    are in syntactic tail position. The tail-call detection logic is in
    `compiler/ori_arc/src/tail_call/mod.rs` (`detect_tail_calls()`).
    Reuse the tail-position check from `tail_call/mod.rs` or extract the
    syntactic check into a shared helper that both `detect_tail_calls()`
    and `extract_contract()` can call.
  - If any recursive call is NOT in tail position, set
    `has_unbounded_stack = true`.
  - Mutual recursion: if any function in a non-trivial SCC has a non-tail
    call to another SCC member, set `has_unbounded_stack = true` for all
    SCC members.

- [ ] Extract syntactic tail-position check:
  `detect_tail_calls()` runs on post-emission IR and checks for safe `RcDec`
  instructions after the call. At contract-extraction time (pre-emission), the
  IR has no `RcDec` instructions. Extract a simpler `is_in_tail_position()`
  helper that checks only structural tail position (call is the last instruction
  before a `Return` terminator, possibly through single-successor jumps). Place
  in `compiler/ori_arc/src/tail_call/mod.rs` (or a shared `graph/` utility).

  **Caveat:** The pre-emission IR may have different block structure than
  post-emission IR (lowering choices, Invoke vs Apply, etc.). The existing
  `detect_tail_calls()` handles both direct (Apply+Return in same block)
  and cross-block (Apply+Jump->Return) patterns, plus Invoke terminators.
  The pre-emission check must handle the same patterns minus RcDec filtering.
  Verify that all three patterns in `tail_call/mod.rs` (direct, cross-block,
  invoke) apply at pre-emission time. Read `tail_call/mod.rs` carefully
  before writing the helper -- the cross-block and Invoke patterns are
  non-trivial (~200 lines combined).

- [ ] Integrate with FIP classification in `extract_contract()`:
  - Current: `FipContract::Certified` requires `!may_allocate` (or
    allocation-balanced).
  - New: additionally requires `!has_unbounded_stack`.
  - A function that is allocation-balanced but has unbounded stack is
    `FipContract::Bounded(n)` (bounded allocation) but NOT `Certified`.
  - Update the FIP classification logic at `interprocedural.rs:528-561`
    to gate `Certified` on `!has_unbounded_stack`.

- [ ] Handle tail-call rewrite ordering:
  The tail-call rewrite pass (pipeline step 8) runs AFTER realization
  (step 5). This means at contract-extraction time, we don't know which
  recursive calls will be rewritten to loops. Two options:

  **(a) Conservative**: Assume no tail-call rewriting. Self-recursive
  functions with any recursive call get `has_unbounded_stack = true`.
  This is pessimistic but sound -- it under-certifies.

  **(b) Syntactic check**: Use the syntactic tail-position check
  extracted above. If all recursive calls are in syntactic tail
  position, assume the tail-call pass will rewrite them. This is the
  same structural check the tail-call pass uses (minus the RcDec
  safety check, which is irrelevant at contract time).

  **Recommended:** Option (b) -- the syntactic check is accurate.

- [ ] Tests:
  - `constant_stack_non_recursive_is_false` -- non-recursive function
  - `constant_stack_tail_recursive_is_false` -- tail-recursive function
  - `constant_stack_non_tail_recursive_is_true` -- tree traversal
  - `constant_stack_mutual_recursion_is_true` -- mutually recursive SCC
    with non-tail cross-calls
  - `fip_certified_requires_constant_stack` -- allocation-balanced but
    non-tail-recursive is NOT Certified
  - `is_in_tail_position_basic` -- unit test for the extracted helper

---

## 12.3 FIP Enforcement Verifier

**File(s):** `compiler/ori_arc/src/aims/verify/mod.rs` (NEW),
`compiler/ori_arc/src/aims/verify/fip.rs` (NEW),
`compiler/ori_arc/src/aims/mod.rs` (add `pub mod verify;`),
`compiler/ori_arc/src/pipeline/aims_pipeline.rs`

Build a post-realization verifier that cross-checks `MemoryContract.fip`
against the emitted IR and `FipEvidence`. This catches silent mismatches
where `extract_contract()` claims FIP but realization didn't achieve it.

- [ ] Create module infrastructure:
  - Create `compiler/ori_arc/src/aims/verify/mod.rs`:
    ```rust
    //! AIMS verification passes.
    //!
    //! Post-realization verifiers that cross-check analysis contracts
    //! against the emitted IR. Catches inconsistencies between what
    //! `extract_contract()` claims and what realization achieved.
    pub mod fip;
    ```
  - Create `compiler/ori_arc/src/aims/verify/fip.rs` (implementation below)
  - Add `pub mod verify;` to `compiler/ori_arc/src/aims/mod.rs` (between
    `pub mod transfer;` and any future modules, or at end of module list)

- [ ] Define `FipVerificationError` enum:
  ```rust
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub enum FipVerificationError {
      /// Contract says Certified but realization has missed reuses.
      CertifiedButHasMissedReuses {
          function: Name,
          missed_count: usize,
      },
      /// Contract says Certified but function has allocations.
      CertifiedButAllocates {
          function: Name,
          alloc_count: usize,
      },
      /// Contract says Certified but function has unbounded stack.
      CertifiedButUnboundedStack {
          function: Name,
      },
      /// Contract says Conditional but required-unique params don't
      /// match the analysis state.
      ConditionalParamMismatch {
          function: Name,
          param_index: usize,
      },
      /// Contract says Bounded(n) but actual net allocation exceeds n.
      BoundedExceeded {
          function: Name,
          declared: u16,
          actual: u16,
      },
  }

  impl std::fmt::Display for FipVerificationError {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          match self {
              Self::CertifiedButHasMissedReuses { function, missed_count } =>
                  write!(f, "FIP Certified but {missed_count} missed reuses in {function:?}"),
              // ... (all variants)
          }
      }
  }
  ```

- [ ] Implement `verify_fip_contract()`:
  ```rust
  pub fn verify_fip_contract(
      func: &ArcFunction,
      contract: &MemoryContract,
      evidence: &FipEvidence,
  ) -> Vec<FipVerificationError> { ... }
  ```

  Checks:
  1. If `contract.fip == Certified`:
     - `evidence.missed_reuses == 0` (no unmatched deallocations)
     - No `Construct` instructions without matching `Reuse` (no allocations)
     - `contract.effects.has_unbounded_stack == false`
  2. If `contract.fip == Conditional { requires_unique_params }`:
     - Each `requires_unique_params[i] == true` implies the parameter
       genuinely needs uniqueness for the FIP fast path
  3. If `contract.fip == Bounded(n)`:
     - Net allocation count (Constructs - Reuses) <= n

  **Section 13 interaction:** TRMC-rewritten functions (Section 13.4) may
  change FIP classification: a function that was `Never` (due to O(n) stack
  from non-tail recursion) may become `Certified` after TRMC rewrites the
  recursion into tail position. The verifier must handle functions that
  were rewritten by TRMC -- the Construct/Reuse counts and stack properties
  may differ from the original function. Since Section 12 runs first in the
  pipeline (verifier at step 5a) and Section 13's TRMC rewrite runs at
  step 3a (before analysis), the verifier naturally sees the post-rewrite
  IR. No special handling needed, but add a test:
  - `verify_trmc_rewritten_function_certified` -- function rewritten by TRMC
    is correctly verified as Certified

- [ ] Wire into pipeline:
  Call `verify_fip_contract()` in `run_aims_pipeline()` (`aims_pipeline.rs`)
  after `realize_rc_reuse()` (step 5) returns the `RealizationResult`, and
  after the `may_deallocate` post-emission update. Insert between the
  current realization call and `verify_and_merge()`:
  ```rust
  // Step 5a: FIP enforcement verification.
  if let Some(contract) = config.contracts.get(&func.name) {
      let errors = crate::aims::verify::fip::verify_fip_contract(
          func, contract, &result.fip_evidence,
      );
      for e in &errors {
          if cfg!(debug_assertions) {
              // In debug: hard failure to catch bugs early.
              panic!("FIP verification failed: {e}");
          } else {
              tracing::warn!("FIP verification: {e}");
          }
      }
  }
  ```
  Note: `config.contracts` is currently `&FxHashMap` (immutable). If using the
  second-pass approach from 12.1, run the verifier in the second pass after all
  `may_deallocate` fields are updated.

- [ ] Relationship to existing `run_aims_verify()`:
  The existing `run_aims_verify()` (step 7, `pipeline/mod.rs:144`) checks
  structural consistency (e.g., `AbsentParamHasUses`). The new FIP verifier
  checks semantic consistency (contract claims vs realization evidence).
  Keep both -- they catch different classes of bugs. The FIP verifier runs
  at step 5a (right after realization); the structural verifier runs at
  step 7 (after emission is complete).

- [ ] Error handling:
  - In debug builds: `debug_assert!` (or `panic!`) on verification failures
    (catches bugs during development).
  - In release builds: `tracing::warn!` on failures (don't crash the
    compiler, but flag the inconsistency).
  - Long-term: FIP verification failures should be compiler errors if
    the user annotated `#fip` on the function (user-requested
    certification that wasn't achieved).

- [ ] Tests:
  Test file: `compiler/ori_arc/src/aims/verify/tests.rs`
  (add `#[cfg(test)] mod tests;` to `verify/fip.rs`)
  - `verify_certified_no_allocations_passes` -- clean FIP function
  - `verify_certified_with_missed_reuse_fails` -- contract says Certified
    but evidence shows missed reuses
  - `verify_certified_with_allocations_fails` -- contract says Certified
    but IR has unmatched Constructs
  - `verify_certified_with_unbounded_stack_fails` -- contract says Certified
    but `has_unbounded_stack == true`
  - `verify_bounded_within_limit_passes` -- Bounded(2) with 2 net allocs
  - `verify_bounded_exceeded_fails` -- Bounded(1) with 3 net allocs
  - `verify_conditional_params_match` -- Conditional with correct params
  - `verify_never_always_passes` -- FipContract::Never skips verification

---

## 12.4 Stale Documentation Cleanup

**File(s):** `compiler/ori_arc/src/aims/contract/mod.rs`

The module-level doc comment at `contract/mod.rs:8-15` says "Stage 1:
FipContract is always Never, TRMC disabled" -- this is no longer true
after Section 09.2 activated FIP inference and effects. Fix the banner
to reflect the current state.

- [ ] Update `contract/mod.rs` module-level doc:
  Replace the "Stage 1" banner with an accurate description of the
  current state:
  ```rust
  //! # Current State
  //!
  //! All contract fields are active and refined by interprocedural
  //! analysis:
  //! - Core fields (`access`, `consumption`, `cardinality`, `uniqueness`)
  //!   are refined by SCC fixed-point iteration
  //! - `EffectSummary` fields are computed from function body instructions
  //! - `FipContract` is inferred from converged effect state and token
  //!   balance (`extract_contract()` in `interprocedural.rs`)
  //! - `ContextBehavior` is always `default()` — populated in Section 13
  //!   when TRMC realization is implemented
  //! - `is_fbip` is `!effects.may_allocate` (inferred metadata)
  ```

- [ ] Review and fix all stale "Stage 1" comments in AIMS codebase (20+
  occurrences across 9 files):
  - `aims/contract/mod.rs:63` -- `all_borrowed()` doc says "Stage 1: pass
    FipContract::Never". Update to: "pass FipContract::Never to disable FIP,
    Certified for optimistic start"
  - `aims/contract/mod.rs:362` -- "Default (conservative) in Stage 1." Remove
    "in Stage 1" -- Default is always conservative regardless of stage
  - `aims/interprocedural.rs:58-59` -- `analyze_function()` doc says "empty in
    Stage 1" for sigs and context_regions. Replace with "empty when no
    interprocedural info available" / "empty when no TRMC candidates detected"
  - `aims/emit_reuse/mod.rs:16,108,190` -- "Stage 1: static-unique only".
    Replace with "v1: static-unique only" (stage-neutral labeling)
  - `aims/emit_reuse/detect.rs:8` -- "Stage 1: static-unique only". Same fix.
  - `aims/emit_reuse/planner.rs:12,60,77,143` -- "Stage 1" references. Same fix.
  - `aims/emit_rc/arg_ownership.rs:8,10,13,31` -- "Stage 1" references. Replace
    with "v1" or "current" as appropriate
  - `aims/immortal/mod.rs:31,64` -- "Stage 1" references. Replace with "v1"
  - `aims/lattice/mod.rs:73,83` -- "Stage 1" references. Replace with "v1"
  - `aims/normalize/mod.rs:14` -- "Detection only -- no IR rewriting." This is
    accurate for current state; keep but remove stage language if present

---

## 12.4a Codebase Hygiene -- Fix Along the Way

These items should be fixed during implementation of 12.1-12.4, not as
separate commits:

- [ ] **WASTE (duplication):** `collect_recursive_call_defs()` in
  `intraprocedural/mod.rs:545-572` duplicates the same logic as
  `collect_recursive_call_sites()` in `normalize/detect.rs:108-149` -- both
  scan for Apply/Invoke where callee == func.name and collect defined vars.
  The only difference is that detect.rs also records the CallSite (block,
  instr). Unify into a single shared helper in `graph/` or `aims/` that
  returns `FxHashMap<ArcVarId, CallSite>` and let intraprocedural callers
  project to `FxHashSet<ArcVarId>` via `.keys()`. Fix during 12.1-12.4
  while touching both files.
- [ ] **STYLE (stale doc comment):** `EffectSummary` doc at
  `contract/mod.rs:300-309` describes `may_deallocate` as "Planned: Stage 2"
  -- will be outdated once 12.1 adds the field. Update the doc comment when
  adding the field (remove the "Planned" note, replace with actual field doc).
- [ ] **STYLE (clippy reason accuracy):** `EffectSummary` clippy reason at
  `contract/mod.rs:313` says "4 independent effect flags from FP² paper" --
  currently accurate (4 bool fields: `may_allocate`, `alloc_only_on_slow_path`,
  `may_share`, `may_throw`) but must be updated to "6" after adding
  `may_deallocate` + `has_unbounded_stack`. Do this in a single step at the end
  of 12.2 (not incrementally).
- [ ] **STYLE (stale `all_borrowed` doc):** `MemoryContract::all_borrowed()` at
  `contract/mod.rs:62-64` documents `fip_initial` with "Stage 1" / "Stage 2"
  labels. Replace with behavior-based docs (see 12.4 list above).

---

## 12.5 Completion Checklist

- [ ] **Prerequisite:** `interprocedural.rs` split into `interprocedural/mod.rs` +
  `interprocedural/extract.rs` (742 lines -> ~480 + ~265)
- [ ] **Prerequisite:** `intraprocedural/mod.rs` post-convergence passes extracted
  to `intraprocedural/post_convergence.rs` (941 lines -> ~300 + ~600)
- [ ] Duplicated `collect_recursive_call_defs` unified with
  `collect_recursive_call_sites`
- [ ] All stale "Stage 1" comments across AIMS codebase updated (20+ occurrences
  in 9 files -- see 12.4 expanded list)
- [ ] `EffectSummary.may_deallocate` field added and wired through all
  sync points (contract, interprocedural, builtins, state_map, join, tests)
- [ ] `may_deallocate` computed post-emission from `FipEvidence.missed_reuses`
- [ ] Post-emission `may_deallocate` update wired into `aims_pipeline.rs`
- [ ] `extract_contract()` uses `may_deallocate` for FIP classification
  (FBIP shortcut preserved as fast path)
- [ ] `has_unbounded_stack` tracking added to `EffectSummary` with all sync
  points (CONSERVATIVE, OPTIMISTIC, join, clippy reason, Default)
- [ ] Syntactic tail-position helper extracted to `tail_call/mod.rs` or
  shared utility, with unit tests
- [ ] Stack-depth check uses syntactic tail-position analysis
- [ ] FIP classification requires `!has_unbounded_stack` for `Certified`
- [ ] `aims/verify/` module created with `mod.rs` + `fip.rs`
- [ ] `pub mod verify;` added to `aims/mod.rs`
- [ ] `verify_fip_contract()` implemented and wired into pipeline (step 5a)
- [ ] FIP verification catches contract/emission mismatches for all
  `FipContract` variants (Certified, Conditional, Bounded, Never)
- [ ] `FipVerificationError` has `Debug`, `PartialEq`, `Eq`, `Display`
- [ ] Stale "Stage 1" banner in `contract/mod.rs` updated
- [ ] All stale stage comments in AIMS module reviewed and fixed
- [ ] `clippy::struct_excessive_bools` reason updated to "6 independent
  effect flags" (may_allocate, alloc_only_on_slow_path, may_deallocate,
  may_share, may_throw, has_unbounded_stack)
- [ ] `cargo test --workspace` green
- [ ] `./test-all.sh` green
- [ ] Valgrind: 0 memory errors on all test programs

**Exit Criteria:** `FipContract::Certified` means the function provably
has no allocation, no deallocation, and constant stack space.
`verify_fip_contract()` rejects any mismatch between the contract and the
emitted code. `cargo test -p ori_arc -- aims::verify::fip` passes with
tests covering all verification paths. The `may_deallocate` field is
computed from actual reuse results, not from a conservative approximation.
