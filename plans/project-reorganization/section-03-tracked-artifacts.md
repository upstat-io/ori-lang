---
section: "03"
title: "Tracked Artifact Removal & .gitignore Drift Fix"
status: not-started
reviewed: false
goal: "Remove the two tracked `build/debug/*` artifacts (`simplest_crash.ll` and `test_simple`) via `git rm --cached`. **No gitignore edit is needed** (TPR-XX-002-codex iteration 3 fix — the iteration 1 premise that `**/build/` doesn't match root-level `build/` was factually wrong; `**/build/` DOES match root-level `build/` as verified via `git check-ignore -v --no-index`; the files are tracked because they were committed before the rule was added, and tracked files override gitignore)."
success_criteria:
  - "`git ls-files build/` returns zero results after this section"
  - "`git check-ignore -v build/debug/simplest_crash.ll` returns a matched `.gitignore` rule (proves the leak is plugged for future files)"
  - "`git ls-files compiler/oric/src/commands/build/` still returns tracked files (proves the negative rule for compiler internals still works)"
  - "`profile.json.gz` remains tracked (intentional performance reference; NOT deleted by this section)"
  - "`./test-all.sh` green post-fix (the gitignore change cannot break any test)"
  - "Satisfies mission criterion: '**Gitignore drift bug fixed**'"
inspired_by:
  - "rustc `.gitignore` pattern pairing — root-anchored `/target/` PLUS `**/target/` with negative rules for source files named `target`"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Tracked Artifact Removal"
    status: not-started
  - id: "03.2"
    title: "Gitignore Verification (No Edit Required)"
    status: not-started
  - id: "03.3"
    title: "Negative Rule Verification"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Tracked Artifact Removal & .gitignore Drift Fix

**Status:** Not Started
**Goal:** Remove the two tracked `build/debug/*` artifacts (`simplest_crash.ll` and `test_simple`) that were committed to git before (or alongside) the existing `**/build/` gitignore rule was added, and that therefore persist as tracked despite the rule. **Important architectural correction (TPR-XX-002-codex iteration 3 fix — 2026-04-11)**: the `**/build/` gitignore rule ALREADY correctly matches these paths — verified empirically via `git check-ignore -v --no-index build/debug/simplest_crash.ll` which returns `.gitignore:5:**/build/`. The reason Pass 1 research saw "no output" from `git check-ignore` (without `--no-index`) is that git's `check-ignore` command by design returns "not ignored" for files that are already TRACKED — tracked files override ignore rules. This is **not a gitignore drift bug**; it's **already-tracked-files-that-should-be-untracked**. The fix is `git rm --cached` on the two files; no gitignore edit is needed (the `**/build/` rule already protects against future leaks).

**Success Criteria:**

