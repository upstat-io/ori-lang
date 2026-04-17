---
section: "07"
title: "Close-out + Supersession (absorbs BUG-04-085 investigation)"
status: not-started
reviewed: false
goal: >
  Final verification, plan annotation cleanup, bug-tracker updates (including formal
  supersession of BUG-04-074, BUG-04-084, BUG-04-042, BUG-04-085), and resolution of
  the LLVM spec runner crash absorbed as §07.0. This section marks the plan complete
  after confirming test-all.sh is green on both backends.
success_criteria:
  - "§07.0 (BUG-04-085 investigation) is resolved: either (a) §02.0's `Tag::Scheme PROPAGATE_MASK` fix is confirmed as the root cause and the downstream consumer is fixed, or (b) §02.0 is confirmed as not-causal and a new bug is filed scoped to the true root cause; in either case, the LLVM spec runner no longer crashes on monomorphized `assert_eq`."
  - "All plan annotations removed from all four trees — verifiable via: `rg 'BUG-04-074|BUG-04-084|BUG-04-042|BUG-04-085|empty-container-typeck' compiler/ --glob '*.rs'`, same for `compiler/ --glob '*.ori'`, `tests/ --glob '*.ori'`, and `library/ --glob '*.ori'` — all four returning zero hits."
  - "07.3 (TPR + hygiene + tooling + sync) passes with zero actionable findings BEFORE any bug-tracker entries are updated to `complete`. Gate includes: `plan_corpus check`, `/tpr-review`, `/impl-hygiene-review`, `/improve-tooling` sweep, and `/sync-claude` sweep — all must be clean."
  - "Bug-tracker entries closed with pointers to owning plan sections (no sibling fix files remain): BUG-04-074 → §03; BUG-04-084 → §03.BUG-FIXES; BUG-04-042 → §08; BUG-04-085 → §07.0."
  - "`plans/bug-tracker/00-overview.md` reflects the reduced open-bug count after closures."
  - "`plans/bug-tracker/section-04-codegen-llvm.md` entries for all four absorbed bugs marked `[x]` with plan pointers."
  - "Final `/tpr-review` (whole plan arc) passes with zero actionable findings."
  - "`/impl-hygiene-review` on the full work arc (all sections) passes."
  - "`timeout 150 ./test-all.sh` green (debug) — including the Ori spec (LLVM backend) row which no longer shows CRASHED. `timeout 150 cargo test --release -p ori_types` and `timeout 150 cargo test --release -p ori_llvm` green (release)."
