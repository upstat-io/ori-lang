---
section: "09"
title: "Final Verification"
status: not-started
reviewed: false
goal: "Prove end-to-end that the reorganization is correct: all 14 mission success criteria from `00-overview.md` are satisfied, `./test-all.sh` + `./full-check.sh` + `lefthook run pre-commit` + `cargo check/clippy/build/test` + `./scripts/sync-version.sh --check` all green, CI dry-run green, zero stale references survive via a global grep, and dual-source `/tpr-review` + `/impl-hygiene-review` come back clean."
success_criteria:
  - "Every mission success criterion in 00-overview.md is checked off"
  - "`./test-all.sh` green (via root wrapper)"
  - "`./scripts/dev/test-all.sh` green (direct)"
  - "`./full-check.sh` green"
  - "`./fmt-all.sh` green"
  - "`./build-all.sh` green"
  - "`./scripts/dev/clippy-all.sh` green"
  - "`./scripts/dev/llvm-test.sh` green"
  - "`lefthook run pre-commit` green"
  - "`cargo check --workspace` green"
  - "`cargo clippy --workspace --all-targets` green"
  - "`cargo build --workspace` green"
  - "`cargo t` (Rust workspace tests) green"
  - "`cargo st` (Ori spec tests) green"
  - "`./scripts/sync-version.sh --check` green"
  - "Global grep sweep verification: `rg '\\./(clippy-all|llvm-build|llvm-clippy|llvm-test)\\.sh'` returns only scripts/dev/ paths"
  - "CI dry-run on a test PR (push to a throwaway branch, verify all workflows pass)"
  - "`/tpr-review plans/project-reorganization/` passes clean (dual-source Codex + Gemini, either zero findings or all findings triaged and resolved)"
  - "`/impl-hygiene-review` passes clean (run AFTER /tpr-review clean per CLAUDE.md §Fix Completeness)"
  - "Baseline reconciliation: `git diff <baseline-commit>..HEAD --stat` shows the exact cumulative effect of all 8 prior sections"
  - "`plans/project-reorganization/00-overview.md` all mission success criteria check-boxes → `[x]`"
  - "Plan `status: not-started` → `status: complete` in overview frontmatter"
inspired_by:
  - "rustc post-refactor verification — full test suite + CI dry-run before declaring complete"
  - "Swift SIL refactor completion gates — verifier-as-test + idempotency check + stress test"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "09.1"
    title: "Full Local Verification Sweep"
    status: not-started
  - id: "09.2"
    title: "Baseline Reconciliation"
    status: not-started
  - id: "09.3"
    title: "CI Dry-Run (Test PR)"
    status: not-started
  - id: "09.4"
    title: "/tpr-review Full Plan Execution"
    status: not-started
  - id: "09.5"
    title: "/impl-hygiene-review Full Plan"
    status: not-started
  - id: "09.6"
    title: "Mission Success Criteria Checklist & Plan Close"
    status: not-started
  - id: "09.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "09.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 09: Final Verification

**Status:** Not Started
**Goal:** This is the terminal gate. Every prior section has run; every file has been moved, deleted, or rewritten; every reference has been swept. §09 proves the end state is correct, complete, and ready for merge. Unlike the per-section completion checklists which verify local correctness, §09 verifies **cumulative correctness** — does the repo as a whole work as intended after all 8 sections land? The verification sweep runs the full test harness, runs lefthook, runs CI on a throwaway branch, runs `/tpr-review` and `/impl-hygiene-review` to get independent confirmation, and reconciles the final state against the §01 baseline to produce a complete before/after diff.

**Success Criteria:** (see frontmatter above — extensive; summarized here)

- [ ] Every mission criterion in overview checked off
- [ ] Full local verification sweep green (8+ commands)
- [ ] Global grep proves zero stale references
- [ ] CI dry-run green on throwaway branch
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] Baseline reconciliation complete
- [ ] Plan status → `complete` in overview frontmatter

