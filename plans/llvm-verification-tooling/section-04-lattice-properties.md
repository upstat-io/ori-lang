---
section: "04"
title: "AIMS Lattice Property Verification"
status: not-started
reviewed: false
goal: "Audit and extend proptest-based verification of algebraic lattice properties (join commutativity, idempotence, partial-order axioms), canonicalization idempotence/convergence/dimension-guarantees, decision predicate semantic contracts and transfer function monotonicity, fixpoint convergence bounds, and BUG-04-057 soundness analysis across the full 7-dimensional AIMS product lattice"
success_criteria:
  - "proptest in ori_arc dev-dependencies and lattice property tests compile and run"
  - "Join laws (commutativity, idempotence) verified via proptest across randomly sampled canonical AimsState pairs, excluding SCALAR sentinel. Associativity blocked by BUG-04-057 (test exists as #[ignore])"
  - "Partial-order axioms (reflexivity, antisymmetry, transitivity) verified for lattice_leq on canonical states"
  - "Canonicalization idempotence: canonicalize(canonicalize(s)) == canonicalize(s) for all sampled states"
  - "Decision predicate properties: semantic contracts for is_rc_dec_unnecessary, is_rc_inc_elidable, can_mutate_in_place, is_rc_needed, needs_cow_check, is_reuse_candidate, is_rc_skip_eligible, is_local verified via proptest; capture_state_update verified to produce canonical output AND tested for monotonicity"
  - "capture_state_update monotonicity: if a <= b then capture_state_update(a, c) <= capture_state_update(b, c)"
  - "Permutation-invariance: n-ary successor merge via fold-join produces same result regardless of input order"
  - "Fixpoint convergence within height bound (15 steps)"
  - "BUG-04-057 soundness analysis complete: formal determination of whether non-associative join threatens fixpoint soundness, with either a fix or documented proof of safety"
  - "All property tests pass under `timeout 150 cargo test -p ori_arc -- lattice::prop_tests` within 150s timeout"
inspired_by:
  - "Lean4 IR Checker (lean4/src/Lean/Compiler/IR/Checker.lean) — algebraic property verification on IR lattice"
  - "GHC demand analysis tests (testsuite/tests/stranal/) — property-based testing of demand lattice operations"
  - "proptest documentation (proptest-rs/proptest) — Arbitrary strategy composition for product types"
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-04-11
sections:
  - id: "04.1"
    title: "Audit Existing proptest Infrastructure and AimsState Strategy"
    status: not-started
  - id: "04.2"
    title: "Join Law Properties and Partial-Order Axioms"
    status: not-started
  - id: "04.3"
    title: "Canonicalization Properties"
    status: not-started
  - id: "04.4"
    title: "Transfer Function Properties"
    status: not-started
  - id: "04.5"
    title: "Fixpoint Convergence and Permutation Invariance"
    status: not-started
  - id: "04.6"
    title: "BUG-04-057 Soundness Analysis"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: AIMS Lattice Property Verification

> **RESET (2026-04-11):** All work in this section was produced by an autopilot session with inadequate planning and TPR oversight. However, the existing code (`compiler/ori_arc/src/aims/lattice/prop_tests.rs`, 534 lines, 22 passing tests + 1 ignored) has been reviewed during this plan revision and is structurally sound — strategies, helpers, and test logic are correct. The tasks below are reframed as **audit + extend**: audit the existing code for correctness, then add the missing tests identified by blind-spot analysis (partial-order axioms, monotonicity, permutation-invariance, distributivity, BUG-04-057 soundness). The existing code should NOT be rewritten — it should be verified and built upon.

**Status:** Not Started
**Goal:** Audit and extend proptest-based verification of algebraic lattice properties (join commutativity, idempotence, partial-order axioms), canonicalization idempotence/convergence/dimension-guarantees, decision predicate semantic contracts and transfer function monotonicity, fixpoint convergence bounds, and BUG-04-057 soundness analysis across the full 7-dimensional AIMS product lattice. The existing exhaustive tests in `lattice/tests.rs` (2,365 lines) cover specific join laws and canonicalization for 2,880 sampled combinations. Property-based testing goes further: it generates random state pairs and triples, catching algebraic bugs in corners that hand-written exhaustive enumeration might miss — particularly in cross-dimension interactions (canonicalization rules 4-8). **Notable discovery: BUG-04-057 — join is non-associative on canonical states due to canonicalization Rule 4 interaction with uniqueness.**

**Success Criteria:**

- [ ] Existing proptest infrastructure audited and verified correct (strategies, helpers, all 22 passing tests) — satisfies mission criterion: "proptest infrastructure available"
- [ ] Join commutativity, idempotence verified for random AimsState pairs; associativity blocked by BUG-04-057 (test exists, `#[ignore]`) — satisfies mission criterion: "Lattice property verification"
- [ ] Partial-order axioms (reflexivity, antisymmetry, transitivity) verified for `lattice_leq` — satisfies mission criterion: "Lattice property verification"
- [ ] Canonicalization idempotence verified for all sampled states — satisfies mission criterion: "Lattice property verification"
- [ ] Transfer function semantic contracts verified AND `capture_state_update` monotonicity tested — satisfies mission criterion: "Transfer properties"
- [ ] Permutation-invariance for n-ary successor merge verified — satisfies mission criterion: "Fixpoint convergence bounds"
- [ ] Fixpoint convergence within height bound (15 steps) — satisfies mission criterion: "Fixpoint convergence bounds"
- [ ] BUG-04-057 soundness formally analyzed — satisfies mission criterion: "Lattice soundness"

**Context:** The AIMS lattice is a product of 7 dimensions: `AccessClass` (2 values), `Consumption` (4), `Cardinality` (3), `Uniqueness` (3), `Locality` (4), `ShapeClass` (5), `EffectClass` (8 = 2^3 boolean flags). Total raw state space: 2 * 4 * 3 * 3 * 4 * 5 * 8 = 11,520 states. After excluding the `SCALAR` sentinel (not a lattice element) and considering canonicalization (which collapses infeasible combinations), the effective lattice is smaller. The existing tests in `lattice/tests.rs` exhaustively verify join laws for the 2,880-state subset sampled in `representative_states()`. Property-based testing complements this by: (1) testing join on arbitrary canonical state pairs and triples (discovering BUG-04-057: non-associativity), (2) testing canonicalization dimension guarantees on raw states, (3) verifying decision predicate semantic contracts, and (4) testing fixpoint convergence bounds.

