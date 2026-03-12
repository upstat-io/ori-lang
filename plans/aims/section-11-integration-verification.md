---
section: "11"
title: "Integration Verification"
status: not-started
goal: "Prove that all 7 dimensions work as one team through concrete programs, quantitative metrics, and regression guards"
depends_on: ["09", "10"]
sections:
  - id: "11.1"
    title: "Cross-Dimension Test Programs"
    status: not-started
  - id: "11.2"
    title: "Synergy Metrics"
    status: not-started
  - id: "11.3"
    title: "Regression Guards"
    status: not-started
  - id: "11.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 11: Integration Verification

**Status:** Not Started

**Goal:** Prove that dimensional fusion (Section 09) and unified realization
(Section 10) produce a system where all 7 dimensions genuinely work as one team,
not just share a struct. Quantify the benefit. Guard against regressions.

**Context:** Stage 1 verification (Section 08) proved AIMS correctness — same
behavior, fewer RC ops, no leaks. This section proves the *integration* works:
that cross-dimensional reasoning produces measurably better results than any
single dimension could achieve alone. Without this, "unified lattice" is
marketing; with it, it's engineering.

**Depends on:** Section 09 (fusion), Section 10 (unified realization).

**Error handling for verification failures:**

Section 11 is primarily a testing/measurement section. Its failure modes are:

1. **Cross-dimension test program doesn't compile:** The Ori test program has a
   syntax or type error. Fix the program, not the compiler (unless the compiler
   bug is genuine). Always verify programs compile under the legacy pipeline
   first (without `--features aims`).

2. **Synergy metric is below threshold (e.g., <20% multi-dim decisions):** This
   is NOT a code failure — it means the cross-dimensional rules aren't firing
   often enough on the test corpus. Response: (a) verify the rules are correctly
   implemented (per-rule unit tests from 11.3), (b) expand the test corpus with
   programs that exercise more cross-dim interactions, (c) if the metric is
   genuinely low on real-world code, adjust the threshold downward with
   justification. The threshold is an aspiration, not a binary pass/fail.

3. **Golden corpus RC regression after Stage 2:** This is a hard failure.
   Bisect using the dependency ladder (disable Shape first, then Effect, then
   Locality) to identify which dimension's activation caused the regression.
   The regression must be fixed before Stage 2 can exit.

---

## 11.0 Pre-Work: Verify Baseline Health

Before writing synergy tests, confirm the existing test suite is clean. These
items protect against regression confusion (new tests failing for unrelated reasons).

- [ ] **`cargo test --workspace --features aims` passes green** — no pre-existing
  failures in `emit_rc/tests.rs`, `emit_reuse/tests.rs`, `intraprocedural/tests.rs`,
  `lattice/tests.rs`, `interprocedural/tests.rs`.
- [ ] **`tests/aims/` existing programs all pass**: `cow_chain.ori`,
  `closure_capture.ori`, `nested_pattern_match.ori`, `mixed_ownership.ori` must
  compile and run correctly before adding `tests/aims/synergy/`.
- [ ] **09.0 cleanup complete** — `EffectClass` deduplication, unused params removed.
  The lattice and transfer modules must be in their final clean state before
  adding cross-dimension tests that inspect their internals.
- [ ] **`realize/tests.rs` location confirmed** — since `realize/` is a new module
  (`realize/mod.rs`), its tests live in `realize/tests.rs` per the sibling-file
  convention. Confirm the module declaration uses `#[cfg(test)] mod tests;` only.

---

## 11.1 Cross-Dimension Test Programs

**File(s):** `tests/aims/synergy/` (NEW directory)

For each significant cross-dimension interaction, write a test program where:
1. One dimension alone gives a conservative answer
2. Combined with another dimension, gives a precise answer
3. The precise answer produces measurably better code (fewer RC ops, static
   reuse where dynamic was required, eliminated runtime check)

Each program includes a comment documenting which dimensions interact and what
the expected improvement is.

**Ori syntax constraints for test programs:**
- No `return` keyword — last expression is block value
- List push returns a new list: `list = list.push(value: x)` (reassignment pattern)
- String concat uses `+`: `"prefix" + str(n) + "suffix"`
- Sum types declared as: `type T = VariantA(field: Type) | VariantB`
- Pattern matching: `match e { VariantA(f) -> expr, VariantB -> expr }`
- All test programs must compile without `--features aims`; the AIMS pipeline
  difference is tested at the Rust level (compile the `ArcFunction` and assert
  on RC counts), not by running the Ori binary twice.
