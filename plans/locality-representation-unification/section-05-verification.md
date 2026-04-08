---
section: "05"
title: "Verification & Documentation"
status: not-started
reviewed: false
goal: "Verify the unified Locality representation works end-to-end across all 5 prior sections, run the plan annotation cleanup scan, update the lattice/mod.rs module documentation with the unified description and prior art citations, and gate completion behind /tpr-review, /impl-hygiene-review, and /improve-tooling retrospective"
success_criteria:
  - "Test matrix covers all 5 Locality variants × all 3 cross-dimension canonicalize rules (15 cells, plus the soundness pin from Section 02.8)"
  - "Cross-crate behavioral test passes — full pipeline EscapeInfo writes Locality, ReprPlan::escapes() reads it correctly"
  - "Plan annotation cleanup scan via plan-annotations.sh returns 0 annotations for this plan's touched files"
  - "compiler/ori_arc/src/aims/lattice/mod.rs module doc describes the unified representation and cites Go + Lean 4 prior art (the deeper explanation lives in dimensions.rs from Section 02.2)"
  - "./test-all.sh green; ./clippy-all.sh green; cargo test -p ori_arc green; cargo test -p ori_repr green"
  - "/tpr-review passed"
  - "/impl-hygiene-review passed (after TPR clean)"
  - "/improve-tooling retrospective completed"
inspired_by:
  - "Sections 00–04 of this plan (the entire system being verified)"
  - ".claude/skills/impl-hygiene-review/plan-annotations.sh (plan annotation scanner)"
  - "diagnostics/aims-compare.sh (behavioral + RC comparison harness)"
