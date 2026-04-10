---
bug: "BUG-04-051"
title: "AIMS dead_cleanup source-1 dedup conflates distinct phi-merged block params with the same lineage source set"
severity: "high"
status: complete
goal: "Source-1 dead-entry drops correctly emit separate RcDec for distinct phi-merged block params that share a lineage source set but hold different runtime values"
success_criteria:
  - "Swapped phi-merge params each get their own RcDec (no leak)"
  - "Let-alias chains still dedup to a single RcDec (no double-free)"
  - "All 16,922+ existing tests pass unchanged"
subsystem: "compiler/ori_arc/src/aims/emit_rc/"
found: "2026-04-07"
source: "tpr-review"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-04-051 — AIMS dead_cleanup source-1 dedup conflates distinct phi params

**Status:** Complete
**Severity:** high
**Goal:** Source-1 dead-entry drops dedup by true SSA equivalence (Let-alias chains), not by lineage source set. Two distinct phi-merged block params with the same source set get separate RcDecs.

**Success Criteria:**
- [ ] Swapped-phi params each get an RcDec at their bypass-safe entry (semantic pin)
- [ ] Let-alias chains produce exactly one RcDec (negative pin — no double-free)
- [ ] Project instructions do NOT create Let-alias relationships (boundary pin)
- [ ] Duplicate Jump args interact correctly with BUG-04-047 (interaction pin)
- [ ] All 16,922+ existing tests pass unchanged

**Context:** TPR-07-022 surfaced during repr-opt §07 Codex review. The lineage table in `take_project/mod.rs` keys on `Vec<usize>` (sorted source set), so two variables with source set `{0, 1}` share one `LineageInfo` and one lineage index. Source-1 in `dead_cleanup.rs` dedupes by lineage index, conflating "could be either source" (lineage) with "is the same SSA value" (alias). BUG-04-047's fix (duplicate terminator RcInc) was previously masked by this dedup — it must remain fixed before this change lands.

---

## 1. Root Cause Analysis

- **Symptom**: Latent memory leak — one enum value never dropped when two take-project sources flow through swapped phi-merge params.
- **Proximate cause**: `dead_cleanup.rs:140-146` — `lineages_dec_emitted: FxHashSet<usize>` dedupes by lineage index. Two distinct block params with the same lineage index share the dedup key.
- **Root cause**: The lineage table keys on `Vec<usize>` (sorted source set), so `{0, 1}` maps to one `LineageInfo` regardless of which variable has it. The dedup conflates "same potential source set" with "same runtime value." Let aliases ARE SSA-equivalent; phi params are NOT.
- **Blast radius**: Localized to source-1 drops in `emit_dead_at_entry_decs`. Bypass-safe entry computation (which uses lineage for reachability) is unaffected — that's about block selection, not value identity.
- **Affected files**:
  - `compiler/ori_arc/src/aims/emit_rc/take_project/mod.rs` — add `let_alias_rep` computation and API
  - `compiler/ori_arc/src/aims/emit_rc/dead_cleanup.rs` — change dedup key from lineage index to Let-alias representative
  - `compiler/ori_arc/src/aims/emit_rc/take_project/tests.rs` — add unit tests

---

## 1.5 Fix Consensus (via /tp-help)

- **Proposed approach (pre-consensus)**: Replace lineage-index dedup with Let-alias-representative dedup. Build a separate union-find over Let edges only. Store `let_alias_rep: FxHashMap<ArcVarId, ArcVarId>` in `TakeMoveFacts`.
- **tp-help run scratch dir**: `/tmp/ori-tpr-VRm1eBEj`

### Round 1
- **Codex summary**: Confirmed approach correct. Identified `LEAK:scattered-knowledge` in current code. Key refinements: (1) factor a canonical `collect_let_edges` helper for SSOT, (2) `let_alias_rep()` should be total for in-class vars (return `Some(self)` for singletons to avoid LEAK:inline-policy), (3) DRIFT cleanup needed on stale comments, (4) BUG-04-047 interaction is a dependency that appears satisfied.
- **Gemini summary**: Confirmed approach is "the correct architectural choice" — aligns dedup with SSA value semantics. Key refinements: (1) extract shared `collect_let_edges` helper, (2) add boundary pin for Project instruction (must NOT create Let-alias relationship), (3) no GVN-style dedup for non-Let pairs (that's the optimizer's job).
- **Agreement points**: Approach correct, need Let-edge extraction helper, BUG-04-047 interaction safe, test matrix must include swapped-phi semantic pin + Let-chain negative pin.
- **Independent code verification**: `dead_cleanup.rs:140-146` confirmed lineage_of dedup. `take_project/mod.rs:272` confirmed lineage table keyed on Vec<usize>. `build_alias_graph:414-445` confirmed Let=bidirectional, Jump=forward-only. `forward_walk.rs:66-86` confirmed BUG-04-047 fix (aggregated RcInc for duplicate terminator uses).
- **Outcome**: Agreement — proceed with refined approach.