depends_on: ["01", "02", "03", "04", "05", "06", "08"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "07.0"
    title: "Investigate BUG-04-085 — LLVM spec runner crash on monomorphized assert_eq"
    status: not-started
  - id: "07.1"
    title: "Final test-all.sh + release verification"
    status: not-started
  - id: "07.2"
    title: "Plan annotation cleanup sweep"
    status: not-started
  - id: "07.3"
    title: "Final TPR + hygiene review gate"
    status: not-started
  - id: "07.4"
    title: "Bug-tracker updates — close BUG-04-074, BUG-04-084, BUG-04-042, BUG-04-085 with plan pointers"
    status: not-started
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

## Intelligence Reconnaissance

Queries run 2026-04-17:

- `scripts/intel-query.sh --human file-symbols "ori_llvm/src/codegen/arc_emitter" --repo ori` — inventory `ArcIrEmitter` symbols (variable-definition ordering, `emitter_utils`) before investigating the BUG-04-085 variable-not-yet-defined crash.
- `scripts/intel-query.sh --human callers "compute_flags" --repo ori` — blast radius of `TypeFlags` computation (including `PROPAGATE_MASK`) to trace §02.0's effect on downstream consumers.
- `scripts/intel-query.sh --human callers "body_type_map" --repo ori` — identify all consumers of `body_type_map` that may have relied on pre-§02.0 `Tag::Scheme HAS_VAR=false` flag semantics.
- `scripts/intel-query.sh --human similar "variable not defined arc emitter ordering" --repo swift,lean4 --limit 3` — prior art for SSA variable-definition ordering bugs in ARC IR emitters.

Results summary (≤500 chars) [ori]: `ArcIrEmitter` lives in `ori_llvm/src/codegen/arc_emitter/`; `emitter_utils.rs` handles variable definition tracking. `body_type_map` consumed by `prepare_mono_cached` in `nounwind/prepare.rs`. `compute_flags` in `ori_types/src/pool/` — `PROPAGATE_MASK` sets `HAS_VAR` on `Tag::Scheme` items since §02.0. [swift]: SIL variable-ordering bugs manifest as "use before definition" in ownership verification — mirrors the `var=N not yet defined` crash signature.

---

## 07.0 Investigate BUG-04-085 — LLVM spec runner crash (absorbed 2026-04-17)

**Origin:** Absorbed from bug-tracker `BUG-04-085` per CLAUDE.md §Ownership & Deferral "Plan-blocker bugs belong IN the plan". The bug was filed 2026-04-15 with explicit notes that §02.0's `Tag::Scheme PROPAGATE_MASK` change may be the causal root. Because this plan introduced §02.0 and the crash may be its downstream symptom, the plan owns investigating and resolving it as part of close-out.

**Failure mode (from bug entry):** `timeout 150 ./test-all.sh` shows `Ori spec (LLVM backend) CRASHED` with cluster of errors on `assert_eq$m$int` (monomorphized `assert_eq<int>`):
- `LLVM IR verification failed after codegen (emit_prepared_functions)` with `Call parameter type does not match function signature!` on `ori_rc_dec({ i64, i64, ptr }, ptr @"_ori_drop$N")`
- Three `ArcIrEmitter: variable not yet defined` errors (var=11, var=2, var=7) — mirrors resolved BUG-04-024 signature
- `error[E4003]: ARC internal error: index assignment reached ARC lowering before desugaring` (BUG-04-070 class)
- `thread 'ori-main' has overflowed its stack` → `SIGABRT`
- Interpreter passes all 4444 spec tests; only `--backend=llvm` crashes

**Causal hypothesis:** `compiler/ori_types/src/pool/mod.rs:655-661` — §02.0's `Tag::Scheme` `PROPAGATE_MASK` fix sets `HAS_VAR` on scheme `Idx` values where previously unset. If the mono pipeline's `body_type_map` or `ArcIrEmitter`'s variable-definition ordering relied on the old flag semantics (specifically: "scheme types have !HAS_VAR so skip the substitute step"), then setting `HAS_VAR` correctly now surfaces as "variable not yet defined" in the downstream consumer.

- [ ] **Reproduce the crash cleanly**: `timeout 150 ./test-all.sh` on a clean post-§08 tree — confirm the specific crash line, the 3 `var=N` errors, and the `assert_eq$m$int` function name.
- [ ] **Isolate §02.0's causal role**: revert `compiler/ori_types/src/pool/mod.rs:655-661`'s `Tag::Scheme` contribution to `PROPAGATE_MASK` on a scratch branch; re-run `test-all.sh`. If the crash disappears, §02.0 is causal. If the crash persists, §02.0 is not causal and the root cause is elsewhere.
- [ ] **If §02.0 IS causal**: identify the downstream consumer that depended on the old flag semantics. Candidate sites: `ArcIrEmitter` variable-definition ordering (`emitter_utils.rs`), the mono pipeline's `body_type_map` construction, `prepare_mono_cached`'s scheme handling. Fix the consumer to work correctly with the new (correct) flag semantics.
- [ ] **If §02.0 is NOT causal**: file a new bug scoped to the true root cause (drop-fn pointer type mismatch? ArcIrEmitter ordering regression independent of §02.0?). That new bug is outside this plan's scope — continue with §07.1.
- [ ] **TDD**: add a spec test or Rust unit test that exercises the failing path — `assert_eq<int>` in a file that also uses polymorphic lambdas (overlap with §08 surface). Verify the test fails on the current code and passes after the fix.
- [ ] **Matrix**: poly-lambda × imported generic × assert_eq type — confirm no crash for `int`, `str`, `bool`, `float` variants.

---

# Section 07: Close-out + Supersession

**Status:** Not Started
**Goal:** Formally close BUG-04-074 and this plan. Verify all code changes are clean,
all plan annotations are removed, and the bug-tracker is updated.

**Depends on:** All previous sections complete.

---

## 07.1 Final Test Verification

```bash
timeout 150 ./test-all.sh          # debug build — must be green
timeout 150 cargo test --release -p ori_types   # release build
timeout 150 cargo test --release -p ori_llvm    # LLVM + AOT release
```

Also verify dual-execution parity on the Section 05 spec tests:
```bash
diagnostics/dual-exec-verify.sh tests/spec/types/collections/empty_list/empty_list_annotated_with_push.ori
diagnostics/dual-exec-verify.sh tests/spec/types/collections/empty_list/empty_list_element_struct.ori
diagnostics/dual-exec-verify.sh tests/spec/types/collections/empty_list/empty_list_with_for_yield.ori
```

---

## 07.2 Plan Annotation Cleanup Sweep

Per `CLAUDE.md §Compiler Coding Guidelines`: "Plan annotations are temporary scaffolding.
They MUST be removed when the plan completes."

Run the sweep over **all three trees** (compiler source, spec tests, and stdlib), using
only this plan's specific markers (not generic `§NN` tokens that match unrelated plans):

```bash
# Plan-specific markers in compiler source (Rust files) — must return zero hits
rg 'BUG-04-074|empty-container-typeck' compiler/ --glob '*.rs'

# Plan-specific markers in AOT fixture files (Ori files in compiler tree) — must return zero hits
# These are created in §05.3 at compiler/ori_llvm/tests/aot/fixtures/empty_list/*.ori
rg 'BUG-04-074|empty-container-typeck' compiler/ --glob '*.ori'

# Plan-specific markers in spec tests (regression comments added in §06.2)
rg 'BUG-04-074|empty-container-typeck' tests/ --glob '*.ori'

# Plan-specific markers in stdlib (edits made in §06.3)
rg 'BUG-04-074|empty-container-typeck' library/ --glob '*.ori'
```

Note: Generic `§NN` tokens (e.g. `§01`, `§03`) are intentionally excluded from the
sweep pattern because they exist in unrelated plans (e.g., `compiler/ori_repr/` contains
`§03` and `§04` references from a different plan). Searching for bare `§NN` produces
false positives and sends the implementer chasing unrelated code. Use only the
plan-specific identifiers above.

Every hit must be removed or converted to a permanent spec comment
(`Spec: Clause N.M` format). No plan-referencing comments in shipped code.

---

## 07.3 Final TPR + Hygiene Gate

**Run this BEFORE updating any external bug trackers.** All validation gates must pass
before declaring BUG-04-074 resolved — marking the bug complete before gates pass leaves
the tracker out of sync if a gate fails.

First, validate the entire plan directory against the plan corpus schema:

```bash
python -m scripts.plan_corpus check plans/empty-container-typeck-phase-contract/
```

This command must pass with zero schema errors before the plan can be marked complete.
If it fails on sections 01–04 or `index.md` (sections written before the plan_corpus
schema was current), update their frontmatter to satisfy the schema before proceeding.

Run final TPR over the full work arc:
```bash
/tpr-review
```

The scope covers all compiler changes across Sections 01–04, all new test files from
Section 05, and all annotation changes from Section 06. The review must return zero
actionable findings before the plan can be marked complete.

Run hygiene review:
```bash
/impl-hygiene-review
```

Auto-scope mode covers the full work arc. Must pass.

Run tooling improvement sweep:
```bash
/improve-tooling
```

Review any test harness, diagnostic script, or developer tooling gaps surfaced during
the implementation arc (Sections 01–06). Flag gaps found in `dual-exec-verify.sh`,
`test-all.sh`, or any diagnostic flag interactions.

Run CLAUDE.md sync sweep:
```bash
/sync-claude
```

Verify no new APIs, commands, or invocation patterns were introduced by Sections 01–06
that need documenting. If `diagnostics/dual-exec-verify.sh` behavior or any new
`ORI_*` flags were added or changed, document the exact invocation pattern in
CLAUDE.md §Commands if not already present.

---

## 07.4 Bug-Tracker Updates

**Only update bug trackers AFTER 07.3 validation gates pass.** This sequencing ensures
the bug is not prematurely marked complete.

1. Update `plans/bug-tracker/fix-BUG-04-074.md`:
   - Set `status: complete`
   - Add pointer: `"Superseded by plans/empty-container-typeck-phase-contract/"`
   - Set `resolved_in:` to the commit SHA of the Section 03 landing

2. Update `plans/bug-tracker/00-overview.md`:
   - Move BUG-04-074 from open to closed
   - Decrement open bug count

3. Update `plans/bug-tracker/section-04-codegen-llvm.md` (or wherever BUG-04-074
   is tracked in the section-scoped trackers):
   - Mark as resolved

---

## 07.R Third Party Review Findings

Round 2 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini). Findings addressed
in this revision.

