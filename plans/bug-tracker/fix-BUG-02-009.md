---
bug: "BUG-02-009"
title: "PC-2 violation: fold/rfold closure's accumulator param left as unbound Tag::Var"
severity: "high"
status: in-progress
goal: "fold/rfold closure's first parameter (accumulator) resolves to the accumulator type in all cases, not just when the body happens to constrain it"
success_criteria:
  - "fold closure with unused accumulator param resolves both param types"
  - "validate_body_types produces zero E2005 on valid fold/rfold programs"
  - "test-all.sh green with no regressions"
subsystem: "compiler/ori_types/src/infer/expr/calls/method_call.rs"
found: "2026-04-15"
source: "section-03 validator wiring (empty-container-typeck-phase-contract plan)"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-02-009 — fold/rfold closure's accumulator param left as unbound Tag::Var

**Status:** In Progress
**Severity:** High
**Goal:** The fold/rfold higher-order constraint handler must unify the closure's first parameter (accumulator) with `ret_ty` so that accumulator types resolve even when the closure body doesn't constrain them through operations.

**Success Criteria:**
- [ ] `items.iter().fold(initial: 0, op: (acc, _x) -> 42)` produces `Lambda (acc, _x) : (int, str) -> int` — no `$tN` vars
- [ ] `items.iter().fold(initial: 0, op: (_a, _b) -> 99)` produces `Lambda (_a, _b) : (int, int) -> int` — no `$tN` vars
- [ ] All existing fold/rfold tests continue to pass
- [ ] `validate_body_types` called on fold-using bodies produces zero E2005 on valid programs

**Context:** Surfaced by §03 validator wiring (empty-container-typeck-phase-contract plan). When `validate_body_types` is wired into the body-checking passes, programs using fold/rfold where the closure body doesn't use the accumulator parameter trigger false E2005 because the accumulator's type remains as unbound `Tag::Var`. This blocks Section 03 of the phase-contract plan.

---

## 1. Root Cause Analysis

- **Symptom**: Lambda parameter types remain as unbound `$tN` (Tag::Var) in the typed IR for fold/rfold closures when the body doesn't constrain the accumulator parameter through operations.
- **Proximate cause**: `unify_higher_order_constraints` in `method_call.rs` lines 228-248 unifies the fold closure's return type and second parameter but NOT the first parameter (accumulator) with `ret_ty`.
- **Root cause**: Missing unification line. The fold signature is `(Acc, T) -> Acc`. The code unifies:
  1. `ret_ty ↔ init_ty` (accumulator type = initial value) ✓
  2. `closure_ret ↔ ret_ty` (return = accumulator) ✓
  3. `second_param ↔ source_elem` (T = iterator element) ✓
  4. `first_param ↔ ret_ty` (first param = accumulator) **MISSING**
  When the closure body uses `acc` in a type-constraining way (e.g., `acc + 1`), the binary operator's default path unifies `acc` with the literal's type, masking the gap. When the body doesn't use `acc` (e.g., returns a constant), the accumulator type remains unresolved.
- **Blast radius**: Only affects fold/rfold closures where the first param is unused or not constrained by body operations. Does not affect map/filter/any/all/find/for_each (they unify their single closure param correctly). Blocks Section 03 validator wiring.
- **Affected files**:
  - `compiler/ori_types/src/infer/expr/calls/method_call.rs` — add first-param unification in fold/rfold case

---

## 1.5 Fix Consensus (via /tp-help)

Independent dual-source design review of the proposed fix approach.

- **Proposed approach (pre-consensus)**: Add `if let Some(&first_param) = params.first() { let _ = engine.unify().unify(first_param, ret_ty); }` in the fold/rfold case of `unify_higher_order_constraints`, after the existing `closure_ret ↔ ret_ty` unification.
- **tp-help run scratch dir**: `/tmp/ori-tpr-vFHQJyum`

