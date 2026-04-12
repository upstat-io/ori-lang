---
section: "05"
title: "Contract Coherence Oracle"
status: not-started
reviewed: false
goal: "Build an independent contract re-derivation oracle that walks the final realized ARC IR (actual RcInc/RcDec/Reuse instructions post-pipeline), derives a MemoryContract from what was actually emitted, and compares it against the inferred MemoryContract — discrepancies are blocking errors under ORI_VERIFY_ARC=1"
success_criteria:
  - "Oracle walks post-pipeline ARC IR and derives ParamContract for each parameter from actual RC instructions"
  - "Oracle accounts for may_deallocate correction in the second pass (batch.rs)"
  - "Oracle comparison with inferred MemoryContract produces clear diagnostics on mismatch"
  - "Mismatch is a blocking error under ORI_VERIFY_ARC=1 (not a warning)"
  - "Oracle passes for all existing test programs (no false positives on correct code)"
  - "A deliberately introduced contract mismatch is caught by the oracle"
inspired_by:
  - "Lean4 IR Checker (lean4/src/Lean/Compiler/IR/Checker.lean) — post-optimization IR verification against pre-optimization contracts"
  - "Swift SIL Verifier (swift/lib/SIL/Verifier/SILVerifier.cpp) — independent verification of SIL invariants after each pass"