- The Ori programs are input to the AIMS analysis; test assertions are in the
  Rust-level tests (11.1 Rust-Level Tests section below).
- Base new programs on existing tests: `tests/aims/nested_pattern_match.ori`
  (sum type + pattern match), `tests/aims/cow_chain.ori` (list mutations),
  `tests/aims/closure_capture.ori` (closure capture).

### Locality × Uniqueness

- [ ] **`block_local_unique.ori`**: Create a struct value in a block, push into a
  list created in the same block, consume the list before block end. Without
  locality: `MaybeShared` → Dynamic COW check on each push. With locality:
  `BlockLocal + Owned + Once → Unique` → StaticUnique on each push, no runtime check.
  Expected: N fewer runtime checks (where N = number of pushes), 0 fewer RC ops.
  Concrete program: create a list in a block, push 3 times using
  `list = list.push(value: x)`, return it. AIMS detects the list is BlockLocal
  (created in this block, never stored to a heap structure before being consumed),
  Owned (no other names reference it), and used at most Once in each push chain.

  Sketch (verify compiles before committing):
  ```ori
  @build_local () -> int = {
      let $nums = [1, 2, 3];
      nums = nums.push(value: 4);
      nums = nums.push(value: 5);
      nums.len()
  };
  ```

- [ ] **`function_local_linear_skip.ori`**: Create a list, pass it to a pure
  function that only reads it (borrowed), consume the result. Without locality:
  RcInc at function entry for each borrowed use, RcDec at each last use.
  With locality+effect: `FunctionLocal + Linear + may_share==false` → RC-skip
  → 2 fewer RC ops (the inc+dec pair at the call boundary is eliminated).
  Expected: 2 fewer RC ops per call site where the argument is provably local.

  "Pure function" in Ori means absence of effectful operations. The callee's
  `MemoryContract.effects.may_share==false` is the machine-readable signal.
  Example callee: `@sum_pure (items: [int]) -> int = items.fold(initial: 0, op: (acc, x) -> acc + x);` —
  reads items but doesn't store it anywhere.

### Effect × Uniqueness