### Round 1
- **Codex summary**: Confirmed the fix is locally correct and acceptable. `UnifyEngine::unify` resolves both sides before linking — no ordering hazard. Recommended placing the new unify after `ret_ty ≡ closure_ret` and before the `second_param ≡ source_elem` block. Flagged that `unify_closure_param_with_iterator_elem` should NOT be reused (wrong semantics for fold). Also noted broader BD-2 GAP (builtins don't use Check mode) as a separate concern.
- **Gemini summary**: Unavailable (HTTP 429 capacity error).
- **Agreement points**: Codex agrees the proposed fix is correct, minimal, and safe within the post-hoc pattern.
- **Disagreement points**: None — single-source only.
- **Independent code verification**: Verified `UnifyEngine::unify` at `compiler/ori_types/src/unify/mod.rs` resolves both sides before linking ✓. Verified `function_params()` returns a fresh `Vec<Idx>` — no borrow conflict ✓. Verified `unify_closure_param_with_iterator_elem` hardcodes "first param = source elem" semantics — wrong for fold ✓.
- **Outcome**: Agreement — proceed with proposed approach (single-source; gemini unavailable).

### Final agreed approach
Add `first_param ↔ ret_ty` unification in the fold/rfold case, placed immediately after `ret_ty ≡ closure_ret` and before the `second_param ≡ source_elem` block. Inline the unification (do not factor into a helper). This is the minimal correct fix within the existing post-hoc unification pattern.

---

## 2. TDD — Test Matrix

Write ALL tests BEFORE the fix. Verify they fail against current code.

### Exact failing case
- [ ] `fold_with_unused_accumulator_resolves_both_params` — `items.iter().fold(initial: 0, op: (acc, _x) -> 42)` — acc must resolve to int, not $tN

### Edge cases
- [ ] `fold_with_neither_param_used_resolves_types` — `items.iter().fold(initial: 0, op: (_a, _b) -> 99)` — both params must resolve
- [ ] `rfold_with_unused_accumulator_resolves_both_params` — same pattern with rfold

### Cross-type coverage
- [ ] `fold_with_str_elem_and_unused_acc` — `["a","b"].iter().fold(initial: 0, op: (_a, _x) -> 0)` — int acc, str elem
- [ ] `fold_with_int_elem_and_unused_acc` — `[1,2,3].iter().fold(initial: 0, op: (_a, _b) -> 0)` — int acc, int elem

### Semantic pin
- [ ] `fold_accumulator_type_matches_initial_value_type` — verifies acc param type == init type even when body doesn't use acc

### Negative pin
- [ ] Existing fold tests with `acc + x` continue to work (regression guard)

### Verify tests fail before fix
- [ ] All new tests that check for resolved param types fail against current code

---

## 2.5 Fix Plan TPR Findings

Plan TPR: Mandatory — high severity. Will run after TDD matrix tests are written.

---

## 3. Implementation

- [ ] In `compiler/ori_types/src/infer/expr/calls/method_call.rs`, in the `"fold" | "rfold"` arm of `unify_higher_order_constraints` (line ~237), after `let _ = engine.unify().unify(ret_ty, closure_ret);`, add:
  ```rust
  // fold/rfold closure is (Acc, T) -> Acc: first param is accumulator
  let params = engine.pool().function_params(resolved_closure);
  if let Some(&first_param) = params.first() {
      let _ = engine.unify().unify(first_param, ret_ty);
  }
  ```
  Note: `params` is already accessed later for `second_param`; restructure to share the access.

---

## R. Third Party Review Findings

{Initially empty — populated by the executor during Phase 5 completion checklist.}

---

## 4. Completion Checklist

Reviews MUST complete before bug closure.

- [ ] All new tests pass unchanged after fix (no test modifications needed)
- [ ] Matrix completeness verified
- [ ] Debug AND release builds pass
- [ ] Interpreter and LLVM produce identical results for all new tests
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks (if memory-touching)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p ori_types` green
- [ ] `/commit-push` — commit all changes before review
- [ ] Plan TPR (Phase 2.5) — pending
- [ ] `/tpr-review` (Phase 5) passed
- [ ] `/impl-hygiene-review` passed
- [ ] Capability regression gate — N/A (fix adds unification, doesn't disable any capability)
- [ ] `/improve-tooling` retrospective completed
- [ ] `/sync-claude` doc sync
- [ ] Bug entry updated `- [x]`
- [ ] Fix section status updated to `complete`
- [ ] Bug-tracker overview count updated
- [ ] Final `/commit-push`

**Exit Criteria:** `ORI_DUMP_AFTER_TYPECK=1 cargo run -- check` on a program containing `items.iter().fold(initial: 0, op: (_a, _b) -> 99)` shows `Lambda (_a, _b) : (int, int) -> int` with zero `$tN` vars. `validate_body_types` called on the body produces zero E2005 errors. `./test-all.sh` passes with no regressions.
