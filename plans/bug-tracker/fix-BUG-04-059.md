---
bug: "BUG-04-059"
title: "AIMS realization uses unsound cross-dimensional uniqueness proofs (DP-10/RL-13 pattern)"
severity: "high"
status: complete
goal: "AIMS realization decisions use only the Uniqueness dimension (or IC-3 ParamContract) to determine RC==1 — never cross-dimensional consumption/cardinality inference"
success_criteria:
  - "MaybeShared + Once + ReusableCtor → DynamicReuse (not StaticReuse)"
  - "MaybeShared + Owned + Linear + Once param → Dynamic (not StaticUnique) unless IC-3 proves uniqueness"
  - "MaybeShared + CollectionBuffer + Once → Dynamic (not StaticUnique)"
  - "Borrow disjointness only accepts Uniqueness::Unique (not cross-dimensional)"
  - "All existing tests pass — no regressions"
subsystem: "compiler/ori_arc/src/aims/realize/decide.rs, compiler/ori_arc/src/aims/emit_reuse/detect.rs, compiler/ori_arc/src/aims/emit_rc/cow.rs, compiler/ori_arc/src/aims/realize/mod.rs, compiler/ori_arc/src/aims/realize/walk_dec.rs, compiler/ori_arc/src/aims/realize/metrics.rs, compiler/ori_arc/src/aims/realize/tests.rs"
found: "2026-04-12"
source: "tpr-review"
third_party_review:
  status: findings
  updated: "2026-04-14"
---

# Fix: BUG-04-059 — AIMS realization uses unsound cross-dimensional uniqueness proofs

**Status:** Complete (commit ad2b3134)
**Severity:** High
**Goal:** Remove all DP-10/RL-13 pattern usage from AIMS realization — uniqueness is established ONLY by the Uniqueness dimension directly or by IC-3 ParamContract from interprocedural analysis.

---

## 1. Root Cause Analysis

- **Symptom**: The AIMS realization layer makes `StaticUnique` and `StaticReuse` decisions based on cross-dimensional proofs that derive past uniqueness (RC==1) from future consumption guarantees (Linear, Once). The formal spec explicitly removed DP-10 and RL-13 as unsound.
- **Proximate cause**: 4 unsound decision sites + 1 helper + 3 downstream consumers of synergy counters:
  1. `decide_reuse()` at `decide.rs:278-290`: `MaybeShared + Once + ReusableCtor → StaticReuse`
  2. `decide_cow()` at `decide.rs:404-408`: `is_param && Owned + Linear + Once → StaticUnique`
  3. `decide_cow()` at `decide.rs:417-430`: `CollectionBuffer/ReusableCtor + Once → StaticUnique`
  4. `detect.rs:86-89`: `MaybeShared + Once + ReusableCtor → is_static_unique`
  5. `cow.rs:37 + 75-79`: `is_cow_aware_unique` used in borrow disjointness check
- **Root cause**: The code implements patterns that the spec removed as unsound. Backward analysis facts (consumption=Linear, cardinality=Once) are FUTURE guarantees ("value won't be duplicated"). They cannot prove PAST facts ("RC is currently 1"). A `MaybeShared` value used `Once` may have RC>1 from an earlier `RcInc` (e.g., stored into a data structure).
- **Blast radius**: Affects COW mutation decisions, allocation reuse decisions, and borrow disjointness checks. Could cause in-place mutation of shared data (use-after-free) or reuse of live memory.
- **Affected files** (expanded via Plan TPR Round 1 verification):
  - `compiler/ori_arc/src/aims/realize/decide.rs` — `decide_reuse()`, `decide_cow()`, `is_cow_aware_unique()`
  - `compiler/ori_arc/src/aims/emit_reuse/detect.rs` — `is_static` computation in `find_reuse_opportunities_from_events`
  - `compiler/ori_arc/src/aims/emit_rc/cow.rs` — `is_borrow_disjoint_from_siblings()`, `is_cow_aware_unique()`
  - `compiler/ori_arc/src/aims/realize/mod.rs` — `synergy.cow_upgrades += 1` at lines 325, 372 (live increments)
  - `compiler/ori_arc/src/aims/realize/walk_dec.rs` — `metrics.cross_dim_reuse += 1` at lines 164-186 (live increments)
  - `compiler/ori_arc/src/aims/realize/metrics.rs` — field definitions, `cross_dim_evidence_total()` helper
  - `compiler/ori_arc/src/aims/realize/tests.rs` — pinning tests + synergy-metric assertion tests (lines 90-125, 537-556, 1237-1309)

**Design note**: The spec's DP-9 defines ONE legitimate MaybeShared → StaticUnique upgrade path: when `IC-3 ParamContract.uniqueness = Unique` (all callers proved to pass unique arguments via SCC fixpoint). The current code does NOT implement this — it uses a different (unsound) heuristic. This fix removes the unsound heuristic. Implementing the correct IC-3 path is tracked as `[BUG-04-079]` (see §Capability regression tracking — blocked by BUG-04-069).

---

## 1.5 Fix Consensus (via /tp-help)

Independent dual-source design review. Run: `/tmp/ori-tpr-GsHU7MD3`

- **Proposed approach (pre-consensus)**: Remove all 4 unsound cross-dimensional paths. MaybeShared → always Dynamic/DynamicReuse. Do not implement IC-3 param path (separate enhancement).

### Round 1
- **Codex summary** (rc=0, 320s, 101 events — thorough): Agrees removal is sound. Found ADDITIONAL site: `tighten_uniqueness_from_callers()` in `interprocedural/mod.rs:101,285` implementing the same removed IC-8 pattern (already tracked as BUG-04-069). Confirms cow.rs borrow disjointness path is also unsound. Notes that `is_borrow_disjoint → StaticUnique` at `decide.rs:434` depends on partially-shipped RL-31 rules. Advises removing `cow_upgrades` + `cross_dim_reuse` synergy counters entirely (not zeroing). Key insight: the "legitimate IC-3 path" doesn't exist in the current code — `intraprocedural/block.rs:460` derives uniqueness from `may_share`/`access`, not `param_contract.uniqueness`.
- **Gemini**: Failed (rc=1, 251s, 13 events — API error). No usable response.
- **Independent code verification**: Confirmed `tighten_uniqueness_from_callers()` exists at `interprocedural/mod.rs:285` per codex's finding. Already tracked as BUG-04-069 — same unsoundness class, separate fix. Confirmed `is_borrow_disjoint` path at `decide.rs:434` uses `is_cow_aware_unique` indirectly through cow.rs.
- **Outcome**: Agreement (codex only — gemini failed). Scope stays at 4 realization sites + cow.rs. BUG-04-069 (interprocedural tightening) remains separate. Remove synergy counters entirely.

