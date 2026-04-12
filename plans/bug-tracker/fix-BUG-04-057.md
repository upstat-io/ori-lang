---
bug: "BUG-04-057"
title: "AIMS lattice join non-associative — canonicalization Rule 4 anti-monotone + Rule 6 locality narrowness"
severity: critical
status: in-progress
goal: "AIMS lattice join is associative, commutative, and idempotent on canonical states; lattice_leq is transitive; capture_state_update is monotone"
success_criteria:
  - "join_associative proptest passes (5000 cases, currently #[ignore])"
  - "lattice_leq_transitive proptest passes (5000 cases, currently #[ignore])"
  - "nary_join_permutation_invariant proptest passes (currently #[ignore])"
  - "capture_state_update_monotone_in_current passes (currently failing)"
  - "capture_state_update_monotone_in_closure passes (currently failing)"
  - "decision_divergence characterization test reflects fixed behavior (currently #[ignore])"
  - "All property tests pass: timeout 150 cargo test -p ori_arc -- lattice::prop_tests"
  - "timeout 150 ./test-all.sh green — no regressions"
subsystem: "compiler/ori_arc/src/aims/lattice/mod.rs"
found: "2026-04-11"
source: continue-roadmap
also_fixes:
  - "BUG-04-058"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-04-057 — AIMS Lattice Join Non-Associative (+ BUG-04-058)

**Status:** In Progress
**Severity:** Critical
**Also fixes:** BUG-04-058 (capture_state_update non-monotone — same Rule 6 narrowness class)
**Goal:** AIMS lattice join is associative on canonical states, lattice_leq is a valid partial order, and all transfer functions are monotone. The fixpoint analysis produces deterministic, order-independent results.

**Success Criteria:**
- [ ] join_associative proptest passes (5000 cases)
- [ ] lattice_leq_transitive proptest passes (5000 cases)
- [ ] nary_join_permutation_invariant proptest passes
- [ ] capture_state_update monotonicity tests pass
- [ ] All property tests green: `timeout 150 cargo test -p ori_arc -- lattice::prop_tests`
- [ ] Full test suite: `timeout 150 ./test-all.sh` green

**Context:** Discovered during plans/llvm-verification-tooling §04 (AIMS Lattice Property Verification). The proptest-based algebraic property tests revealed that `join(join(a,b), c) ≠ join(a, join(b,c))` for specific canonical state triples, and that `lattice_leq(a,b) && lattice_leq(b,c)` does not imply `lattice_leq(a,c)`. Root cause is anti-monotone canonicalization Rule 4 in `canonicalize_single_pass()`. BUG-04-058 shares the same root cause class (Rule 6 locality narrowness).

---

## 1. Root Cause Analysis

- **Symptom**: `join_associative` proptest fails — uniqueness dimension diverges depending on associative grouping. `lattice_leq_transitive` proptest fails — join-based partial order is non-transitive. `capture_state_update_monotone` fails — Rule 6 narrowness.
- **Proximate cause**: `canonicalize_single_pass()` Rule 4 promotes uniqueness DOWN (MaybeShared → Unique) at join points. Rule 6 uses exact `== HeapEscaping` instead of `>= HeapEscaping`.
- **Root cause**: Rule 4 is anti-monotone — it injects optimistic information (Unique) into the canonical form based on locality (BlockLocal), but this promotion fires only on intermediate join results where locality happens to be BlockLocal. When a subsequent join widens locality to FunctionLocal, the promotion is lost in one associative grouping but retained in another. Rule 6 fails to apply its constraint at Unknown locality (which subsumes HeapEscaping), making `capture_state_update` non-monotone.
- **Blast radius**: (1) Non-deterministic: n-ary join fold produces different results for different orderings (masked by deterministic reverse-postorder). (2) Join-based lattice_leq is NOT a valid partial order. (3) capture_state_update violates arc.md "Non-monotone transfer = unsound analysis". (4) Potentially unsound optimization: Rule 4 at join points can override MaybeShared evidence from predecessors, skipping needed COW checks.
- **Affected files**:
  - `compiler/ori_arc/src/aims/lattice/mod.rs:355-373` — Remove Rule 4, fix Rule 6 `==` → `>=`
  - `compiler/ori_arc/src/aims/lattice/mod.rs:144` — Fix misleading BOTTOM doc comment (ShapeClass::NonReusable is TOP not bottom)
  - `compiler/ori_arc/src/aims/intraprocedural/state_map.rs:427` — Update stale "only reachable through Rule 4" comment
  - `compiler/ori_arc/src/aims/lattice/tests.rs` — Update tests expecting Rule 4 promotion behavior
  - `compiler/ori_arc/src/aims/lattice/prop_tests.rs` — Un-ignore BUG-04-057/058 tests

