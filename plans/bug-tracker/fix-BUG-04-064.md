---
bug: "BUG-04-064"
title: "DP-5 can_mutate_in_place does not check overlapping borrows or project_alias_sources"
severity: "high"
status: in-progress
goal: "Unique COW sites correctly fall back to StaticShared when active borrows from the aggregate exist"
success_criteria:
  - "decide_cow() for Unique variables checks active borrows before returning StaticUnique"
  - "Semantic pin test: Unique aggregate with live borrow returns StaticShared, not StaticUnique"
  - "Negative pin test: Unique aggregate with NO borrows still returns StaticUnique"
  - "All existing tests pass unchanged — no regressions"
subsystem: "compiler/ori_arc/src/aims/realize/decide.rs, compiler/ori_arc/src/aims/emit_rc/cow.rs"
found: "2026-04-12"
source: "tpr-review"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-04-064 — DP-5 can_mutate_in_place does not check overlapping borrows

**Status:** In Progress
**Severity:** high
**Goal:** Unique COW sites correctly fall back to StaticShared when active borrows from the aggregate exist, per spec DP-5 + DP-9.

**Success Criteria:**
- [ ] `decide_cow()` for Unique variables checks active borrows before returning StaticUnique
- [ ] Semantic pin: Unique aggregate with live borrow → StaticShared
- [ ] Negative pin: Unique aggregate with no borrows → StaticUnique (preserved)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green

**Context:** Spec DP-5 defines `can_mutate_in_place(s, var, field, point) ⟺ s.access = Owned ∧ s.uniqueness = Unique ∧ no_active_overlapping_borrows(var, field, point)`. Spec DP-9 says `Unique AND NOT can_mutate_in_place → StaticShared`. The current `decide_cow()` unconditionally returns `StaticUnique` for all Unique variables without any borrow overlap check. This means in-place mutation can corrupt active borrowed projections. Found by dual-source tpr-review (both codex and gemini flagged).

---

## 1. Root Cause Analysis

- **Symptom**: Unique variables at COW sites get `StaticUnique` annotation regardless of whether active borrows from the aggregate exist
- **Proximate cause**: `decide_cow()` at `realize/decide.rs:403-404` has `Uniqueness::Unique => CowMode::StaticUnique` with no borrow check
- **Root cause**: DP-5's borrow overlap check was never wired into the Unique path of `decide_cow()`. The `can_mutate_in_place()` function in `transfer/mod.rs:433` exists as a spec reference but is dead code (zero production callers). The borrow checking infrastructure (`is_borrow_disjoint_from_siblings` in `emit_rc/cow.rs`) is only used for the MaybeShared path, not the Unique path.
- **Blast radius**: Any COW method call on a Unique aggregate that has active borrowed projections could corrupt those borrows. Localized to `realize/decide.rs` + `realize/mod.rs` + `emit_rc/cow.rs`.
- **Affected files**:
  - `compiler/ori_arc/src/aims/realize/decide.rs` — add borrow check to Unique path in `decide_cow()`
  - `compiler/ori_arc/src/aims/realize/mod.rs` — pass borrow info to `AnnotationSiteContext`
  - `compiler/ori_arc/src/aims/emit_rc/cow.rs` — add `has_active_borrows_from_aggregate()` helper

**Note on project_alias_sources**: Spec DP-5 also requires checking `project_alias_sources` for transitive aliases. This data is computed during intraprocedural analysis but NOT stored on `AimsStateMap` — it's used only for demand propagation then discarded. Storing it on the state map would be an architectural change beyond this point fix. This fix implements the `borrow_sources` check (direct borrows); the `project_alias_sources` gap is a separate precision improvement to track.

**Note on can_mutate_in_place()**: The function is dead code. This fix does NOT wire it up — the borrow check is added directly to `decide_cow()` via a new `has_active_borrows` context field. Whether to refactor `can_mutate_in_place` as the canonical DP-5 predicate is a separate cleanup.

---

## 1.5 Fix Consensus (via /tp-help)

Independent dual-source design review. Scratch dir: `/tmp/ori-tpr-g69iJOao`.

- **Proposed approach (pre-consensus)**: Block-level liveness check via `var_state_at_block_entry(block, borrow_var).consumption != Dead` for borrows from the aggregate.

### Round 1
- **Codex summary**: Agreed on direction (Unique must check borrows) but flagged three GAPs: (1) block-entry liveness is unsound — borrows defined mid-block are removed at definition and appear Dead at entry, (2) `BorrowSource::Unknown` from CFG join provenance merging is missed by `borrows_from_source()`, (3) LEAK — adding a second partial DP-5 helper alongside dead `can_mutate_in_place()`. Recommended site-local replay or pre-merge computation for instruction-level liveness.
- **Gemini summary**: Also flagged block-level liveness as "fatal soundness flaw" — same mechanism (backward analysis removes defined variables). Pushed harder: also flagged `project_alias_sources` omission as unsound (not conservative as claimed — missing aliases means false negatives in overlap check). Recommended wiring canonical `can_mutate_in_place()` predicate, and instruction-level liveness via backward replay.
- **Agreement points**: Both agree (1) block-entry liveness is unsound for mid-block borrows, (2) the fix direction is correct, (3) `can_mutate_in_place()` shouldn't remain dead code
- **Disagreement points**: Gemini says omitting `project_alias_sources` is actively unsound; Codex says it's a GAP but acceptable if scoped honestly. Both push for instruction-level liveness, which is architecturally expensive.
- **Independent code verification**:
  - Block-entry liveness unsoundness: VERIFIED at `state_map.rs:307` — backward analysis removes defined variables at definition site; a mid-block Project appears Dead at block entry even while live at a later instruction in the same block.
  - `BorrowSource::Unknown` join: VERIFIED at `lattice/tests.rs:1399` — different-source borrows join to Unknown, which `borrows_from_source()` (line 477-489) does NOT return.
  - `is_borrow_disjoint_from_siblings` also uses block-entry state: VERIFIED at `cow.rs:54` — same unsoundness in the MaybeShared path (separate bug to file).