**Context:** Per CLAUDE.md §Fix Completeness, a plan is not done until:
- Matrix tests cover every relevant dimension
- At least one semantic pin exists (for this plan: the hot-path wrapper forwarding IS the semantic pin — if the wrapper is reverted, `./test-all.sh` fails to find its implementation)
- Debug AND release builds pass
- `./test-all.sh` green
- `/tpr-review` passed
- `/impl-hygiene-review` passed AFTER TPR clean

§09 enforces all of the above as explicit checklist items.

**Reference implementations:**
- **rustc**: post-refactor gates include `./x.py test`, `./x.py check`, `./x.py fmt`, full CI green on try build
- **Swift**: post-SIL-refactor gates include `utils/build-script --test --verifier-test`, stress-test and idempotency checks
- **Gleam**: post-refactor gates include `cargo test`, format, and a throwaway-branch CI run

**Depends on:** §01-§08 (this section is the terminal gate, so it depends on everything).

---

## 09.1 Full Local Verification Sweep

**File(s):** No edits — verification only

Run every verification command that exercises the reorganized state. Each must exit 0.

- [ ] **Test suite (via wrapper)**:
  ```bash
  cd /home/eric/projects/ori_lang
  timeout 150 ./test-all.sh 2>&1 | tee plans/project-reorganization/baseline/test-all-after.log
  echo "Exit: $?"
  ```
  - [ ] Exit 0
  - [ ] Output saved as post-plan baseline for comparison with `baseline/test-all-before.log`

- [ ] **Test suite (direct, no wrapper)**:
  ```bash
  timeout 150 ./scripts/dev/test-all.sh 2>&1 | tail -15
  # Expected: identical to wrapper output
  ```
  - [ ] Exit 0

- [ ] **Full check (wrapper → internal chain)**:
  ```bash
  timeout 150 ./full-check.sh 2>&1 | tail -15
  # Expected: runs clippy-all then test-all, both pass
  ```
  - [ ] Exit 0

- [ ] **Fmt check**:
  ```bash
  timeout 60 ./fmt-all.sh 2>&1 | tail -10
  ```
  - [ ] Exit 0

- [ ] **Build**:
  ```bash
  timeout 150 ./build-all.sh 2>&1 | tail -10
  ```
  - [ ] Exit 0

- [ ] **Clippy (cleanly-moved)**:
  ```bash
  timeout 150 ./scripts/dev/clippy-all.sh 2>&1 | tail -10
  ```
  - [ ] Exit 0

- [ ] **LLVM test (cleanly-moved)**:
  ```bash
  timeout 150 ./scripts/dev/llvm-test.sh 2>&1 | tail -10
  ```
  - [ ] Exit 0

- [ ] **Cargo workspace commands**:
  ```bash
  cargo check --workspace 2>&1 | tail -5 && echo OK || echo FAIL
  cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5 && echo OK || echo FAIL
  cargo build --workspace 2>&1 | tail -5 && echo OK || echo FAIL
  timeout 150 cargo t 2>&1 | tail -15 && echo OK || echo FAIL
  timeout 150 cargo st 2>&1 | tail -15 && echo OK || echo FAIL
  ```
  - [ ] All 5 commands OK

- [ ] **Version sync check**:
  ```bash
  ./scripts/sync-version.sh --check 2>&1 | tail -5
  ```
  - [ ] "All versions in sync"

- [ ] **lefthook pre-commit simulation**:
  ```bash
  lefthook run pre-commit 2>&1 | tail -15
  # Expected: fmt-all runs (via wrapper), full-check runs (via wrapper), version-sync runs
  ```
  - [ ] Exit 0, all hooks pass

- [ ] **Global grep sweep — zero stale references**:
  ```bash
  rg '\./(clippy-all|llvm-build|llvm-clippy|llvm-test)\.sh' 2>&1 \
    | grep -v '^scripts/dev/' \
    | grep -v '^plans/project-reorganization/' \
    | grep -v '^plans/completed/'
  # Expected: empty (plans/completed/ may still have historical references — those are archival and acceptable)
  ```
  - [ ] Zero active references
  - [ ] Note: `plans/completed/` is explicitly excluded from the sweep because it's archival history — if someone ran `/review-plan` on a completed plan and found stale references, that would be a separate cleanup, not §08's responsibility

