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
  status: none
  updated: null
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
- Partial-order axioms (reflexivity, antisymmetry, transitivity) for `lattice_leq`
- `capture_state_update` monotonicity test (it IS a state transformer, not just a predicate)
- `seq_add` distributivity over `join` at the `AimsState` level (only tested exhaustively for `Cardinality` in `tests.rs`)
- Permutation-invariance for n-ary successor merges (pairwise commutativity does not guarantee this when associativity is broken)
- BUG-04-057 soundness analysis (currently just an `#[ignore]` test — insufficient for a fundamental soundness issue)
- Generator skew analysis (uniform raw->canonicalize may underrepresent Rule 3/4/6/8 trigger states)

**Critical invariant:** `AimsState::SCALAR` must be excluded from all lattice-law tests. It is a sentinel, not a lattice element — joining with `SCALAR` is undefined behavior in the lattice algebra.

**Partial order definition:** `ShapeClass` and `EffectClass` lack `PartialOrd`. For lattice-law tests, define `a <= b` as `a.join(b) == b` (the standard lattice-theoretic definition). This avoids adding `PartialOrd` to types where it's not natural (flat lattice `ShapeClass`, bitfield `EffectClass`).

**Cross-section dependency — BLOCKER for Sections 05 and 06:**
- **Section 05 (Contract Coherence Oracle)** depends on lattice properties for contract stability. If BUG-04-057 means the analysis is order-dependent, inferred contracts may also be structurally unstable — the contract oracle would compare against a non-deterministic baseline. Section 05 MUST NOT proceed until 04.6 (BUG-04-057 soundness analysis) is complete and either: (a) the bug is fixed, or (b) a formal proof demonstrates that the analysis is sound despite non-associativity.
- **Section 06 (Protocol Builtins)** depends on correct lattice properties for RC balance reasoning. Non-protocol runtime contracts (`ori_list_push`, `ori_iter_*` consumers) have the same ownership risk but are not verified in Section 06's current scope — this is a coverage gap that should be cross-referenced.

**Reference implementations:**
- **Lean4** `src/Lean/Compiler/IR/Checker.lean`: algebraic property verification on IR lattice — checks that join is sound w.r.t. the semantics.
- **proptest** (`proptest-rs/proptest`): `prop_compose!` and `Strategy` for building complex type generators from simple dimension strategies.