- **Outcome**: Persuaded divergence — adopting a function-wide borrow existence check (no liveness parameter) instead of block-level liveness. This is maximally conservative but avoids the unsound block-entry approach entirely.

### Final agreed approach
Use **function-wide borrow existence check** — if ANY borrow from the aggregate exists ANYWHERE in the function (via `borrows_from_source(aggregate).next().is_some()`), return `StaticShared`. No block/instruction liveness parameter needed because the check is maximally conservative: if a borrow was ever taken from the aggregate, we assume it might be live at any COW site. This avoids the block-entry liveness unsoundness entirely. Precision improvement (instruction-level liveness for more precise dead-borrow filtering) tracked as future work. Follow-up bugs to file: (1) MaybeShared path in `is_borrow_disjoint_from_siblings` has the same block-entry liveness flaw, (2) `BorrowSource::Unknown` borrows are invisible to `borrows_from_source()`.

---

## 2. TDD — Test Matrix

Write ALL tests BEFORE the fix. Verify they fail against current code.

### Semantic pin (the core behavior change)
- [ ] `decide_cow_unique_with_borrow_from_same_source_returns_static_shared` — Unique aggregate with any borrow from same source → StaticShared (NOT StaticUnique)

### Negative pin (preserved behavior)
- [ ] `decide_cow_unique_without_borrows_returns_static_unique` — Unique aggregate with NO borrows → StaticUnique (unchanged)

### Edge cases
- [ ] `decide_cow_unique_with_borrow_from_different_source_returns_static_unique` — borrows exist but from a DIFFERENT source → StaticUnique (not affected)
- [ ] `decide_cow_unique_rc_incremented_with_no_borrows_returns_dynamic` — RC-incremented Unique → Dynamic (existing behavior preserved, RC guard fires first)
- [ ] `has_borrows_from_aggregate_empty_returns_false` — helper unit test: no borrows → false
- [ ] `has_borrows_from_aggregate_with_exact_borrow_returns_true` — helper unit test: Exact borrow from target → true

### Verify tests fail before fix
- [ ] The semantic pin test fails (returns StaticUnique instead of StaticShared)
- [ ] The negative pin and edge case tests pass (they test preserved behavior)

---

## 2.5 Fix Plan TPR Findings

**Gate:** Mandatory — complexity-elevated subsystem (AIMS)

Pending — will run after Phase 2 finalization.

---

## 3. Implementation

- [ ] Add `has_borrows_from_aggregate(state_map, aggregate) -> bool` helper to `emit_rc/cow.rs`:
  - Function-wide check: `state_map.borrows_from_source(aggregate).next().is_some()`
  - No block parameter, no liveness check — maximally conservative
  - Sound: if any borrow from the aggregate exists anywhere in the function, report true

- [ ] Add `has_active_borrows: bool` field to `AnnotationSiteContext` in `realize/decide.rs`

- [ ] Compute `has_active_borrows` in `realize/mod.rs` at both COW annotation sites (line ~311 and ~354):
  - `has_active_borrows: has_borrows_from_aggregate(ctx.state_map, var)`

- [ ] Modify `decide_cow()` in `realize/decide.rs`:
  ```rust
  Uniqueness::Unique => {
      if ctx.has_active_borrows {
          CowMode::StaticShared
      } else {
          CowMode::StaticUnique
      }
  }
  ```

- [ ] Update `can_mutate_in_place()` doc comment in `transfer/mod.rs` to clarify it is the lattice-subset check (Owned + Unique), not the full DP-5 predicate. Full DP-5 decision lives in `decide_cow()`.

- [ ] File follow-up bugs via `/add-bug`:
  1. `is_borrow_disjoint_from_siblings` uses block-entry state for source uniqueness — same unsound pattern
  2. `BorrowSource::Unknown` borrows invisible to `borrows_from_source()` — merged-provenance gap

---

## R. Third Party Review Findings

{Initially empty — populated during Phase 5.}

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix
- [ ] Matrix completeness verified
- [ ] Debug AND release builds pass
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p ori_arc` green
- [ ] `/commit-push` — commit all changes before review
- [ ] Plan TPR (Phase 2.5) — pending
- [ ] `/tpr-review` (Phase 5 — code review)
- [ ] `/impl-hygiene-review`
- [ ] Capability regression gate — N/A (fix adds a check, does not disable capability)
- [ ] `/improve-tooling` retrospective
- [ ] `/sync-claude` doc sync
- [ ] Bug entry updated
- [ ] Fix section status → complete
- [ ] Bug-tracker overview count updated
- [ ] Final `/commit-push`

**Exit Criteria:** `decide_cow()` returns `StaticShared` for Unique aggregates with live borrows, `StaticUnique` for Unique aggregates without, as proven by the semantic/negative pin tests. All 15,300+ tests pass with zero regressions. `cargo test -p ori_arc` specifically passes all lattice, transfer, and realization tests.
