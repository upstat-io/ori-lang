---
section: "03"
title: "Tracked Artifact Removal & .gitignore Drift Fix"
status: not-started
reviewed: false
goal: "Remove the two tracked `build/debug/*` artifacts (`simplest_crash.ll` and `test_simple`) that leaked past the `**/build/` gitignore pattern, and add a root-anchored `/build/` rule to `.gitignore` so future leaks cannot recur — without breaking the existing negative rules for `compiler/oric/src/commands/build/` which must continue to be tracked."
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
    title: ".gitignore Root-Anchored /build/ Rule Addition"
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
**Goal:** Fix the active `.gitignore` drift bug that caused `build/debug/simplest_crash.ll` and `build/debug/test_simple` to be tracked in git despite the `**/build/` pattern. The root cause is that `**/build/` does NOT match a root-level `/build/` directory due to gitignore pattern resolution rules — gitignore treats `**` as "zero or more path components," which sometimes excludes the root-level case. Fix: add a root-anchored `/build/` rule alongside the existing `**/build/` rule, remove the two leaked files via `git rm --cached`, and verify the existing negative rule for `compiler/oric/src/commands/build/` (a legitimate source-tree directory named `build`) is unaffected.

**Success Criteria:**

- [ ] `git ls-files build/` returns empty — both leaked files are no longer tracked
- [ ] `git check-ignore -v build/debug/simplest_crash.ll` returns a matched rule with the NEW `/build/` rule as the source
- [ ] `git ls-files compiler/oric/src/commands/build/` returns its existing tracked files (negative rule unbroken)
- [ ] `profile.json.gz` is still in `git ls-files` (this section does NOT delete it — it's intentionally tracked as a perf reference)
- [ ] `.gitignore` has a new root-anchored `/build/` rule, documented with a comment explaining the pairing with `**/build/`
- [ ] `./test-all.sh` green post-fix
- [ ] Single atomic commit lands: `fix(plan): remove tracked build/debug leak + gitignore root anchor`
- [ ] Satisfies mission criterion: "**Gitignore drift bug fixed**"

**Context:** The Pass 1 research agent ran `git check-ignore -v build/debug/simplest_crash.ll` and received NO output, proving the file is not ignored by any rule. Simultaneously, `git ls-files build/` returned both `build/debug/simplest_crash.ll` and `build/debug/test_simple`, proving they're tracked. The overview's Known Bugs table documents this as an **active drift bug**, not historical residue.

Why does `**/build/` not match `/build/`? Because gitignore's `**` pattern requires at least one directory component before or after it in certain positions. `**/build/` matches `anything/build/` (where "anything" is one or more path components) but does NOT match a root-level `build/` because there's no "anything" before it. The correct fix is to pair it with a root-anchored `/build/` rule, which is exactly how rustc's `.gitignore` handles the same pattern for `target/`.

The critical constraint is that `compiler/oric/src/commands/build/` is a legitimate source directory containing tracked Rust files. The existing `.gitignore` has two negative rules that preserve it:
```
!compiler/oric/src/commands/build/
!compiler/oric/src/commands/target/
```
The new `/build/` rule we add MUST NOT conflict with these negative rules. Fortunately, `/build/` is root-anchored (starts with `/`), so it only matches a top-level `build/` directory — it does not match `compiler/oric/src/commands/build/`. The negative rules are safe.

**Reference implementations:**
- **rustc** (`rust/.gitignore`): has both `/target/` (root-anchored) and `**/target/` (nested). This is the canonical pattern pairing for build-output directories that can appear at multiple levels.

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

- [ ] Remove the tracked files from git index only (files stay on disk — they will be gitignored by the rule added in 03.2):
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

## 03.2 .gitignore Root-Anchored /build/ Rule Addition

**File(s):** `.gitignore`

Add a root-anchored `/build/` rule to `.gitignore` so that any future root-level `build/` directory contents are ignored. The rule must be placed alongside the existing `**/build/` rule (so future readers understand the pairing) and must NOT break the existing negative rules for `compiler/oric/src/commands/build/`.

- [ ] Read the current `.gitignore` header section (first 15 lines) to confirm the existing rules and negative-rule placement:
  ```bash
  sed -n '1,15p' .gitignore
  ```
  Expected current state (from Pass 1 research):
  ```
  # Build artifacts
  /target/
  /target-llvm/
  **/target/
  **/build/
  # Source-tree subcommand directories that collide with the cargo build-dir
  # patterns above. Add a negative rule for each subcommand directory under
  # `compiler/oric/src/commands/` whose name matches `target` or `build`.
  # Without these, `git add` silently ignores newly-added files in these
  # directories, requiring `-f` and confusing contributors.
  !compiler/oric/src/commands/build/
  !compiler/oric/src/commands/target/
  ```

- [ ] Add `/build/` after `/target-llvm/` (before the nested `**/target/` and `**/build/` rules), so the root-anchored rules are grouped at the top:
  ```bash
  # Use Edit tool on .gitignore
  # old_string: '/target-llvm/\n**/target/\n**/build/'
  # new_string: '/target-llvm/\n/build/\n**/target/\n**/build/'
  ```

  Resulting `.gitignore` head (lines 1-15):
  ```
  # Build artifacts
  /target/
  /target-llvm/
  /build/
  **/target/
  **/build/
  # Source-tree subcommand directories that collide with the cargo build-dir
  # patterns above. Add a negative rule for each subcommand directory under
  # `compiler/oric/src/commands/` whose name matches `target` or `build`.
  # Without these, `git add` silently ignores newly-added files in these
  # directories, requiring `-f` and confusing contributors.
  !compiler/oric/src/commands/build/
  !compiler/oric/src/commands/target/
  ```

- [ ] Verify the edit:
  ```bash
  sed -n '1,15p' .gitignore
  # Verify /build/ appears on its own line after /target-llvm/
  ```

- [ ] Test the new rule with `git check-ignore -v`:
  ```bash
  git check-ignore -v build/debug/simplest_crash.ll
  # Expected output: .gitignore:<N>:/build/    build/debug/simplest_crash.ll
  # where <N> is the line number of the new /build/ rule
  ```
  - [ ] Verify: the new rule is cited as the match source
  - [ ] Verify: `git check-ignore -v compiler/oric/src/commands/build/hello.rs` (or any tracked file in that dir) returns `.gitignore:<M>:!compiler/oric/src/commands/build/` — the negative rule wins

- [ ] **Subsection close-out (03.2)** — MANDATORY before starting 03.3:
  - [ ] `.gitignore` edit applied and visible in `git diff .gitignore`
  - [ ] `git check-ignore -v` confirms the new rule is matching the correct files
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the pattern-pairing discovery journey. Did I have to look up how
        gitignore `**` resolution actually works (i.e., why `**/build/`
        doesn't match root-level `/build/`)? Is that explanation captured
        anywhere in the repo's docs? If not: is a brief
        `docs/development/gitignore-patterns.md` or a CLAUDE.md note worth
        adding so future maintainers don't re-discover this? If yes, commit
        via `docs(development): document gitignore ** pattern resolution —
        surfaced by project-reorganization/section-03.2 retrospective`. If
        no: "Retrospective 03.2: pattern knowledge is general gitignore
        knowledge; no Ori-specific docs gap."

---

## 03.3 Negative Rule Verification

**File(s):** No edits — verification only

Prove that the new `/build/` rule does NOT break the existing negative rules for `compiler/oric/src/commands/build/`. This is the regression guard: if the new rule accidentally shadowed the negative rules, legitimate tracked source files would suddenly become "should be ignored" and future `git add` operations would silently skip them.

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

- [ ] Commit everything from §03 as a single atomic commit:
  ```bash
  cd /home/eric/projects/ori_lang
  git add .gitignore  # the new rule
  # (build/debug/*.ll and test_simple are already staged for removal from 03.1)
  git commit -m "fix(plan): remove tracked build/debug leak + gitignore root anchor

Two files leaked past the '**/build/' gitignore pattern:
  build/debug/simplest_crash.ll
  build/debug/test_simple

Root cause: '**/build/' does not match root-level '/build/' due to
gitignore ** pattern resolution. Fix: add root-anchored '/build/' rule
alongside the existing nested '**/build/' rule.

The existing negative rules for compiler/oric/src/commands/build/ are
preserved — the new rule is root-anchored, so it doesn't match nested
source directories.

profile.json.gz stays tracked (intentional perf reference).

Refs: plans/project-reorganization/section-03-tracked-artifacts.md"
  ```

- [ ] Verify the commit:
  ```bash
  git log --oneline -1
  # Expected: fix(plan): remove tracked build/debug leak + gitignore root anchor

  git show --stat HEAD
  # Expected: .gitignore +1/-0, build/debug/simplest_crash.ll -1/-0 (deleted), build/debug/test_simple -1/-0 (deleted)
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

<!-- Reserved for dual-source /tpr-review findings. -->

- None.

---

## 03.N Completion Checklist

- [ ] `build/debug/simplest_crash.ll` removed from git tracking (03.1)
- [ ] `build/debug/test_simple` removed from git tracking (03.1)
- [ ] `/build/` rule added to `.gitignore` (03.2)
- [ ] `git check-ignore -v build/debug/simplest_crash.ll` confirms new rule matches (03.2)
- [ ] Negative rules for `compiler/oric/src/commands/build/` verified intact (03.3)
- [ ] `profile.json.gz` still tracked (NOT deleted by this section)
- [ ] `./test-all.sh` green post-fix
- [ ] Single atomic commit lands: `fix(plan): remove tracked build/debug leak + gitignore root anchor`
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

**Exit Criteria:** `git ls-files build/` returns empty. `git check-ignore -v build/debug/simplest_crash.ll` matches the new `/build/` rule. `git ls-files compiler/oric/src/commands/build/` returns its tracked files (negative rule preserved). `profile.json.gz` still tracked. `./test-all.sh` green. A single commit (`fix(plan): remove tracked build/debug leak + gitignore root anchor`) contains exactly 3 file operations (2 deletions + 1 gitignore edit). Mission success criterion "Gitignore drift bug fixed" is satisfied.