### Final agreed approach (Round 1)
1. Remove 4 unsound cross-dimensional paths in decide.rs, detect.rs, cow.rs
2. Remove both `is_cow_aware_unique()` helper functions
3. Remove `cow_upgrades` + `cross_dim_reuse` synergy counters from metrics.rs
4. Update all affected tests to assert correct conservative behavior
5. BUG-04-069 (interprocedural `tighten_uniqueness_from_callers`) stays separate

---

## 2. TDD — Test Matrix

(Matrix expanded after Plan TPR Round 1 — see §2.5 findings TPR-04-002-codex + TPR-04-003-codex + TPR-04-002-gemini.)

### §2.1 `decide_cow()` — MaybeShared × Shape × is_param × access × consumption matrix

**Semantic pins — MaybeShared stays Dynamic after unsound branches removed:**
- [ ] `decide_cow_maybe_shared_reusable_ctor_struct_once_returns_dynamic` — MaybeShared + ReusableCtor(Struct) + Once → Dynamic
- [ ] `decide_cow_maybe_shared_reusable_ctor_enum_variant_once_returns_dynamic` — MaybeShared + ReusableCtor(EnumVariant) + Once → Dynamic
- [ ] `decide_cow_maybe_shared_context_hole_once_returns_dynamic` — MaybeShared + ContextHole + Once → Dynamic
- [ ] `decide_cow_maybe_shared_collection_buffer_once_returns_dynamic` — MaybeShared + CollectionBuffer + Once → Dynamic
- [ ] `decide_cow_maybe_shared_param_owned_linear_once_returns_dynamic` — is_param=true + Owned + Linear + Once → Dynamic
- [ ] `decide_cow_maybe_shared_param_owned_affine_once_returns_dynamic` — is_param=true + Owned + Affine + Once → Dynamic (boundary: consumption≠Linear)
- [ ] `decide_cow_maybe_shared_nonparam_owned_linear_once_returns_dynamic` — is_param=false + Owned + Linear + Once → Dynamic (proves param-only helper no longer matters)

**Negative pins — reject the removed promotion paths:**
- [ ] `decide_cow_rejects_cross_dimensional_param_owned_linear_once_static_unique` — assert that `is_cow_aware_unique` pattern no longer returns StaticUnique
- [ ] `decide_cow_rejects_cross_dimensional_collection_buffer_once_static_unique` — assert CollectionBuffer+Once no longer returns StaticUnique
- [ ] `decide_cow_rejects_cross_dimensional_reusable_ctor_once_static_unique` — assert ReusableCtor+Once no longer returns StaticUnique

**Preserved-behavior pins (regression guards for sound paths):**
- [ ] `decide_cow_unique_returns_static_unique` — Unique → StaticUnique (unchanged)
- [ ] `decide_cow_shared_returns_static_shared` — Shared → StaticShared (unchanged)
- [ ] `decide_cow_maybe_shared_no_conditions_returns_dynamic` — plain MaybeShared → Dynamic (unchanged baseline)

### §2.2 `decide_reuse()` — MaybeShared × Shape × Cardinality matrix

**Semantic pins:**
- [ ] `decide_reuse_maybe_shared_reusable_ctor_struct_once_returns_dynamic_reuse` — MaybeShared + ReusableCtor(Struct) + Once → DynamicReuse
- [ ] `decide_reuse_maybe_shared_reusable_ctor_enum_variant_once_returns_dynamic_reuse` — MaybeShared + ReusableCtor(EnumVariant) + Once → DynamicReuse
- [ ] `decide_reuse_maybe_shared_context_hole_once_returns_dynamic_reuse` — MaybeShared + ContextHole + Once → DynamicReuse
- [ ] `decide_reuse_maybe_shared_collection_buffer_once_returns_dynamic_reuse` — (already correct per existing test; preserved)

**Negative pin:**
- [ ] `decide_reuse_rejects_cross_dimensional_maybe_shared_once_ctor_static_reuse` — assert MaybeShared+Once+ReusableCtor no longer returns StaticReuse

**Preserved-behavior pins:**
- [ ] `decide_reuse_unique_reusable_ctor_returns_static_reuse` — Unique + ReusableCtor → StaticReuse (unchanged)
- [ ] `decide_reuse_unique_enum_variant_returns_static_reuse` — Unique + ReusableCtor(EnumVariant) → StaticReuse (existing, preserved)
- [ ] `decide_reuse_shared_returns_none` — Shared → None (unchanged)
- [ ] `decide_reuse_nonreusable_returns_none` — NonReusable shape → None (unchanged)

### §2.3 Borrow-disjoint regression — HELPER layer, not decide_cow()

Per TPR-04-003-codex, the actual regression surface introduced by removing `is_cow_aware_unique` is in `is_borrow_disjoint_from_siblings()` in `cow.rs:23-39` — not in `decide_cow()`. A pure `decide_cow()` test with `ctx.is_borrow_disjoint=true` is testing the WRONG layer because `decide.rs:432-435` still returns StaticUnique whenever that flag is set; the change is in how the flag gets populated upstream.

**Helper-layer tests (in `cow/tests.rs` or equivalent):**
- [ ] `is_borrow_disjoint_from_siblings_unique_source_disjoint_fields_returns_true` — Uniqueness::Unique source with disjoint field borrows → true (preserved positive — RL-31 sound case)
- [ ] `is_borrow_disjoint_from_siblings_maybe_shared_source_rejects_cross_dim_uniqueness` — MaybeShared source with Owned+Linear+Once → false (negative pin: rejects the removed `is_cow_aware_unique` path)
- [ ] `is_borrow_disjoint_from_siblings_unique_source_overlapping_field_returns_false` — Uniqueness::Unique source with SAME-field sibling borrow → false (unchanged — RL-10 disjointness requirement)
- [ ] `is_borrow_disjoint_from_siblings_unique_source_whole_object_sibling_returns_false` — Uniqueness::Unique source with whole-object (None field) sibling → false (unchanged)

**Integration test (end-to-end: receiver with disjoint borrow reaches `decide_cow()` with correct flag):**
- [ ] `decide_cow_maybe_shared_with_unique_source_disjoint_borrow_stays_static_unique` — receiver's `is_borrow_disjoint_from_siblings()` returns true (Unique source, disjoint field) → `is_borrow_disjoint=true` → `decide_cow` returns StaticUnique (preserved positive, per TPR-04-002-gemini)

