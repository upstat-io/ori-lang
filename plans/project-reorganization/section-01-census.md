---
section: "01"
title: "Census & Classification Baseline"
status: not-started
reviewed: false
goal: "Produce a frozen, citable baseline snapshot of the current repository root state, reference blast radius, and script classification before any other section modifies anything — so every subsequent section can cite this baseline as its ground truth and any mid-execution drift is immediately detectable."
success_criteria:
  - "`./test-all.sh` captured as green at plan-execution start; baseline result committed as `plans/project-reorganization/baseline/test-all-before.log`"
  - "`git status --porcelain` captured; baseline saved as `plans/project-reorganization/baseline/git-status-before.txt`"
  - "`ls -la` of `/home/eric/projects/ori_lang/` captured; baseline saved as `plans/project-reorganization/baseline/root-ls-before.txt`"
  - "Refreshed reference count via `rg '\\./(test-all|clippy-all|build-all|fmt-all|full-check|llvm-test|llvm-build|llvm-clippy|install|setup|rebuild-playground)\\.sh' --files-with-matches | wc -l` and `| wc -l` returns `230` files and `747` total matches (or an updated number which supersedes the overview's metric)"
  - "Public script externality re-verified: `rg 'install\\.sh' README.md docs/guide/ CONTRIBUTING.md ../ori-lang-website/ compiler/ori_llvm/src/aot/runtime.rs` returns at least 5 matches; the overview classification is confirmed or updated"
  - "Every entry in the overview's Known Bugs table is still reproducible at plan-execution start (no silent fixes that would mask later detection)"
  - "Baseline artifacts committed in a single commit titled `docs(plan): freeze project-reorganization baseline` so all subsequent sections can `git diff` against it"
inspired_by:
  - "rustc `x.py` baseline snapshots before multi-commit refactors"
  - "Swift SIL verifier baseline captures before ARC optimizer changes"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Baseline Snapshot Capture"
    status: not-started
  - id: "01.2"
    title: "Reference Inventory Refresh"
    status: not-started
  - id: "01.3"
    title: "Public-API Script Externality Re-verification"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Census & Classification Baseline

**Status:** Not Started
**Goal:** Freeze the current repository state as a reference baseline — `./test-all.sh` green, `git status` clean, root layout captured, reference blast radius refreshed, public-script externality re-confirmed — before any subsequent section begins to modify anything. This section produces artifacts that every other section cites to prove "we started here" and to detect mid-execution drift.

**Success Criteria:**

- [ ] `./test-all.sh` green captured as `plans/project-reorganization/baseline/test-all-before.log` (full stdout + stderr, timestamped)
- [ ] `git status --porcelain` captured as `plans/project-reorganization/baseline/git-status-before.txt`
- [ ] `ls -la` of ori_lang root captured as `plans/project-reorganization/baseline/root-ls-before.txt`
- [ ] `git ls-files` root-entry list captured as `plans/project-reorganization/baseline/tracked-root-before.txt`
- [ ] Refreshed `rg` blast radius matches or supersedes overview's 747/230 metric — any discrepancy is investigated and resolved (not ignored)
- [ ] `install.sh` / `setup.sh` / `rebuild-playground.sh` externality claims from the Pass 1 research are re-verified against the current working tree
- [ ] Baseline commit lands cleanly: `git log --oneline -1` shows `docs(plan): freeze project-reorganization baseline`
- [ ] Satisfies mission criterion: "**Root entry inventory**" — the baseline is the taxonomy ground truth

**Context:** This plan reorganizes 46+ root entries, sweeps 747 script references across 230 files, deletes ~1200 lines of dead code, and touches a sibling repository. Before a single file is moved, the starting state must be frozen as a citable artifact. Subsequent sections (§02 through §08) each compare their post-work state against this baseline to detect regressions. Without §01 as a ground-truth anchor, mid-execution drift is invisible: we'd have no way to prove that §04 didn't accidentally perturb something §03 touched, or that §08's reference sweep didn't miss a file that was referenced in the initial Pass 1 research but had drifted by the time §08 ran.

The Pass 1 research agents produced the census data used throughout `00-overview.md`. §01 refreshes that data at plan-execution start and commits the refreshed numbers as the authoritative baseline. Any delta between Pass 1's numbers and §01's numbers indicates the repo drifted during plan-writing — which is expected and acceptable, as long as the delta is captured and reconciled.