### [[TPR-07-001-codex]] [MEDIUM] Scope the annotation cleanup sweep to this plan's markers and all affected trees

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:77`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** Section 07 originally grepped `compiler/` for
`BUG-04-074|empty-container-typeck|§01|§02|§03|§04`. That misses temporary plan comments
in spec tests (section-05-test-matrix.md instructs implementers to add `Regression:
BUG-04-074` comments under `tests/spec/...`). It also produces unrelated false-positive
hits because generic `§03`/`§04` markers already exist in
`compiler/ori_repr/src/narrowing/tests.rs:1` and `compiler/ori_parse/src/dispatch.rs:134`
(from a different plan).

**Fix:** Split the sweep into two targeted commands that use only plan-specific identifiers
(`BUG-04-074|empty-container-typeck`) and cover both `compiler/` and `tests/`. Dropped
generic `§NN` tokens from the pattern to eliminate false positives from unrelated plans.
Updated the success_criteria in the frontmatter to match.

---

### [[TPR-07-002-gemini]] [LOW] Expand plan annotation cleanup regex to cover all sections

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:77`
**Reviewer:** Gemini | **Status:** Fixed (subsumed by TPR-07-001-codex)

**Evidence:** Section 07.2 used a regex that explicitly omitted `§05`, `§06`, and `§07`.
If developers left code annotations referencing those sections, the final cleanup sweep
would miss them.