**Existing code state:** `compiler/ori_arc/src/aims/lattice/prop_tests.rs` (534 lines) already implements:
- 7 dimension strategies + `raw_aims_state_strategy()` + `canonical_aims_state_strategy()`
- `lattice_leq()` helper
- 2 smoke tests (raw generates non-scalar, canonical is fixpoint)
- 5 join law tests (commutativity, associativity `#[ignore]`, idempotence, bottom absorption, top absorption)
- 4 canonicalization tests (idempotence, join-result-is-canonical, convergence bound, dimension guarantees)
- 9 semantic contract tests (rc_dec_unnecessary, rc_inc_elidable, can_mutate_in_place, capture_state_update canonical, is_rc_needed, needs_cow_check, is_reuse_candidate, is_rc_skip_eligible, is_local)
- 3 fixpoint tests (ascending chain, top fixpoint, cardinality seq_add convergence)

**Missing (identified by blind-spot analysis):**
- Partial-order axioms (reflexivity, antisymmetry, transitivity) for `lattice_leq` -- **transitivity is a CRITICAL GATE**: if it fails, `lattice_leq` is not a valid partial order and ALL monotonicity tests are invalid
- `capture_state_update` monotonicity test (it IS a state transformer, not just a predicate) -- uses constructive generation to avoid `prop_assume!` discard rate issues
- `Cardinality::seq_add` distributivity over `Cardinality::alt_join` as proptest (only `Cardinality`-level -- there is no `AimsState`-level `seq_add`, so no cross-dimension interaction to test here)
- Permutation-invariance for n-ary successor merges (pairwise commutativity does not guarantee this when associativity is broken) -- fold modeled to match actual `compute_block_exit_state()` behavior
- BUG-04-057 soundness analysis: exhaustive enumeration over ALL canonical state triples (not proptest sampling), checking ALL downstream decision predicates (not just 3)
- Generator skew analysis (uniform raw->canonicalize may underrepresent Rule 3/4/6/8 trigger states) -- Rule 6 generator corrected: was `Borrowed+HeapEscaping` (unreachable due to Rule 8), now correctly `Owned+HeapEscaping+Unique`

**Critical invariant:** `AimsState::SCALAR` must be excluded from all lattice-law tests. It is a sentinel, not a lattice element — joining with `SCALAR` is undefined behavior in the lattice algebra.

**Partial order definition:** `ShapeClass` and `EffectClass` lack `PartialOrd`. For lattice-law tests, define `a <= b` as `a.join(b) == b` (the standard lattice-theoretic definition). This avoids adding `PartialOrd` to types where it's not natural (flat lattice `ShapeClass`, bitfield `EffectClass`).

**Cross-section dependency -- BLOCKER for Section 05; Section 06 partially independent:**
- **Section 05 (Contract Coherence Oracle)** `depends_on: ["03", "04"]` -- depends on lattice properties for contract stability. If BUG-04-057 means the analysis is order-dependent, inferred contracts may also be structurally unstable -- the contract oracle would compare against a non-deterministic baseline. Section 05 MUST NOT proceed until 04.6 (BUG-04-057 soundness analysis) is complete and either: (a) the bug is fixed, or (b) a formal proof demonstrates that the analysis is sound despite non-associativity. This matches the plan-wide dependency in `00-overview.md` and `section-05-contract-oracle.md`.
- **Section 06 (Protocol Builtins)** `depends_on: ["01"]` (NOT `["04"]`) -- Section 06's ownership pin matrix tests and RC balance codegen audit tests are independent of BUG-04-057. They test protocol builtin ownership values and verify RC balance through existing LLVM codegen audit infrastructure, which operates at the concrete instruction level, not through lattice-theoretic reasoning. Section 06 can proceed in parallel with Section 04. Non-protocol runtime contracts (`ori_list_push`, `ori_iter_*` consumers) have the same ownership risk but are not verified in Section 06's current scope -- this is a coverage gap that should be cross-referenced when Section 06 is implemented.

**Reference implementations:**
- **Lean4** `src/Lean/Compiler/IR/Checker.lean`: algebraic property verification on IR lattice — checks that join is sound w.r.t. the semantics.
- **proptest** (`proptest-rs/proptest`): `prop_compose!` and `Strategy` for building complex type generators from simple dimension strategies.