- [ ] **Global grep for `website/` (ori_lang-internal)**:
  ```bash
  rg 'website/' . --type sh --type md --type toml --type yaml 2>&1 \
    | grep -v '^plans/project-reorganization/' \
    | grep -v 'ori-lang-website' \
    | grep -v 'playground-wasm'
  # Expected: only legitimate sibling-repo references
  ```

- [ ] **Subsection close-out (09.1)** — MANDATORY before starting 09.2:
  - [ ] All 13+ verification commands green
  - [ ] Zero stale references
  - [ ] test-all-after.log captured for baseline comparison
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — 13
        verification commands is exactly the case where a
        `scripts/dev/verify-reorg.sh` or more generally a
        `scripts/dev/full-verify.sh` helper would pay off. If the
        `verify-everything.sh` helper from §06.6's retrospective was built,
        it may subsume this. Verify and commit any extension via
        `build(scripts): extend verify-everything.sh with version-sync and
        lefthook — surfaced by project-reorganization/section-09.1
        retrospective`.

---

## 09.2 Baseline Reconciliation

**File(s):** No edits — verification only

Compare the post-plan state against the §01 baseline to produce a complete before/after diff.

- [ ] Find the §01 baseline commit:
  ```bash
  cd /home/eric/projects/ori_lang
  BASELINE_COMMIT=$(git log --oneline --all | grep "docs(plan): freeze project-reorganization baseline" | head -1 | awk '{print $1}')
  echo "Baseline commit: $BASELINE_COMMIT"
  ```
  - [ ] Commit hash captured

- [ ] Full diff stat:
  ```bash
  git diff "$BASELINE_COMMIT"..HEAD --stat | tee plans/project-reorganization/baseline/diff-baseline-to-end.txt
  ```
  - [ ] Output saved
  - [ ] Review the scope: does the change set match what §02-§08 collectively described? (~240+ file changes in §08 alone, plus §02, §03, §04, §05, §06, §07 contributions)

- [ ] Per-category diff summary:
  ```bash
  {
    echo "=== Files added ==="
    git diff "$BASELINE_COMMIT"..HEAD --stat --diff-filter=A | head -30
    echo ""
    echo "=== Files deleted ==="
    git diff "$BASELINE_COMMIT"..HEAD --stat --diff-filter=D | head -30
    echo ""
    echo "=== Files renamed ==="
    git diff "$BASELINE_COMMIT"..HEAD --stat --diff-filter=R | head -30
    echo ""
    echo "=== Files modified ==="
    git diff "$BASELINE_COMMIT"..HEAD --stat --diff-filter=M | head -30
  } > plans/project-reorganization/baseline/diff-breakdown.txt
  ```
  - [ ] Every category's result matches expectations from the plan sections

- [ ] Expected category counts (from overview — reconciled with the 30/12/18 scratchpad math finalized in iteration 2 per TPR-XX-005-codex iteration 3 fix):
  - **Added**: 4 hot-path wrappers at root + **12 scratchpad migrations** (11 with Astro `docsSchema` + 1 with body-format proposal schema) + 4 test-all baseline artifacts + 9 section files + 2 plan index/overview (the plan itself) + 1 new diagnostic helper (`scripts/diagnostics/_repo-root.sh`) = ~32 new files
  - **Deleted**: ~20 LSP files + 3 blog files (moved cross-repo, so deletions on the ori_lang side) + **18 scratchpad deletions** (includes `07-modern-lang-repos.md` per conditional DELETE) + 2 build/debug leaks + 1 `.gitkeep` + 1 `rebuild-playground.sh` + 1 `samples/` dir (if still there) = ~45 deletions
  - **Renamed**: 8 scripts moved to `scripts/dev/` + 16+ diagnostics scripts moved to `scripts/diagnostics/` = ~24 renames
  - **Modified**: `.gitignore` (if §03 decides the `/build/` addition is redundant per iteration 3 finding — see updated §03) + `Cargo.toml` + `sync-version.sh` + `auto-release.yml` + CLAUDE.md + CONTRIBUTING.md + ~200 `plans/**/*.md` (Pattern A sweep) + `.claude/rules/*.md` + `.claude/skills/*` + LSP docs + formatter docs + roadmap section-22 + 7 edited diagnostic scripts (repo-root resolution fix) + `compiler/oric/src/llvm_dump/mod.rs` comment + `compiler/ori_llvm/tests/aot/util/aot.rs:168` string = ~240+ modifications
  - [ ] Actual counts approximately match the reconciled estimates