depends_on: ["04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Test matrix verification (5 variants × 3 rules)"
    status: not-started
  - id: "05.2"
    title: "Cross-crate behavioral test"
    status: not-started
  - id: "05.3"
    title: "Plan annotation cleanup scan"
    status: not-started
  - id: "05.4"
    title: "Update lattice/mod.rs module documentation"
    status: not-started
  - id: "05.5"
    title: "Final test suite + clippy gate"
    status: not-started
  - id: "05.6"
    title: "/tpr-review (full plan)"
    status: not-started
  - id: "05.7"
    title: "/impl-hygiene-review (after TPR clean)"
    status: not-started
  - id: "05.8"
    title: "/improve-tooling retrospective"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
# Verification section — TPR runs at 05.6, hygiene at 05.7, tooling retrospective at 05.8.
# These are gates, not just checklist items; each must pass before the next runs.
---

# Section 05: Verification & Documentation

**Status:** Not Started
**Goal:** Verify the unified `Locality` representation works end-to-end across all 5 prior sections. This is the **gating section** — every prior section's success criteria must roll up into this section's checks before the plan can be marked complete. Section 05 also updates the `lattice/mod.rs` module documentation with the unified representation description, runs the plan annotation cleanup scan, and gates final completion behind `/tpr-review`, `/impl-hygiene-review`, and the `/improve-tooling` retrospective.

**Success Criteria:**

- [ ] Test matrix covers all 5 `Locality` variants × all 3 cross-dimension canonicalize rules (15 cells, plus the soundness pin from Section 02.8). Verified by running `cargo test -p ori_arc -- --test-threads=1 lattice::tests::canonicalize` and observing the expected number of test invocations
- [ ] Cross-crate behavioral test passes — `EscapeInfo` writes `Locality`, `ReprPlan::escapes()` reads it correctly. The round-trip test from Section 03.4 covers the storage layer; this section adds an integration-level test that runs both ends through the actual pipeline.
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` returns 0 annotations across all this plan's touched source files
- [ ] `compiler/ori_arc/src/aims/lattice/mod.rs` module doc updated to describe the unified representation. Brief — the deeper Lean 4 inversion explanation already lives in `dimensions.rs` per Section 02.2
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p ori_arc` and `cargo test -p ori_repr` both green
- [ ] `/tpr-review` passed (final, full-plan)
- [ ] `/impl-hygiene-review` passed (must run AFTER `/tpr-review` is clean)
- [ ] `/improve-tooling` retrospective completed (mandatory at every section close, even if no gaps)
- [ ] Connects upward to mission criteria: "test-all.sh green", "clippy-all.sh green", "/tpr-review + /impl-hygiene-review + /improve-tooling retrospective complete"

**Context:** Sections 00–04 each have their own completion criteria and test gates. Section 05 is the **integration verification** — it confirms that the whole plan composes correctly. Specifically:

1. Section 02 added the variant and the per-rule firing tests for `ArgEscaping`. Section 05 verifies that the **5×3 = 15 test matrix** is actually exercised by the test suite (not just defined).
2. Section 03 added the `EscapeInfo` storage and the cross-crate behavioral test. Section 05 verifies that the **end-to-end flow** from `EscapeInfo::join_escape_scope` through `ReprPlan::escapes()` works in a more realistic scenario than the unit tests.
3. Section 04 updated 5 plan-text targets in `repr-opt/`. Section 05 runs `/review-plan` (or equivalent) to verify the cross-references resolve.
4. All 5 prior sections may have left **plan annotations** (scaffolding code comments referencing this plan's section numbers — see Section 02.3 where the `canonicalize.rs` rule comment intentionally references `plans/locality-representation-unification/section-01.md`). Section 05 cleans them up per `impl-hygiene.md` §Comments: *"Plan annotations are temporary scaffolding... They MUST be removed when the plan completes."*
5. The `lattice/mod.rs` module doc was untouched by Section 02 (which only modified `dimensions.rs` for the new variant + Lean 4 comment). Section 05 updates it now that the unified representation is in place across all sibling files.
6. The plan completes only after `/tpr-review`, `/impl-hygiene-review`, and `/improve-tooling` retrospective all pass. These are sequential gates: TPR first (catches code review issues), then hygiene (catches structural issues that TPR missed), then improve-tooling (the mandatory retrospective even if no gaps were discovered).

**Reference implementations:**

- **`compiler/ori_arc/src/aims/lattice/tests.rs`** (post-Section-02.8): the test matrix this section verifies. Section 02.8 added the new variant cases; Section 05 verifies they are wired into the test runner.
- **`.claude/skills/impl-hygiene-review/plan-annotations.sh`**: the scanner that detects stale plan annotations. Section 00.5/00.6 used it to find stale `Section 09.x` references; Section 05.3 uses it to verify this plan's own annotations are cleaned up.
- **`diagnostics/aims-compare.sh`**: optional behavioral + RC comparison harness. Useful as a sanity check that this plan's changes don't affect codegen output (they shouldn't, since `repr-opt §08` hasn't shipped yet — `EscapeInfo` is empty, so `escapes()` still returns `true` for everything, matching the pre-plan behavior).
- **`compiler/ori_arc/src/aims/lattice/mod.rs`** (post-Section-00 split): the dispatch hub whose module doc this section updates.

**Depends on:** Section 04 (the cross-plan coordination must complete first; Section 05's `/review-plan` gate depends on `repr-opt` being internally consistent).

---

## 05.1 Test matrix verification (5 variants × 3 rules)

**File(s):** `compiler/ori_arc/src/aims/lattice/tests.rs` (verifying tests Section 02.8 added; this subsection runs them and confirms coverage)

**Context:** Section 02.8 added per-rule firing tests for `ArgEscaping` (Rule 8 widens borrows, Rule 6 does NOT fire — soundness pin, Rule 4 does NOT promote). Section 05.1 verifies that the **full 5 × 3 = 15-cell matrix** is exercised by the test suite. The cells are:

| Variant ↓ Rule → | Rule 4 (BlockLocal+Owned+Once → Unique) | Rule 6 (HeapEscaping+Unique → MaybeShared) | Rule 8 (Borrowed → ≤FunctionLocal) |
|---|---|---|---|
| `BlockLocal` | **fires** (rule precondition) | does not fire | **fires** as upper bound (Borrowed locality already ≤ FunctionLocal) |
| `FunctionLocal` | does not fire | does not fire | does not widen (already at the boundary) |
| `ArgEscaping` (NEW) | does not fire | **does NOT fire (soundness pin)** | widens to FunctionLocal for Borrowed |
| `HeapEscaping` | does not fire | **fires** (rule precondition) | widens to FunctionLocal for Borrowed |
| `Unknown` | does not fire | does not fire | widens to FunctionLocal for Borrowed |

That's 15 distinct cells. Section 02.8's tests cover the `ArgEscaping` row (3 cells), the boundary tests in the existing test suite cover the other rows. Section 05.1 enumerates the matrix and confirms each cell has a test.

- [ ] Re-read `compiler/ori_arc/src/aims/lattice/tests.rs` and find every test function named `rule_4_*`, `rule_6_*`, `rule_8_*`. Build a coverage table of variant × rule → test function name in the section's completion notes.

- [ ] For any cell missing a test, ADD a small test that exercises it. The cells are mostly covered by Section 02.8's ArgEscaping tests + the pre-existing test suite's tests for the other four variants, but the verification here is to catch any gap.

- [ ] **Add a self-verifying matrix coverage test** that iterates over all 5 `Locality` variants and asserts `AimsState::canonicalize()` is idempotent for every variant. This test has REAL assertions (idempotence of canonicalize is load-bearing) and doubles as a proof that every variant is exercised by the test runner. Per `.claude/rules/tests.md` §Self-verifying matrix completeness — a matrix loop that silently skips cells is worse than no matrix at all.
  ```rust
  /// Matrix coverage with REAL assertions: canonicalize is idempotent for
  /// every Locality variant across every other AimsState dimension at BOTTOM.
  /// Also asserts the matrix visited every variant (self-verifying count).
  ///
  /// This replaces the earlier documentation-only "matrix_coverage_documentation"
  /// placeholder that had zero assertions. A test without assertions is not a
  /// test — it's a comment-as-test. See .claude/rules/tests.md §Test Hygiene
  /// rule 1 ("no orphan tests").
  #[test]
  fn canonicalize_idempotent_for_every_locality_variant() {
      use Locality::*;
      let mut visited = 0;
      for locality in all_locality() {
          let mut state = AimsState {
              locality,
              ..AimsState::FRESH
          };
          state.canonicalize();
          let first = state;
          state.canonicalize();
          assert_eq!(
              state, first,
              "canonicalize() must be idempotent for Locality::{:?}", locality
          );
          visited += 1;
      }
      assert_eq!(
          visited, 5,
          "matrix must visit all 5 Locality variants (self-verifying completeness)"
      );
      // Silence unused-import warning if we're in a clippy-strict mode.
      let _ = (BlockLocal, FunctionLocal, ArgEscaping, HeapEscaping, Unknown);
  }
  ```
  This test: (1) exercises every variant via `all_locality()` (the helper Section 02.7 extended to 5 variants), (2) asserts canonicalize idempotence (a real load-bearing invariant from `.claude/rules/tests.md` §Negative Testing rule 6), and (3) asserts the iteration count matches the expected variant count (proves no cells were silently skipped).

- [ ] Run `cargo test -p ori_arc -- lattice::tests` and verify all matrix tests pass. The expected count includes all the new ArgEscaping tests from Section 02.8 plus the idempotence matrix test above.

- [ ] Document the matrix coverage verification in the section's completion notes: record the coverage table (variant × rule → test function name) built above, the canonicalize-idempotence matrix test's visit count, and any gaps found during the coverage audit.

---

## 05.2 Cross-crate behavioral test

**File(s):** Either `compiler/ori_repr/src/escape/tests.rs` (extending Section 03.4's tests) OR `compiler/ori_repr/src/tests.rs` (the integration-level test file)

**Context:** Section 03.4 added a **unit-level** round-trip test inside `escape/tests.rs`: it constructs an `EscapeInfo`, populates it with each `Locality` variant, and queries it back. Section 05.2 adds an **integration-level** test that runs the same flow through `ReprPlan::escapes()`, simulating how `repr-opt §08` will eventually use the API.

The test does NOT need a real escape analysis — it just needs to:
1. Construct a `ReprPlan`
2. Construct an `EscapeInfo` for a fake function name with a few variables in different localities
3. Call `plan.set_escape_info(func, info)`
4. Call `plan.escapes(func, var)` for each variable and verify the expected boolean

This test catches any boundary issue between the storage layer (Section 03) and the query layer (also Section 03, but via `ReprPlan::escapes()` rather than directly via `EscapeInfo::escapes()`).

- [ ] Add an integration test:
  ```rust
  /// Integration test: full pipeline from EscapeInfo population to
  /// ReprPlan::escapes() query. Simulates how repr-opt §08 will eventually
  /// use the API once it ships.
  #[test]
  fn full_pipeline_escape_info_to_repr_plan_escapes_query() {
      use ori_arc::aims::lattice::Locality;
      use ori_arc::ir::ArcVarId;
      use ori_ir::Name;

      // Construct a ReprPlan with a populated EscapeInfo for one function.
      // Name::new takes (shard: u32, local: u32) — see compiler/ori_ir/src/name/mod.rs:33
      // ArcVarId::new(raw: u32) — see compiler/ori_arc/src/ir/mod.rs:74
      //
      let mut plan = ReprPlan::default();
      let func = Name::new(0, 1);
      let mut info = EscapeInfo::default();

      let var_block_local = ArcVarId::new(0);
      let var_function_local = ArcVarId::new(1);
      let var_arg_escaping = ArcVarId::new(2);
      let var_heap_escaping = ArcVarId::new(3);
      let var_unknown = ArcVarId::new(4);
      let var_unset = ArcVarId::new(5);

      info.join_escape_scope(var_block_local, Locality::BlockLocal);
      info.join_escape_scope(var_function_local, Locality::FunctionLocal);
      info.join_escape_scope(var_arg_escaping, Locality::ArgEscaping);
      info.join_escape_scope(var_heap_escaping, Locality::HeapEscaping);
      info.join_escape_scope(var_unknown, Locality::Unknown);
      // var_unset is intentionally not added — should fall back to "escapes"

      plan.set_escape_info(func, info);

      // Local variants are NOT escaping
      assert!(!plan.escapes(func, var_block_local));
      assert!(!plan.escapes(func, var_function_local));

      // ArgEscaping IS escaping (locality > FunctionLocal)
      assert!(plan.escapes(func, var_arg_escaping));

      // HeapEscaping and Unknown are also escaping
      assert!(plan.escapes(func, var_heap_escaping));
      assert!(plan.escapes(func, var_unknown));

      // Unset variables fall back to "escapes" (the conservative default)
      assert!(plan.escapes(func, var_unset));

      // A different function (not analyzed) should also return "escapes"
      let other_func = Name::new(0, 2);
      assert!(plan.escapes(other_func, var_block_local));
  }
  ```

- [ ] Run `cargo test -p ori_repr -- full_pipeline` and verify the test passes.

- [ ] If `diagnostics/aims-compare.sh` is feasible to run on a small synthetic test program: run it to verify behavioral parity (no codegen change). The expected outcome is "no change" because `EscapeInfo` is unpopulated in normal compilation (until `repr-opt §08` ships) and the conservative default still returns `true` for everything.

---

## 05.3 Plan annotation cleanup scan

**File(s):** Various — wherever this plan's sections added scaffolding plan annotations to source code

**Context:** Per `impl-hygiene.md` §Comments:
> Plan annotations are temporary scaffolding: Annotations referencing plans (`TPR-04-005`, `CROSS-04-014`, `§04.3 Phase A`, `BUG-04-07`, `Section 04.2`, `section-04-name`) are allowed during active plan execution — they aid navigation. They MUST be removed when the plan completes. Every plan MUST include a final cleanup section to strip all its code annotations. Stale annotations from completed plans are hygiene violations (DRIFT category). Run `.claude/skills/impl-hygiene-review/plan-annotations.sh` to scan. Only spec references (`Spec: Clause N.M`) are permanent.

This plan added at least one explicit plan annotation in source code (Section 02.3 added a comment in `canonicalize.rs` referencing `plans/locality-representation-unification/section-01.md` near Rule 6). There may be others added during the implementation of Sections 02–04 that the implementer decided to annotate for navigation. Section 05.3 finds and removes them all.

- [ ] Run `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` to scan the entire compiler tree for annotations referencing this plan. The script returns a list of `(file, line, annotation)` triples.

**Known annotations introduced by this plan (must be cleaned up by 05.3):**

| File | Subsection that added it | Cleanup action |
|---|---|---|
| `compiler/ori_arc/src/aims/lattice/canonicalize.rs` (Rule 6 comment) | Section 02.3 | Rewrite to point at `dimensions.rs` Locality doc comment instead of `plans/locality-representation-unification/section-01.md §01.2` |

This list is exhaustive at plan-creation time. If Section 02 or any later section adds additional plan annotations during execution, the implementer must add them to this table before running 05.3.

- [ ] For each annotation:
  - **If the annotation references a plan section** (e.g., `// See plans/locality-representation-unification/section-01.md`): rewrite it to preserve the rationale without the navigation. For example, the `canonicalize.rs` Rule 6 comment from Section 02.3 currently says:
    ```rust
    // Rule 6: ... See plans/locality-representation-unification/section-01.md §01.2 for ...
    ```
    Rewrite to:
    ```rust
    // Rule 6: ... See dimensions.rs Locality doc comment for the prior art (Go leakCallee, Lean 4 borrow) ...
    ```
    The reference now points to a permanent location (the source comment) instead of the plan corpus.
  - **If the annotation is purely navigational** (e.g., `// TPR-LOC-005`): delete it entirely. Navigation aids that no longer have a target are pure noise.
  - **If the annotation is a `Spec:` reference** (e.g., `// Spec: Clause 5.3 — escape semantics`): keep it. Spec references are permanent.

- [ ] After the rewrites, run `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` again and verify it returns **0 annotations**.

- [ ] Run `cargo doc -p ori_arc 2>&1 | grep warning` and `cargo doc -p ori_repr 2>&1 | grep warning` to verify no doc-link warnings were introduced by the rewrites (broken `[link]` references are easy to introduce when rewriting comments).

- [ ] Run `cargo test -p ori_arc -p ori_repr` to verify nothing broke (the rewrites are comment-only, so behavior is unchanged — verify regardless).

---

## 05.4 Update lattice/mod.rs module documentation

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs` (post-Section-00 dispatch hub, currently ≤80 lines per Section 00 success criteria)

**Context:** Section 00 turned `lattice/mod.rs` into a dispatch hub with module doc, mod declarations, and re-exports. The module doc Section 00 wrote was a brief overview — Section 05.4 updates it to describe the **unified representation** post-Section-02 and add the prior art citations from Section 01.5.

The Section 00 module doc says:
```rust
//! Unified ownership lattice for ARC analysis.
//!
//! [`AimsState`] is a product of seven dimensions, each a small finite lattice.
//! Join is componentwise. Transfer functions (in `transfer.rs`) update one or
//! more dimensions simultaneously.
//!
//! Module structure:
//! - [`dimensions`]    — per-dimension enums (AccessClass, Consumption, ...)
//! - [`state`]         — `AimsState` product type, constants, join, predicates
//! - [`canonicalize`]  — feasibility invariant enforcement (Rules 4/6/8 etc.)
//! - [`borrow_source`] — `BorrowSource` side table for borrow provenance
//! - [`size_class`]    — `SizeClass` newtype for reuse compatibility
//!
//! References: Perceus (PLDI 2021), GHC demand analysis (POPL 2014),
//! Lean 4 borrow inference (IFL 2019), Linearity ≠ Uniqueness (ESOP 2022),
//! `OxCaml` (ICFP 2024).
```

Section 05.4 expands this to mention the unified `Locality` chain post-Section-02, the brief prior art citations from Pass 4, and a pointer to `dimensions.rs` for the deeper Lean 4 inversion explanation.

- [ ] Update the module doc to:
  ```rust
  //! Unified ownership lattice for ARC analysis.
  //!
  //! [`AimsState`] is a product of seven dimensions, each a small finite lattice.
  //! Join is componentwise. Transfer functions (in `transfer.rs`) update one or
  //! more dimensions simultaneously.
  //!
  //! ## Module structure
  //!
  //! - [`dimensions`]    — per-dimension enums (AccessClass, Consumption,
  //!   Cardinality, Uniqueness, Locality, ShapeClass, EffectClass)
  //! - [`state`]         — `AimsState` product type, constants, join, predicates
  //! - [`canonicalize`]  — feasibility invariant enforcement (Rules 4/6/8 etc.)
  //! - [`borrow_source`] — `BorrowSource` side table for borrow provenance
  //! - [`size_class`]    — `SizeClass` newtype for reuse compatibility
  //!
  //! ## Locality classification (SSOT)
  //!
  //! The [`dimensions::Locality`] enum is the **single source of truth** for
  //! escape-scope classification across the compiler. Its 5 variants form an
  //! ordered chain:
  //!
  //! ```text
  //! BlockLocal < FunctionLocal < ArgEscaping < HeapEscaping < Unknown
  //! ```
  //!
  //! - **`BlockLocal`**: does not escape its defining basic block
  //! - **`FunctionLocal`**: does not escape its defining function
  //! - **`ArgEscaping`**: flows to a callee via parameter, but the callee does
  //!   not retain a reference past the call return (matches Go's `leakCallee`
  //!   semantics in `cmd/compile/internal/escape/leaks.go`)
  //! - **`HeapEscaping`**: escapes to the heap, return value, or global
  //! - **`Unknown`**: conservative default
  //!
  //! `Locality` is consumed cross-crate by `ori_repr::escape::EscapeInfo`
  //! (per-function `FxHashMap<ArcVarId, Locality>`) and queried via
  //! `ReprPlan::escapes()`. Thread-locality is a **separate** concern routed
  //! through `RcStrategy::NonAtomic`, NOT a parallel `Locality` axis. Owner-
  //! rooted regions are a separate concern routed through
  //! [`borrow_source::BorrowSource`], NOT a parallel `Locality` axis.
  //!
  //! For the deeper prior art comparison with Lean 4's inverted `borrow=true`
  //! framing, see the `Locality` enum doc comment in [`dimensions`].
  //!
  //! ## Lattice properties
  //!
  //! Idempotent, commutative, associative join. Monotonic transfer. Finite
  //! height 16 (was 15 before the `ArgEscaping` variant was added). See
  //! tests for exhaustive verification.
  //!
  //! ## References
  //!
  //! - Perceus (PLDI 2021)
  //! - GHC demand analysis (POPL 2014)
  //! - Lean 4 borrow inference (IFL 2019)
  //! - Linearity ≠ Uniqueness (ESOP 2022)
  //! - `OxCaml` modes (ICFP 2024)
  //! - Go escape analysis (`cmd/compile/internal/escape/leaks.go` —
  //!   the `leakCallee` distinction that `ArgEscaping` mirrors)
  //! - Lean 4 borrow analysis (`Borrow.lean:58-60` — same distinction with
  //!   inverted widening direction)
  //! - Racordon 2019 thesis §6.2.2 (transient ownership at conditional joins)
  ```

- [ ] Verify the `lattice/mod.rs` file is still ≤80 lines after the doc update. If the new doc pushes it over, that's acceptable for a dispatch hub (the limit is "production code"; doc comments don't count toward bloat for a pure module index file). But if it's wildly over (e.g., 200+ lines), trim the prior art section to a one-line citation.

- [ ] Run `cargo doc -p ori_arc 2>&1 | grep warning` and verify no broken doc links. The `[`dimensions`]` and `[`borrow_source::BorrowSource`]` references must resolve.

- [ ] Verify the chain height claim (16) matches what `state.rs::CHAIN_HEIGHT` actually defines.

---

## 05.5 Final test suite + clippy gate

**File(s):** None — this subsection runs the test gates

**Context:** All prior sections have their own gates. Section 05.5 runs the **full** suite as a final integration check. Per the mission success criteria:

- `./test-all.sh` green
- `./clippy-all.sh` green

Per CLAUDE.md mandatory test timeouts: every test command MUST use `timeout 150` (max 2 minutes 30 seconds). Exceeding the timeout means a hanging test was introduced — kill it, find it, fix it.

- [ ] Run `timeout 150 ./test-all.sh` and verify exit status 0
- [ ] Run `timeout 150 ./clippy-all.sh` and verify exit status 0
- [ ] Run `cargo test -p ori_arc` and verify the full ori_arc test suite passes (with the new ArgEscaping cases)
- [ ] Run `cargo test -p ori_repr` and verify the full ori_repr test suite passes (with the new EscapeInfo tests)
- [ ] Run `cargo test -p ori_llvm` and verify no regression (the cross-crate import should not affect ori_llvm, but verify regardless)

- [ ] If any test fails, **DO NOT** mark the section complete. The failure is either:
  1. A regression introduced by this plan — fix it
  2. A flaky test — investigate the root cause (CLAUDE.md is unambiguous: flaky tests ARE bugs, not noise)
  3. A pre-existing failure unrelated to this plan — file `/add-bug` and proceed (but document the connection in completion notes)

- [ ] Document the test counts in the section's completion notes: "Section 05.5 verification: cargo test -p ori_arc passed N tests; cargo test -p ori_repr passed M tests; ./test-all.sh passed in T seconds with exit status 0."

---

## 05.6 /tpr-review (full plan)

**File(s):** None — this subsection invokes `/tpr-review` as a gate

**Context:** Per the schema's completion checklist template, `/tpr-review` is the **first** of the three final gates. It runs Codex over the full plan (all 6 sections) and the complete diff to find code review issues, design drift, or missed edge cases.

- [ ] Invoke `/tpr-review` with the plan name `locality-representation-unification` (or as the skill expects)
- [ ] Wait for the review to complete (5–35 minutes per CLAUDE.md). DO NOT timeout this command — it's a review task, not a test command. Per CLAUDE.md: *"REVIEW/AGENT TIMEOUTS: Review/analysis tasks legitimately take 5–35 minutes... NEVER use short timeouts on these."*
- [ ] Read the review output. For each finding:
  - **Critical / High**: fix immediately. Do NOT mark this section complete until all critical/high findings are resolved.
  - **Medium**: triage — fix if quick, file `/add-bug` if larger
  - **Low**: file `/add-bug` for follow-up, do not block this section
- [ ] After all findings are addressed, re-run `/tpr-review` and verify it returns clean (or that all findings are triaged with explicit resolution notes)

---

## 05.7 /impl-hygiene-review (after TPR clean)

**File(s):** None — this subsection invokes `/impl-hygiene-review` as a gate

**Context:** Per the schema, `/impl-hygiene-review` runs **after** `/tpr-review` is clean. The hygiene review catches structural issues (LEAK, DRIFT, BLOAT, etc. from `impl-hygiene.md`) that TPR may have missed because TPR focuses on code correctness while hygiene focuses on architectural shape.

- [ ] Verify `/tpr-review` is clean before invoking this subsection. Do NOT skip ahead.
- [ ] Invoke `/impl-hygiene-review` with the plan scope (Auto Mode autoscopes across the active work arc per CLAUDE.md)
- [ ] Wait for the review to complete. Same timeout rules as 05.6.
- [ ] Read the review output. The most likely findings for this plan are:
  - **LEAK** findings: should be 0 — Section 02.6 fixed the `may_escape` LEAK and no new ones were introduced
  - **DRIFT** findings: should be 0 — Section 00.5/00.6 cleaned up the stale Section 09.x annotations and Section 05.3 cleaned up this plan's own annotations
  - **BLOAT** findings: should be 0 — Section 00 split the over-limit files
  - **WASTE / EXPOSURE / NOTE**: triage as appropriate
- [ ] For each finding, fix or triage per the hygiene review skill's protocol
- [ ] Re-run `/impl-hygiene-review` and verify clean

---

## 05.8 /improve-tooling retrospective

**File(s):** None — this subsection invokes `/improve-tooling` retrospective mode

**Context:** Per CLAUDE.md: *"`/improve-tooling` retrospective completed — MANDATORY at every section close, even when nothing felt painful — that is exactly when blind spots accumulate."*

The retrospective for this plan should specifically reflect on:
1. **Section 00 (file splits)**: Was there a missing helper for "verify byte-equivalent move of code from a monolithic file to a sibling structure"? Did `cargo check` give clear-enough errors when imports needed updating, or did the implementer need a custom script?
2. **Section 02 (consumer migration)**: Was there a missing tool to "find all exhaustive matches on an enum across the codebase"? Did the migration recipe table from this section reduce ambiguity, or was it still error-prone?
3. **Section 03 (cross-crate boundary)**: Was there a missing helper for "find all callers of a public method across crate boundaries"? Did the size-assertion deletion (the `assert_eq!(size_of::<EscapeInfo>(), 0)` at `tests.rs:308`) catch the implementer by surprise?
4. **Section 04 (plan corpus coordination)**: Was there a missing tool to "find every cross-reference between two plan files" or "verify literal copy of text from one section to another"?
5. **Section 05 (verification)**: Were the test matrix coverage, plan annotation scan, and module doc update each catchable by existing diagnostics, or did the implementer need new tools?

- [ ] Invoke `/improve-tooling` in retrospective mode with the section reference `locality-representation-unification §05.8`
- [ ] Wait for the retrospective to complete
- [ ] For each suggested improvement:
  - **Implement NOW** (zero deferral per CLAUDE.md). Each improvement gets a SEPARATE `/commit-push` from this plan's implementation work.
  - Example commit message: `tools(diagnostics): add cross-crate-callers helper to scripts/find-callers.sh — surfaced by locality-representation-unification §05.8 retrospective`
- [ ] Verify each implemented improvement actually solves the original friction (by re-running the diagnostic on a test case where the friction would have appeared)
- [ ] If genuinely no gaps were found, document briefly: *"§05.8 retrospective: no tooling gaps — relied on existing wc -l, cargo check, plan-annotations.sh, and rg/grep for cross-reference verification."*

---

## 05.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers. -->

- None.

---

## 05.N Completion Checklist

- [ ] Test matrix verification (05.1) complete — 5 variants × 3 rules = 15 cells, all covered
- [ ] Self-verifying matrix test (`canonicalize_idempotent_for_every_locality_variant`) passes AND its visit count assertion equals 5 (proves every variant was exercised, not silently skipped)
- [ ] Soundness pin test from Section 02.8 (`rule_6_does_not_fire_on_arg_escaping_unique`) is passing
- [ ] Return-value widening test from Section 02.8 (`return_widening_promotes_arg_escaping_to_heap_escaping`) is passing — proves block.rs:155 correctly upgrades ArgEscaping → HeapEscaping for returns (soundness condition 4 producer-side guard)
- [ ] ParamContract::may_escape() derivation tests from Section 02.8 are passing — all 5 `Locality` variants exercised through the derived method
- [ ] Cross-crate behavioral test (05.2) passes — `full_pipeline_escape_info_to_repr_plan_escapes_query`
- [ ] Plan annotation cleanup (05.3) complete — `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` returns 0 annotations
- [ ] `lattice/mod.rs` module doc updated (05.4) with the unified Locality description, the SSOT statement, and the prior art citations
- [ ] `cargo doc -p ori_arc 2>&1 | grep warning` reports no new doc warnings
- [ ] Final test gate (05.5): `timeout 150 ./test-all.sh` exit 0
- [ ] Final clippy gate (05.5): `timeout 150 ./clippy-all.sh` exit 0
- [ ] `cargo test -p ori_arc` green
- [ ] `cargo test -p ori_repr` green
- [ ] `cargo test -p ori_llvm` green (no regression)
- [ ] `/tpr-review` (05.6) passed — Codex review found no critical or major issues, or all findings triaged
- [ ] `/impl-hygiene-review` (05.7) passed — must run AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` (05.8) retrospective completed — every accepted improvement implemented NOW (zero deferral) with separate `/commit-push`
- [ ] **Plan sync — final**:
  - [ ] This section's frontmatter `status` → `complete`, all subsection statuses → complete
  - [ ] `00-overview.md` Quick Reference table: all 6 sections show status `complete`
  - [ ] `00-overview.md` mission success criteria: all 17 checkboxes ticked
  - [ ] `index.md` all 6 section statuses → `Complete`
  - [ ] `index.md` plan-level frontmatter `status: complete` (the plan moves from active queue to completed plans)
  - [ ] Move the plan directory from `plans/locality-representation-unification/` to `plans/completed/locality-representation-unification/` via `git mv`
  - [ ] Update any other plans that referenced `locality-representation-unification` as a `depends_on` — change the path reference if needed
  - [ ] Update CLAUDE.md memory entries if any (e.g., the AIMS lattice memory may need to mention the new variant)

**Exit Criteria:** A reviewer running `git diff plans/repr-opt/` sees only the 5 cross-plan coordination edits from Section 04. Running `git diff compiler/ori_arc/` sees the new variant, the `may_escape` conversion, the test extension, and the file splits from Section 00. Running `git diff compiler/ori_repr/` sees the new `EscapeInfo` storage and the `escapes()` body replacement. `timeout 150 ./test-all.sh` returns exit 0 with the test count increased by approximately 80–100 from the new test cases. `bash .claude/skills/impl-hygiene-review/plan-annotations.sh` returns 0 references to this plan in source files. `/tpr-review`, `/impl-hygiene-review`, and `/improve-tooling` retrospective have all run and produced clean (or fully triaged) outputs. The plan is moved to `plans/completed/locality-representation-unification/` and `repr-opt §08`/`§09`/`§10` can begin executing against the unified `Locality` substrate.