### §2.4 Synergy-counter removal — TRIM tests, don't wholesale delete (Round 2 correction per TPR-04-002-codex-r2)

`realize/tests.rs:1237-1309` contains 5 synergy-metrics tests. Per the Round 2 Plan TPR finding (TPR-04-002-codex), only the test(s) whose entire behavior is tied to removed fields should be deleted wholesale. Mixed tests must be TRIMMED to preserve coverage of surviving fields. The `reuse_percent()` and `has_cross_dim_evidence()` helpers remain LIVE APIs after this fix (per `metrics.rs:63-90`); `reuse_decisions`, `total_rc_decisions`, `total_cow_decisions`, `natural_fip`, `canonicalize_cross_fires` remain as `SynergyMetrics` fields.

Per-test disposition:
- [ ] `synergy_metrics_default_is_zero` (tests.rs:1237-1247) — TRIM: remove `cow_upgrades` and `cross_dim_reuse` field assertions; keep assertions on surviving fields (`total_rc_decisions`, `reuse_decisions`, `total_cow_decisions`, `natural_fip`, `canonicalize_cross_fires`).
- [ ] `synergy_metrics_merge_additive` (tests.rs:1249-1277) — TRIM: remove `cow_upgrades` / `cross_dim_reuse` from the struct-literal initialization AND from merge-result assertions; keep the surviving-field behavior (tests `reuse_decisions`, `total_rc_decisions`, `total_cow_decisions`, `natural_fip`, `canonicalize_cross_fires` merge correctly).
- [ ] `synergy_metrics_reuse_percent` (tests.rs:1279-1288) — PRESERVE unchanged (uses only `reuse_decisions` + `total_rc_decisions`).
- [ ] `synergy_metrics_percent_zero_total` (tests.rs:1290-1294) — PRESERVE unchanged (uses only `Default::default()`).
- [ ] `synergy_metrics_cross_dim_evidence` (tests.rs:1296-1310) — TRIM (per TPR-04-001-codex-r3 + TPR-04-002-gemini-r3): the test initializes `canonicalize_cross_fires: 100` (SURVIVING field) alongside `cross_dim_reuse: 5` and `cow_upgrades: 3` (REMOVED). `cross_dim_evidence_total()` aggregates all three; `has_cross_dim_evidence()` checks total > 0. After counter removal, `cross_dim_evidence_total()` must be refactored to sum only surviving canonicalize-stage counters (currently just `canonicalize_cross_fires`). Trim the test to: (a) remove `cow_upgrades` and `cross_dim_reuse` from the struct initialization, (b) update assertion to `assert_eq!(m.cross_dim_evidence_total(), 100)` (only `canonicalize_cross_fires` remains), (c) keep `has_cross_dim_evidence()` assertion — still valid on trimmed input. This preserves coverage for BOTH surviving helpers (`cross_dim_evidence_total()` and `has_cross_dim_evidence()`).

### Verify tests fail before fix (bug-exposing semantic pins only)

Per TPR-04-002-codex-r4: only tests that EXERCISE a removed unsound path need to fail before the fix. Boundary cells and shape variants that aren't currently hit by the unsound code already pass against HEAD — they are preservation tests, not regression tests.

**Must fail against current HEAD (bug-exposing semantic + negative pins):**
- `decide_cow_maybe_shared_param_owned_linear_once_returns_dynamic` (today: StaticUnique via `is_cow_aware_unique`)
- `decide_cow_maybe_shared_collection_buffer_once_returns_dynamic` (today: StaticUnique via non-param CollectionBuffer branch)
- `decide_cow_maybe_shared_reusable_ctor_struct_once_returns_dynamic` (today: StaticUnique via non-param ReusableCtor branch)
- `decide_cow_maybe_shared_reusable_ctor_enum_variant_once_returns_dynamic` (today: StaticUnique — `decide_cow()` uses `matches!(shape, ShapeClass::ReusableCtor(_))` which matches BOTH `ReusableCtor(Struct)` AND `ReusableCtor(EnumVariant)`; per TPR-04-001-codex-r5 verified against `decide.rs:425-429`)
- `decide_reuse_maybe_shared_reusable_ctor_struct_once_returns_dynamic_reuse` (today: StaticReuse via cross-dim branch)
- `decide_reuse_maybe_shared_reusable_ctor_enum_variant_once_returns_dynamic_reuse` (today: StaticReuse — `decide_reuse()` at `decide.rs:278-287` also uses `matches!(shape, ShapeClass::ReusableCtor(_))`, matches EnumVariant)
- All four `decide_cow_rejects_cross_dimensional_*` and `decide_reuse_rejects_cross_dimensional_*` negative pins (today: current code returns the removed StaticUnique/StaticReuse that the negative pins assert is NOT produced)
- `is_borrow_disjoint_from_siblings_maybe_shared_source_rejects_cross_dim_uniqueness` (today: returns true via `is_cow_aware_unique`)

**Already pass against current HEAD (boundary + preservation pins — verify they REMAIN passing after fix):**
- `decide_cow_maybe_shared_context_hole_once_returns_dynamic` — `ContextHole` is NOT a `ReusableCtor(_)` variant; `matches!(shape, ShapeClass::ReusableCtor(_))` does not match `ShapeClass::ContextHole`; already returns Dynamic → preservation
- `decide_cow_maybe_shared_param_owned_affine_once_returns_dynamic` — `is_cow_aware_unique` requires Linear, so Affine already bypasses → preservation
- `decide_cow_maybe_shared_nonparam_owned_linear_once_returns_dynamic` — `is_cow_aware_unique` is gated on `is_param=true`, so non-param already bypasses → preservation
- `is_borrow_disjoint_from_siblings_unique_source_disjoint_fields_returns_true` — Uniqueness::Unique source already passes the strict check → preservation
- `decide_reuse_maybe_shared_context_hole_once_returns_dynamic_reuse` — `ContextHole` is not a `ReusableCtor(_)` variant, already returns DynamicReuse → preservation
- `decide_cow_maybe_shared_with_unique_source_disjoint_borrow_stays_static_unique` (integration) — Unique source + disjoint borrow → StaticUnique today → preservation

---

## 2.5 Fix Plan TPR Findings

**Gate:** Mandatory — severity is high AND complexity-elevated subsystem (AIMS)

**Run:** `/tmp/ori-tpr-SoZI4xrX` (2026-04-14, custom-mode adversarial plan review)