- [ ] **Subsection close-out (09.2)** — MANDATORY before starting 09.3:
  - [ ] Baseline commit hash captured
  - [ ] Full diff stat saved
  - [ ] Per-category breakdown saved
  - [ ] Expected counts match actual counts within reason
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        "compare HEAD to a baseline commit" pattern is useful. Is there a
        `scripts/dev/plan-diff-summary.sh <plan-name>` helper that looks
        up the baseline commit for a given plan (from plans/<plan>/baseline/
        artifacts) and produces the per-category summary? Likely valuable
        for any plan that uses §01-style baselines. If yes, build and
        commit via `build(scripts): add plan-diff-summary.sh — surfaced by
        project-reorganization/section-09.2 retrospective`.

---

## 09.3 CI Dry-Run (Test PR)

**File(s):** No edits — verification only

Push the post-plan state to a throwaway branch and verify CI workflows pass. This catches any CI breakage that the local sweep missed.

- [ ] Decide on a throwaway branch name:
  ```
  project-reorg-ci-test-YYYY-MM-DD
  ```

- [ ] Push the current HEAD to the throwaway branch:
  ```bash
  cd /home/eric/projects/ori_lang
  BRANCH="project-reorg-ci-test-$(date +%Y-%m-%d)"
  git push origin HEAD:"$BRANCH"
  ```

- [ ] Open a draft PR against `master` (or `main`):
  ```bash
  gh pr create --base master --head "$BRANCH" --title "DRAFT: project-reorganization plan CI verification" --body "This is a draft PR created for CI verification only. Do not merge.

See plans/project-reorganization/section-09-verification.md §09.3.

This PR tests that:
- .github/workflows/ci.yml passes with the reorganized scripts
- .github/workflows/auto-release.yml still parses (doesn't run on PRs)
- Version sync works (./scripts/sync-version.sh --check)

If all checks pass, this PR is closed without merging and the branch is deleted." --draft
  ```

- [ ] Monitor CI:
  ```bash
  gh pr checks  # shows status of each workflow
  # Wait for all to complete (may take ~15-25 minutes for the full Ori CI)
  ```
  - [ ] All workflows green
  - [ ] If any workflow red: investigate — CI breakage is a blocker for plan completion

- [ ] Close the draft PR and delete the branch:
  ```bash
  gh pr close "$BRANCH" --comment "CI verification complete — closing draft PR per §09.3 completion. See plans/project-reorganization/section-09-verification.md for the verification record."
  git push origin --delete "$BRANCH"
  ```

- [ ] **Subsection close-out (09.3)** — MANDATORY before starting 09.4:
  - [ ] Throwaway branch pushed
  - [ ] Draft PR opened
  - [ ] All CI workflows green
  - [ ] Draft PR closed and branch deleted
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — CI
        dry-runs are a standard pattern. Is there a `scripts/dev/ci-dry-run.sh
        <plan-name>` helper that pushes a throwaway branch, opens a draft
        PR, waits for checks, and cleans up? Broadly useful beyond this
        plan. If yes, build and commit via `build(scripts): add
        ci-dry-run.sh — surfaced by project-reorganization/section-09.3
        retrospective`.