**Depends on:** The lattice module code (`lattice/mod.rs`, `lattice/dimensions.rs`, `lattice/prop_tests.rs`) is self-contained — subsections 04.1-04.5 have no external dependencies. Subsection 04.6 (BUG-04-057 soundness analysis) requires reading `intraprocedural/block.rs` (to verify actual fold behavior), `emit_rc/cow.rs` (COW emission's uniqueness consumption), `emit_reuse/detect.rs` and `emit_reuse/planner.rs` (reuse decisions), and `transfer/mod.rs` (`can_mutate_in_place`) — these are read-only consumers, not code dependencies.

**Cross-section note for Section 06:** Section 06 `depends_on: ["01"]` does not list Section 04. If 04.6 determines BUG-04-057 affects RC decisions, Section 06's dependency metadata must be updated by whoever implements Section 06. This is outside our edit scope but is recorded here.

---

## 04.1 Audit Existing proptest Infrastructure and AimsState Strategy

**File(s):** `compiler/ori_arc/Cargo.toml`, `compiler/ori_arc/src/aims/lattice/prop_tests.rs`, `compiler/ori_arc/src/aims/lattice/mod.rs`

Audit the existing proptest setup, strategy definitions, and helper functions. Verify that the strategies generate a representative distribution and that the `lattice_leq` helper correctly implements the lattice partial order.

- [ ] Verify `proptest` is in `compiler/ori_arc/Cargo.toml` dev-dependencies (already present):
  ```toml
  [dev-dependencies]
  pretty_assertions.workspace = true
  proptest.workspace = true
  ```

- [ ] Verify `#[cfg(test)] mod prop_tests;` exists in `compiler/ori_arc/src/aims/lattice/mod.rs` (already present after existing `mod tests;`).

- [ ] Audit the 7 dimension strategies for completeness — each must enumerate ALL variants of its dimension:
  ```rust
  // Existing — verify each covers all variants:
  fn access_class_strategy()  // Borrowed, Owned — complete (2/2)
  fn consumption_strategy()   // Dead, Linear, Affine, Unrestricted — complete (4/4)
  fn cardinality_strategy()   // Absent, Once, Many — complete (3/3)
  fn uniqueness_strategy()    // Unique, MaybeShared, Shared — complete (3/3)
  fn locality_strategy()      // BlockLocal, FunctionLocal, HeapEscaping, Unknown — complete (4/4)
  fn shape_class_strategy()   // NonReusable, ReusableCtor(Struct), ReusableCtor(EnumVariant), CollectionBuffer, ContextHole — complete (5/5)
  fn effect_class_strategy()  // 3 booleans — complete (8/8)
  ```

- [ ] Audit generator skew: the uniform `raw_aims_state_strategy() -> prop_filter(SCALAR) -> prop_map(canonicalize)` approach generates all 11,519 non-SCALAR raw states uniformly, but after canonicalization many raw states collapse to the same canonical form. States triggering Rules 3/4/6/8 may be underrepresented in the canonical distribution. **Task:** Add targeted generators that specifically produce states near Rule 3/4/6/8 boundaries (e.g., `Shared + ReusableCtor` for Rule 3, `BlockLocal + Owned + Once + MaybeShared` for Rule 4) and mix them into a weighted strategy:
  ```rust
  /// Targeted strategy for states near canonicalization rule boundaries.
  /// Supplements uniform generation to ensure adequate coverage of
  /// cross-dimension interaction trigger zones.
  fn rule_boundary_aims_state_strategy() -> impl Strategy<Value = AimsState> {
      prop_oneof![
          // Rule 3 trigger zone: Shared + ReusableCtor
          (consumption_strategy(), cardinality_strategy(), locality_strategy(), effect_class_strategy())
              .prop_map(|(c, card, loc, eff)| AimsState {
                  access: AccessClass::Owned,
                  consumption: c,
                  cardinality: card,
                  uniqueness: Uniqueness::Shared,
                  locality: loc,
                  shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
                  effect: eff,
              }),
          // Rule 4 trigger zone: BlockLocal + Owned + <=Once + MaybeShared
          (effect_class_strategy(), shape_class_strategy())
              .prop_map(|(eff, shape)| AimsState {
                  access: AccessClass::Owned,
                  consumption: Consumption::Linear,
                  cardinality: Cardinality::Once,
                  uniqueness: Uniqueness::MaybeShared,
                  locality: Locality::BlockLocal,
                  shape,
                  effect: eff,
              }),
          // Rule 6 trigger zone: Owned + HeapEscaping + Unique.
          // IMPORTANT: Rule 8 fires before Rule 6 in canonicalize_single_pass()
          // (mod.rs:329-338). Rule 8 clamps Borrowed+HeapEscaping to
          // Borrowed+FunctionLocal, so Borrowed values NEVER reach Rule 6.
          // Only Owned+HeapEscaping+Unique triggers Rule 6 (→ MaybeShared).
          (consumption_strategy(), cardinality_strategy(), effect_class_strategy(), shape_class_strategy())
              .prop_map(|(c, card, eff, shape)| AimsState {
                  access: AccessClass::Owned,
                  consumption: c,
                  cardinality: card,
                  uniqueness: Uniqueness::Unique,
                  locality: Locality::HeapEscaping,
                  shape,
                  effect: eff,
              }),
      ]
      .prop_filter("exclude SCALAR sentinel", |s| !s.is_scalar())
  }
  ```

- [ ] Verify the two existing smoke tests pass and correctly validate strategy invariants (raw non-SCALAR, canonical fixpoint).

- [ ] **Subsection close-out (04.1)** — MANDATORY before starting 04.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 04.2 Join Law Properties and Partial-Order Axioms

**File(s):** `compiler/ori_arc/src/aims/lattice/prop_tests.rs`

Audit the existing join law tests and add missing partial-order axiom tests. The partial order `lattice_leq` is used pervasively in subsequent tests -- its own axioms MUST be verified first.

**CRITICAL GATE -- Transitivity:** The definition `a <= b iff a.join(b) == b` is the standard lattice-theoretic partial order, BUT it is only guaranteed to be transitive when join is associative. BUG-04-057 proves join is NOT associative. If transitivity fails, then `lattice_leq` is NOT a valid partial order, and ALL downstream monotonicity tests (04.4) that use it are testing against an invalid ordering relation. The transitivity test is therefore the FIRST critical gate in this section: if it fails, 04.4 must be redesigned to use componentwise dimension-by-dimension ordering instead.

### Existing tests to audit (verify they pass and are correct):

- [ ] Audit join commutativity test: `a.join(b) == b.join(a)` — already exists and passes.

- [ ] Audit join associativity test: `a.join(b.join(c)) == (a.join(b)).join(c)` — already exists as `#[ignore]` due to BUG-04-057. Verify the `#[ignore]` message is accurate and the counterexample in the bug tracker matches.

- [ ] Audit join idempotence test: `a.join(a) == a` — already exists and passes.

- [ ] Audit join absorption tests: `a.join(BOTTOM) >= a` and `a.join(TOP) == TOP` — already exist and pass.

### New tests — partial-order axioms for `lattice_leq`:

The `lattice_leq` helper (`a <= b iff a.join(b) == b`) is used by every subsequent monotonicity and ordering test. Its axioms must be independently verified on canonical states.

- [ ] Add reflexivity test: `lattice_leq(a, a)` for all canonical states:
  ```rust
  proptest! {
      #[test]
      fn lattice_leq_reflexive(a in canonical_aims_state_strategy()) {
          assert!(
              lattice_leq(&a, &a),
              "lattice_leq must be reflexive: a={a:?}"
          );
      }
  }
  ```

- [ ] Add antisymmetry test: if `lattice_leq(a, b)` and `lattice_leq(b, a)` then `a == b`:
  ```rust
  proptest! {
      #[test]
      fn lattice_leq_antisymmetric(
          a in canonical_aims_state_strategy(),
          b in canonical_aims_state_strategy(),
      ) {
          if lattice_leq(&a, &b) && lattice_leq(&b, &a) {
              assert_eq!(
                  a, b,
                  "lattice_leq must be antisymmetric: a={a:?}, b={b:?}"
              );
          }
      }
  }
  ```

- [ ] Add transitivity test: if `lattice_leq(a, b)` and `lattice_leq(b, c)` then `lattice_leq(a, c)`:
  ```rust
  proptest! {
      #![proptest_config(ProptestConfig::with_cases(5000))]
      #[test]
      fn lattice_leq_transitive(
          a in canonical_aims_state_strategy(),
          diff_ab in canonical_aims_state_strategy(),
          diff_bc in canonical_aims_state_strategy(),
      ) {
          // Constructive generation: build b = a.join(diff_ab) so a <= b holds
          // by construction. Then build c = b.join(diff_bc) so b <= c holds.
          // This avoids prop_assume!(lattice_leq(...)) which would discard
          // the vast majority of random triples (most pairs are incomparable),
          // likely hitting proptest's TooManyRejects limit.
          let b = a.join(&diff_ab);
          let c = b.join(&diff_bc);
          // Verify the construction actually produced the ordering:
          assert!(lattice_leq(&a, &b), "construction failed: a={a:?}, b={b:?}");
          assert!(lattice_leq(&b, &c), "construction failed: b={b:?}, c={c:?}");
          // The actual transitivity check:
          assert!(
              lattice_leq(&a, &c),
              "lattice_leq must be transitive: a={a:?}, b={b:?}, c={c:?}"
          );
      }
  }
  ```
  **Design note (blind-spot analysis):** The original approach using `prop_assume!(lattice_leq(&a, &b) && lattice_leq(&b, &c))` on random canonical states would discard the vast majority of cases (most pairs are incomparable in the product lattice), likely hitting proptest's `TooManyRejects` limit. Constructive generation guarantees `a <= b <= c` by definition (`b = a.join(diff)` implies `a <= b` because `a.join(a.join(diff)) == a.join(diff) == b`). This gives 100% hit rate while still exploring diverse state triples.

  **CRITICAL GATE:** If this test FAILS, then `lattice_leq` defined as `a.join(b) == b` is NOT a valid partial order when join is non-associative (BUG-04-057). Transitivity failure would invalidate ALL downstream monotonicity tests (04.4 `capture_state_update` monotonicity) and ALL ordering assertions in 04.5/04.6. If transitivity fails: (1) STOP all further work in this section, (2) escalate BUG-04-057 to critical, (3) the partial order definition must be reconsidered (possibly componentwise `<=` on each dimension separately, which is always a valid partial order regardless of join associativity).

- [ ] **TPR checkpoint** — `/tpr-review` covering 04.1-04.2 implementation work (covered by mandatory section-close TPR per /continue-roadmap §Step 10)

- [ ] **Subsection close-out (04.2)** — MANDATORY before starting 04.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 04.3 Canonicalization Properties

**File(s):** `compiler/ori_arc/src/aims/lattice/prop_tests.rs`

Audit the existing canonicalization property tests. These are complete and correct — no new tests needed.

### Existing tests to audit (verify they pass and are correct):

- [ ] Audit canonicalization idempotence: `canonicalize(canonicalize(s)) == canonicalize(s)` — already exists and passes.

- [ ] Audit join output is canonical: `canonicalize(a.join(b)) == a.join(b)` — already exists and passes.

- [ ] Audit canonicalization convergence bound: convergence within 3 rounds — already exists and passes.

- [ ] Audit canonicalization dimension guarantees: per-dimension direction invariants — already exists and passes. Verify the direction claims match the actual canonicalization rules:
  - access: unchanged (no rule touches it)
  - consumption: only decreases (Rule 1: -> Dead for Absent)
  - cardinality: only decreases (Rule 1: -> Absent for Dead)
  - uniqueness: EITHER direction (Rule 4 down, Rule 6 up)
  - locality: only decreases (Rule 8: -> <=FunctionLocal for Borrowed)
  - shape: can move to NonReusable (Rule 3: Shared+ReusableCtor -> NonReusable)
  - effect: unchanged (no rule touches it)

- [ ] **Subsection close-out (04.3)** — MANDATORY before starting 04.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 04.4 Transfer Function Properties

**File(s):** `compiler/ori_arc/src/aims/lattice/prop_tests.rs`, `compiler/ori_arc/src/aims/transfer/mod.rs`

Audit the existing semantic contract tests and add the missing `capture_state_update` monotonicity test. The existing tests correctly verify that the 8 decision predicates match their semantic definitions (they are predicates, not lattice morphisms — monotonicity does not apply to them). However, `capture_state_update` IS a state transformer (AimsState -> AimsState), and the `arc.md` rule "Non-monotone transfer = unsound analysis" applies to it. The existing test only verifies it produces canonical output — it does NOT test monotonicity.

### Existing tests to audit (verify they pass and are correct):

- [ ] Audit semantic contract tests for all 8 decision predicates:
  - `rc_dec_unnecessary_semantic_contract` — matches: `Absent || Dead`
  - `rc_inc_elidable_semantic_contract` — matches: `Once && Linear`
  - `can_mutate_in_place_semantic_contract` — matches: `Owned && Unique`
  - `is_rc_needed_semantic_contract` — matches: `Owned && !Dead && !SCALAR`
  - `needs_cow_check_semantic_contract` — matches: `MaybeShared`
  - `is_reuse_candidate_semantic_contract` — matches: `Owned && !Shared && !NonReusable`
  - `is_rc_skip_eligible_semantic_contract` — matches: `local && Owned && Linear && !SCALAR`
  - `is_local_semantic_contract` — matches: `BlockLocal || FunctionLocal`

- [ ] Audit `capture_state_update_produces_canonical` — verifies output is canonical. Already exists and passes.

### New tests — `capture_state_update` monotonicity:

`capture_state_update(current, closure_state) -> AimsState` is a genuine state transformer: it takes the current variable state and the closure's demanded state, and computes the state for captured variables. Per `arc.md` "Non-monotone transfer = unsound analysis", this function MUST be monotone: if `a <= b` (in the lattice), then `capture_state_update(a, c) <= capture_state_update(b, c)` for all `c`. A non-monotone `capture_state_update` would mean that more information about a variable could lead to LESS conservative (more aggressive) optimization of its capture — a soundness violation.

- [ ] Add `capture_state_update` monotonicity test (first argument):
  ```rust
  proptest! {
      #![proptest_config(ProptestConfig::with_cases(5000))]
      #[test]
      fn capture_state_update_monotone_in_current(
          a in canonical_aims_state_strategy(),
          diff in canonical_aims_state_strategy(),
          closure in canonical_aims_state_strategy(),
      ) {
          use crate::aims::transfer::capture_state_update;
          // Constructive: b = a.join(diff) guarantees a <= b.
          let b = a.join(&diff);
          let fa = capture_state_update(&a, &closure);
          let fb = capture_state_update(&b, &closure);
          assert!(
              lattice_leq(&fa, &fb),
              "capture_state_update must be monotone in current: \
               a={a:?}, b={b:?}, closure={closure:?}, f(a)={fa:?}, f(b)={fb:?}"
          );
      }
  }
  ```

- [ ] Add `capture_state_update` monotonicity test (second argument -- closure state):
  ```rust
  proptest! {
      #![proptest_config(ProptestConfig::with_cases(5000))]
      #[test]
      fn capture_state_update_monotone_in_closure(
          current in canonical_aims_state_strategy(),
          c1 in canonical_aims_state_strategy(),
          diff in canonical_aims_state_strategy(),
      ) {
          use crate::aims::transfer::capture_state_update;
          // Constructive: c2 = c1.join(diff) guarantees c1 <= c2.
          let c2 = c1.join(&diff);
          let fc1 = capture_state_update(&current, &c1);
          let fc2 = capture_state_update(&current, &c2);
          assert!(
              lattice_leq(&fc1, &fc2),
              "capture_state_update must be monotone in closure: \
               current={current:?}, c1={c1:?}, c2={c2:?}, f(c1)={fc1:?}, f(c2)={fc2:?}"
          );
      }
  }
  ```

  **Note on `prop_assume!` avoidance:** Both tests above use constructive generation (`b = a.join(diff)`) to guarantee the ordering relation, avoiding `prop_assume!(lattice_leq(...))` which would discard most random pairs (most canonical states are incomparable in the 7D product lattice). This ensures 100% hit rate and avoids `TooManyRejects` failures.

- [ ] **TPR checkpoint** — `/tpr-review` covering 04.3-04.4 implementation work (covered by mandatory section-close TPR per /continue-roadmap §Step 10)

- [ ] **Subsection close-out (04.4)** — MANDATORY before starting 04.5:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 04.5 Fixpoint Convergence and Permutation Invariance

**File(s):** `compiler/ori_arc/src/aims/lattice/prop_tests.rs`

Audit the existing fixpoint convergence tests and add the missing permutation-invariance and `seq_add` distributivity tests. Permutation-invariance is critical: the backward CFG analysis folds successor ENTRY states via join (computing what successors demand), and pairwise commutativity alone does NOT guarantee order-independence when associativity is broken (BUG-04-057). If the fold result depends on successor ordering, the analysis is non-deterministic.

### Existing tests to audit (verify they pass and are correct):

- [ ] Audit ascending chain convergence: chain stabilizes within lattice height 15 — already exists and passes.

- [ ] Audit TOP fixpoint: `TOP.join(s) == TOP` for all s — already exists and passes.

- [ ] Audit `Cardinality::seq_add` convergence: chain reaches `Many` within 2 steps — already exists and passes.

### New tests — permutation-invariance for n-ary merges:

At CFG join points, the analysis folds multiple predecessor states: `fold(join, predecessors)`. With associative join, fold order is irrelevant. But BUG-04-057 proves join is NOT associative. This test checks whether different orderings of the SAME set of predecessor states produce different results — which would make the analysis non-deterministic.

- [ ] Add permutation-invariance test for n-ary join:
  ```rust
  proptest! {
      #![proptest_config(ProptestConfig::with_cases(2000))]
      #[test]
      fn nary_join_permutation_invariant(
          states in proptest::collection::vec(canonical_aims_state_strategy(), 2..8),
      ) {
          // Model the REAL analysis fold from compute_block_exit_state()
          // (intraprocedural/block.rs:76-90): the real code uses an empty
          // HashMap, so the first successor state is taken verbatim via
          // map_or(succ_state, ...), and subsequent successors are joined in.
          // This is equivalent to seeding with BOTTOM because:
          //   BOTTOM_canonical.join(x) == x for all canonical x
          // (BOTTOM is componentwise minimum, join is componentwise max,
          //  and canonicalization is a no-op on already-canonical states).
          // We use the first-element seed to match the real code exactly.
          let fold_join = |order: &[AimsState]| -> AimsState {
              let mut iter = order.iter();
              let first = *iter.next().unwrap();
              let mut acc = first;
              for s in iter {
                  acc = acc.join(s);
              }
              acc
          };

          let forward = fold_join(&states);

          // Fold in reverse order
          let reversed: Vec<_> = states.iter().copied().rev().collect();
          let backward = fold_join(&reversed);

          assert_eq!(
              forward, backward,
              "n-ary join must be permutation-invariant: \
               forward={forward:?}, backward={backward:?}, states={states:?}"
          );
      }
  }
  ```
  **Important:** If this test FAILS, it confirms that BUG-04-057 has real consequences for the analysis -- the fixpoint result depends on worklist iteration order. This would escalate BUG-04-057 from "high" to "critical" and block Section 05 entirely (Section 06 is independently implementable per its `depends_on: ["01"]`).

  **Design note (blind-spot analysis):** The real `compute_block_exit_state()` in `intraprocedural/block.rs:76-90` does NOT seed with `AimsState::BOTTOM`. It starts with an empty `FxHashMap` and uses `map_or(succ_state, |existing| existing.join(&succ_state))`, meaning the first successor's state is taken verbatim and subsequent successors are joined in. This is mathematically equivalent to a BOTTOM-seeded fold (since `BOTTOM.join(x) == x`), but the test models the real code exactly to avoid any confusion about the equivalence.

- [ ] Add shuffled permutation test for stronger coverage (test multiple random orderings, not just forward/reverse):
  ```rust
  proptest! {
      #![proptest_config(ProptestConfig::with_cases(1000))]
      #[test]
      fn nary_join_shuffled_permutations(
          states in proptest::collection::vec(canonical_aims_state_strategy(), 3..6),
          seed in any::<u64>(),
      ) {
          use std::collections::hash_map::DefaultHasher;
          use std::hash::{Hash, Hasher};

          let fold_join = |order: &[AimsState]| -> AimsState {
              let mut iter = order.iter();
              let first = *iter.next().unwrap();
              let mut acc = first;
              for s in iter {
                  acc = acc.join(s);
              }
              acc
          };

          let baseline = fold_join(&states);

          // Create a deterministic permutation using the seed
          let mut shuffled = states.clone();
          // Simple Fisher-Yates shuffle using seed-derived indices
          for i in (1..shuffled.len()).rev() {
              let mut hasher = DefaultHasher::new();
              seed.hash(&mut hasher);
              i.hash(&mut hasher);
              let j = (hasher.finish() as usize) % (i + 1);
              shuffled.swap(i, j);
          }

          let permuted = fold_join(&shuffled);
          assert_eq!(
              baseline, permuted,
              "n-ary join must produce same result regardless of fold order"
          );
      }
  }
  ```

### New test -- `seq_add` distributivity over `alt_join` at Cardinality level:

The `dimensions.rs` doc claims `seq_add` distributes over `alt_join` for `Cardinality`, and this is exhaustively tested in `tests.rs` for the `Cardinality` dimension alone. This proptest duplicates that exhaustive coverage with proptest's shrinking for better counterexample reporting if the property ever breaks. **This test operates on the `Cardinality` dimension only, not at the full `AimsState` level** -- `seq_add` is a `Cardinality`-only operation (other dimensions use `join`/`max` for both sequential and alternative composition, as documented in `dimensions.rs`). There is no `AimsState`-level `seq_add` to test.

- [ ] Add `Cardinality::seq_add` distributivity over `Cardinality::alt_join` proptest:
  ```rust
  proptest! {
      #[test]
      fn cardinality_seq_add_distributes_over_alt_join(
          a in cardinality_strategy(),
          b in cardinality_strategy(),
          c in cardinality_strategy(),
      ) {
          let lhs = a.seq_add(b.alt_join(c));
          let rhs = a.seq_add(b).alt_join(a.seq_add(c));
          assert_eq!(
              lhs, rhs,
              "seq_add must distribute over alt_join: a={a:?}, b={b:?}, c={c:?}"
          );
      }
  }
  ```
  Note: This is `Cardinality`-only (3 values = 27 triples total), so the exhaustive test in `tests.rs` already covers the full space. This proptest exists for regression safety with minimal cost.

- [ ] **Subsection close-out (04.5)** — MANDATORY before starting 04.6:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 04.6 BUG-04-057 Soundness Analysis

**File(s):** `compiler/ori_arc/src/aims/lattice/prop_tests.rs`, `compiler/ori_arc/src/aims/lattice/mod.rs`, `compiler/ori_arc/src/aims/intraprocedural/mod.rs`

BUG-04-057 (join non-associativity in uniqueness dimension) is a FUNDAMENTAL SOUNDNESS ISSUE, not just a failed test with `#[ignore]`. Non-associative join means that the fold order over successor entry states at CFG split points determines the analysis result — making RC placement decisions potentially order-dependent. The current analysis is masked by deterministic reverse-postorder processing, but this is fragile: any change to the CFG traversal order could surface different analysis results.

This subsection requires a formal analysis of whether BUG-04-057 threatens fixpoint soundness. The analysis must produce one of three outcomes:

### Option A: The bug is benign (proof of safety)

If the non-associativity only affects the uniqueness dimension in states that are Dead+Absent (the counterexample in the bug tracker), and these states never influence RC decisions (because `is_rc_needed` returns false for Dead states), then the analysis may be sound despite non-associativity. This requires a formal argument:

- [ ] Enumerate the state space where associativity fails -- characterize ALL counterexamples exhaustively:

  **Approach:** The canonical state space has ~3,928 states (verified empirically by enumerating all 11,519 non-SCALAR raw states and canonicalizing). Full triple enumeration (~60.6 billion) is infeasible within the 150s test timeout. Instead, use a **two-tier strategy**: (1) exhaustive enumeration over all canonical STATE PAIRS to identify which `(a, b)` pairs produce join results that are sensitive to associativity (the `ab` intermediate), then (2) for each sensitive pair, sweep over all canonical `c` values. This reduces the search space dramatically because most pairs produce joins where Rule 4 never triggers. If the sensitive-pair set is unexpectedly large (making the test exceed 150s), further tier refinement is needed — sampling CANNOT discharge the exhaustive proof required by Option A. In that case, partition the sensitive set by dimension values and sweep each partition separately, or escalate to Option B/C.

  ```rust
  /// Two-tier characterization: first find sensitive (a,b) pairs where
  /// the join intermediate is in the Rule 4 trigger zone, then sweep c
  /// for each. Falls back to proptest if tier-1 set is too large.
  #[test]
  fn associativity_divergence_with_canonical_triples_detects_decision_impact() {
      let canonical_states = collect_all_canonical_states();
      let n = canonical_states.len();
      eprintln!("canonical state count: {n}");
      // Expected: ~3,928 canonical states. Full triple sweep (~60B)
      // is infeasible within 150s; use two-tier approach instead.

      // Tier 1: find all (a,b) pairs where the intermediate ab has
      // MaybeShared or Unique uniqueness (the Rule 4 trigger zone).
      let mut sensitive_pairs: Vec<(usize, usize)> = Vec::new();
      for (i, a) in canonical_states.iter().enumerate() {
          for (j, b) in canonical_states.iter().enumerate() {
              let ab = a.join(b);
              if ab.uniqueness == Uniqueness::MaybeShared
                  || ab.uniqueness == Uniqueness::Unique {
                  sensitive_pairs.push((i, j));
              }
          }
      }
      eprintln!("sensitive pairs: {}", sensitive_pairs.len());

      // Tier 2: for each sensitive pair, sweep all c values
      let mut failure_count = 0u64;
      let mut uniqueness_only_count = 0u64;
      let mut decision_divergence = false;

      for &(i, j) in &sensitive_pairs {
          let a = &canonical_states[i];
          let b = &canonical_states[j];
          let ab = a.join(b);
          for c in &canonical_states {
              let ab_c = ab.join(c);
              let a_bc = a.join(&b.join(c));
              if ab_c != a_bc {
                  failure_count += 1;

                  let diverges_non_uniqueness = ab_c.access != a_bc.access
                      || ab_c.consumption != a_bc.consumption
                      || ab_c.cardinality != a_bc.cardinality
                      || ab_c.locality != a_bc.locality
                      || ab_c.shape != a_bc.shape
                      || ab_c.effect != a_bc.effect;

                  if diverges_non_uniqueness {
                      panic!(
                          "associativity failure in non-uniqueness dimension — \
                           escalate BUG-04-057: a={a:?}, b={b:?}, c={c:?}, \
                           ab_c={ab_c:?}, a_bc={a_bc:?}"
                      );
                  }
                  uniqueness_only_count += 1;

                  // Check ALL downstream uniqueness consumers:
                  //   - is_rc_needed, needs_cow_check, is_reuse_candidate,
                  //     is_rc_skip_eligible, is_local (lattice predicates)
                  //   - can_mutate_in_place (transfer/mod.rs:433)
                  //   - compute_effective_may_share (block.rs:409)
                  //   - FIP balance (fip_balance.rs:272)
                  //   - state_map immortal (state_map.rs:428)
                  //   - realize/decide.rs:484
                  //   - post_convergence.rs:259,336
                  //   - COW emission (emit_rc/cow.rs:37)
                  //   - reuse detection (emit_reuse/detect.rs:86-87)
                  //   - reuse planning (emit_reuse/planner.rs:83)
                  use crate::aims::transfer::can_mutate_in_place;
                  let predicates_match =
                      ab_c.is_rc_needed() == a_bc.is_rc_needed()
                      && ab_c.needs_cow_check() == a_bc.needs_cow_check()
                      && ab_c.is_reuse_candidate() == a_bc.is_reuse_candidate()
                      && ab_c.is_rc_skip_eligible() == a_bc.is_rc_skip_eligible()
                      && ab_c.is_local() == a_bc.is_local()
                      && can_mutate_in_place(&ab_c) == can_mutate_in_place(&a_bc)
                      // Direct uniqueness equality — covers ALL consumers
                      // that branch on `uniqueness == Unique` (FIP balance,
                      // compute_effective_may_share, state_map, etc.)
                      && (ab_c.uniqueness == Uniqueness::Unique)
                          == (a_bc.uniqueness == Uniqueness::Unique);
                  if !predicates_match {
                      decision_divergence = true;
                      eprintln!(
                          "DECISION DIVERGENCE: a={a:?}, b={b:?}, c={c:?}, \
                           ab_c={ab_c:?}, a_bc={a_bc:?}"
                      );
                  }
              }
          }
      }

      eprintln!(
          "associativity failures: {failure_count} total, \
           {uniqueness_only_count} uniqueness-only, \
           across {} sensitive pairs × {n} c-values",
          sensitive_pairs.len()
      );

      assert!(
          !decision_divergence,
          "BUG-04-057 affects downstream decisions — fix required. \
           {failure_count} failures, see stderr for details."
      );
  }

  /// Collect all canonical AimsState values by exhaustive enumeration.
  fn collect_all_canonical_states() -> Vec<AimsState> {
      use std::collections::HashSet;
      let accesses = [AccessClass::Borrowed, AccessClass::Owned];
      let consumptions = [
          Consumption::Dead, Consumption::Linear,
          Consumption::Affine, Consumption::Unrestricted,
      ];
      let cardinalities = [Cardinality::Absent, Cardinality::Once, Cardinality::Many];
      let uniquenesses = [Uniqueness::Unique, Uniqueness::MaybeShared, Uniqueness::Shared];
      let localities = [
          Locality::BlockLocal, Locality::FunctionLocal,
          Locality::HeapEscaping, Locality::Unknown,
      ];
      let shapes = [
          ShapeClass::NonReusable,
          ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
          ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
          ShapeClass::CollectionBuffer,
          ShapeClass::ContextHole,
      ];
      let effects = [
          EffectClass { may_alloc: false, may_share: false, may_throw: false },
          EffectClass { may_alloc: true, may_share: false, may_throw: false },
          EffectClass { may_alloc: false, may_share: true, may_throw: false },
          EffectClass { may_alloc: true, may_share: true, may_throw: false },
          EffectClass { may_alloc: false, may_share: false, may_throw: true },
          EffectClass { may_alloc: true, may_share: false, may_throw: true },
          EffectClass { may_alloc: false, may_share: true, may_throw: true },
          EffectClass { may_alloc: true, may_share: true, may_throw: true },
      ];

      let mut seen = HashSet::new();
      for &access in &accesses {
          for &consumption in &consumptions {
              for &cardinality in &cardinalities {
                  for &uniqueness in &uniquenesses {
                      for &locality in &localities {
                          for &shape in &shapes {
                              for &effect in &effects {
                                  let mut s = AimsState {
                                      access, consumption, cardinality,
                                      uniqueness, locality, shape, effect,
                                  };
                                  if s.is_scalar() { continue; }
                                  s.canonicalize();
                                  seen.insert(s);
                              }
                          }
                      }
                  }
              }
          }
      }
      seen.into_iter().collect()
  }
  ```

  **Test function naming:** The function name follows `<subject>_<scenario>_<expected>` convention per `impl-hygiene.md`. Provenance (BUG-04-057) goes in `///` doc comment only.

  **Runtime estimate:** With ~3,928 canonical states, the full pair space is ~15.4M pairs. The sensitive subset (uniqueness in {MaybeShared, Unique}) is expected to be much smaller. Each sensitive pair sweeps ~3,928 c-values with 2 joins + 7 predicate checks per triple. Runtime depends on the sensitive-pair count — if ~10% of pairs are sensitive (~1.5M), the total is ~6B operations at ~10ns/op ≈ ~60s, within the 150s budget. If the sensitive set is larger, the test will log the count and may need further tier refinement.

- [ ] If the characterization test passes (all associativity failures are uniqueness-only AND do not affect ANY downstream decision predicate -- `is_rc_needed`, `needs_cow_check`, `is_reuse_candidate`, `is_rc_skip_eligible`, `is_local`, `can_mutate_in_place`), document the formal argument in a code comment on `join()` and update BUG-04-057 severity from `high` to `low` with the formal justification. The argument must explicitly address why uniqueness divergence is harmless for each consumer:
  - `emit_rc/cow.rs:37` -- COW emission checks `uniqueness != Unique`
  - `emit_reuse/detect.rs:86-87` -- reuse detection checks `uniqueness == Unique || MaybeShared`
  - `emit_reuse/planner.rs:83` -- reuse planning filters `uniqueness == Unique`
  - `transfer/mod.rs:433-434` -- `can_mutate_in_place` requires `Owned && Unique`
  - `intraprocedural/block.rs:409` -- `compute_effective_may_share` checks `uniqueness == Unique`
  - `intraprocedural/fip_balance.rs:272` -- FIP balance checks `uniqueness == Unique`
  - `intraprocedural/state_map.rs:428` -- immortal check branches on `uniqueness == Unique`
  - `realize/decide.rs:484` -- realization decisions use uniqueness
  - `intraprocedural/post_convergence.rs:259,336` -- post-convergence adjustments use uniqueness

### Option B: The bug affects RC decisions (fix required)

If the characterization test reveals that associativity failures CAN change RC/COW/reuse decisions, the bug MUST be fixed before Section 05's correctness-gate authority can be trusted. Section 06 remains independently implementable per its `depends_on: ["01"]` unless this analysis proves a concrete dependency. The fix would be in `canonicalize_single_pass()` — the root cause is that Rule 4 (BlockLocal+Owned+<=Once+MaybeShared -> Unique) fires on different intermediate states depending on fold order.

- [ ] If fix is required: file via `/fix-bug BUG-04-057` for full plan-section rigor (root cause analysis, TDD matrix, implementation). The fix section file will be at `plans/bug-tracker/fix-BUG-04-057.md`.

### Option C: Permutation-invariance test (04.5) already fails

If the `nary_join_permutation_invariant` test from 04.5 FAILS, then BUG-04-057 has immediate practical consequences — the analysis result depends on predecessor ordering. This automatically escalates to Option B (fix required) and additionally requires auditing all `analyze_function()` call sites to ensure they use a canonical ordering.

- [ ] Based on 04.5 permutation-invariance test results, determine which option applies and execute accordingly.

- [ ] Update BUG-04-057 entry in `plans/bug-tracker/section-04-codegen-llvm.md` with the analysis results.

- [ ] If Option A (benign): remove `#[ignore]` from `join_associative` and replace with a targeted characterization test (the exhaustive one above) that documents exactly WHICH triples fail and WHY non-associativity is safe. The `#[ignore]` annotation is a deferral placeholder -- it must be resolved to either a passing test (with documented safety proof) or escalated to `/fix-bug`.

- [ ] **Subsection close-out (04.6)** — MANDATORY before starting 04.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 04.R Third Party Review Findings

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

- [x] `[TPR-04-001-codex][high]` `section-04-lattice-properties.md:552` — Fix GAP in 04.6 exhaustive characterization sizing.
  Evidence: Canonical state count is ~3,928, not ~300-500. Full triple sweep (~60B) is infeasible within 150s timeout.
  Resolved: Fixed on 2026-04-11. Replaced exhaustive triple sweep with two-tier approach (sensitive-pair filtering + per-pair c-sweep). Updated runtime estimate.
- [x] `[TPR-04-002-codex][high]` `section-04-lattice-properties.md:593` — Close GAP in 04.6 uniqueness consumer coverage.
  Evidence: Additional uniqueness consumers at realize/decide.rs:484, post_convergence.rs:259,336, fip_balance.rs:272, state_map.rs:428 were not covered.
  Resolved: Fixed on 2026-04-11. Expanded consumer list in characterization test and formal argument section. Added direct `uniqueness == Unique` equality check to predicates_match.
- [x] `[TPR-04-003-codex][medium]` `section-04-lattice-properties.md:459` — Resolve DRIFT in 05 vs 06 blocker text.
  Evidence: 04.5 and 04.N still referenced "Sections 05/06" despite revised dependency model.
  Resolved: Fixed on 2026-04-11. Updated 04.5, 04.6 Option B, and 04.N to reflect that only Section 05's correctness-gate authority is blocked; Section 06 is independently implementable.
- [x] `[TPR-04-001-gemini][medium]` `section-04-lattice-properties.md:401` — Include FIP conditional check in characterization.
  Evidence: `compute_effective_may_share` at block.rs:409 directly checks `uniqueness == Unique` but was not in the predicate list.
  Resolved: Fixed on 2026-04-11. Added direct `(uniqueness == Unique)` equality check to the characterization test, covering compute_effective_may_share and all other direct uniqueness consumers.
- [x] `[TPR-04-005-codex][medium]` `section-04-lattice-properties.md:737` — Remaining Section 06 blocker drift in Option B.
  Evidence: Option B still said "Sections 05/06 proceed" after round 1 fixes.
  Resolved: Fixed on 2026-04-11. Updated Option B to match revised dependency model.

---

## 04.N Completion Checklist

### Existing code audit
- [ ] Existing `prop_tests.rs` (534 lines, 22 tests) audited and verified correct
- [ ] `proptest` is in `compiler/ori_arc/Cargo.toml` dev-dependencies
- [ ] `AimsState` proptest strategies generate all 7 dimensions independently, exclude `SCALAR`
- [ ] Generator skew addressed: rule-boundary-targeted strategy added alongside uniform strategy

### Join law properties (04.2)
- [ ] Join commutativity: `a.join(b) == b.join(a)` for random pairs — audited
- [ ] Join associativity: test exists with `#[ignore]` — **BUG-04-057** — audited
- [ ] Join idempotence: `a.join(a) == a` for random canonical states — audited
- [ ] Join absorption: `a.join(BOTTOM) >= a`, `a.join(TOP) == TOP` — audited

### Partial-order axioms (NEW -- 04.2)
- [ ] `lattice_leq` reflexivity: `lattice_leq(a, a)` for all canonical states
- [ ] `lattice_leq` antisymmetry: `leq(a,b) && leq(b,a)` implies `a == b`
- [ ] `lattice_leq` transitivity: constructive generation (`b=a.join(diff)`, `c=b.join(diff2)`) -- **CRITICAL GATE**: if this fails, all 04.4 monotonicity tests are invalid

### Canonicalization properties (04.3)
- [ ] Canonicalization idempotence: `canonicalize(canonicalize(s)) == canonicalize(s)` — audited
- [ ] Canonicalization convergence within 3 rounds — audited
- [ ] Canonicalization per-dimension guarantees — audited
- [ ] Join output is canonical: `canonicalize(a.join(b)) == a.join(b)` — audited

### Transfer function properties (04.4)
- [ ] 8 decision predicate semantic contracts verified -- audited
- [ ] `capture_state_update` produces canonical output -- audited
- [ ] `capture_state_update` monotonicity in first argument (NEW, constructive generation)
- [ ] `capture_state_update` monotonicity in second argument (NEW, constructive generation)

### Fixpoint convergence and permutation invariance (04.5)
- [ ] Ascending chain convergence within height bound (15 steps) -- audited
- [ ] TOP is a fixpoint -- audited
- [ ] `Cardinality::seq_add` convergence within 2 steps -- audited
- [ ] N-ary join permutation invariance -- forward vs reverse (NEW, first-element-seed fold matching `block.rs`)
- [ ] N-ary join shuffled permutations (NEW, first-element-seed fold matching `block.rs`)
- [ ] `Cardinality::seq_add` distributivity over `alt_join` proptest (NEW, Cardinality-only)

### BUG-04-057 soundness analysis (NEW -- 04.6)
- [ ] Associativity failure characterization: ALL counterexamples enumerated exhaustively (not sampled)
- [ ] Impact analysis: determine if failures affect ANY downstream uniqueness consumer (6 lattice predicates + can_mutate_in_place + direct uniqueness==Unique checks in FIP balance, compute_effective_may_share, state_map, realize/decide, post_convergence, COW emission, reuse detection/planning)
- [ ] Resolution: Option A (benign -- proof documented), Option B (fix via `/fix-bug`), or Option C (permutation test fails -- escalate)
- [ ] BUG-04-057 bug tracker entry updated with analysis results
- [ ] Cross-section blocker resolved: Section 05 unblocked (or blocked with documented reason). Section 06 is independently implementable per `depends_on: ["01"]`.

### Standard close-out
- [ ] All property tests pass: `timeout 150 cargo test -p ori_arc -- lattice::prop_tests`
- [ ] No regressions: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: run `plan-annotations.sh --cleanup-only --plan llvm-verification-tooling` — remove stale annotations from completed items
- [ ] All TPR findings triaged and resolved (check 04.R block)
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` updated
  - [ ] `00-overview.md` Quick Reference updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed — both reviewers clean on final code
- [ ] `/impl-hygiene-review` passed — zero critical/major findings (mod.rs BLOAT pre-existing, prop_tests.rs exempt as test file)
- [ ] `/improve-tooling` section-close sweep

**Exit Criteria:** `timeout 150 cargo test -p ori_arc -- lattice::prop_tests` runs all property-based lattice tests and passes. proptest has verified join commutativity/idempotence, partial-order axioms, canonicalization idempotence/convergence/dimension-guarantees, decision predicate semantic contracts, `capture_state_update` monotonicity, permutation invariance for n-ary merges, and fixpoint convergence across thousands of randomly generated `AimsState` values. BUG-04-057 soundness formally analyzed with one of three outcomes documented. The `SCALAR` sentinel is excluded from all property tests. All tests complete within the 150-second timeout.