- [ ] **`pure_callee_preserves.ori`**: Create a list, call a pure aggregation
  function (may_share==false), then push to the list after the call. Without
  effect: uniqueness degrades to `MaybeShared` after call (conservative — callee
  might have RcInc'd it). With effect: `may_share == false` → uniqueness
  preserved through call → StaticUnique COW on the subsequent push.
  Expected: 1 fewer runtime check.

  Sketch:
  ```ori
  @sum_pure (items: [int]) -> int = items.fold(initial: 0, op: (acc, x) -> acc + x);

  @test () -> int = {
      let $nums = [1, 2, 3];
      let $total = sum_pure(items: nums);
      // AIMS should know: nums is still unique after sum_pure (may_share==false)
      nums = nums.push(value: 4);  // should be StaticUnique COW
      total + nums.len()
  };
  ```

- [ ] **`effect_fip_natural.ori`**: Define a simple sum type (e.g., `IntOpt`
  wrapping an `int`), write a function that pattern-matches on it and returns a
  new variant. The function creates one allocation for each consumed allocation
  (net allocation = 0). Without effect analysis: needs a separate FIP pass.
  With effect: `may_alloc==true` but allocation-balanced (one Construct paired
  with one Consume via reuse, net allocation = 0) → FIP falls out of converged
  state. Expected: `extract_contract()` produces a `MemoryContract` with
  `fip != FipContract::Never` (FIP certification via converged effect/token
  state, without a separate FIP pass).
  Note: `is_auto_fbip()` is NOT the right observable here — it only checks
  whether all COW operations are `StaticUnique` (a COW-level property), not
  whether `MemoryContract.fip` certifies FIP. The test must assert on the
  extracted contract's FIP status.

  A simple non-recursive tagged value demonstrates FIP without recursive type
  complexity. The function destroys one variant (consumes input) and creates one
  variant (output). The function DOES allocate (`may_alloc = true`) but every
  allocation is paired with a deallocation via reuse — `fip_token_balanced`
  is true. See 09.2 Effect Activation for `fip_token_balanced` tracking and
  FIP classification in `extract_contract()`.

### Shape × Uniqueness × Cardinality

- [ ] **`reuse_during_analysis.ori`**: Define a simple sum type and a function
  that maps it to the same type (destructure + reconstruct). Without shape
  activation: reuse detected during emission scan (emit_reuse/detect.rs scans
  death events). With shape: `ReusableCtor + Once + Unique` → reuse recorded
  in event table during analysis → `realize()` emits static Reset+Reuse.
  Expected: same code output, but simpler emission path (verified by Rust-level
  test inspecting the state map, not by observing different binary output).

  The improvement is internal (same output code, different emission path). The
  Rust-level test must assert that the reuse opportunity was recorded in the
  `AimsStateMap` event table (a `ReusableAllocation` event for the correct
  variable) after `analyze_function()` and before `emit_reuse()`, rather than
  being found by `emit_reuse/detect.rs`'s death scan.

  Sketch:
  ```ori
  type IntBox = Box(value: int);

  @increment (b: IntBox) -> IntBox = {
      let Box(v) = b;
      Box(value: v + 1)
  };
  ```

- [ ] **`collection_buffer_unique.ori`**: Create a list, push to it multiple times
  in sequence (all from the same unique source), return it. Without shape:
  COW check on each push (Dynamic — each push sees MaybeShared). With shape:
  `CollectionBuffer + Unique` → StaticUnique on each push.
  Expected: N fewer runtime checks (one per push).

  Sketch (similar to `tests/aims/cow_chain.ori` which already exists):
  ```ori
  @build_unique () -> [int] = {
      let $list = [1, 2, 3];
      list = list.push(value: 4);
      list = list.push(value: 5);
      list = list.push(value: 6);
      list
  };
  ```
  Note: `tests/aims/cow_chain.ori` already has a similar pattern. Verify whether
  it exercises this case before creating a duplicate. The synergy test should
  focus on asserting `CollectionBuffer+Unique` detection in the state map.

### Locality × Effect (combined)

- [ ] **`local_pure_chain.ori`**: Chain of 3 pure function calls on a locally-created
  list. Without combined dimensions: each call site conservatively adds RC ops
  (inc before call, dec after). With locality (list stays FunctionLocal) AND
  effect (all callees have may_share==false): the inc+dec pair at each call
  boundary is eliminated. Expected: 6 fewer RC ops (2 per call boundary × 3 calls).

  This optimization (RC-skip for function-local linear values) is the
  "FunctionLocal + Linear -> RC-skip" rule from 09.2 Locality Activation.
  Requires the RC-skip inference to be implemented before this test demonstrates
  the improvement. This test is an end-to-end proof that combined
  locality+effect analysis enables a specific optimization.

  Sketch:
  ```ori
  @count_evens (items: [int]) -> int =
      items.fold(initial: 0, op: (acc, x) -> if x % 2 == 0 then acc + 1 else acc);

  @count_odds (items: [int]) -> int =
      items.fold(initial: 0, op: (acc, x) -> if x % 2 != 0 then acc + 1 else acc);

  @count_positives (items: [int]) -> int =
      items.fold(initial: 0, op: (acc, x) -> if x > 0 then acc + 1 else acc);

  @stats (nums: [int]) -> (int, int, int) = {
      let $local = nums;  // local copy
      let $e = count_evens(items: local);
      let $o = count_odds(items: local);
      let $p = count_positives(items: local);
      (e, o, p)
  };
  ```

### Full 7-Dimension

- [ ] **`seven_dimensions.ori`**: A program that exercises all 7 dimensions in one
  function: creates a local sum-type value (locality=BlockLocal), with a reusable
  constructor (shape=ReusableCtor), used once (cardinality=Once), owned
  (access=Owned), consumed linearly (consumption=Linear), provably unique
  (uniqueness=Unique from combined locality+access), with no side effects
  (effect=NONE — the callee it calls has may_share==false).
  Pattern matches on the value, reconstructs it with reuse (same ctor kind),
  returns the result. Expected: 0 RC ops on the local value, static reuse,
  no COW checks, FIP-natural (alloc-balanced).

  The "7 dimensions" test is about asserting in the Rust test that all 7
  dimensions are at their optimal values for variable `b` at the match site,
  not about program complexity. The same `IntBox` increment function from above
  suffices.

  Sketch:
  ```ori
  type IntBox = Box(value: int);

  @inc_box (b: IntBox) -> IntBox = match b { Box(v) -> Box(value: v + 1) };
  ```

  The Rust-level test asserts that at the `match b` site, the state for `b` is:
  - `access = Owned`, `consumption = Linear`, `cardinality = Once`
  - `uniqueness = Unique` (inferred from BlockLocal + Owned + Once via canonicalize)
  - `locality = BlockLocal` (b is the parameter — function-local)
  - `shape = ReusableCtor(Box)`, `effect = NONE`

### Rust-Level Tests

- [ ] **`cross_dimension_synergy_tests` module** in `compiler/ori_arc/src/aims/realize/tests.rs`:
  For each Ori test program above, a Rust-level test that builds the equivalent
  `ArcFunction`, runs the AIMS pipeline, and asserts:
  - The specific cross-dimension rule fired (state contains expected values)
  - The output has the expected improvement (RC count, reuse ops, COW mode)
  - Removing any one dimension's contribution would regress the output

  Use the same pattern as existing Rust tests in
  `compiler/ori_arc/src/aims/intraprocedural/tests.rs` and
  `compiler/ori_arc/src/aims/emit_rc/tests.rs` — build `ArcFunction`s manually
  with `ArcIrBuilder`. See `fn make_*` helper functions in these files for the
  established API pattern.

---

## 11.2 Synergy Metrics

**File(s):** `compiler/ori_arc/src/aims/realize/metrics.rs` (NEW)

Quantify how much cross-dimensional reasoning contributes.

- [ ] **Define `SynergyMetrics`:**
  ```rust
  pub struct SynergyMetrics {
      /// RC decisions that required 2+ dimensions (not just access+consumption)
      pub multi_dim_rc_decisions: usize,
      /// Total RC decisions made
      pub total_rc_decisions: usize,
      /// COW decisions upgraded from Dynamic to StaticUnique via cross-dim proof
      pub cow_upgrades: usize,
      /// Reuse opportunities found via cross-dim proof (shape+uniqueness+cardinality)
      pub cross_dim_reuse: usize,
      /// FIP certifications achieved via extract_contract() reading converged
      /// effect+locality state (contract layer, not realization)
      pub natural_fip: usize,
      /// Canonicalize cross-dimension rules fired (total across all iterations)
      pub canonicalize_cross_fires: usize,
  }
  ```

- [ ] **Instrument `decide()` in `realize/decide.rs`** to populate RC/reuse/COW
  metrics as a side effect of making decisions. Each decision that reads 2+
  dimensions increments the appropriate counter. `natural_fip` is populated by
  `extract_contract()` in interprocedural analysis (not by `decide()`).

- [ ] **Instrument `canonicalize()`** to count cross-dimension rule firings.
  Only count rules 4+ (the new ones from Section 09.3).

- [ ] **Report metrics** in pipeline output (via `tracing::info!` at end of
  `realize()`). Also available programmatically in `RealizationResult.synergy_metrics`.

- [ ] **Baseline measurements** on golden corpus and full spec suite:
  | Metric | Golden Corpus | Full Spec | Benchmarks |
  |--------|--------------|-----------|------------|
  | Multi-dim RC % | ? | ? | ? |
  | COW upgrades | ? | ? | ? |
  | Cross-dim reuse | ? | ? | ? |
  | Natural FIP | ? | ? | ? |
  | Canonicalize cross-fires | ? | ? | ? |

  Fill in after Sections 09+10 are implemented. These become the baseline for
  regression detection.

---

## 11.3 Regression Guards

**File(s):** `compiler/ori_arc/src/aims/realize/tests.rs`

- [ ] **Per-rule regression test.**
  For each cross-dimension rule in canonicalize (Section 09.3), a test that:
  1. Creates a state where the rule should fire
  2. Asserts the rule fires (state changes)
  3. Creates a state where the rule should NOT fire (one precondition unmet)
  4. Asserts the rule does NOT fire (state unchanged)

- [ ] **Synergy regression test.**
  For each cross-dimension test program (Section 11.1), a test that:
  1. Runs the AIMS pipeline on the program
  2. Asserts `synergy_metrics.multi_dim_rc_decisions > 0` (cross-dim reasoning
     was used)
  3. Disables one specific cross-dimension rule (via config flag or mock state)
  4. Asserts the output is WORSE without the rule (more RC ops, Dynamic COW
     where StaticUnique was before, etc.)

  **Disable-rule mechanism options:**
  - **Option A:** `AimsPipelineConfig.disabled_canonicalize_rules: FxHashSet<CanonicalizeRule>`.
    Clean but adds test-only logic to production code.
  - **Option B:** Manually construct pre/post states and remove rules temporarily.
    Unwieldy.
  - **Option C:** Per-rule unit tests (state in, state out) + end-to-end RC count
    improvement tests. No need to "disable" rules programmatically.

  **Recommendation:** Option C. Per-rule unit tests (11.3) verify each rule fires
  correctly. End-to-end tests verify overall RC improvement. Add
  `AimsPipelineConfig` support (Option A) only if needed for debugging; add TODO
  in pipeline if deferred.

- [ ] **Golden corpus regression gate.**
  After Sections 09+10, re-measure RC operation counts on golden corpus.
  Assert: count ≤ Stage 1 count (integration must not regress what Stage 1
  achieved). Any regression is a bug to investigate, not an acceptable trade-off.

- [ ] **Compilation speed regression gate.**
  `hyperfine` comparison of AIMS compilation speed before and after Sections 09+10.
  Gate: ≤ 10% regression on any program. The extra canonicalize rules and
  convergence feedback add cost; it must be bounded.

---

## 11.4 Completion Checklist

### Pre-Work (11.0)
- [ ] `cargo test --workspace --features aims` green before any new tests added
- [ ] Existing `tests/aims/` programs compile and pass
- [ ] 09.0 cleanup (EffectClass dedup, unused params) confirmed complete
- [ ] `realize/tests.rs` sibling file convention confirmed

### Integration Tests
- [ ] `tests/aims/synergy/` directory with 8+ cross-dimension test programs
  (verify all programs compile with `cargo st tests/aims/synergy/` before adding to plan)
- [ ] Each test program documents which dimensions interact and expected improvement
- [ ] Existing `tests/aims/` programs checked: cow_chain.ori, closure_capture.ori,
  nested_pattern_match.ori, mixed_ownership.ori should all pass before adding synergy tests
- [ ] `SynergyMetrics` struct defined; RC/reuse/COW metrics populated by
  `realize_rc_reuse()` Phase 1; `natural_fip` populated by `extract_contract()`
  in interprocedural analysis (not by realization)
- [ ] `SynergyMetrics` from `realize_annotations()` Phase 2 (canonicalize_cross_fires)
  merged into single `RealizationResult.synergy_metrics`
- [ ] Baseline measurements recorded for golden corpus, spec suite, benchmarks
- [ ] Per-rule regression tests: each canonicalize rule (Rules 4-8) has fire/no-fire
  test pair in `compiler/ori_arc/src/aims/lattice/tests.rs`
- [ ] Synergy regression tests: end-to-end RC count improvement verified for each
  cross-dimension test program (see Option C note in 11.3)
- [ ] `AimsPipelineConfig.disabled_canonicalize_rules` considered (see Option A note
  in 11.3); if deferred, add TODO in aims_pipeline.rs
- [ ] Golden corpus RC count ≤ Stage 1 (no regression from integration work)
- [ ] Compilation speed ≤ 10% regression
- [ ] `cargo test --workspace --features aims` green
- [ ] `./test-all.sh` green
- [ ] Valgrind: 0 memory errors

### Test File Locations
- [ ] Ori test programs: `tests/aims/synergy/*.ori` (8+ programs per 11.1)
- [ ] Rust unit tests: `compiler/ori_arc/src/aims/realize/tests.rs` (cross_dimension_synergy_tests module)
- [ ] Per-rule unit tests: `compiler/ori_arc/src/aims/lattice/tests.rs` (Rules 4-8 fire/no-fire)
- [ ] Synergy metrics tests: `compiler/ori_arc/src/aims/realize/tests.rs` (metrics accumulation)
- [ ] Verify ALL Ori programs compile with `cargo st tests/aims/synergy/` before adding
  Rust-level assertions. Do NOT write Rust tests for Ori programs that don't compile.

### Stage 2 Exit Gate (combines 09 + 10 + 11)
- [ ] All Section 09 exit criteria met (every dimension influences at least one other,
  ≥12 cross-dimension interactions, all gates passed)
- [ ] All Section 10 exit criteria met (two-phase realize, output equivalence, no LLVM
  emitter changes)
- [ ] All Section 11 exit criteria below met
- [ ] `aims-shadow` feature retired per the retirement plan in 00-overview.md
- [ ] `aims` feature flag retired — AIMS is the sole pipeline
- [ ] Legacy pipeline code deleted (`borrow/`, `liveness/`, `rc_insert/`, `rc_elim/`,
  `rc_identity/`, `uniqueness/`, `reset_reuse/`, `expand_reuse/`)
- [ ] `run_arc_pipeline()` calls AIMS directly without feature dispatch

**Exit Criteria:** Cross-dimensional reasoning is *measured*, not just claimed.
`SynergyMetrics` shows that ≥20% of RC decisions required 2+ dimensions.
At least 5 cross-dimension test programs demonstrate optimization that NO single
dimension could achieve alone. Per-rule unit tests (Rules 4-8) verify each
canonicalize rule fires and doesn't fire under correct conditions.
End-to-end tests verify measurable output improvement for each synergy program.