---

## 09.4 /tpr-review Full Plan Execution

**File(s):** No edits — independent dual-source review

Run `/tpr-review` on the full plan directory. This is the mandatory independent review per CLAUDE.md §Fix Completeness. If findings come back, they must all be triaged before §09 completes.

- [ ] Invoke `/tpr-review`:
  ```
  Skill: tpr-review
  Args: plans/project-reorganization/
  ```

- [ ] Wait for the dual-source review to complete (expect ~20-45 min wall time per the dual-tpr polling protocol — this is the legitimate review timeout window, NOT a test command)

- [ ] Review findings:
  - [ ] Zero findings: `/tpr-review` clean, proceed to 09.5
  - [ ] Findings: record in §09.R, triage each per CLAUDE.md §NEVER reason out of TPR findings:
    - Accepted findings → fix them, re-run `/tpr-review` until clean
    - Rejected findings → document rejection rationale with explicit architectural reason
    - NEVER use "pre-existing", "out of scope", "future improvement", or any deferral language

- [ ] **Subsection close-out (09.4)** — MANDATORY before starting 09.5:
  - [ ] `/tpr-review` executed
  - [ ] Findings triaged (all fixed or rejected with rationale)
  - [ ] Re-runs until clean (if findings existed)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] `third_party_review.status` in section-09 frontmatter → `resolved` if findings existed, `none` if clean
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        /tpr-review flow is already well-instrumented via the dual-tpr-gemini
        plan. No likely tooling gap at the per-subsection level. Document:
        "Retrospective 09.4: /tpr-review tooling is mature; no gaps."

---

## 09.5 /impl-hygiene-review Full Plan

**File(s):** No edits — hygiene review

Run `/impl-hygiene-review` on the plan's scope. Per CLAUDE.md §Fix Completeness, this MUST run AFTER `/tpr-review` is clean — hygiene review assumes TPR-level correctness.

- [ ] Invoke `/impl-hygiene-review`:
  ```
  Skill: impl-hygiene-review
  ```
  (The skill autoscopes to the active work arc per its skill description.)

- [ ] Wait for hygiene review completion (similar 20-45 min timeout window)

- [ ] Review findings in impl-hygiene vocabulary (LEAK, DRIFT, GAP, WASTE, EXPOSURE, BLOAT, NOTE):
  - [ ] Zero critical/major findings: proceed
  - [ ] Findings: triage each:
    - `LEAK` findings → fix (the reorganization introduced a leak, which is a blocker)
    - `DRIFT` findings → fix (the reorganization failed to sync a reference)
    - `WASTE` findings → fix (unnecessary artifacts introduced by the reorganization)
    - `BLOAT` findings → reconsider scope (did we migrate too much scratchpad content?)
    - `NOTE` findings → document and move on (may be optional improvements)

- [ ] **Subsection close-out (09.5)** — MANDATORY before starting 09.6:
  - [ ] `/impl-hygiene-review` executed
  - [ ] All findings triaged
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** —
        similar to 09.4, the impl-hygiene-review flow is mature. Document:
        "Retrospective 09.5: /impl-hygiene-review tooling is mature; no gaps."

---

## 09.6 Mission Success Criteria Checklist & Plan Close

**File(s):** `plans/project-reorganization/00-overview.md` (frontmatter update + success criteria checkboxes), `plans/project-reorganization/index.md` (status updates)

The final step — mark the plan complete by updating the overview's mission success criteria checkboxes to `[x]`, transitioning the overview's `status: not-started` to `status: complete`, and updating the index.