**Fix:** Same as TPR-07-001-codex — the updated sweep uses plan-specific identifiers only
(`BUG-04-074|empty-container-typeck`) which covers all sections by design: any
annotation referencing this plan's work will include the bug ID or plan name.

---

### [[TPR-07-003-codex]] [MEDIUM] Add a whole-plan corpus validation gate before close-out

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:104`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** Section 07's final gate originally ran only `/tpr-review` and
`/impl-hygiene-review`. The command
`python -m scripts.plan_corpus check plans/empty-container-typeck-phase-contract/`
currently fails on `index.md`, `section-01-value-restriction.md`,
`section-02-validator-module.md`, and `section-04-codegen-assertions.md`. Without an
explicit gate, Section 07 could declare the plan complete while the owning plan directory
is still schema-invalid to the plan tooling.

**Fix:** Added `python -m scripts.plan_corpus check plans/empty-container-typeck-phase-contract/`
as the first step in 07.4. The gate must pass before TPR or hygiene review proceeds.
Also added to the 07.N checklist as an explicit `- [ ]` item.

Round 3 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini). Findings addressed
in this revision.

### [[TPR-07-004-codex]] [MEDIUM] Sweep library/ directory for plan markers during close-out

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:§07.2`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** Section 06.3 directs implementers to annotate `library/std/` code. Section 07.2
only swept `compiler/` (with `*.rs` filter, missing `.ori` AOT fixtures) and `tests/`. The
`library/` tree was entirely missing from the cleanup sweep.

**Fix:** Added two new sweep commands: one for `compiler/ --glob '*.ori'` (AOT fixtures) and
one for `library/ --glob '*.ori'` (stdlib). Updated success_criteria to reference all four
trees. Updated 07.N checklist to explicitly call out all four trees.

---