### Final agreed approach
Replace lineage-index dedup with Let-alias-representative dedup:
1. Extract `collect_let_edges(func) -> Vec<(ArcVarId, ArcVarId)>` helper for SSOT.
2. Build Let-only union-find in `analyze()`. Store `let_alias_rep: FxHashMap<ArcVarId, ArcVarId>` in `TakeMoveFacts`.
3. `let_alias_rep()` is total for in-class vars (returns `Some(var)` for singletons).
4. Source-1 dedup: `let_reps_dec_emitted: FxHashSet<ArcVarId>` keyed on Let-alias representative.
5. Update stale comments referencing "same lineage => SSA-equivalent."
6. `build_alias_graph` consumes `collect_let_edges` for its Let edges (SSOT).

---

## 2. TDD — Test Matrix

Write ALL tests BEFORE the fix. Verify they fail against current code.

### Semantic pin
- [ ] `swapped_phi_params_with_same_lineage_get_separate_decs` — two block params receiving swapped sources, both emit RcDec

### Negative pin
- [ ] `let_alias_chain_emits_single_dec` — `let %1 = %0; let %2 = %1` all share one RcDec (no double-free)

### Boundary pin
- [ ] `project_does_not_create_let_alias` — `project %src[0]` into `%dst` → different Let-reps

### Interaction pin
- [ ] `duplicate_jump_args_to_distinct_params_not_deduped_by_let_rep` — `Jump(target, [v, v])` with two target params → both params are distinct Let-reps

### Verify tests fail before fix
- [ ] Semantic pin fails (swapped params get deduped to one RcDec due to shared lineage index)
- [ ] Other pins pass (they test existing correct behavior)

---

## 3. Implementation

- [ ] Extract `collect_let_edges(func: &ArcFunction) -> Vec<(ArcVarId, ArcVarId)>` in `take_project/mod.rs`
- [ ] Refactor `build_alias_graph` to consume `collect_let_edges` for its Let portion
- [ ] Add `compute_let_alias_reps(func, in_class) -> FxHashMap<ArcVarId, ArcVarId>` using union-find over Let edges only
- [ ] Add `let_alias_rep: FxHashMap<ArcVarId, ArcVarId>` field to `TakeMoveFacts`
- [ ] Add `pub(crate) fn let_alias_rep(&self, var: ArcVarId) -> Option<ArcVarId>` accessor (total for in-class vars)
- [ ] In `dead_cleanup.rs` source 1: replace `lineages_dec_emitted: FxHashSet<usize>` with `let_reps_dec_emitted: FxHashSet<ArcVarId>`, dedup by `let_alias_rep(var)`
- [ ] Update stale comments in `take_project/mod.rs:148-152` and `dead_cleanup.rs:57-72`

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix
- [ ] Matrix completeness verified
- [ ] Debug AND release builds pass (`cargo b && cargo b --release`)
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on affected test programs
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p ori_arc` green
- [ ] `/commit-push` — commit all changes before review
- [ ] Bug entry in `plans/bug-tracker/section-04-codegen-llvm.md` updated: `- [x]`
- [ ] Fix section frontmatter `status` updated to `complete`
- [ ] TPR-07-022 in `plans/repr-opt/section-07-enum-repr.md` marked `[x]` resolved
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed (AFTER TPR clean)
- [ ] `/improve-tooling` retrospective completed

**Exit Criteria:** `cargo test -p ori_arc take_project::tests` passes all unit tests including the new swapped-phi semantic pin. All 16,922+ tests pass via `./test-all.sh`. The `let_alias_rep`-based dedup correctly emits separate RcDecs for distinct phi params while deduping Let-alias chains. TPR re-review confirms TPR-07-022 is resolved.