**Reference implementations:**
- **rustc** (`src/ci/` baseline gather scripts) — CI captures `git describe`, `rustc --version`, and toolchain state as a "reproduction baseline" before any multi-commit refactor lands
- **Swift** (`utils/build-script-impl` preflight checks) — every multi-commit SIL pass change captures the full SIL output of a canonical test program before the change; post-change SIL is diffed against this baseline to prove exact RC operation preservation

**Depends on:** None (this is the first section; it is the foundation everything else cites).

---

## 01.1 Baseline Snapshot Capture

**File(s):** `plans/project-reorganization/baseline/` (new directory, 4 new files)

Capture the starting state of `ori_lang/` as immutable, committed artifacts that subsequent sections compare against. The baseline must be captured BEFORE any section begins work, and committed as a single atomic `docs(plan):` commit so the timestamp is the clear T=0 anchor.

- [ ] Create the baseline directory:
  ```bash
  mkdir -p /home/eric/projects/ori_lang/plans/project-reorganization/baseline
  ```

- [ ] Capture `./test-all.sh` baseline (MUST be green — if it's not green, the plan cannot proceed; fix the pre-existing failure via `/fix-bug` first per CLAUDE.md §Zero Deferral). **Use `set -o pipefail` or capture via `${PIPESTATUS[0]}`** — a naked `cmd | tee file` followed by `$?` returns `tee`'s exit code (always 0), NOT the real `./test-all.sh` exit code, and a failing red baseline would silently be recorded as `exit code: 0`, poisoning every subsequent section (TPR-XX-001-codex iteration 3 fix — 2026-04-11):
  ```bash
  cd /home/eric/projects/ori_lang
  date "+Baseline captured %Y-%m-%d %H:%M:%S %Z" > plans/project-reorganization/baseline/test-all-before.log
  # Enable pipefail so the pipeline exit code reflects test-all.sh, not tee
  set -o pipefail
  timeout 150 ./test-all.sh 2>&1 | tee -a plans/project-reorganization/baseline/test-all-before.log
  test_all_exit=$?
  set +o pipefail
  echo "test-all exit code: $test_all_exit" >> plans/project-reorganization/baseline/test-all-before.log
  ```
  Alternative using `${PIPESTATUS[0]}` (explicit producer-side capture, no pipefail required):
  ```bash
  timeout 150 ./test-all.sh 2>&1 | tee -a plans/project-reorganization/baseline/test-all-before.log
  test_all_exit=${PIPESTATUS[0]}
  echo "test-all exit code: $test_all_exit" >> plans/project-reorganization/baseline/test-all-before.log
  ```
  - [ ] Verify the log shows a green run (no red failures, `test-all exit code: 0`)
  - [ ] Verify the recorded `test_all_exit` reflects `./test-all.sh`, NOT `tee` (sanity check: deliberately inject `false |` between `./test-all.sh` and `tee` in a scratch run — you should see `test-all exit code: 1`, not 0)
  - [ ] If red: STOP. Do not proceed with plan execution. File the failure via `/add-bug` and fix via `/fix-bug` before returning to this plan.

- [ ] Capture `git status --porcelain` baseline (may be dirty — that's OK, this just records the starting point):
  ```bash
  cd /home/eric/projects/ori_lang
  git status --porcelain > plans/project-reorganization/baseline/git-status-before.txt
  ```
  - [ ] Verify: the file exists and contains exactly the lines shown by `git status --porcelain`

- [ ] Capture root directory listing (both `ls -la` and `ls -1` for easy diff):
  ```bash
  cd /home/eric/projects/ori_lang
  ls -la > plans/project-reorganization/baseline/root-ls-before.txt
  ls -1 | sort > plans/project-reorganization/baseline/root-ls1-before.txt
  ```
  - [ ] Verify: `root-ls-before.txt` shows all 46 visible entries from the Pass 1 census
  - [ ] Verify: the count matches `wc -l plans/project-reorganization/baseline/root-ls1-before.txt`

- [ ] Capture `git ls-files` for root-level entries (tracked-only list, for git-diff purposes):
  ```bash
  cd /home/eric/projects/ori_lang
  git ls-files | awk -F/ '{print $1}' | sort -u > plans/project-reorganization/baseline/tracked-root-before.txt
  ```
  - [ ] Verify: file exists and contains ~37 tracked root entries per the Pass 1 census

- [ ] **Verification**: run each capture command once and re-capture to disk — the commands must be deterministic (no timestamps embedded in capture payload beyond the single header line).

- [ ] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [ ] All tasks above are `[x]` and all 4 baseline files exist
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the capture journey: did `./test-all.sh` take unexpectedly long? Was
        the tee+timeout+exit-code pattern something any existing diagnostic
        script already solves? Is there friction in manually capturing four
        snapshots that a single `scripts/dev/snapshot-baseline.sh` would
        eliminate? If yes, add the helper script NOW and commit via
        `build(diagnostics): add snapshot-baseline.sh — surfaced by
        project-reorganization/section-01.1 retrospective`. If no: document
        "Retrospective 01.1: no tooling gaps — direct bash capture sufficient."
        Do not silently skip. See `.claude/skills/improve-tooling/SKILL.md`
        "Per-Subsection Workflow" for the full protocol.

---

## 01.2 Reference Inventory Refresh

**File(s):** `plans/project-reorganization/baseline/reference-inventory-before.txt` (new)

Re-run the exact `ripgrep` command the Pass 1 research used and commit the result as the authoritative blast-radius snapshot. The overview's claim of **747 matches / 230 files** was produced during Pass 1; between Pass 1 and plan execution, other ori_lang work may have added or removed references. §01.2 refreshes the count and either confirms or supersedes the overview's metric.

**Context:** §08 (Script Consolidation) uses this baseline to prove its atomic reference sweep is complete. If the baseline says 747 references exist and §08's post-sweep count is `747 - N` (where `N` is the number of references that still legitimately point to unchanged hot-path wrapper paths at root), the math proves zero stale references.

- [ ] Run the refreshed blast-radius `rg` in two forms (full matches and file-count):
  ```bash
  cd /home/eric/projects/ori_lang
  # Full grep with line numbers and file paths
  rg -n '\./(test-all|clippy-all|build-all|fmt-all|full-check|llvm-test|llvm-build|llvm-clippy|install|setup|rebuild-playground)\.sh' \
    > plans/project-reorganization/baseline/reference-inventory-before.txt

  # Per-script counts (for math validation against overview metrics)
  for script in test-all clippy-all build-all fmt-all full-check llvm-test llvm-build llvm-clippy install setup rebuild-playground; do
    count=$(rg -c "\./${script}\.sh" 2>/dev/null | awk -F: '{sum += $NF} END {print sum+0}')
    files=$(rg -l "\./${script}\.sh" 2>/dev/null | wc -l)
    printf "%-25s matches=%5d files=%5d\n" "$script" "$count" "$files"
  done > plans/project-reorganization/baseline/reference-counts-before.txt
  ```

- [ ] Compare the refreshed counts against the overview's metrics:
  - Overview says: 747 matches / 230 files, test-all.sh=503/224, clippy-all.sh=175/108, etc.
  - [ ] If refreshed numbers match: confirm overview. No action needed.
  - [ ] If refreshed numbers differ by more than ~5%: investigate the delta. Did a section-07-enum-repr edit land between Pass 1 and now? Was a new plan file added? Update the overview's Metrics table IN THIS COMMIT (same commit as the baseline capture) with the refreshed numbers and a note explaining the delta.
  - [ ] If refreshed numbers differ dramatically (>10%): STOP. The plan's assumptions may be invalid. Re-run the Pass 1 research before proceeding.

- [ ] **Verification — hot-path 4 are actually hot-path**: the overview's Option C decision rests on test-all/full-check/fmt-all/build-all being the most-invoked scripts. Re-verify this is still true by checking the per-script counts. If `clippy-all.sh` has more references than `build-all.sh` (it does — 175 vs 2), that's expected and already baked into the decision (clippy-all is #2 by count but NOT hot-path by daily-use frequency).

- [ ] **Subsection close-out (01.2)** — MANDATORY before starting 01.3:
  - [ ] Both baseline files exist and contain expected data
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the counting journey: did the per-script loop feel clunky? Is there
        a single `rg` invocation that would produce per-pattern counts directly
        (e.g., `rg --count-matches` with an alternation pattern)? Would a
        reusable `scripts/dev/ref-blast-radius.sh` helper be worth adding so
        future plans don't reinvent this query? If yes, add it now and commit
        via `build(scripts): add ref-blast-radius.sh — surfaced by
        project-reorganization/section-01.2 retrospective`. If no: document
        "Retrospective 01.2: no tooling gaps — per-script `rg -c` loop
        adequate for one-time baseline."

---

## 01.3 Public-API Script Externality Re-verification

**File(s):** `plans/project-reorganization/baseline/externality-verification.txt` (new)

The overview classifies `install.sh` as PUBLIC_API (externally surfaced via README, docs/guide, runtime error messages, and `ori-lang-website/package.json:7`), `setup.sh` as semi-public (CONTRIBUTING.md:14), and `rebuild-playground.sh` as obsolete (0 references, dead `./website/playground-wasm/` path). Before §05 deletes rebuild-playground.sh and §08 leaves install.sh + setup.sh alone, re-verify these classifications are still valid at plan-execution time.

**Context:** Script externality can drift quickly — a blog post, a tutorial, a new CI script, or a sibling repo update could add or remove an external reference between Pass 1 and plan execution. If `install.sh` is no longer referenced by README (for whatever reason), its public classification changes and §08 might be able to treat it differently. If `rebuild-playground.sh` suddenly gains a reference (unlikely but possible), §05 cannot delete it without breaking the new consumer.

- [ ] Re-verify `install.sh` is externally surfaced:
  ```bash
  cd /home/eric/projects/ori_lang
  {
    echo "=== install.sh references ==="
    rg -n 'install\.sh' README.md CONTRIBUTING.md docs/guide/ ../ori-lang-website/ compiler/ori_llvm/src/aot/runtime.rs 2>&1
    echo ""
    echo "=== Expected (from Pass 1): ==="
    echo "- README.md: 3+ refs (curl install command)"
    echo "- docs/guide/01-getting-started.md: 1 ref"
    echo "- compiler/ori_llvm/src/aot/runtime.rs:168: 1 ref (error message)"
    echo "- ori-lang-website/package.json: 1 ref"
  } >> plans/project-reorganization/baseline/externality-verification.txt
  ```
  - [ ] Verify: at least 5 external references exist. If fewer: investigate before proceeding (install.sh may have lost its public surface, which would change §08's handling).

- [ ] Re-verify `setup.sh` is referenced by CONTRIBUTING.md:
  ```bash
  {
    echo ""
    echo "=== setup.sh references ==="
    rg -n 'setup\.sh' CONTRIBUTING.md docs/ 2>&1 || echo "no matches"
  } >> plans/project-reorganization/baseline/externality-verification.txt
  ```
  - [ ] Verify: CONTRIBUTING.md:14 reference exists. If not: investigate.

- [ ] Re-verify `rebuild-playground.sh` is dead (no references, dead path):
  ```bash
  {
    echo ""
    echo "=== rebuild-playground.sh references ==="
    rg -n 'rebuild-playground\.sh' . 2>&1 | grep -v '^plans/project-reorganization/' || echo "no matches outside plan dir"
    echo ""
    echo "=== Does ./website/ still exist in ori_lang? ==="
    ls -d ./website 2>&1 || echo "website/ does not exist (expected — was moved to sibling repo)"
    echo ""
    echo "=== Is rebuild-playground.sh still referenced in ori-lang-website? ==="
    rg -n 'rebuild-playground' ../ori-lang-website/ 2>&1 || echo "no matches in ori-lang-website (expected — website has its own scripts/rebuild-wasm.sh)"
  } >> plans/project-reorganization/baseline/externality-verification.txt
  ```
  - [ ] Verify: no references outside this plan directory (the plan itself mentions rebuild-playground.sh, which is fine). If any external reference appears: STOP, re-classify the script, update §05.

- [ ] Commit the baseline in a single atomic commit:
  ```bash
  cd /home/eric/projects/ori_lang
  git add plans/project-reorganization/baseline/
  git commit -m "docs(plan): freeze project-reorganization baseline

Capture pre-execution state of:
- ./test-all.sh (green)
- git status + ls -la
- Reference blast radius (rg inventory)
- Public-API script externality verification

Baseline artifacts are the ground truth cited by subsequent sections §02-§09
to detect mid-execution drift."
  ```

- [ ] **Subsection close-out (01.3)** — MANDATORY before starting 01.R:
  - [ ] Externality verification file exists and contains all three checks
  - [ ] Baseline commit has landed (verify with `git log --oneline -1`)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the verification journey: was the externality-check pattern
        (bash block with echo headers + rg + fallback echo) clunky enough to
        warrant a reusable `scripts/dev/check-script-externality.sh` helper
        that takes a script name and prints all external references? Future
        reorg plans will need this exact check. If yes, add now and commit
        via `build(scripts): add check-script-externality.sh — surfaced by
        project-reorganization/section-01.3 retrospective`. If no: document
        "Retrospective 01.3: one-time baseline check; no reusable tooling
        warranted." Either way, do not silently skip.

---

## 01.R Third Party Review Findings

<!-- Dual-source /tpr-review iteration 3 (2026-04-11) surfaced one finding on §01.1 baseline capture. Resolved inline. -->

- [x] `[TPR-XX-001-codex][high]` (iteration 3) `plans/project-reorganization/section-01-census.md:83` — GAP: Capture the real test-all exit status in §01.
  Evidence: §01.1 ran `timeout 150 ./test-all.sh 2>&1 | tee -a ...` and then captured `$?` on the next line. In POSIX shells without `pipefail`, `$?` reflects the LAST command in the pipeline (i.e., `tee`, which always exits 0 unless the log file is unwritable), NOT the producer (`./test-all.sh`). A failing red baseline would be written to `test-all-before.log` as `exit code: 0`, blessing a broken starting state as green.
  Impact: Poisons every subsequent section that treats §01 as the plan's ground-truth baseline. If §01 passes green but test-all is actually red, every `./test-all.sh green` success criterion downstream becomes false.
  Required plan update: Enable `set -o pipefail` for the capture block or read `${PIPESTATUS[0]}` to capture the producer's real exit code.
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11 iteration 3. Updated §01.1 to use `set -o pipefail` with explicit `test_all_exit=$?` capture, and provided an alternative form using `${PIPESTATUS[0]}`. Added a sanity-check verification item: "deliberately inject `false |` between `./test-all.sh` and `tee` in a scratch run — you should see `test-all exit code: 1`, not 0". The exit code in `test-all-before.log` now reflects the real `./test-all.sh` status.

---

## 01.N Completion Checklist

- [ ] All 4 baseline files exist in `plans/project-reorganization/baseline/`:
  - `test-all-before.log` (green test run captured)
  - `git-status-before.txt`
  - `root-ls-before.txt` + `root-ls1-before.txt`
  - `tracked-root-before.txt`
  - `reference-inventory-before.txt` + `reference-counts-before.txt`
  - `externality-verification.txt`
- [ ] `./test-all.sh` baseline log shows `exit code: 0`
- [ ] Refreshed reference count matches or supersedes overview's 747/230 metric (delta investigated if >5%)
- [ ] `install.sh`, `setup.sh`, `rebuild-playground.sh` classification re-verified (install = public, setup = semi-public, rebuild-playground = obsolete)
- [ ] Baseline commit lands cleanly as `docs(plan): freeze project-reorganization baseline` (single atomic commit, `git log --oneline -1` confirms)
- [ ] Plan annotation cleanup: N/A for this section (no `.rs` files modified)
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, all subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status for §01 updated to `Complete`
  - [ ] `00-overview.md` mission success criteria checkboxes updated if now satisfied (§01 contributes to "Root entry inventory" criterion — mark as partial until §02-§08 land)
  - [ ] `index.md` Quick Reference table updated
  - [ ] Refreshed metrics propagated into `00-overview.md` Metrics tables (if reference count changed)
- [ ] `/tpr-review` passed (final, full-section) — independent dual-source review (Codex + Gemini) found no critical or major issues. **Note**: §01 is an extremely low-risk section (capture artifacts only, no file moves or deletions). TPR should be a rapid review; anything beyond minor wording feedback indicates the section is overcomplicated.
- [ ] `/impl-hygiene-review` passed — hygiene review found no critical or major findings. MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` **section-close sweep** — MANDATORY safety net. Per-subsection retrospectives (01.1, 01.2, 01.3) should already be committed. Verify each has either an "improvements made" entry OR a documented "no gaps" negative finding. Cross-subsection pattern check: was there friction in running three separate bash captures that a single `scripts/dev/freeze-plan-baseline.sh <plan-name>` helper would eliminate? If yes, add it now as a reusable helper for future multi-commit plans and commit via `build(scripts): add freeze-plan-baseline.sh — surfaced by project-reorganization/section-01 close sweep`. If no cross-cutting gaps: document "Section-01 close sweep: per-subsection retrospectives covered everything; no cross-subsection patterns required new tooling."

**Exit Criteria:** The `plans/project-reorganization/baseline/` directory contains 7+ files recording the exact starting state of the repository. `git log --oneline -1` shows `docs(plan): freeze project-reorganization baseline`. Every subsequent section (§02-§09) can cite this commit's hash as T=0 and `git diff <baseline-commit>..HEAD` will show the cumulative effect of the reorganization. The baseline commit is immutable — it is NEVER amended, reverted, or squashed. If the baseline proves wrong mid-execution (e.g., a reference count was miscounted), a follow-up `docs(plan): amend project-reorganization baseline` commit adds corrected data; the original baseline commit stays in history.