**Reference implementations:**
- **Swift (SIL ARC)**: Joins only ever lose precision. Precision recovery done via explicit `is_unique` runtime barrier instructions, never by post-join canonicalization.
- **Koka (FBIP) / Lean 4**: Separate precision-gaining operations from standard dataflow transfer. Uniqueness proven strictly by forward ownership transfer and usage counts, never "guessed" from locality.

---

## 1.5 Fix Consensus (via /tp-help)

Independent dual-source design review of the proposed fix approach.

- **Proposed approach (pre-consensus)**: (1) Remove Rule 4 from canonicalize_single_pass() entirely — FRESH already starts with Unique, no transfer function needs it. (2) Change Rule 6 from `== HeapEscaping` to `>= HeapEscaping`.
- **tp-help run scratch dir**: `/tmp/ori-tpr-QRrlJQKP`

### Round 1
- **Codex summary**: Agrees Rule 4 removal is safe. Classified as `LEAK:validation-bypass`. Verified all transfer functions — none need Rule 4. Flagged DRIFT at state_map.rs:427 (stale comment) and specific test files needing updates. Noted `builtins/mod.rs:304` RETURN_UNIQUE uses Unknown+Unique but that's a contract fact not AimsState.
- **Gemini summary**: Agrees Rule 4 removal is "mathematically required". Provided formal anti-monotonicity proof for both Rule 4 and Rule 6. Confirmed via reference compilers (Swift, Koka, Lean4) — all avoid anti-monotone post-join promotions. Flagged ShapeClass BOTTOM doc comment as misleading.
- **Agreement points**: Both approve both fixes. Both independently verified no transfer function relies on Rule 4. Both confirmed Rule 6 `>=` is correct. Both say only Rule 4 is anti-monotone.
- **Disagreement points**: None.
- **Independent code verification**:
  - state_map.rs:427 "only reachable through Rule 4" — CONFIRMED stale (FRESH produces same combination)
  - mod.rs:144 "NonReusable is bottom" — CONFIRMED misleading (NonReusable is TOP of flat lattice)
  - builtins/mod.rs RETURN_UNIQUE — NOT checked (contract fact, separate from AimsState canonicalize)
  - All transfer functions verified by Claude before tp-help, confirmed by both reviewers independently
- **Outcome**: Agreement → proceed with original approach

### Final agreed approach
1. Remove Rule 4 from `canonicalize_single_pass()` entirely
2. Change Rule 6 condition from `== HeapEscaping` to `>= HeapEscaping`
3. Fix stale comments/docs (state_map.rs:427, mod.rs:144 BOTTOM doc)
4. Update tests expecting Rule 4 behavior
5. Un-ignore BUG-04-057/058 property tests

---

## 2. TDD — Test Matrix

The existing proptest infrastructure already provides the test matrix. The tests exist but are `#[ignore]` due to the known failures.

### Exact failing case (already exist as #[ignore])
- [ ] `join_associative` — proptest with 5000 canonical state triples (prop_tests.rs:309)
- [ ] `lattice_leq_transitive` — proptest with 5000 canonical state triples (prop_tests.rs:420)