- [ ] Open `00-overview.md` and check off each mission success criterion:
  - [ ] Criterion: "Root entry inventory" → checked (delivered by §02, §03, §04, §05, §06, §07, §08)
  - [ ] Criterion: "Zero stale root references" → checked (delivered by §08)
  - [ ] Criterion: "Lefthook pre-commit passes" → checked (delivered by §08, verified 09.1)
  - [ ] Criterion: "CI green" → checked (delivered by §08, verified 09.3)
  - [ ] Criterion: "./test-all.sh green" → checked (delivered by §08, verified 09.1)
  - [ ] Criterion: "Blog is served from website" → checked (delivered by §04)
  - [ ] Criterion: "LSP deletion is atomic" → checked (delivered by §06)
  - [ ] Criterion: "Scratchpad is empty or absent" → checked (delivered by §07)
  - [ ] Criterion: "Gitignore drift bug fixed" → checked (delivered by §03)
  - [ ] Criterion: "Stale website/ paths repaired" → checked (delivered by §02, §05)
  - [ ] Criterion: "Hot-path muscle memory preserved" → checked (delivered by §08)
  - [ ] Criterion: "/tpr-review clean" → checked (delivered by §09.4)
  - [ ] Criterion: "/impl-hygiene-review clean" → checked (delivered by §09.5)
  - [ ] Criterion: "./test-all.sh green after all sections complete" → checked (delivered by §09.1)

- [ ] Update overview frontmatter: `status: not-started` → `status: complete`

- [ ] Update `index.md` Quick Reference table: all sections status → `Complete`

- [ ] Update every section's frontmatter `status` field to `complete`:
  ```bash
  cd /home/eric/projects/ori_lang
  for f in plans/project-reorganization/section-*.md; do
    sed -i 's/^status: not-started$/status: complete/' "$f"
  done
  grep -l 'status: not-started' plans/project-reorganization/ || echo "all sections complete"
  ```

- [ ] Commit the final plan-sync update:
  ```bash
  git add plans/project-reorganization/
  git commit -m "docs(plan): mark project-reorganization plan complete

All 14 mission success criteria in 00-overview.md satisfied.
All 9 sections (§01-§09) complete.
/tpr-review clean, /impl-hygiene-review clean.
CI dry-run passed, ./test-all.sh green, lefthook green.

Final state (vs §01 baseline):
- 46 → ~38 visible root entries
- 37 → ~32 tracked root entries
- 11 → 6 root .sh files (install, setup + 4 hot-path wrappers)
- 747 → 747 total references (sweep preserved hot-path, updated cleanly-moved)
- 8 internal dev scripts consolidated to scripts/dev/
- 16 diagnostics consolidated to scripts/diagnostics/
- tools/ori-lsp deleted
- scratchpad/ triaged (12 migrated, 18 deleted, directory removed; 30 files processed matching §01 baseline)
- blog/ migrated cross-repo to ori-lang-website
- 2 build/debug leak files removed, .gitignore /build/ rule added
- 2 stale website/ paths repaired
- profile.json.gz preserved (intentional)
- install.sh + setup.sh preserved (public API)

Refs: plans/project-reorganization/"
  ```

- [ ] Verify final commit:
  ```bash
  git log --oneline -1
  # Expected: docs(plan): mark project-reorganization plan complete
  ```

- [ ] **Subsection close-out (09.6)** — MANDATORY before 09.R:
  - [ ] All mission criteria checked
  - [ ] Overview status → complete
  - [ ] Index updated
  - [ ] Final commit landed
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        sed-update-all-sections-status pattern is a reusable one. Is there
        a `scripts/dev/plan-mark-complete.sh <plan-dir>` helper that
        updates overview status + section statuses + index status in one
        command? Likely useful for every future plan. If yes, build and
        commit via `build(scripts): add plan-mark-complete.sh — surfaced
        by project-reorganization/section-09.6 retrospective`.

---

## 09.R Third Party Review Findings

<!-- Dual-source /tpr-review iteration 3 (2026-04-11) caught a low-severity propagation gap in §09.2's baseline reconciliation counts. Resolved inline. -->