**Depends on:** Nothing — independent of other sections. The lattice module is self-contained.

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
          // Rule 6 trigger zone: Unique + non-Owned or non-local
          (consumption_strategy(), cardinality_strategy(), effect_class_strategy(), shape_class_strategy())
              .prop_map(|(c, card, eff, shape)| AimsState {
                  access: AccessClass::Borrowed,
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

Audit the existing join law tests and add missing partial-order axiom tests. The partial order `lattice_leq` is used pervasively in subsequent tests — its own axioms MUST be verified first.

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
          b in canonical_aims_state_strategy(),
          c in canonical_aims_state_strategy(),
      ) {
          prop_assume!(lattice_leq(&a, &b) && lattice_leq(&b, &c));
              assert!(
                  lattice_leq(&a, &c),
                  "lattice_leq must be transitive: a={a:?}, b={b:?}, c={c:?}"
              );
      }
  }
  ```
  Note: use `prop_assume!` (not `if` guards) so proptest counts rejected cases as discards and generates more cases to compensate. Configure with 5000 cases for adequate hitting rate.

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
          b in canonical_aims_state_strategy(),
          closure in canonical_aims_state_strategy(),
      ) {
          use crate::aims::transfer::capture_state_update;
          prop_assume!(lattice_leq(&a, &b));
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

- [ ] Add `capture_state_update` monotonicity test (second argument — closure state):
  ```rust
  proptest! {
      #![proptest_config(ProptestConfig::with_cases(5000))]
      #[test]
      fn capture_state_update_monotone_in_closure(
          current in canonical_aims_state_strategy(),
          c1 in canonical_aims_state_strategy(),
          c2 in canonical_aims_state_strategy(),
      ) {
          use crate::aims::transfer::capture_state_update;
          prop_assume!(lattice_leq(&c1, &c2));
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
          // Fold left-to-right
          let mut forward = AimsState::BOTTOM;
          forward.canonicalize();
          for s in &states {
              forward = forward.join(s);
          }

          // Fold right-to-left
          let mut backward = AimsState::BOTTOM;
          backward.canonicalize();
          for s in states.iter().rev() {
              backward = backward.join(s);
          }

          assert_eq!(
              forward, backward,
              "n-ary join must be permutation-invariant: \
               forward={forward:?}, backward={backward:?}, states={states:?}"
          );
      }
  }
  ```
  **Important:** If this test FAILS, it confirms that BUG-04-057 has real consequences for the analysis — the fixpoint result depends on worklist iteration order. This would escalate BUG-04-057 from "high" to "critical" and block Sections 05/06.

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
              let mut acc = AimsState::BOTTOM;
              acc.canonicalize();
              for s in order {
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

### New test — `seq_add` distributivity over `join` at AimsState level:

The `dimensions.rs` doc claims `seq_add` distributes over `alt_join` for `Cardinality`, and this is exhaustively tested in `tests.rs` for the `Cardinality` dimension alone. However, there is no proptest verifying this at the full `AimsState` level where cross-dimension canonicalization rules interact. Since `seq_add` only applies to `Cardinality` (other dimensions use `join` for both sequential and alternative composition), this test focuses on the `Cardinality` dimension but within full `AimsState` context to catch canonicalization interference.

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
  Note: This duplicates the exhaustive test in `tests.rs` but adds proptest shrinking for better counterexample reporting if the property ever breaks.

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

- [ ] Enumerate the state space where associativity fails — characterize ALL counterexamples, not just the one in the bug tracker:
  ```rust
  /// Find all (a, b, c) triples where associativity fails.
  /// First, compute the count of canonical states. If <= 500, use exhaustive
  /// enumeration over all triples instead of proptest sampling. If > 500,
  /// run with proptest cases=100_000 to maximize coverage.
  proptest! {
      #![proptest_config(ProptestConfig::with_cases(100_000))]
      #[test]
      fn characterize_associativity_failures(
          a in canonical_aims_state_strategy(),
          b in canonical_aims_state_strategy(),
          c in canonical_aims_state_strategy(),
      ) {
          let ab_c = a.join(&b).join(&c);
          let a_bc = a.join(&b.join(&c));
          if ab_c != a_bc {
              // Log the diverging dimensions for analysis
              // (proptest will shrink to minimal counterexamples)
              let diverges_uniqueness = ab_c.uniqueness != a_bc.uniqueness;
              let diverges_other = ab_c.access != a_bc.access
                  || ab_c.consumption != a_bc.consumption
                  || ab_c.cardinality != a_bc.cardinality
                  || ab_c.locality != a_bc.locality
                  || ab_c.shape != a_bc.shape
                  || ab_c.effect != a_bc.effect;
              // If non-uniqueness dimensions diverge, this is worse than expected
              assert!(
                  !diverges_other,
                  "associativity failure in non-uniqueness dimension — \
                   escalate BUG-04-057: ab_c={ab_c:?}, a_bc={a_bc:?}"
              );
              // For uniqueness-only divergence, verify neither result
              // influences RC decisions differently
              assert_eq!(
                  ab_c.is_rc_needed(), a_bc.is_rc_needed(),
                  "associativity failure changes RC decision — \
                   BUG-04-057 is unsound: ab_c={ab_c:?}, a_bc={a_bc:?}"
              );
              assert_eq!(
                  ab_c.needs_cow_check(), a_bc.needs_cow_check(),
                  "associativity failure changes COW decision — \
                   BUG-04-057 is unsound: ab_c={ab_c:?}, a_bc={a_bc:?}"
              );
              assert_eq!(
                  ab_c.is_reuse_candidate(), a_bc.is_reuse_candidate(),
                  "associativity failure changes reuse decision — \
                   BUG-04-057 is unsound: ab_c={ab_c:?}, a_bc={a_bc:?}"
              );
          }
      }
  }
  ```

- [ ] If the characterization test passes (all associativity failures are uniqueness-only AND do not affect RC/COW/reuse decisions), document the formal argument in a code comment on `join()` and update BUG-04-057 severity from `high` to `low` with the formal justification.

### Option B: The bug affects RC decisions (fix required)

If the characterization test reveals that associativity failures CAN change RC/COW/reuse decisions, the bug MUST be fixed before Sections 05/06 proceed. The fix would be in `canonicalize_single_pass()` — the root cause is that Rule 4 (BlockLocal+Owned+<=Once+MaybeShared -> Unique) fires on different intermediate states depending on fold order.

- [ ] If fix is required: file via `/fix-bug BUG-04-057` for full plan-section rigor (root cause analysis, TDD matrix, implementation). The fix section file will be at `plans/bug-tracker/fix-BUG-04-057.md`.

### Option C: Permutation-invariance test (04.5) already fails

If the `nary_join_permutation_invariant` test from 04.5 FAILS, then BUG-04-057 has immediate practical consequences — the analysis result depends on predecessor ordering. This automatically escalates to Option B (fix required) and additionally requires auditing all `analyze_function()` call sites to ensure they use a canonical ordering.

- [ ] Based on 04.5 permutation-invariance test results, determine which option applies and execute accordingly.

- [ ] Update BUG-04-057 entry in `plans/bug-tracker/section-04-codegen-llvm.md` with the analysis results.

- [ ] If Option A (benign): remove `#[ignore]` from `join_associative` and replace with a targeted test that documents exactly WHY non-associativity is safe (e.g., "uniqueness divergence in Dead+Absent states does not affect RC decisions").

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