depends_on: ["03", "04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Contract Re-Derivation from Realized IR"
    status: not-started
  - id: "05.2"
    title: "May-Deallocate Second-Pass Accounting"
    status: not-started
  - id: "05.3"
    title: "Oracle Comparison and Diagnostics"
    status: not-started
  - id: "05.4"
    title: "Integration into AIMS Pipeline"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Contract Coherence Oracle

> **RESET (2026-04-11):** All work in this section was produced by an autopilot session with inadequate planning and TPR oversight. Implementation code may exist in the codebase (commits from the autopilot session) but the design, test coverage, and verification cannot be trusted as valid. This section must be re-done from scratch with proper planning, review (`/review-plan`), and verification (`/tpr-review` + `/impl-hygiene-review`). The existing code should be audited during re-implementation — it may be partially reusable but must not be assumed correct.

**Status:** Not Started
**Goal:** Build an independent contract re-derivation oracle that walks the final realized ARC IR (actual `RcInc`/`RcDec`/`Reuse` instructions after the full AIMS pipeline completes), derives a `MemoryContract` from what was actually emitted, and compares it against the inferred `MemoryContract` from interprocedural analysis. Discrepancies are blocking errors under `ORI_VERIFY_ARC=1`. This catches the class of bugs where the AIMS analysis infers a correct contract but the realization pipeline (steps 5-12) emits IR that violates it — or where the analysis infers an incorrect contract that happens to produce working code by accident.

**Success Criteria:**

- [ ] Oracle re-derives `ParamContract` per parameter from post-pipeline IR — satisfies mission criterion: "Contract coherence oracle"
- [ ] Oracle accounts for `may_deallocate` second-pass correction — satisfies mission criterion: "Contract coherence oracle"
- [ ] Mismatch is blocking error under `ORI_VERIFY_ARC=1` — satisfies mission criterion: "Verifier failures become blocking gates"
- [ ] All existing test programs pass oracle (zero false positives) — satisfies mission criterion: "No regressions"
- [ ] Deliberately introduced mismatch caught — satisfies mission criterion: "Regression detection"

**Context:** The AIMS pipeline has a fundamental coherence requirement (`.claude/rules/arc.md` §Non-Negotiable Invariant #1): "Contracts and realization must agree." Currently, this is partially enforced by `run_aims_verify()` at steps 7 and 11, but those checks verify structural ARC IR properties — not whether the emitted RC instructions are consistent with what the contract claims. For example, if `MemoryContract` says a parameter has `Consumption::Linear` (consumed once, no RC needed), but the realized IR contains an `RcInc` for that parameter, the contract and realization disagree. The existing verifiers would not catch this — the IR is structurally valid, just semantically inconsistent with the contract.

The oracle is conceptually simple: after the pipeline completes (step 12), walk the realized ARC IR for each function, observe the actual RC operations per parameter, and derive what the contract *should* be based on the evidence. Compare with the inferred contract. Any inconsistency is a bug — either in the analysis (wrong contract) or in the realization (wrong RC emission).

**Critical caveat:** `may_deallocate` is corrected in the second pass (`batch.rs:150-157`). The oracle must run AFTER this correction, not before. The second pass counts missed reuses and sets `contract.effects.may_deallocate = *missed_reuses > 0`, then recomputes FIP. The oracle must see the corrected contract, not the optimistic pre-second-pass version.

**Reference implementations:**
- **Lean4** `src/Lean/Compiler/IR/Checker.lean`: verifies that the post-optimization IR satisfies the pre-optimization function contract — conceptually identical to this oracle.
- **Swift** `lib/SIL/Verifier/SILVerifier.cpp`: `verifyOwnership()` checks that SIL ownership annotations match actual ownership IR.

**Depends on:** Section 03 (snapshot infrastructure provides artifact capture patterns used for oracle output), Section 04 (lattice properties must be pinned before building an oracle that depends on lattice correctness).

---

## 05.1 Contract Re-Derivation from Realized IR

**File(s):** `compiler/ori_arc/src/aims/verify/oracle.rs` (new), `compiler/ori_arc/src/aims/verify/mod.rs`

Walk the post-pipeline ARC IR for a function and derive the actual `ParamContract` for each parameter by observing the RC instructions emitted for parameter variables.

- [ ] Create `compiler/ori_arc/src/aims/verify/oracle.rs`. Add `pub mod oracle;` to `compiler/ori_arc/src/aims/verify/mod.rs`.

- [ ] Define the oracle's re-derived contract type:
  ```rust
  //! Contract coherence oracle.
  //!
  //! Walks realized ARC IR and re-derives a [`MemoryContract`] from the
  //! actual RC operations emitted. Compares against the inferred contract
  //! to verify coherence.

  use crate::aims::contract::{MemoryContract, ParamContract};
  use crate::aims::lattice::{AccessClass, Cardinality, Consumption};
  use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

  /// A contract re-derived from walking realized ARC IR.
  ///
  /// Each field is derived from OBSERVING what the pipeline actually emitted,
  /// not from the analysis's inferred state.
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct RealizedParamContract {
      /// Derived access: Owned if the param has any RcInc/RcDec, Borrowed otherwise.
      pub access: AccessClass,
      /// Derived consumption: based on RC operation pattern.
      /// - No RC ops → Linear (consumed exactly once)
      /// - RcInc only → Unrestricted (value is shared/copied)
      /// - RcDec only → Affine (may be dropped)
      /// - Both → Unrestricted
      pub consumption: Consumption,
      /// Derived cardinality: based on forward use count of the param variable.
      pub cardinality: Cardinality,
  }
  ```

- [ ] Implement `derive_param_contracts(func: &ArcFunction) -> Vec<RealizedParamContract>`:
  ```rust
  pub fn derive_param_contracts(func: &ArcFunction) -> Vec<RealizedParamContract> {
      let num_params = func.params.len();
      let mut rc_incs: Vec<u32> = vec![0; num_params];
      let mut rc_decs: Vec<u32> = vec![0; num_params];
      let mut use_counts: Vec<u32> = vec![0; num_params];

      // Walk all blocks, all instructions
      for block in &func.blocks {
          for instr in &block.instrs {
              match instr {
                  ArcInstr::RcInc { var } => {
                      if let Some(idx) = param_index(func, *var) {
                          rc_incs[idx] += 1;
                      }
                  }
                  ArcInstr::RcDec { var } => {
                      if let Some(idx) = param_index(func, *var) {
                          rc_decs[idx] += 1;
                      }
                  }
                  _ => {
                      // Count uses of param variables in other instructions
                      for used_var in instr.used_vars() {
                          if let Some(idx) = param_index(func, used_var) {
                              use_counts[idx] += 1;
                          }
                      }
                  }
              }
          }
      }

      (0..num_params)
          .map(|i| derive_single_param(rc_incs[i], rc_decs[i], use_counts[i]))
          .collect()
  }
  ```

- [ ] Implement `param_index()` helper that maps an `ArcVarId` to a parameter index (if the var is a parameter). Parameters are the first N variables in the function — check `func.params` to find the mapping.

- [ ] Implement `derive_single_param()` that infers `RealizedParamContract` from RC operation counts and use counts.

- [ ] Add tests in `compiler/ori_arc/src/aims/verify/oracle/tests.rs`:
  - `test_derive_param_linear_when_no_rc_ops`
  - `test_derive_param_unrestricted_when_rc_inc_present`
  - `test_derive_param_affine_when_only_rc_dec`
  - `test_derive_param_borrowed_when_no_ownership_ops`

- [ ] **Subsection close-out (05.1)** — MANDATORY before starting 05.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging journey for 05.1 specifically: which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where test failures gave unhelpful messages. Implement every accepted improvement NOW and commit each via SEPARATE `/commit-push` using a valid conventional-commit type.

---

## 05.2 May-Deallocate Second-Pass Accounting

**File(s):** `compiler/ori_arc/src/aims/verify/oracle.rs`

The oracle must account for the second pass in `batch.rs` that corrects `may_deallocate` and recomputes FIP. The pipeline flow is:

1. Per-function pipeline (steps 3-12) runs with optimistic `may_deallocate=false`
2. Second pass (`run_second_pass` in `batch.rs:98`) counts missed reuses
3. `contract.effects.may_deallocate = *missed_reuses > 0` (line 156)
4. FIP recomputed via `recompute_fip_for_may_deallocate()` (line 157)

The oracle's contract re-derivation must observe this corrected state.

- [ ] Add `Reuse` instruction counting to the oracle walk. A `Reuse` instruction that fails at runtime (slow path: Dec + fresh alloc) is a "missed reuse" — but at IR level, the presence of `Reuse` instructions means the pipeline attempted reuse. The oracle should count:
  - Number of `ArcInstr::Reuse` instructions → attempted reuses
  - The pipeline's `missed_reuse_counts` (from `run_aims_pipeline` return) → actual misses
  The `may_deallocate` field should be consistent: if missed reuses > 0, the corrected contract has `may_deallocate = true`.

- [ ] Derive the oracle's `EffectSummary` from the realized IR:
  ```rust
  pub struct RealizedEffects {
      /// Whether the function body contains Construct instructions
      /// that allocate heap memory (non-scalar types).
      pub may_allocate: bool,
      /// Whether missed reuses were detected (from second pass).
      pub may_deallocate: bool,
  }
  ```

- [ ] The oracle comparison for `may_deallocate` must use the POST-second-pass contract. Ensure the oracle runs at the right pipeline point — after `run_second_pass()` in `batch.rs`, not before.

- [ ] Add tests:
  - `test_oracle_may_deallocate_false_when_all_reuses_succeed`
  - `test_oracle_may_deallocate_true_when_missed_reuses_present`
  - `test_oracle_fip_downgrade_after_may_deallocate_correction`

- [ ] **TPR checkpoint** — `/tpr-review` covering 05.1–05.2 implementation work

- [ ] **Subsection close-out (05.2)** — MANDATORY before starting 05.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 05.1's close-out, scoped to 05.2's debugging journey.

---

## 05.3 Oracle Comparison and Diagnostics

**File(s):** `compiler/ori_arc/src/aims/verify/oracle.rs`

Compare the oracle's re-derived contract against the inferred `MemoryContract` and produce clear, actionable diagnostics on mismatch.

- [ ] Define mismatch types:
  ```rust
  /// A coherence mismatch between inferred and realized contracts.
  #[derive(Clone, Debug)]
  pub enum CoherenceMismatch {
      /// Parameter access class differs.
      ParamAccess {
          param_index: usize,
          param_name: String,
          inferred: AccessClass,
          realized: AccessClass,
      },
      /// Parameter consumption mode differs.
      ParamConsumption {
          param_index: usize,
          param_name: String,
          inferred: Consumption,
          realized: Consumption,
      },
      /// Parameter cardinality differs.
      ParamCardinality {
          param_index: usize,
          param_name: String,
          inferred: Cardinality,
          realized: Cardinality,
      },
      /// Effect summary disagrees (may_deallocate, may_allocate).
      EffectMismatch {
          field: &'static str,
          inferred: bool,
          realized: bool,
      },
      /// FIP certification status disagrees.
      FipMismatch {
          inferred: String,  // Debug representation
          realized: String,
      },
  }
  ```

- [ ] Implement `verify_coherence(func: &ArcFunction, inferred: &MemoryContract, missed_reuses: u32) -> Vec<CoherenceMismatch>`:
  ```rust
  pub fn verify_coherence(
      func: &ArcFunction,
      inferred: &MemoryContract,
      missed_reuses: u32,
  ) -> Vec<CoherenceMismatch> {
      let realized_params = derive_param_contracts(func);
      let mut mismatches = Vec::new();

      for (i, (inferred_p, realized_p)) in
          inferred.params.iter().zip(realized_params.iter()).enumerate()
      {
          // Compare access class
          if !access_compatible(inferred_p.access, realized_p.access) {
              mismatches.push(CoherenceMismatch::ParamAccess { /* ... */ });
          }
          // Compare consumption (with tolerance for conservative inference)
          if !consumption_compatible(inferred_p.consumption, realized_p.consumption) {
              mismatches.push(CoherenceMismatch::ParamConsumption { /* ... */ });
          }
          // Compare cardinality
          if !cardinality_compatible(inferred_p.cardinality, realized_p.cardinality) {
              mismatches.push(CoherenceMismatch::ParamCardinality { /* ... */ });
          }
      }

      // Compare effects (post-second-pass)
      let realized_may_deallocate = missed_reuses > 0;
      if inferred.effects.may_deallocate != realized_may_deallocate {
          mismatches.push(CoherenceMismatch::EffectMismatch { /* ... */ });
      }

      mismatches
  }
  ```

- [ ] Define compatibility predicates. The oracle comparison must tolerate conservative inference — if the analysis inferred `Unrestricted` (most conservative) but the realized IR only needed `Linear`, that's not a mismatch (the inference was safe). The mismatch direction matters:
  - **Unsafe mismatch (BUG):** inferred `Linear` but realized IR has `RcInc` → analysis too optimistic
  - **Conservative mismatch (OK):** inferred `Unrestricted` but realized IR is `Linear` → analysis conservative
  The oracle should flag only unsafe mismatches as errors. Conservative mismatches can be reported at `info` level for optimization diagnostics.

- [ ] Add tests:
  - `test_oracle_detects_param_access_mismatch`
  - `test_oracle_accepts_conservative_inference`
  - `test_oracle_rejects_unsafe_optimistic_inference`
  - `test_oracle_diagnostic_message_is_actionable`

- [ ] **Subsection close-out (05.3)** — MANDATORY before starting 05.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 05.4 Integration into AIMS Pipeline

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline/postprocess.rs`

Wire the oracle into the AIMS pipeline so it runs after the second pass (when contracts are finalized) under `ORI_VERIFY_ARC=1`. Mismatches become blocking errors per Section 01's verifier gate semantics.

- [ ] Add oracle invocation in `run_aims_pipeline_all()` (in `batch.rs`), after `run_second_pass()`:
  ```rust
  // After run_second_pass() — contracts are now finalized:
  if verify_arc {
      let _span = tracing::info_span!("contract_coherence_oracle").entered();
      for func in functions.iter() {
          let contract = &contracts[&func.name];
          let missed = missed_reuse_counts.get(&func.name).copied().unwrap_or(0);
          let mismatches = oracle::verify_coherence(func, contract, missed);
          if !mismatches.is_empty() {
              let unsafe_mismatches: Vec<_> = mismatches
                  .iter()
                  .filter(|m| m.is_unsafe())
                  .collect();
              if !unsafe_mismatches.is_empty() {
                  // Blocking error under verification mode
                  problems.push(ArcProblem::ContractCoherenceViolation {
                      function: func.name.clone(),
                      mismatches: unsafe_mismatches,
                  });
              }
          }
      }
  }
  ```

- [ ] Add `ContractCoherenceViolation` variant to `ArcProblem` (or the appropriate error type in `compiler/ori_arc/src/lower/mod.rs`).

- [ ] Verify that the oracle passes for all existing tests. Run `ORI_VERIFY_ARC=1 timeout 150 ./test-all.sh` and confirm zero oracle-triggered failures. Any failures are pre-existing contract coherence bugs — file each via `/add-bug` and fix before proceeding.

- [ ] Add a regression test that deliberately introduces a contract mismatch (e.g., by hardcoding a param contract field to the wrong value) and verifies the oracle catches it:
  ```rust
  #[test]
  fn oracle_catches_deliberately_wrong_contract() {
      // Set up a function where the contract says Linear but the IR has RcInc
      // Verify oracle returns a CoherenceMismatch::ParamConsumption
  }
  ```

- [ ] **Bug-tracker routing:** Contract coherence violations discovered during implementation should be filed under `plans/bug-tracker/section-04-*.md` (ARC/LLVM subsystem).

- [ ] **Subsection close-out (05.4)** — MANDATORY before starting 05.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 05.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- all items in this block are marked resolved
- `third_party_review.status` becomes `resolved` or `none`
-->

- None.

---

## 05.N Completion Checklist

- [ ] `oracle.rs` exists in `compiler/ori_arc/src/aims/verify/` with oracle implementation
- [ ] `derive_param_contracts()` correctly walks post-pipeline IR to derive per-param RC behavior
- [ ] `may_deallocate` second-pass correction accounted for (oracle runs after `run_second_pass()`)
- [ ] Compatibility predicates distinguish unsafe (bug) from conservative (OK) mismatches
- [ ] `verify_coherence()` produces clear, actionable diagnostic messages
- [ ] Oracle wired into `batch.rs` after second pass, gated by `verify_arc`
- [ ] Unsafe mismatches are blocking errors under `ORI_VERIFY_ARC=1`
- [ ] `ArcProblem::ContractCoherenceViolation` variant added
- [ ] All existing test programs pass oracle: `ORI_VERIFY_ARC=1 timeout 150 ./test-all.sh` green
- [ ] Deliberately introduced mismatch caught by regression test
- [ ] No regressions: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 05` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference updated
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** `ORI_VERIFY_ARC=1 timeout 150 ./test-all.sh` passes with the contract coherence oracle active. The oracle walks post-pipeline ARC IR, re-derives `ParamContract` from actual RC instructions, compares against inferred `MemoryContract`, and reports zero unsafe mismatches for all test programs. Deliberately introduced mismatches are caught as blocking errors. The oracle accounts for the `may_deallocate` second-pass correction in `batch.rs`.