- [ ] `git ls-files build/` returns empty — both leaked files are no longer tracked
- [ ] `git check-ignore -v --no-index build/debug/simplest_crash.ll` returns a matched rule (verifying `**/build/` already covers the path for future files — using `--no-index` because `check-ignore` ignores tracked files by default)
- [ ] `git ls-files compiler/oric/src/commands/build/` returns its existing tracked files (the negative rule `!compiler/oric/src/commands/build/` is still working)
- [ ] `profile.json.gz` is still in `git ls-files` (this section does NOT delete it — it's intentionally tracked as a perf reference)
- [ ] `.gitignore` is UNCHANGED from plan-start state (no new `/build/` rule added — that was my iteration 1 false premise corrected in iteration 3; the existing `**/build/` already handles root-level `/build/` correctly via its `**` path component semantics)
- [ ] `./test-all.sh` green post-fix
- [ ] Single atomic commit lands: `fix(plan): remove tracked build/debug leak`
- [ ] Satisfies mission criterion: "**Gitignore drift bug fixed**" (reclassified: the bug is tracked-files-that-should-not-be, NOT an actual gitignore pattern gap)

**Context (TPR iteration 3 rewrite):** The Pass 1 research agent ran `git check-ignore -v build/debug/simplest_crash.ll` and received NO output. I originally interpreted this as "the file is not ignored by any rule" and concluded `**/build/` had a pattern gap. **This was wrong.** Fresh empirical verification on 2026-04-11:

```bash
# Tests whether the PATH would be ignored IF it weren't tracked
$ git check-ignore -v --no-index build/debug/simplest_crash.ll
.gitignore:5:**/build/	build/debug/simplest_crash.ll

# Tests whether the file is CURRENTLY subject to ignore (no for tracked files)
$ git check-ignore -v build/debug/simplest_crash.ll
(no output — because tracked files override ignore rules)
```

The `**/build/` rule works correctly — `**` matches zero or more path components, which INCLUDES the zero-component case (root-level `build/`). The rule matches `build/` at any depth, including root. My earlier claim ("gitignore treats `**` as zero-or-more path components, which sometimes excludes the root-level case") was incorrect. `**/build/` DOES match root-level `build/`, full stop.

So why are the files tracked if the rule matches? Because **gitignore rules do not apply retroactively to already-tracked files**. Once a file is in the git index, subsequent additions to `.gitignore` don't remove it — you must explicitly `git rm --cached` it. This is git's normal behavior, not a bug. The files were likely committed before the `**/build/` rule was added to `.gitignore`, or via `git add -f` at some point. The `git check-ignore` command (without `--no-index`) reflects the CURRENT state, which correctly reports "not ignored" for tracked files.

**Implication**: the fix is entirely `git rm --cached` on the two tracked files. The existing `**/build/` rule already prevents FUTURE leaks. No gitignore edit is needed. The previous iteration 1 / iteration 2 instruction to add a root-anchored `/build/` rule is withdrawn as unnecessary redundancy.

The critical constraint is that `compiler/oric/src/commands/build/` is a legitimate source directory containing tracked Rust files. The existing `.gitignore` has two negative rules that preserve it:
```
!compiler/oric/src/commands/build/
!compiler/oric/src/commands/target/
```
These negative rules continue working — no change needed. The `**/build/` rule + the negative rule correctly handle all cases: `build/debug/*` at root is ignored (for future files); `compiler/oric/src/commands/build/` is tracked (negative rule wins).

**Reference implementations:**
- **git gitignore documentation**: `**/build/` matches `build/` at any depth, including root level. The `**` glob matches zero or more path components.
- **TPR iteration 3 empirical verification**: `git check-ignore -v --no-index build/debug/simplest_crash.ll` returned `.gitignore:5:**/build/` — confirming the rule matches. The `--no-index` flag is load-bearing: without it, `check-ignore` reports "not ignored" for tracked files by design, which is what led iteration 1 to the false root-cause.

**Depends on:** §01 (baseline must prove the tracked state before the fix).

---

## 03.1 Tracked Artifact Removal

**File(s):** `build/debug/simplest_crash.ll`, `build/debug/test_simple`

Remove the two leaked files from git tracking (but not from disk — they stay as gitignored local artifacts, matching the intent).

- [ ] Verify the files are currently tracked:
  ```bash
  cd /home/eric/projects/ori_lang
  git ls-files build/
  # Expected: build/debug/simplest_crash.ll + build/debug/test_simple
  ```
  - [ ] Both files are listed. If either is missing, investigate — someone may have already removed it between baseline and now.

- [ ] Verify nothing ELSE is tracked under `build/`:
  ```bash
  git ls-files build/ | wc -l
  # Expected: 2
  ```
  - [ ] If >2: STOP. Another file leaked through the gitignore pattern; investigate and add to the removal list.
  - [ ] If 0: the files were already removed; skip to 03.2 and document "already removed by {commit hash}"

- [ ] Remove the tracked files from git index only (files stay on disk — they are ALREADY covered by the existing `**/build/` rule at `.gitignore:5`, per iteration 3's `--no-index` verification; no new rule is added in 03.2):
  ```bash
  git rm --cached build/debug/simplest_crash.ll build/debug/test_simple
  ```
  - **Why `--cached` and not plain `git rm`**: the files on disk are legitimate local artifacts (debug IR dump and a compiled test binary). Developers may still want them for debugging. `--cached` removes them from git tracking without deleting the files.

- [ ] Verify the removal staged:
  ```bash
  git status | grep 'build/'
  # Expected: 'deleted: build/debug/simplest_crash.ll' and similar for test_simple (staged)
  ```

- [ ] Do NOT commit yet — the .gitignore fix in 03.2 must land in the same commit.

- [ ] **Subsection close-out (03.1)** — MANDATORY before starting 03.2:
  - [ ] Both files are staged for removal (`git status` shows `deleted:` entries)
  - [ ] Files still exist on disk (`ls build/debug/` shows them — they'll be gitignored after 03.2 commits)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on whether `git ls-files build/` was the right discovery command.
        Is there a `scripts/dev/gitignore-leaks.sh` helper that would scan
        the entire repo for "tracked files that `git check-ignore` says should
        be ignored" (the generalization of "find all leaked-past-gitignore
        artifacts")? That would be broadly useful. If yes, add and commit
        via `build(scripts): add gitignore-leaks.sh — surfaced by
        project-reorganization/section-03.1 retrospective`. If no:
        "Retrospective 03.1: no tooling gaps — direct git ls-files adequate."

---

## 03.2 Gitignore Verification (No Edit Required) — iteration 3 correction

**File(s):** `.gitignore` (READ-ONLY verification; NO edits per TPR-XX-002-codex iteration 3 fix)

**CORRECTION — iteration 3**: This subsection previously prescribed adding a root-anchored `/build/` rule to `.gitignore`. That was based on a factually incorrect reading of `git check-ignore` output (we ran it on tracked files without `--no-index`, which returns "not ignored" by design — git's check-ignore defers to the index for tracked files). Fresh empirical verification (2026-04-11):

```bash
$ git check-ignore -v --no-index build/debug/simplest_crash.ll
.gitignore:5:**/build/	build/debug/simplest_crash.ll

$ git check-ignore -v --no-index build/debug/test_simple
.gitignore:5:**/build/	build/debug/test_simple
```

The existing `**/build/` rule at `.gitignore:5` **already matches both files**. No new gitignore rule is needed — the rule works correctly. This subsection is therefore a **verification-only step**, not an editing step.

- [ ] Verify the `**/build/` rule already covers the root-level `build/` path with `--no-index` (bypassing the tracked-file override):
  ```bash
  git check-ignore -v --no-index build/debug/simplest_crash.ll
  git check-ignore -v --no-index build/debug/test_simple
  # Expected output for each: .gitignore:5:**/build/	<path>
  ```
  - [ ] Both commands match `.gitignore:5:**/build/`
  - [ ] Without `--no-index`, both return empty (because §03.1 already removed tracking, or because the files are still in the index). After §03.1 completes AND the commit lands, `git check-ignore -v <path>` (without `--no-index`) should ALSO match the rule because the files are no longer in the index.

- [ ] Verify the existing negative rule for `compiler/oric/src/commands/build/` still applies (to guard against accidentally ignoring source code named `build`):
  ```bash
  # Pick one tracked file from the compiler command directory
  file=$(git ls-files compiler/oric/src/commands/build/ | head -1)
  git check-ignore -v "$file" || echo "not ignored (expected — negative rule wins)"
  ```
  - [ ] Exit code 1 (not ignored — meaning the negative rule at `.gitignore:11:!compiler/oric/src/commands/build/` correctly exempts these files)

- [ ] `.gitignore` is UNCHANGED — verify:
  ```bash
  git diff .gitignore
  # Expected: no output (no changes)
  ```
  - [ ] `.gitignore` diff is empty

- [ ] **Subsection close-out (03.2)** — MANDATORY before starting 03.3:
  - [ ] `.gitignore` is NOT edited in this section (iteration 3 correction — the rule already works)
  - [ ] `git check-ignore -v --no-index` confirms `**/build/` matches root-level `build/`
  - [ ] Negative rule for `compiler/oric/src/commands/build/` still works
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        real tooling gap surfaced here is that `git check-ignore` without
        `--no-index` silently returns "not ignored" for tracked files, which
        misleads newcomers into thinking gitignore patterns don't match.
        Is this gotcha documented anywhere in the repo? If not: is a brief
        `docs/development/gitignore-gotchas.md` or a CLAUDE.md note worth
        adding so future maintainers don't fall into the same trap iteration
        1 fell into? Yes — add it. Commit via
        `docs(development): document git check-ignore tracked-file gotcha —
        surfaced by project-reorganization/section-03.2 retrospective (TPR
        iteration 3)`.

---

## 03.3 Negative Rule Verification

**File(s):** No edits — verification only

Prove that the existing `**/build/` rule still does NOT break the existing negative rules for `compiler/oric/src/commands/build/`. This is the regression guard: since we are NOT adding a new rule in iteration 3, this should be a trivial verification — the negative rules continue to work exactly as they did before this section ran.

- [ ] Verify the tracked contents of `compiler/oric/src/commands/build/` are still tracked:
  ```bash
  git ls-files compiler/oric/src/commands/build/ | head -20
  # Expected: multiple tracked Rust files (mod.rs, etc.)
  ```
  - [ ] At least one file is still tracked
  - [ ] The file list matches the baseline captured in §01.1

- [ ] Verify that git check-ignore treats those files correctly:
  ```bash
  # Pick one tracked file from the command directory
  file=$(git ls-files compiler/oric/src/commands/build/ | head -1)
  git check-ignore -v "$file" || echo "not ignored (expected)"
  ```
  - [ ] Expected result: `git check-ignore -v` returns with exit code 1 and no output, meaning the file is NOT ignored (because the negative rule wins).

- [ ] Verify `./test-all.sh` still green (guards against compiler build breakage from a botched gitignore edit):
  ```bash
  timeout 150 ./test-all.sh 2>&1 | tail -10
  ```
  - [ ] Exit code 0
  - [ ] All phases PASS

- [ ] Verify `profile.json.gz` is still tracked (not accidentally deleted by this section — it's intentional):
  ```bash
  git ls-files profile.json.gz
  # Expected: 'profile.json.gz'
  ```

- [ ] Commit everything from §03 as a single atomic commit. **Per TPR-XX-002-codex iteration 3 fix**: the commit does NOT include a `.gitignore` edit. Only the 2 `git rm --cached` removals land — the existing `**/build/` rule is unchanged.
  ```bash
  cd /home/eric/projects/ori_lang
  # build/debug/simplest_crash.ll and build/debug/test_simple are already staged
  # for removal from 03.1. NO .gitignore edit — the existing **/build/ rule
  # already covers root-level build/ (verified in 03.2 via --no-index).
  git commit -m "fix(plan): remove tracked build/debug leak

Two files were tracked despite the '**/build/' gitignore rule because
they were committed before the rule was added to .gitignore (or via
'git add -f' at some point). Once in the index, gitignore rules no
longer apply — the fix is 'git rm --cached', NOT a gitignore edit.

  build/debug/simplest_crash.ll
  build/debug/test_simple

Empirical verification (2026-04-11 TPR iteration 3):
  git check-ignore -v --no-index build/debug/simplest_crash.ll
  → .gitignore:5:**/build/	build/debug/simplest_crash.ll

The '**/build/' rule at .gitignore:5 already matches root-level build/.
The iteration 1 premise that '**/build/ doesn't match root-level build/
due to ** path component semantics' was factually wrong. The existing
negative rules for compiler/oric/src/commands/build/ are preserved and
continue to work.

profile.json.gz stays tracked (intentional perf reference).

Refs: plans/project-reorganization/section-03-tracked-artifacts.md"
  ```

- [ ] Verify the commit:
  ```bash
  git log --oneline -1
  # Expected: fix(plan): remove tracked build/debug leak

  git show --stat HEAD
  # Expected: build/debug/simplest_crash.ll -1/-0 (deleted),
  #           build/debug/test_simple -1/-0 (deleted)
  # NOT expected: .gitignore change (that was iteration 1's false premise)
  ```

- [ ] **Subsection close-out (03.3)** — MANDATORY before 03.R:
  - [ ] Negative rules verified intact
  - [ ] `./test-all.sh` green
  - [ ] `profile.json.gz` still tracked
  - [ ] Commit landed atomically (3 files in one commit)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the verification approach. Was the "check-ignore on a tracked
        source file to prove negative rule wins" verification obvious, or
        would a `scripts/dev/verify-gitignore.sh` helper that runs standard
        sanity checks (tracked files still tracked, ignored files still
        ignored, pattern collisions flagged) help future .gitignore edits?
        If yes: add and commit via `build(scripts): add verify-gitignore.sh
        — surfaced by project-reorganization/section-03.3 retrospective`.
        Otherwise: "Retrospective 03.3: no tooling gaps — direct git
        check-ignore adequate for single-rule verification."

---

## 03.R Third Party Review Findings

<!-- Dual-source /tpr-review iteration 3 (2026-04-11) surfaced a factual correction: the iteration 1 premise that `**/build/` has a pattern gap was wrong. The rule works; the files are just tracked-too-early. Section rewritten. -->

- [x] `[TPR-XX-002-codex][high]` (iteration 3) `plans/project-reorganization/section-03-tracked-artifacts.md:41` — WASTE: Remove the false /build/ root-cause from §03.
  Evidence: §03 claimed `**/build/` does NOT match root-level `build/` directory due to gitignore pattern resolution. Fresh empirical verification with `git check-ignore -v --no-index build/debug/simplest_crash.ll` returned `.gitignore:5:**/build/ build/debug/simplest_crash.ll` — the rule DOES match. The no-output probe cited in Pass 1 research was from `git check-ignore` WITHOUT `--no-index` run on TRACKED files; git's `check-ignore` defers to the index for tracked files, returning "not ignored" by design. The iteration 1 plan prescribed an unnecessary `/build/` root-anchored rule addition.
  Impact: §03 and 00-overview.md were anchored to incorrect gitignore semantics, prescribing unnecessary `.gitignore` churn instead of identifying the real problem: tracked-files-that-should-not-be (committed before the rule existed, or via `git add -f`).
  Required plan update: Rewrite §03 (goal, success_criteria, context, §03.2 subsection, commit message template) and 00-overview.md (mission success criterion, current-state diagram, Known Bugs table) to treat this as removal of already-tracked build artifacts under an existing ignore rule. Drop the `/build/` edit. Fix verification commands to use `git check-ignore --no-index` where pre-removal ignore semantics are being demonstrated.
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11 iteration 3. Rewrote §03 goal, success_criteria, context paragraph, reference implementations, and §03.2 subsection (now "Gitignore Verification (No Edit Required)" — verification-only, not editing). Updated commit message template to drop the `.gitignore` add. Updated 00-overview.md mission success criterion "Gitignore drift bug fixed" to "Tracked build/debug artifacts removed" with the empirical `--no-index` proof. Updated 00-overview.md Known Bugs row to reflect the correct root cause. The iteration 1 false premise is now explicitly documented and corrected inline.

<!-- Iteration 4 (2026-04-11) caught 4 §03 propagation misses from iteration 3's rewrite (frontmatter title, §03.1 body, §03.3 body, §03.N completion checklist, §03 exit criteria, §09 final commit, §00 overview implementation sequence + effort table, index.md). All fixed in one pass. -->

- [x] `[TPR-XX-001-gemini][high]` + `[TPR-XX-001-codex][medium]` (iteration 4) `plans/project-reorganization/section-03-tracked-artifacts.md:multiple` — Remove vestigial gitignore edit references from §03 and propagate the §03 rewrite to every reference.
  Evidence: Iteration 3's §03 rewrite updated the main body but left stale pre-rewrite text in multiple execution-critical places: §03 frontmatter sections list still titled §03.2 as ".gitignore Root-Anchored /build/ Rule Addition"; §03.1 body said "files will be gitignored by the rule added in 03.2"; §03.3 body said "new /build/ rule must not break..."; §03.N completion checklist had "`/build/` rule added to `.gitignore` (03.2)" and "gitignore root anchor" commit message; §03 exit criteria said "3 file operations (2 deletions + 1 gitignore edit)".
  Impact: The §03 propagation was incomplete — executors following the checklist or reading the frontmatter sections list would still attempt to edit `.gitignore`, contradicting the corrected §03.2 body.
  Required plan update: Rename §03.2 frontmatter title; remove `/build/ rule` text from §03.1 and §03.3 bodies; update §03.N completion checklist items; update §03 exit criteria to say "2 file operations (both deletions)".
  Basis: direct_file_inspection + fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11 iteration 4. (a) §03.2 frontmatter title → "Gitignore Verification (No Edit Required)". (b) §03.1 body: "gitignored by the rule added in 03.2" → "ALREADY covered by the existing `**/build/` rule at `.gitignore:5`, per iteration 3's `--no-index` verification". (c) §03.3 body: "Prove that the new `/build/` rule does NOT break..." → "Prove that the existing `**/build/` rule still does NOT break...". (d) §03.N checklist: "`/build/` rule added to `.gitignore` (03.2)" → "`.gitignore` verified UNCHANGED (03.2)"; "gitignore root anchor" commit message tag removed. (e) §03 exit criteria: "3 file operations (2 deletions + 1 gitignore edit)" → "**2 file operations** (both deletions; NO `.gitignore` edit)". Now every reference to iteration 1's false premise is corrected.

- [x] `[TPR-XX-003-gemini][low]` + `[TPR-XX-002-gemini][medium]` (iteration 4) `plans/project-reorganization/00-overview.md:157,250,382` + `section-09-verification.md:255,459` — Remove vestigial gitignore edit from overview and §09 final commit template.
  Evidence: 00-overview.md line 250 Implementation Sequence still said "§03 Tracked artifact removal + /build/ gitignore fix"; line 382 Estimated Effort table said "§03 Tracked Artifact + Gitignore Fix"; line 157 dependency graph comment said "gitignore fix". §09 final commit template line 459 still claimed ".gitignore /build/ rule added" and line 255 said "Modified: .gitignore (if §03 decides...)" which was vague.
  Impact: Overview and §09 final commit template disagreed with §03's iteration 3 correction.
  Required plan update: Update overview implementation sequence / effort table / dependency graph and §09 final commit template / modified files list to remove all ".gitignore rule added" references.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-11 iteration 4. (a) 00-overview dependency graph `gitignore fix` → `gitignore verify`. (b) Implementation Sequence `§03 Tracked artifact removal + /build/ gitignore fix` → `§03 Tracked artifact removal (+ `.gitignore` verification; no edit — iteration 3 correction)`. (c) Estimated Effort table `§03 Tracked Artifact + Gitignore Fix` → `§03 Tracked Artifact Removal & Gitignore Verification`. (d) §09 final commit template `2 build/debug leak files removed, .gitignore /build/ rule added` → `2 build/debug leak files removed via git rm --cached (no .gitignore edit — iteration 3 correction...)`. (e) §09.2 Modified bullet: `.gitignore (if §03 decides...)` removed; added explicit note that `.gitignore` is NOT modified + a verification gate `git diff BASELINE_COMMIT..HEAD --stat --diff-filter=M -- .gitignore` returns empty.

- [x] `[TPR-XX-002-codex][medium]` (iteration 4) `plans/project-reorganization/section-09-verification.md:252` — Recompute §09.2 category counts from the actual baseline diff.
  Evidence: §09.2 said it compared `BASELINE_COMMIT..HEAD`, but its expected counts were based on pre-baseline or non-git-visible work. The Added bullet counted files that already exist at the baseline commit (4 test-all baseline artifacts, 9 section files, 2 plan index/overview). The Deleted bullet counted filesystem-only cleanup (scratchpad/, samples/) that would not appear in git diff because those paths are gitignored.
  Impact: The §09.2 expected-counts cross-check was mathematically wrong — executors could see a correct diff stat but compare it against non-git-visible estimates and conclude something was missing.
  Required plan update: Recalculate Added/Deleted/Renamed/Modified expectations relative to `BASELINE_COMMIT..HEAD`: exclude pre-baseline plan files and baseline artifacts, exclude untracked scratchpad and samples cleanup from Deleted, use the real tracked `tools/ori-lsp/` footprint, and keep `.gitignore` out of Modified.
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11 iteration 4. Rewrote §09.2 "Expected category counts" with explicit "relative to `BASELINE_COMMIT..HEAD` (git-diff visible)" framing: Added = ~17 (dropped pre-baseline artifacts + section files + plan index/overview — they are committed AT baseline); Deleted = ~27 (git-tracked only — scratchpad + samples filesystem cleanup reported via §01 baseline snapshot comparison, NOT git diff); Modified = ~240+ WITHOUT `.gitignore`. Added an explicit verification gate: `git diff BASELINE_COMMIT..HEAD --stat --diff-filter=M -- .gitignore` returns empty.

- [x] `[TPR-XX-003-codex][low]` (iteration 4) `plans/project-reorganization/index.md:107,153` + `section-07-scratchpad-triage.md:62,169,561` — Propagate the reconciled §07/§08 metadata into remaining navigation text.
  Evidence: `index.md:107` still said "scratchpad/ audit, 31 files, 14 migrate, 17 delete"; `index.md:153` still said "08.7 NEW invocation matrix (29 cells"; §07:62 still said "17+ files deleted"; §07:169 still said "All 31 files classified"; §07:561 still said "17 or 18 depending on modern-lang-repos decision".
  Impact: The iteration 2 / iteration 3 scratchpad math propagation missed the index.md keyword clusters and multiple §07 checklist items.
  Required plan update: Update `index.md` §07 and §08 entries to 30/12/18 and 33-cell; rename §07.6 body "17 files" to "18 files"; remove "17+" / "17 or 18" wording.
  Basis: fresh_verification. Confidence: low.
  Resolved: Fixed on 2026-04-11 iteration 4. (a) `index.md:107` updated to "30 files, 12 migrate, 18 delete". (b) `index.md:153` updated to "33 cells: 29 exercisable + 4 PATH SKIP". (c) §07.N checklist "17+ files deleted" → "18 files deleted... reconciled math: 30 total − 12 migrations = 18 deletions". (d) §07.1 "All 31 files classified" → "All 30 files classified, live count at §01.1 baseline". (e) §07.6 "17 or 18 depending on modern-lang-repos decision" → "18 total, per reconciled math that includes `07-modern-lang-repos.md` in the DELETE list".

- [x] `[TPR-XX-004-codex][low]` (iteration 4) `plans/project-reorganization/section-01-census.md:90` — Document PIPESTATUS alternative is bash-only.
  Evidence: The primary §01.1 pipefail fix at lines 83-88 was correct. The documented alternative at lines 90-95 used `${PIPESTATUS[0]}`, which works in bash but is unset in zsh (zsh uses lowercase, 1-indexed `${pipestatus[1]}`). Presented as interchangeable with the portable pipefail form.
  Impact: A zsh user following the "alternative" would record an empty exit code and silently pass red baselines.
  Required plan update: Label the `${PIPESTATUS[0]}` snippet as bash-only or add a zsh alternative.
  Basis: fresh_verification. Confidence: low.
  Resolved: Fixed on 2026-04-11 iteration 4. Added explicit label "BASH ONLY — zsh users: use the pipefail form above instead" above the `${PIPESTATUS[0]}` snippet. Added prose explaining that `set -o pipefail` is the portable default and is recommended; the PIPESTATUS alternative is only safe if the invoking shell is known to be bash. Added a comment inside the bash-only code block.

---

## 03.N Completion Checklist

- [ ] `build/debug/simplest_crash.ll` removed from git tracking (03.1)
- [ ] `build/debug/test_simple` removed from git tracking (03.1)
- [ ] `.gitignore` verified UNCHANGED (03.2) — no `/build/` rule added; the existing `**/build/` rule already covers root-level `build/` per iteration 3's empirical `--no-index` verification
- [ ] `git check-ignore -v build/debug/simplest_crash.ll` confirms new rule matches (03.2)
- [ ] Negative rules for `compiler/oric/src/commands/build/` verified intact (03.3)
- [ ] `profile.json.gz` still tracked (NOT deleted by this section)
- [ ] `./test-all.sh` green post-fix
- [ ] Single atomic commit lands: `fix(plan): remove tracked build/debug leak` (NO `+ gitignore root anchor` — iteration 3 correction)
- [ ] Plan annotation cleanup: N/A (no `.rs` files modified)
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table for §03 → `Complete`
  - [ ] `00-overview.md` mission success criterion "Gitignore drift bug fixed" → checked
  - [ ] `00-overview.md` Known Bugs table row for `build/debug/*` leak → `Fixed`
  - [ ] `index.md` updated
- [ ] `/tpr-review` passed — simple section; review should be brief
- [ ] `/impl-hygiene-review` passed (AFTER TPR clean)
- [ ] `/improve-tooling` **section-close sweep** — per-subsection retrospectives (03.1, 03.2, 03.3) should already be committed. Cross-subsection pattern check: is there a single reusable `scripts/dev/gitignore-sanity.sh` helper that combines the leak-finder (03.1), the pattern-pairing knowledge (03.2), and the negative-rule verifier (03.3)? If yes, consolidate the three per-subsection helpers (if any were built) into one and commit via `build(scripts): consolidate gitignore-sanity.sh — surfaced by section-03 close sweep`. If no cross-cutting value: document "Section-03 close sweep: per-subsection retrospectives covered everything; the three helpers are independent enough that consolidation is not warranted."

**Exit Criteria:** `git ls-files build/` returns empty. `git check-ignore -v --no-index build/debug/simplest_crash.ll` matches `.gitignore:5:**/build/` (the existing rule — no new rule needed). `git ls-files compiler/oric/src/commands/build/` returns its tracked files (negative rule preserved). `profile.json.gz` still tracked. `./test-all.sh` green. A single commit (`fix(plan): remove tracked build/debug leak`) contains exactly **2 file operations** (both deletions; NO `.gitignore` edit per iteration 3 correction). Mission success criterion "Tracked build/debug artifacts removed" is satisfied.