### [[TPR-07-005-codex]] [MEDIUM] Add release ori_llvm test and section-04 tracker update to 07.N

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:§07.N`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** Section 07.1 specifies `cargo test --release -p ori_llvm` but the 07.N checklist
item for "07.1 complete" only said "`./test-all.sh` green debug + release". Section 07.3 (now
07.4) step 3 requires updating `plans/bug-tracker/section-04-codegen-llvm.md` but the 07.N
checklist for "07.3 complete" (now "07.4 complete") omitted this.

**Fix:** Updated 07.N "07.1 complete" item to include `cargo test --release -p ori_llvm`. Updated
"07.4 complete" item to explicitly include `plans/bug-tracker/section-04-codegen-llvm.md` resolution.

---

### [[TPR-07-006-gemini]] [HIGH] Reorder 07.3/07.4 so validation gates run before bug-tracker closure

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:§07.3/07.4`
**Reviewer:** Gemini | **Status:** Fixed

**Evidence:** Section 07.3 was "Bug-Tracker Updates" (marks BUG-04-074 complete) and 07.4 was
"Final TPR + Hygiene Gate". This ordering allows the bug to be declared resolved in external
trackers before the final `/tpr-review` and `/impl-hygiene-review` pass. If a gate fails after
the tracker is updated, the tracker is out of sync.

**Fix:** Swapped 07.3 and 07.4: 07.3 is now "Final TPR + Hygiene Gate" and 07.4 is
"Bug-Tracker Updates (runs AFTER 07.3 validation passes)". Added explicit note in 07.4 header
that bug trackers must only be updated after 07.3 passes. Updated frontmatter sections list
and success_criteria to reflect the new ordering.

---

### [[TPR-07-007-gemini]] [LOW] section-04-codegen-llvm.md missing from 07.N 07.3 checklist item

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:§07.N`
**Reviewer:** Gemini | **Status:** Fixed (subsumed by TPR-07-005-codex)

**Evidence:** The 07.N completion checklist for the bug-tracker step only listed
`fix-BUG-04-074.md` and `00-overview.md`, omitting the `section-04-codegen-llvm.md` update
that step 3 of the bug-tracker section explicitly requires.

**Fix:** Same as TPR-07-005-codex — the updated 07.N checklist for "07.4 complete" now
explicitly includes `plans/bug-tracker/section-04-codegen-llvm.md entry resolved`.

---

Round 4 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini). Findings addressed
in this revision.

### [[TPR-07-R4-001-codex]] [MEDIUM] /improve-tooling and /sync-claude dropped from 07.3 and 07.N

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:§07.3`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** `00-overview.md:165` and `index.md:119` both specify Phase 6 / Section 07
close-out as `/tpr-review` → `/impl-hygiene-review` → `/improve-tooling` sweep → `/sync-claude`.
After the 07.3/07.4 reorder in Round 3, the implementation of 07.3 and the 07.N checklist
only included `/tpr-review` and `/impl-hygiene-review`. Both `/improve-tooling` and
`/sync-claude` were silently dropped, leaving the section cross-inconsistent with the
approved overview and creating two unanchored mandatory close-out tasks.

**Fix:** Added `/improve-tooling` and `/sync-claude` blocks to 07.3 body with instructions
for each. Updated the success_criteria to name all five gates. Updated the 07.N "07.3
complete" checklist item to explicitly list `/improve-tooling` and `/sync-claude`.

---

## 07.N Completion Checklist

- [ ] **07.1 complete** — `./test-all.sh` green debug + release; `cargo test --release -p ori_llvm` green; dual-exec parity verified on ≥3 spec files
- [ ] **07.2 complete** — zero `BUG-04-074|empty-container-typeck` hits in all four trees: `compiler/*.rs`, `compiler/*.ori` (AOT fixtures), `tests/*.ori`, `library/*.ori`
- [ ] **07.3 complete** — `plan_corpus check plans/empty-container-typeck-phase-contract/` passes; final `/tpr-review` clean; `/impl-hygiene-review` clean; `/improve-tooling` sweep clean; `/sync-claude` sweep clean
- [ ] **07.4 complete** — `fix-BUG-04-074.md` status `complete`; `00-overview.md` updated; `plans/bug-tracker/section-04-codegen-llvm.md` entry resolved
- [ ] This section's own status set to `complete` in frontmatter
- [ ] `plans/empty-container-typeck-phase-contract/` plan index updated: all sections complete

**Exit criteria:** BUG-04-074 is formally closed. The empty-container typeck
phase-contract enforcement plan is complete. The empty list ambiguity bug no longer
exists in the Ori compiler.
