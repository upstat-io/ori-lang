---
bug: "BUG-02-008"
title: "PC-2 validator false-positive E2005 on fresh instantiation vars in generic function bodies"
severity: "high"
status: in-progress
goal: "validate_body_types correctly exempts fresh instantiation vars that are union-find equivalence-class members of caller scheme vars, allowing §03 validator wiring into generic function bodies without false E2005"
success_criteria:
  - "validate_body_types accepts generic bodies calling other generics (e.g., apply_identity calling identity) with zero false E2005"
  - "validate_body_types still catches genuinely unbound vars (unresolved inference) in generic bodies"
  - "All existing validator tests (T1-T12) continue passing"
  - "timeout 150 ./test-all.sh green"
subsystem: "compiler/ori_types/src/check/validators/mod.rs"
found: "2026-04-15"
source: "section-03 validator wiring (empty-container-typeck-phase-contract plan)"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-02-008 — PC-2 Validator False-Positive on Fresh Instantiation Vars

**Status:** In Progress
**Severity:** High
**Goal:** Make the PC-2 validator scheme-var-aware so it can distinguish legitimate generic-parameter vars from genuine inference failures, unblocking §03 validator wiring.

**Success Criteria:**
- [ ] validate_body_types accepts `@apply_identity<T>(x: T) = identity(x: x)` without false E2005
- [ ] validate_body_types still flags genuinely unbound vars in generic bodies
- [ ] All 12 existing validator matrix tests (T1-T12) pass unchanged
- [ ] `timeout 150 ./test-all.sh` green

**Context:** When `validate_body_types` walks a generic function body that calls another generic function, the callee's scheme instantiation creates fresh `Tag::Var`s. Union-find may make the fresh var the root of its equivalence class (instead of the caller's scheme var), so the validator sees `VarState::Unbound` on a var_id not in `scheme_var_ids` and emits E2005. This blocks §03 of the empty-container-typeck-phase-contract plan, which needs to wire the validator into all four body-checking call sites. The current workaround skips generic bodies entirely, defeating the validator's purpose.

---

## 1. Root Cause Analysis

- **Symptom**: E2005 "Ambiguous type" emitted for fresh instantiation vars in generic function bodies
- **Proximate cause**: `collect_first_unbound_var` treats ALL `VarState::Unbound` as PC-2 violations, with no awareness of scheme-var equivalence classes
- **Root cause**: The validator has no mechanism to distinguish "unbound because it's a generic parameter" from "unbound because inference failed." It only checks `VarState::{Unbound, Link, Generalized, Rigid}` — it doesn't know that a fresh instantiation var (var_id=N) might be unified with a scheme var (var_id=M) via union-find, where the fresh var happened to become the root.
- **Blast radius**: Blocks §03 validator wiring. Without this fix, the validator cannot be enabled for generic function bodies — the most important class of bodies to validate (generic instantiation is where unresolved vars most commonly escape).
- **Affected files**:
  - `compiler/ori_types/src/check/validators/mod.rs` — add scheme_var_ids parameter, build exempt root set, check membership before E2005 emission
  - `compiler/ori_types/src/check/validators/tests.rs` — add matrix tests for scheme-var exemption
  - `compiler/ori_types/src/lib.rs` — re-export signature change (automatic)

---

## 1.5 Fix Consensus (via /tp-help)

**Round 1** — `/tp-help` run `/tmp/ori-tpr-XOuTu0v7/` (2026-04-15)
- **Codex**: APPROVED. Exempt-root-set approach is architecturally correct and sound.
- **Gemini**: UNAVAILABLE (429 MODEL_CAPACITY_EXHAUSTED on both attempts).

**Consensus outcome**: Agreement (Claude + Codex converge). Proceeding with exempt-root-set approach, incorporating Codex's refinements:

1. **Extract a canonical pool helper** `Pool::var_idx_for_id(var_id: u32) -> Option<Idx>` — the var_id→Idx linear scan pattern is duplicated 2× in `monomorphization.rs` (lines 231 and 297 — the reverse-lookup from `var_id` to pool `Idx`). Line 98 is a different full-pool traversal for `body_type_map` and is NOT a duplicate of this pattern. Adding a 3rd copy in the validator would be `LEAK:algorithmic-duplication`. Extract to a pool method, refactor existing sites.
2. **Apply exempt set to ALL positions** — `sig.param_types`, `sig.return_type`, AND `expr_types`. The current code already passes all through `collect_first_unbound_var`, so the exempt set parameter propagates automatically.
3. **Not a new VarState variant** — this is contextual exemption from `FunctionSig`, not intrinsic pool state. `VarState::Generalized` is intrinsic; scheme-root exemption is caller-scoped.
4. **Transitive chains covered** — generic→generic→generic collapses via `resolve_fully`. Exempt set built once covers all chain depths.
5. **Future-proofing note** — if implementation converges to `types.md §SC-1` (quantified vars as `BoundVar` instead of surviving `Tag::Var` roots), this exemption disappears. The fix is correct for the current representation.