### Round 1 — dual-source consensus

- **Codex** (rc=0, 733s, 223 events — thorough): 5 findings (1 concerning synergy-counter consumer scope, 1 TDD matrix expansion, 1 borrow-disjoint test layer placement, 1 test rename hygiene, 1 tracker formatting). Read ~20 files including all affected code sites plus `interprocedural/tests.rs`, `walk_dec.rs`, `realize/mod.rs`.
- **Gemini** (rc=0, 675s, 76 events): 5 findings (1 HIGH claiming BUG-04-069 launders the unsoundness; 1 MEDIUM on positive disjoint-borrow pin; 3 INFORMATIONAL confirmations). Gemini missed `walk_dec.rs` in the synergy-counter grep (incomplete verification).
- **Thoroughness**: ASYMMETRY: MODERATE (byte ratio 3.2x, event ratio 2.9x — codex was more thorough). Walltime ratio 1.1x — both invested similar time. Claude's thoroughness judgment: ACCEPTED — both reviewers read the grounding rules, both ran greps, both formed substantive findings. Codex's depth was notably higher but gemini's thinner depth did not manifest as skimming (refused to manufacture findings).

### Findings triage (independent verification per `feedback_reviewer_grounding_and_trust.md`)

- **[TPR-04-001-codex][medium]** `plans/bug-tracker/fix-BUG-04-059.md:115` — Synergy counter removal scope incomplete. **VERIFIED**: grep confirms `synergy.cow_upgrades += 1` at `realize/mod.rs:325, 372` and `metrics.cross_dim_reuse += 1` at `walk_dec.rs:164-186`. **Action**: expanded §3 step 7 from "remove or zero out" to "remove entirely" with explicit 4-file list.
- **[TPR-04-002-codex][medium]** `fix-BUG-04-059.md:73` — TDD matrix missing shape variants and boundary cells. **VERIFIED**: `aims-rules.md` §1.6 distinguishes `ReusableCtor(Struct)`, `ReusableCtor(EnumVariant)`, `CollectionBuffer`, `ContextHole`; existing tests only cover Struct + one CollectionBuffer cell. **Action**: §2.1/§2.2 expanded to cover all four shape variants plus `is_param + Owned + Affine + Once` boundary and `is_param=false + Owned + Linear + Once` proof-of-irrelevance.
- **[TPR-04-003-codex][medium]** `fix-BUG-04-059.md:78` — Borrow-disjoint test on wrong layer. **VERIFIED**: existing test at `realize/tests.rs:122-125` manually sets `ctx.is_borrow_disjoint = true` and tests `decide_cow()`. The change is in `is_borrow_disjoint_from_siblings()` which POPULATES the flag — a pure `decide_cow()` test doesn't cover the regression surface. **Action**: §2.3 replaces the direct decide_cow test with helper-layer tests on `is_borrow_disjoint_from_siblings()` plus an integration-layer preserved-positive test.
- **[TPR-04-004-codex][low]** `fix-BUG-04-059.md:114` — Rename stale unsound test names. **VERIFIED**: current tests `cow_param_cow_aware_unique`, `cow_cross_dim_collection_buffer_once`, `cow_cross_dim_reusable_ctor_once`, `decide_cross_dimensional_maybe_shared_once_ctor_is_static_reuse` describe removed behavior. Per `impl-hygiene.md` §Test Function Naming, names must describe CURRENT behavior. **Action**: §3 step 6 updated to require rename-or-replace for these four tests.
- **[TPR-04-005-codex][low]** `fix-BUG-04-059.md:121` — Tracker block formatting. **Round 1 triage: initially REJECTED as false positive (my inspection missed lines 594-597 of BUG-04-069's continuation text that had been orphaned by BUG-04-079 insertion). Round 2 CORRECTION**: verified by both Round 2 reviewers (codex TPR-04-001-codex-r2 + gemini TPR-04-001-gemini-r2) — BUG-04-069 DID have a multi-line body (Repro/Subsystem/Found/Note) that was orphaned below the newly-inserted BUG-04-079 entry at lines 594-597. Codex Round 1 was CORRECT. My Round 1 rejection was a triage error. **Action (applied Round 2)**: reordered entries so BUG-04-069 header+body appears as a contiguous bullet BEFORE BUG-04-079. Audit trail preserved: this entry is now reclassified from "rejected" to "ACCEPTED (applied Round 2)".
- **[TPR-04-001-gemini][high→medium]** `interprocedural/mod.rs:285` — BUG-04-069 launders the unsoundness. **PARTIALLY VERIFIED**: Gemini's claim that `tighten_uniqueness_from_callers` produces `ParamContract.uniqueness = Unique` via the unsound IC-8 pattern is correct. BUT codex's independent verification (TPR-04-005-codex) shows `AnnotationSiteContext` has NO `param_contract` field and `decide_cow()` does NOT read `ParamContract.uniqueness` today. The laundering path is DORMANT — it only activates when BUG-04-079 implements the ParamContract plumbing. **Severity downgrade**: high→medium (coupling is future-tense, not current). **Action**: §Capability regression tracking strengthened with explicit BUG-04-079 ⇒ BUG-04-069 dependency. BUG-04-079 tracker entry annotated `<!-- blocked-by:BUG-04-069 -->`. NO immediate fix to `tighten_uniqueness_from_callers` required for this bug.
- **[TPR-04-002-gemini][medium]** `fix-BUG-04-059.md:95` — Add positive pin for preserved disjoint borrow. **VERIFIED**: §2.2 pre-expansion had only the negative pin. **Action**: §2.3 integration test `decide_cow_maybe_shared_with_unique_source_disjoint_borrow_stays_static_unique` added (converges with codex's TPR-04-003).
- **[TPR-04-003-gemini][informational]** — Test rename recommendation. Duplicates TPR-04-004-codex. **Action**: same as TPR-04-004-codex (no separate action).
- **[TPR-04-004-gemini][informational]** — Preserved `is_borrow_disjoint` path soundness verified. No action.
- **[TPR-04-005-gemini][informational]** — Synergy-counter removal scope verification (INCOMPLETE per TPR-04-001-codex). **Action**: codex's more-complete finding supersedes; gemini's partial verification is noted but not relied upon.

### Round 1 outcome (corrected per Round 2 findings)

- **Accepted findings (7)**: TPR-04-001-codex, TPR-04-002-codex, TPR-04-003-codex, TPR-04-004-codex, **TPR-04-005-codex (reclassified from rejected to accepted after Round 2 reviewers confirmed the split was real)**, TPR-04-001-gemini (downgraded to medium), TPR-04-002-gemini
- **Rejected findings (0)**: none
- **Informational (3)**: TPR-04-003-gemini (duplicate), TPR-04-004-gemini, TPR-04-005-gemini (superseded)
- **Plan changes applied (Round 1)**: §1 (affected files expanded), §2.1/§2.2/§2.3/§2.4 (TDD matrix restructured), §3 step 6-7 (rename strategy + full 4-file counter removal), §Capability regression (BUG-04-069 coupling), BUG-04-079 entry (blocked-by annotation). **Round 1 triage error**: TPR-04-005-codex tracker-split was rejected but actually true; corrected in Round 2.

### Round 2 — dual-source re-verification

**Run:** `/tmp/ori-tpr-BeIbeYZ2` (2026-04-14, custom-mode verification of Round 1 fixes)

- **Codex** (rc=0, 286s, 71 events): 2 actionable + 1 informational. Flagged the Round 1 triage error (TPR-04-001-codex-r2: BUG-04-069 IS split at section-04-codegen-llvm.md:586/594-597) AND a new over-deletion concern in §3 step 7 (TPR-04-002-codex-r2: `synergy_metrics_reuse_percent` and `synergy_metrics_percent_zero_total` tests cover surviving `reuse_percent()` behavior — wholesale deletion discards valid coverage). Informational confirmation (TPR-04-003-codex-r2) that all other Round 1 changes landed correctly and `<!-- blocked-by:BUG-04-069 -->` format matches existing tracker conventions.
- **Gemini** (rc=0, 92s, 35 events): 1 actionable + 1 informational. Agreed (title differs but same issue) with codex's BUG-04-069-split finding. Informational confirmation of Round 1 findings.
- **Thoroughness**: ASYMMETRY: HIGH (3.1x walltime, 18x bytes — codex notably deeper). But gemini's thin review DID catch the BUG-04-069 split (agreement with codex's independent finding). Findings-present thin round ⇒ NOT wasted; counter reset; strengthening flag remains true for Round 3.

### Round 2 findings triage

- **[TPR-04-001-codex-r2][medium]** — Reclassify TPR-04-005-codex and fix the split. **VERIFIED**: reading section-04-codegen-llvm.md lines 586-597 confirms BUG-04-069's multi-line body (Repro/Subsystem/Found/Note at lines 594-597) sits AFTER the inserted BUG-04-079 block (lines 587-593). **Action**: reordered so BUG-04-069 header + body is a contiguous bullet before BUG-04-079. Round 1 TPR-04-005-codex triage reclassified to ACCEPTED (applied Round 2). This §2.5 section updated with corrected counts.
- **[TPR-04-002-codex-r2][medium]** — Synergy-metrics test deletion too aggressive. **VERIFIED**: reading tests.rs:1237-1310 confirms 5 synergy tests, of which 2 (`synergy_metrics_reuse_percent`, `synergy_metrics_percent_zero_total`) use ONLY surviving fields, 2 (`synergy_metrics_default_is_zero`, `synergy_metrics_merge_additive`) mix removable+surviving assertions, and 1 (`synergy_metrics_cross_dim_evidence`) is exclusively on removed `cross_dim_evidence_total()` behavior. **Action**: §2.4 rewritten with per-test disposition (trim vs preserve vs delete).
- **[TPR-04-001-gemini-r2][high]** — Agreement with TPR-04-001-codex-r2 on BUG-04-069 split. Same resolution.
- **[TPR-04-003-codex-r2][informational]** — Confirmation that other Round 1 changes landed correctly. No action.
- **[TPR-04-002-gemini-r2][informational]** — Confirmation all 6 Round 1 accepted findings remediated. No action.

Round 3 TPR re-verification pending after these Round 2 fixes land.

### Round 3 — dual-source re-verification with strengthened thoroughness directive

**Run:** `/tmp/ori-tpr-ia9vmUBG` (2026-04-14, custom-mode verification of Round 2 fixes with Thoroughness Re-review Directive)

- **Codex** (rc=0, 294s, 89 events): 2 actionable + 1 informational. Caught that §3 step 7 references a nonexistent `Display` impl (TPR-04-002-codex-r3) and that `has_cross_dim_evidence()` helper coverage would be lost if `synergy_metrics_cross_dim_evidence` is deleted outright (TPR-04-001-codex-r3).
- **Gemini** (rc=0, 216s, 69 events — 2.3x deeper than Round 2): 2 actionable + 1 informational. Wrote a Python script parsing all 8 bug-tracker section files for orphaned body patterns — found none outside BUG-04-047's benign duplicate. Caught §3 step 7 vs §2.4 contradiction (TPR-04-001-gemini-r3) and convergent over-deletion concern on `synergy_metrics_cross_dim_evidence` preserving `canonicalize_cross_fires` coverage (TPR-04-002-gemini-r3).
- **Thoroughness**: ASYMMETRY: MODERATE (walltime 1.4x, events 1.3x — notably improved from Round 2's 3.1x/2.0x; byte ratio 21x is just codex's verbosity). Strengthened directive worked — both reviewers ran diagnostics, both read rules, both populated `scope_actually_reviewed` fully. Thoroughness judgment: ACCEPTED.

### Round 3 findings triage

- **[TPR-04-001-codex-r3][medium]** + **[TPR-04-002-gemini-r3][medium]** — `synergy_metrics_cross_dim_evidence` over-deletion (converge). **VERIFIED**: test at tests.rs:1296-1310 mixes removed fields (`cow_upgrades: 3`, `cross_dim_reuse: 5`) with surviving field (`canonicalize_cross_fires: 100`) and asserts on both `cross_dim_evidence_total()` (returns 108, aggregates all 3) and `has_cross_dim_evidence()` (returns true when total > 0). After removal, `cross_dim_evidence_total()` must be refactored to sum only `canonicalize_cross_fires`; trimmed test then asserts 100 instead of 108. **Action**: §2.4 disposition changed from "DELETE entirely" to "TRIM" with explicit trimmed-assertion shape.
- **[TPR-04-002-codex-r3][low]** — §3 step 7 references nonexistent `Display` impl. **VERIFIED**: `rg 'impl .*Display.*SynergyMetrics' compiler/ori_arc/src/aims/realize/metrics.rs` returns no matches. Actual surfaces are doc comments (lines 16-20, 76-81) and `report()` method (lines 107-123) which still logs removed fields via structured `tracing` fields. **Action**: §3 step 7 rewritten with the accurate surfaces list.
- **[TPR-04-001-gemini-r3][medium]** — §3 step 7 contradicts §2.4. **VERIFIED**: old step 7 said "DELETE all tests... (lines 1242-1309)" which conflicts with §2.4's per-test disposition. **Action**: §3 step 7 now references §2.4's precise guidance explicitly, no inline DELETE instruction.
- **[TPR-04-003-codex-r3][informational]** + **[TPR-04-003-gemini-r3][informational]** — Both reviewers confirm Round 2 fixes (BUG-04-069 split repair, audit-trail counts, §4 completion checklist) are consistent and no other orphan patterns exist in bug-tracker. **No action**.

Round 4 TPR re-verification pending after Round 3 fixes land.

### Round 4 — dual-source re-verification

**Run:** `/tmp/ori-tpr-aJxlMKfO` (2026-04-14, verification of Round 3 fixes)

- **Codex** (rc=0, 224s, 75 events): 2 actionable + 0 informational. Caught §3 missing explicit step for §2.3 helper-layer test home creation (TPR-04-001-codex-r4) and boundary-cell classification drift in §2 "Verify tests fail before fix" (TPR-04-002-codex-r4).
- **Gemini** (rc=0, 86s, 33 events): 0 actionable + 1 informational. Reports plan verified clean and ready for Phase 3 Implementation.
- **Thoroughness**: ASYMMETRY: MODERATE (walltime 2.6x, events 2.3x — gemini thinner). Gemini missed the findings codex caught; codex's investigation found real issues. Findings-present round is NOT wasted; counter reset; strengthening flag stays True because gemini thinness persists.

### Round 4 findings triage

- **[TPR-04-001-codex-r4][medium]** — §3 missing explicit step for §2.3 helper-layer tests. **VERIFIED**: `rg '#\[cfg\(test\)\]|mod tests;' compiler/ori_arc/src/aims/emit_rc/cow.rs` returns nothing. §3 step 5 only removes logic from cow.rs; §3 step 6 discusses only tests in realize/tests.rs. The §2.3 helper/integration tests have no implementation home specified. **Action**: §3 step 5a added — creates `compiler/ori_arc/src/aims/emit_rc/cow/tests.rs` + `#[cfg(test)] mod tests;` declaration + maps all §2.3 tests there. Legacy `cow_borrow_disjoint_maybe_shared` renamed to `decide_cow_maybe_shared_with_unique_source_disjoint_borrow_stays_static_unique` and moved to new home. §3 step 6 MOVE+RENAME note added.
- **[TPR-04-002-codex-r4][low]** — "All semantic pins fail before fix" assertion too strong. **VERIFIED**: inspecting current `decide_cow()` code shows `is_cow_aware_unique` requires `is_param + Linear`, so param+Affine already bypasses → Dynamic → test already passes. Similarly ContextHole isn't matched by `ReusableCtor(_)` patterns → already Dynamic. But `matches!(shape, ShapeClass::ReusableCtor(_))` DOES match EnumVariant → EnumVariant cells DO fail before fix (upgraded to bug-exposing). **Action**: §2 "Verify tests fail before fix" section split into two buckets: "must fail against HEAD" (bug-exposing) and "already pass against HEAD" (preservation). Each cell explicitly classified per current-code inspection.
- **[TPR-04-001-gemini-r4][informational]** — Plan verified clean. No action.

Round 5 TPR re-verification pending after Round 4 fixes land.

### Round 5 — dual-source re-verification

**Run:** `/tmp/ori-tpr-cRh7Vi3W` (2026-04-14, verification of Round 4 fixes)

- **Codex** (rc=0, 213s, 70 events): 1 medium finding (TPR-04-001-codex-r5: EnumVariant cells ambiguously placed with "upgrade to bug-exposing if verified" hedge; verified against code that EnumVariant DOES hit removed `ReusableCtor(_)` patterns in both `decide_cow()` and `decide_reuse()`).
- **Gemini** (rc=0, 233s, 26 events): 1 informational finding confirming plan clean. Note: gemini's envelope used `description` instead of `title` field (schema-nonconformant), which caused `merge-findings.py` to crash — content was still informational-only.
- **Thoroughness**: ASYMMETRY: MODERATE (walltime 1.1x — balanced, events 2.7x — codex more events, bytes 30.7x — codex verbose). Both reviewers invested substantial time. Depth sufficient.

### Round 5 findings triage

- **[TPR-04-001-codex-r5][medium]** — EnumVariant cells need unambiguous placement. **VERIFIED**: `decide_cow()` at `decide.rs:425-429` and `decide_reuse()` at `decide.rs:278-287` both use `matches!(shape, ShapeClass::ReusableCtor(_))`, which matches `ReusableCtor(Struct)` and `ReusableCtor(EnumVariant)` but NOT `ContextHole` — per spec `aims-rules.md §1.6`, `ContextHole` is a distinct top-level `ShapeClass` variant, not nested inside `ReusableCtor(_)`. Therefore EnumVariant hits the removed unsound path; ContextHole does not. **Action**: moved both EnumVariant cells from "already pass" bucket to "must fail against HEAD" bucket with explicit code-location verification. Removed the "upgrade to bug-exposing if verified" hedge text. ContextHole remains in preservation bucket.

Round 6 TPR re-verification pending after Round 5 fix lands.

### Round 6 — dual-source re-verification

**Run:** `/tmp/ori-tpr-N9Z8Dc90` (2026-04-14, verification of Round 5 fix)

- **Codex** (rc=0, 165s, 57 events): 1 LOW finding (TPR-04-001-codex-r6: Round 5 triage note contained a self-correcting parenthetical — wording quality, not correctness). All substantive verifications passed.
- **Gemini** (rc=0, 62s, 23 events): **0 findings** (first fully clean gemini envelope) — all 4 Round 5 verification checks passed, no contradictions remain.
- **Thoroughness**: ASYMMETRY: MODERATE (walltime 2.7x, events 2.5x). Gemini faster but verified all 4 specific Round 5 checks + cross-referenced against `lattice/dimensions.rs` for ShapeClass variants.

### Round 6 findings triage

- **[TPR-04-001-codex-r6][low]** — Round 5 triage self-correcting parenthetical. **Action**: rewrote the Round 5 triage note with a clean direct statement (no mid-sentence self-correction). Purely wording; no semantic change.

Round 7 TPR re-verification pending after Round 6 fix lands.

### Round 7 — dual-source re-verification (CLEAN PASS)

**Run:** `/tmp/ori-tpr-ddce2jV0` (2026-04-14, verification of Round 6 wording fix)

- **Codex** (rc=0, 77s, 44 events): **0 findings**. Rich scope: read 10 files including decide.rs + dimensions.rs for ShapeClass verification. Confirms the Round 6 rewrite is code-consistent.
- **Gemini** (rc=0, 32s, 9 events): **0 findings**. Scope fields empty in envelope (schema-compliance artifact), but free-form prose explicitly confirms "plan is verified clean, coherent, and ready for Phase 3 Implementation."
- **Thoroughness**: Codex rich; gemini thin but narrow scope justified (Round 6 was a 1-line wording fix). Judgment: ACCEPTED as CLEAN PASS.

### CLEAN PASS outcome

Both reviewers returned zero actionable findings after 7 rounds of convergence. Plan TPR (Phase 2.5) is COMPLETE. Proceeding to Phase 3 Implementation.

**Convergence trajectory**:
- Round 1: 10 findings (7 actionable + 3 informational) — core plan gaps
- Round 2: 4 findings (2 actionable) — Round 1 triage error + test-deletion scope
- Round 3: 6 findings (4 actionable) — plan/code contradiction refinements
- Round 4: 3 findings (2 actionable) — test-home creation + preservation-vs-bug-exposing split
- Round 5: 2 findings (1 actionable) — EnumVariant bucket placement
- Round 6: 2 findings (1 LOW) — Round 5 wording self-correction
- Round 7: 0 findings — CLEAN PASS

Total actionable findings applied across all rounds: **17** (plus 1 originally-rejected / reclassified-accepted from Round 1's TPR-04-005-codex).

---

## 3. Implementation

### Proposed approach (expanded after Plan TPR Round 1)

1. **`decide_reuse()` in `decide.rs`**: Remove the `MaybeShared + Once + ReusableCtor → StaticReuse` branch. All MaybeShared → DynamicReuse.
2. **`decide_cow()` in `decide.rs`**: Remove three unsound subcases:
   - Remove `is_param && is_cow_aware_unique(ctx) → StaticUnique`
   - Remove `CollectionBuffer + Once → StaticUnique`
   - Remove `ReusableCtor + Once → StaticUnique`
3. **`is_cow_aware_unique()` in `decide.rs`**: Remove (dead code after step 2)
4. **`detect.rs`**: Simplify `is_static` to only check `death.uniqueness == Uniqueness::Unique`
5. **`cow.rs`**: Remove `is_cow_aware_unique()` and its use in `is_borrow_disjoint_from_siblings()` — only accept `Uniqueness::Unique`
5a. **Create `compiler/ori_arc/src/aims/emit_rc/cow/tests.rs`** (per TPR-04-001-codex-r4) — cow.rs currently has no `#[cfg(test)] mod tests;` declaration and no sibling `tests.rs`. To implement the §2.3 helper-layer tests, create the test home following `impl-hygiene.md` §Test Hygiene ("Rust tests in sibling `tests.rs`: `#[cfg(test)] mod tests;` in source, body in `tests.rs`"):
   - Add `#[cfg(test)] mod tests;` declaration to `compiler/ori_arc/src/aims/emit_rc/cow.rs`
   - Convert `cow.rs` into `cow/mod.rs` (or equivalent module form) if required by the project's existing emit_rc layout — match the pattern of sibling emit_rc submodules
   - Create `compiler/ori_arc/src/aims/emit_rc/cow/tests.rs` containing the four §2.3 helper-layer tests plus the §2.3 integration test `decide_cow_maybe_shared_with_unique_source_disjoint_borrow_stays_static_unique`
   - Existing `realize/tests.rs::cow_borrow_disjoint_maybe_shared` (line 122) is RENAMED and MOVED to the new home as the integration test — keeping the preservation semantics but under the correct name matching the new scenario
6. **Test maintenance — RENAME-OR-REPLACE, not assertion flip** (per TPR-04-004-codex + TPR-04-003-gemini): the following tests have names that describe REMOVED behavior. Delete or rename each; do NOT merely flip their assertions.
   - `cow_param_cow_aware_unique` (tests.rs:90) → delete; replaced by `decide_cow_maybe_shared_param_owned_linear_once_returns_dynamic`
   - `cow_cross_dim_collection_buffer_once` (tests.rs:106) → delete; replaced by `decide_cow_maybe_shared_collection_buffer_once_returns_dynamic`
   - `cow_cross_dim_reusable_ctor_once` (tests.rs:114) → delete; replaced by `decide_cow_maybe_shared_reusable_ctor_struct_once_returns_dynamic`
   - `decide_cross_dimensional_maybe_shared_once_ctor_is_static_reuse` (tests.rs:537) → delete; replaced by `decide_reuse_maybe_shared_reusable_ctor_struct_once_returns_dynamic_reuse`
   - MOVE + RENAME (per TPR-04-001-codex-r4): `cow_borrow_disjoint_maybe_shared` (realize/tests.rs:122) → rename to `decide_cow_maybe_shared_with_unique_source_disjoint_borrow_stays_static_unique` AND move to the new `compiler/ori_arc/src/aims/emit_rc/cow/tests.rs` as the §2.3 integration test. This is a rename (not deletion) because the preservation semantics are the same — the name change reflects the post-fix scenario precisely.
7. **Remove `cow_upgrades` + `cross_dim_reuse` fields and their increment sites** (per TPR-04-001-codex + TPR-04-002-codex-r3) across ALL four files. Preserve surviving helpers (`cross_dim_evidence_total()`, `has_cross_dim_evidence()`, `reuse_percent()`) with refactored bodies so they operate only on surviving fields.
   - `realize/metrics.rs` (actual surfaces — there is NO `Display` impl; remove "Display impl" reference and instead point at):
     - The `cow_upgrades` field and `cross_dim_reuse` field themselves (remove from struct definition)
     - Doc comments at lines 16-20 and 76-81 (remove mentions of removed fields; retain doc for surviving fields and helpers)
     - `cross_dim_evidence_total()` method (refactor to `self.canonicalize_cross_fires` only — removes the `+ self.cross_dim_reuse + self.cow_upgrades` terms)
     - `has_cross_dim_evidence()` method (unchanged signature; body still `self.cross_dim_evidence_total() > 0` works after refactor)
     - `merge()` method (remove `self.cow_upgrades += other.cow_upgrades` and `self.cross_dim_reuse += other.cross_dim_reuse`)
     - `report()` method at lines 107-123 (remove `cow_upgrades = self.cow_upgrades` and `cross_dim_reuse = self.cross_dim_reuse` from the structured log fields; keep the method itself and all other fields)
   - `realize/mod.rs`: remove `synergy.cow_upgrades += 1` at lines 325, 372
   - `realize/walk_dec.rs`: remove `is_cross_dim_reuse_candidate` gate and `metrics.cross_dim_reuse += 1` at lines 164-186
   - `realize/tests.rs`: apply per-test disposition per §2.4 (TRIM `synergy_metrics_default_is_zero`, `synergy_metrics_merge_additive`, `synergy_metrics_cross_dim_evidence`; PRESERVE unchanged `synergy_metrics_reuse_percent` and `synergy_metrics_percent_zero_total`). Do NOT delete the entire 1237-1310 block — follow §2.4's precise guidance.

### Capability regression tracking

This fix disables four `MaybeShared → StaticUnique/StaticReuse` upgrade paths in AIMS realization to achieve soundness. Per CLAUDE.md (Phase 4 step 6 — MANDATORY), the re-enablement of the spec-approved equivalent is tracked as a concrete bug-tracker entry:

- **Tracking artifact**: `[BUG-04-079][medium]` in `plans/bug-tracker/section-04-codegen-llvm.md`
- **BUG-04-079 is blocked by BUG-04-069** (per TPR-04-001-gemini Plan TPR finding). Rationale: BUG-04-079 plumbs `ParamContract.uniqueness` into `AnnotationSiteContext` so the spec-approved DP-9 `MaybeShared + parameter + IC-3 ParamContract.uniqueness = Unique → StaticUnique` path can fire. However, BUG-04-069 (`tighten_uniqueness_from_callers` in `interprocedural/mod.rs`) uses the unsound IC-8 pattern to produce `ParamContract.uniqueness = Unique` from `Owned + Linear + Once` — same class of unsoundness as the local DP-10 pattern. Today this is DORMANT because no realization site reads `ParamContract.uniqueness`. But if BUG-04-079 lands WITHOUT BUG-04-069 being fixed first, the local fix in this bug is laundered through the interprocedural tightening: callees would receive a falsely-Unique ParamContract, and `decide_cow()` would emit `StaticUnique` via the perfectly sound `Unique → StaticUnique` path, while the underlying Unique flag was derived from the same unsound cross-dimensional inference this fix removes.
- **Soundness argument (for this fix alone, without BUG-04-079)**: The four removed subcases (`is_param + Owned + Linear + Once`, `Once + CollectionBuffer`, `Once + ReusableCtor` in both `decide_cow()` and `decide_reuse()`, and `is_cow_aware_unique` in `cow.rs`) all derive past uniqueness (RC == 1) from future consumption/cardinality. Spec (`aims-rules.md` §DP-10 removal rationale) explicitly forbids this. After removal, MaybeShared values at these decision sites always take Dynamic/DynamicReuse (runtime `IsShared` check) — safe for all aliasing patterns.
- **Spec-approved re-enablement path (FUTURE work, BUG-04-079)**: DP-9 permits `MaybeShared + parameter + IC-3 ParamContract.uniqueness = Unique → StaticUnique` — this uses a PAST guarantee from the interprocedural SCC fixpoint (IC-3 fixpoint joins caller-side uniqueness across ALL call sites), not a future-use inference. Callers with retained aliases naturally cause IC-3 to converge to `MaybeShared`, preventing unsound promotion. **BUT** this requires BUG-04-069 be fixed first so the `ParamContract.uniqueness = Unique` that BUG-04-079 consumes is actually sound.
- **Dependency chain**: BUG-04-059 (this fix, removes local unsoundness) → BUG-04-069 (must fix before BUG-04-079, removes interprocedural unsoundness) → BUG-04-079 (spec-approved re-enablement)
- **No `#[ignore]`'d tests**: this fix deletes/renames tests to assert the new sound behavior rather than ignoring them; there are no dormant tests to un-ignore.

---

## R. Third Party Review Findings

(Phase 5 code-review TPR findings — populated after implementation lands.)

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix
- [ ] Matrix completeness verified (§2.1 + §2.2 + §2.3 + §2.4)
- [ ] Debug AND release builds pass
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p ori_arc` green
- [ ] `/commit-push` — commit all changes before review
- [x] Plan TPR (Phase 2.5) Round 1 — complete, 7 findings accepted (1 was reclassified from rejected after Round 2), 3 informational
- [x] Plan TPR (Phase 2.5) Round 2 — complete, 2 new actionable findings applied (BUG-04-069/079 split repair + synergy-metrics test-deletion refinement), 2 informational confirmations
- [x] Plan TPR (Phase 2.5) Round 3 — complete (with strengthened thoroughness directive), 4 actionable findings applied (synergy_metrics_cross_dim_evidence trim vs delete + §3 step 7 precision on metrics.rs surfaces + §3 step 7 vs §2.4 consistency), 2 informational confirmations
- [x] Plan TPR (Phase 2.5) Round 4 — complete, 2 new actionable findings applied (§3 step 5a for §2.3 helper-test home creation + §2 "Verify tests fail before fix" split into bug-exposing vs preservation), 1 informational (gemini agrees plan is clean)
- [x] Plan TPR (Phase 2.5) Round 5 — complete, 1 actionable finding applied (EnumVariant cells moved from hedged-preservation to bug-exposing after code verification against decide.rs:425-429), 1 informational (gemini confirms plan clean)
- [x] Plan TPR (Phase 2.5) Round 6 — complete, 1 LOW-severity wording fix applied (Round 5 triage note self-correction removed), gemini 0 findings (first fully clean gemini envelope)
- [x] Plan TPR (Phase 2.5) Round 7 — **CLEAN PASS** — both reviewers 0 actionable findings. Plan TPR COMPLETE (7 rounds, 17 actionable findings applied, convergence achieved).
- [ ] `/tpr-review` (Phase 5 — code review) passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed
- [ ] Capability regression gate: BUG-04-079 blocked-by annotation present in tracker
- [ ] Bug entry updated: `- [x]` with resolution details
- [ ] Fix section status updated to `complete`
- [ ] Bug-tracker overview open bug count updated
- [ ] `/sync-claude` doc sync
- [ ] Final `/commit-push`

**Exit Criteria:** All 4 unsound cross-dimensional paths removed. Synergy counters removed from all 4 files. MaybeShared decisions always Dynamic/DynamicReuse. Uniqueness::Unique path unchanged. All tests pass. BUG-04-079 correctly blocked on BUG-04-069.