### Permutation invariance (already exist as #[ignore])
- [ ] `nary_join_permutation_invariant` — all permutations of n states produce same fold-join (prop_tests.rs:838)
- [ ] `nary_join_permutation_invariant_shuffled` — shuffled variant (prop_tests.rs:869)

### Transfer function monotonicity
- [ ] `capture_state_update_monotone_in_current` — currently failing (prop_tests.rs:715)
- [ ] `capture_state_update_monotone_in_closure` — currently failing (prop_tests.rs:737)

### Decision divergence (already exists as #[ignore])
- [ ] `decision_divergence_characterization` — verify fix resolves decision divergence (prop_tests.rs:1043)

### Semantic pin (NEW — to be written)
- [ ] Rule 6 fires at Unknown locality: state with Unknown+Unique must canonicalize to Unknown+MaybeShared
- [ ] Join of BlockLocal+MaybeShared states does NOT promote to Unique (regression guard for removed Rule 4)

### Negative pin (NEW — to be written)
- [ ] Canonical state with Unknown+Unique must NOT exist after canonicalization (proves Rule 6 applied)

### Verify tests fail before fix
- [ ] All #[ignore] tests fail against current code (they do — that's why they're ignored)
- [ ] New semantic/negative pin tests fail against current code

---

## 3. Implementation

- [ ] **Remove Rule 4 from canonicalize_single_pass()** — delete lines 355-373 in `compiler/ori_arc/src/aims/lattice/mod.rs`
  ```rust
  // DELETE this block:
  // Rule 4 (Section 09.2/09.3): BlockLocal + Owned + ≤Once → Unique.
  // ... entire if block ...
  ```
- [ ] **Fix Rule 6: `==` → `>=`** — change line 350 from `== Locality::HeapEscaping` to `>= Locality::HeapEscaping`
  ```rust
  // BEFORE:
  if self.locality == Locality::HeapEscaping && self.uniqueness == Uniqueness::Unique {
  // AFTER:
  if self.locality >= Locality::HeapEscaping && self.uniqueness == Uniqueness::Unique {
  ```
- [ ] **Fix BOTTOM doc comment** — line 144: change "NonReusable is bottom" to "NonReusable is top (absorbing element of the flat ShapeClass join)"
- [ ] **Update state_map.rs:427** — remove stale "only reachable through Rule 4 promotion" comment
- [ ] **Update rule numbering in canonicalize_single_pass comments** — Rule 5 comment references Rule 4; adjust
- [ ] **Update lattice/tests.rs** — fix tests that expect Rule 4 promotion behavior
- [ ] **Un-ignore prop_tests.rs** — remove `#[ignore = "BUG-04-057: ..."]` from all 5 ignored tests
- [ ] **Write semantic/negative pin tests** — per TDD matrix above
- [ ] **Update Rule 6 comment** — reflect `>=` change in the doc comment

---

## R. Third Party Review Findings

{Initially empty — populated during Phase 5 completion checklist.}

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix
- [ ] Matrix completeness verified — all property tests cover the fix
- [ ] Debug AND release builds pass (`cargo b && cargo b --release`)
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `timeout 150 cargo test -p ori_arc` green
- [ ] `/commit-push` — commit all changes before review
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed (after TPR is clean)
- [ ] `/improve-tooling` retrospective completed
- [ ] Bug entry BUG-04-057 in section-04-codegen-llvm.md updated: `- [x]` with resolution
- [ ] Bug entry BUG-04-058 in section-04-codegen-llvm.md updated: `- [x]` with resolution
- [ ] Fix section frontmatter `status` updated to `complete`
- [ ] Bug-tracker `00-overview.md` Quick Reference open count updated
- [ ] Final `/commit-push` — commit closure artifacts

**Exit Criteria:** `timeout 150 cargo test -p ori_arc -- lattice::prop_tests` runs 22+ property tests (all previously-ignored tests now active) with 0 failures. `timeout 150 ./test-all.sh` reports 0 failures. The join operation is provably associative, commutative, and idempotent on canonical states. lattice_leq is a valid partial order. capture_state_update is monotone under componentwise ordering.