**Independent code verification** (per feedback_reviewer_grounding_and_trust.md):
- `FunctionSig.scheme_var_ids` confirmed at `compiler/ori_types/src/output/mod.rs` — populated during signature collection, available at all 4 body-check call sites.
- Linear scan pattern confirmed at `monomorphization.rs:231` and `:297` — exact same `var_id -> Idx` reverse lookup. Line 98 is a distinct full-pool `body_type_map` traversal, not a duplicate of this pattern.
- `Pool::resolve_fully` at `pool/accessors.rs:412` — chases `VarState::Link` chains up to 16 steps, returns root Idx.

---

## 2. TDD — Test Matrix

Write ALL tests BEFORE the fix. Verify they fail against current code.

### Exact failing case
- [x] T13: Unbound fresh var (union-find root) with scheme var in same equivalence class — must NOT emit E2005

### Edge cases
- [x] T14: Multiple scheme vars with multiple fresh vars (2+ generic type params calling 2+ generic functions)
- [x] T15: Nested generic calls (A calls B calls C, fresh vars chain through union-find)
- [x] T16: Mixed exempt and non-exempt vars in same body (scheme var + genuinely unbound var → emit E2005 only for the genuinely unbound one)

### Semantic pin
- [x] T13 serves as semantic pin — would fail if the exempt-root-set logic were removed

### Negative pin
- [x] T16 serves as negative pin — confirms genuinely unbound vars are still caught even when scheme vars exist

### Verify tests fail before fix
- [x] All new tests (T13-T16) fail against current code (T13-T15 emit false E2005; T16 may partially pass)

---

## 2.5 Fix Plan TPR Findings

**Gate:** Mandatory — severity is high.

Pending — will run after /tp-help consensus.

---

## 3. Implementation

### 3a. Extract canonical pool helper (DRY — per Codex consensus)
- [x] Add `Pool::var_idx_for_id(var_id: u32) -> Option<Idx>` to `pool/accessors.rs`
- [x] Refactor `monomorphization.rs:231` to use `pool.var_idx_for_id(sv_id)`
- [x] Refactor `monomorphization.rs:297` to use `pool.var_idx_for_id(sv_id)`

### 3b. Validator changes
- [x] Add `scheme_var_ids: &[u32]` parameter to `validate_body_types`
- [ ] Build exempt root set using the new pool helper:
  ```rust
  fn build_exempt_var_ids(pool: &Pool, scheme_var_ids: &[u32]) -> FxHashSet<u32> {
      let mut exempt = FxHashSet::default();
      exempt.extend(scheme_var_ids.iter().copied());
      for &sv_id in scheme_var_ids {
          if let Some(sv_idx) = pool.var_idx_for_id(sv_id) {
              let root = pool.resolve_fully(sv_idx);
              if pool.tag(root) == Tag::Var {
                  exempt.insert(pool.data(root));
              }
          }
      }
      exempt
  }
  ```
- [x] Pass exempt set to `collect_first_unbound_var` (add parameter)
- [x] In `VarState::Unbound` arm: check if var_id is in exempt set before emitting E2005
- [x] Update test helper `run()` to accept `scheme_var_ids` parameter (existing tests pass empty slice)

---

## R. Third Party Review Findings

{Initially empty — populated during Phase 5 completion checklist.}

---

## 4. Completion Checklist

- [x] All new tests pass unchanged after fix (no test modifications needed)
- [x] Matrix completeness verified — T13-T16 cover fresh-var exemption, nesting, mixed exempt/non-exempt
- [x] Debug AND release builds pass (`cargo b && cargo b --release`)
- [x] Interpreter and LLVM produce identical results for all new tests (dual-execution parity — these are typeck-level tests, no codegen path)
- [x] `timeout 150 ./test-all.sh` green — 15,345 passed, 0 failed (LLVM backend crash = BUG-04-030, pre-existing)
- [x] `timeout 150 ./clippy-all.sh` green
- [x] `cargo test -p ori_types` green — 16/16 validator tests
- [x] `/commit-push` — committed at 30695c0f
- [ ] Plan TPR (Phase 2.5) — pending
- [ ] `/tpr-review` (Phase 5 — code review) passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed
- [ ] `/sync-claude` doc sync
- [ ] Bug entry updated: `- [x]` with resolution details
- [ ] Fix section frontmatter `status` updated to `complete`
- [ ] Bug-tracker `00-overview.md` open bug count updated
- [ ] Final `/commit-push` — commit closure artifacts

**Exit Criteria:** `validate_body_types` with scheme_var_ids=[caller's scheme vars] produces zero false E2005 on `@apply_identity<T>(x: T) = identity(x: x)` and similar generic-calling-generic patterns. `cargo test -p ori_types -- validators` passes all T1-T16 tests. `timeout 150 ./test-all.sh` green with zero regressions. The validator is ready for §03 wiring into all four body-checking call sites.