- [x] `[TPR-XX-005-codex][low]` (iteration 3) `plans/project-reorganization/section-09-verification.md:252` — DRIFT: Finish the 12/18 scratchpad math update in §09.
  Evidence: §09.2 still expected "14 scratchpad migrations" at line 252 and "17-18 scratchpad deletions" at line 253, even though §07 and the overview were reconciled in iteration 2 to 12 migrations and 18 deletions. The final commit template at line 457 was fixed in iteration 2, but the baseline-reconciliation bullets at §09.2 were not.
  Impact: The final verification math still disagreed with the canonical §07 inventory, so the end-state diff review could look "wrong" even when the implementation matched the corrected 30-file scratchpad contract.
  Required plan update: Update §09.2's expected added/deleted counts to the reconciled scratchpad numbers.
  Basis: fresh_verification. Confidence: low.
  Resolved: Fixed on 2026-04-11 iteration 3. Updated §09.2 "Expected category counts" bullets with the reconciled 12/18 math and the per-destination schema breakdown. Added the `_repo-root.sh` new helper to the Added count. Called out in the Modified bullet that `.gitignore` is UNCHANGED per the TPR-XX-002-codex iteration 3 correction (iteration 1's false-premise `/build/` edit was withdrawn).

- None at plan start. Populated during §09.4 execution. (Updated 2026-04-11 iteration 3 with 1 resolved finding above.)

---

## 09.N Completion Checklist

- [ ] Full local verification sweep green (09.1)
- [ ] Baseline reconciliation documented (09.2)
- [ ] CI dry-run green (09.3)
- [ ] `/tpr-review` clean (09.4)
- [ ] `/impl-hygiene-review` clean after TPR (09.5)
- [ ] All mission success criteria checked (09.6)
- [ ] Overview `status: complete` (09.6)
- [ ] Final plan-close commit landed (09.6)
- [ ] Plan annotation cleanup: N/A (plan did not add temporary annotations to `.rs` files)
- [ ] **Plan sync** — final synchronization:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table for §09 → `Complete`
  - [ ] `00-overview.md` all mission success criteria checkboxes → `[x]`
  - [ ] `00-overview.md` frontmatter `status: not-started` → `status: complete`
  - [ ] `index.md` Quick Reference table all sections → `Complete`
  - [ ] No cross-plan blocker resolutions needed (this plan did not resolve any other plan's blockers — it's self-contained)
- [ ] `/tpr-review` passed (already executed in 09.4 — this is a self-reference for the completion checklist; the review IS the work of 09.4)
- [ ] `/impl-hygiene-review` passed (already executed in 09.5)
- [ ] `/improve-tooling` **section-close sweep** — per-subsection retrospectives (09.1-09.6) should already be committed. Cross-subsection pattern check: the biggest reusable helpers surfaced by §09 are `plan-diff-summary.sh` (09.2), `ci-dry-run.sh` (09.3), and `plan-mark-complete.sh` (09.6). Verify each was built (or the decision to not build was documented). ALSO: given this is the final section of the entire plan, do a **plan-wide tooling audit** — look across ALL 9 sections' retrospectives and identify the most valuable helpers to formalize. Commit any missed helpers via `build(scripts): finalize project-reorganization plan tooling — plan-wide close sweep`. The plan-wide audit is what makes `/improve-tooling` discipline compound across many plans.

**Exit Criteria:** The reorganization is complete. `00-overview.md` status is `complete`. Every mission success criterion is checked. `./test-all.sh` green, `./full-check.sh` green, `lefthook run pre-commit` green, `cargo c/cl/b/t/st` all green, `./scripts/sync-version.sh --check` green. CI dry-run PR was green and closed. `/tpr-review` and `/impl-hygiene-review` both came back clean (or all findings were triaged and resolved). Global grep sweep returns zero stale references (outside archival `plans/completed/`). The baseline reconciliation shows a coherent before/after diff matching the expected scope from the overview. The plan is ready for merge and §00 overview's `Quick Reference` table shows all 9 sections `Complete`. The project-reorganization plan directory can be moved to `plans/completed/project-reorganization/` in a follow-up maintenance commit (not part of this plan's scope, per the convention that plans self-archive to completed/ only after their merge).