- None. (Previous TPR findings from autopilot session cleared during 2026-04-11 reset — untrusted work.)

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

### Partial-order axioms (NEW — 04.2)
- [ ] `lattice_leq` reflexivity: `lattice_leq(a, a)` for all canonical states
- [ ] `lattice_leq` antisymmetry: `leq(a,b) && leq(b,a)` implies `a == b`
- [ ] `lattice_leq` transitivity: `leq(a,b) && leq(b,c)` implies `leq(a,c)`

### Canonicalization properties (04.3)
- [ ] Canonicalization idempotence: `canonicalize(canonicalize(s)) == canonicalize(s)` — audited
- [ ] Canonicalization convergence within 3 rounds — audited
- [ ] Canonicalization per-dimension guarantees — audited
- [ ] Join output is canonical: `canonicalize(a.join(b)) == a.join(b)` — audited

### Transfer function properties (04.4)
- [ ] 8 decision predicate semantic contracts verified — audited
- [ ] `capture_state_update` produces canonical output — audited
- [ ] `capture_state_update` monotonicity in first argument (NEW)
- [ ] `capture_state_update` monotonicity in second argument (NEW)

### Fixpoint convergence and permutation invariance (04.5)
- [ ] Ascending chain convergence within height bound (15 steps) — audited
- [ ] TOP is a fixpoint — audited
- [ ] `Cardinality::seq_add` convergence within 2 steps — audited
- [ ] N-ary join permutation invariance — forward vs reverse (NEW)
- [ ] N-ary join shuffled permutations (NEW)
- [ ] `Cardinality::seq_add` distributivity over `alt_join` proptest (NEW)

### BUG-04-057 soundness analysis (NEW — 04.6)
- [ ] Associativity failure characterization: all counterexamples enumerated
- [ ] Impact analysis: determine if failures affect RC/COW/reuse decisions
- [ ] Resolution: Option A (benign — proof documented), Option B (fix via `/fix-bug`), or Option C (permutation test fails — escalate)
- [ ] BUG-04-057 bug tracker entry updated with analysis results
- [ ] Cross-section blocker resolved: Sections 05/06 unblocked (or blocked with documented reason)

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
