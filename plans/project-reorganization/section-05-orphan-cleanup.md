---
section: "05"
title: "Orphan Cleanup"
status: in-progress
reviewed: false
goal: "Delete three confirmed orphan entries — the empty `samples/` directory, the phantom `compiler/ori_lsp/.gitkeep` placeholder (and let the now-empty directory disappear from the filesystem since git doesn't track empty dirs; the canonical LSP reservation is preserved by `plans/roadmap/section-22-tooling.md:150-156`, not by an empty filesystem directory), and the obsolete `rebuild-playground.sh` script (confirmed 0 references, dead `./website/` path) — without touching the stale `tools/ori-lsp/` prototype which has its own dedicated §06 treatment."
success_criteria:
  - "`samples/` does not exist (or is empty if the directory itself is gitignored per the current .gitignore rule `/samples/`)"
  - "`git ls-files compiler/ori_lsp/` is empty (`.gitkeep` removed). The directory ITSELF may or may not exist on the filesystem after the deletion — both states are acceptable because git does not track empty directories. The LSP canonical-home reservation is a plan-level claim at `plans/roadmap/section-22-tooling.md:150-156`, NOT a filesystem-level claim."
  - "`git ls-files | grep rebuild-playground.sh` returns empty"
  - "No rebuild-playground.sh references appear anywhere in ori_lang, ori-lang-website, or any .github workflow"
  - "`./test-all.sh` green post-cleanup (nothing this section touches is exercised by the test suite)"
  - "Single atomic commit lands: `chore(plan): delete orphan directories and obsolete rebuild-playground.sh`"
inspired_by:
  - "rustc `.gitignore`-driven directory cleanup during workspace restructuring"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Delete samples/ Directory"
    status: not-started
  - id: "05.2"
    title: "Delete compiler/ori_lsp/.gitkeep Phantom"
    status: not-started
  - id: "05.3"
    title: "Delete rebuild-playground.sh"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Orphan Cleanup

**Status:** Not Started
**Goal:** Remove three orphan entries whose classification and non-use are confirmed by Pass 1 research. Each deletion is independent (no cross-dependencies) and low-risk (none of these files are referenced by running code, tests, or CI). The `tools/ori-lsp/` directory — the other major orphan — is explicitly NOT handled here because its deletion requires atomic updates to 20+ referencing files across Cargo.toml, release workflows, LSP design docs, version sync scripts, and Tier-3 doc rewrites. That work lives in §06.

**Success Criteria:**

- [ ] `samples/` removed from filesystem (if tracked) or confirmed as gitignored-and-absent
- [ ] `compiler/ori_lsp/.gitkeep` removed from git tracking
- [ ] `compiler/ori_lsp/` directory may or may not exist on the filesystem after the deletion — **both states are acceptable**. The canonical LSP reservation is preserved by `plans/roadmap/section-22-tooling.md:150-156` (a plan-level commitment), NOT by a filesystem artifact. When a real LSP crate is implemented later, `mkdir -p compiler/ori_lsp/` + `cargo new --lib .` will recreate the directory with actual content.
- [ ] `rebuild-playground.sh` removed from git tracking and filesystem
- [ ] `rg 'rebuild-playground' ori_lang/ ../ori-lang-website/` returns zero matches
- [ ] `./test-all.sh` green post-cleanup
- [ ] Single atomic commit: `chore(plan): delete orphan directories and obsolete rebuild-playground.sh`

**Context:** The Pass 1 research classified these three entries as orphans with the following evidence:

1. **`samples/`**: The Pass 1 census shows `samples/` as an empty directory with 0 tracked files. `.gitignore` already contains `/samples/` (line 24), so even if someone created files in it, they wouldn't be tracked. The directory's existence at all is leftover from an earlier planning state where "samples" was intended but never populated.

2. **`compiler/ori_lsp/.gitkeep`**: The Pass 1 research and §06 LSP disposition consensus both confirmed that `compiler/ori_lsp/` is a phantom directory — its only tracked file is `.gitkeep`. The roadmap at `plans/roadmap/section-22-tooling.md:150-156` declares `compiler/ori_lsp/` as the canonical future home for the LSP, but the directory itself is empty. We keep the directory reservation (the "canonical future home" claim remains valid), but we remove the `.gitkeep` because it's not needed — the directory will be re-created (or stay in git's tree-cache) when the real LSP implementation lands.

3. **`rebuild-playground.sh`**: Pass 1 Agent 1 verified zero references across the entire ori_lang repo AND the ori-lang-website repo. The script's body references `./website/playground-wasm/`, which no longer exists (the website extracted to a sibling repo and has its own `scripts/rebuild-wasm.sh`). Classification: **fully obsolete dead code**.

**Reference implementations:**
- **rustc** (post-unification cleanup when `librustc_*` was split into `rustc_*`): removed stale `.gitkeep` files and empty placeholder directories in a small follow-up commit separate from the main refactor
- **Swift** (after cmake → swiftpm transition): removed stale shell scripts as `chore:` commits independent of the main migration

**Depends on:** §01 (the baseline must prove these three entries exist before we delete them, so mid-execution drift is detectable).

---

## 05.1 Delete samples/ Directory

**File(s):** `samples/` directory

- [ ] Verify current state:
  ```bash
  cd /home/eric/projects/ori_lang
  ls -la samples/ 2>&1
  git ls-files samples/ 2>&1
  ```
  - [ ] `ls`: directory exists, probably empty
  - [ ] `git ls-files`: empty (gitignored by `/samples/` rule at .gitignore:24)

- [ ] Decision: since `samples/` is already gitignored and empty, the simplest action is `rmdir` on the filesystem:
  ```bash
  rmdir samples 2>&1
  # If rmdir fails (directory not empty), list contents first
  ```
  - [ ] If directory has any files: investigate. The Pass 1 research said "empty" — any content is drift. Capture the content, decide whether to migrate (unlikely) or delete, then proceed.
  - [ ] If rmdir succeeds: the directory is gone.

- [ ] Verify `samples/` is gone:
  ```bash
  ls samples 2>&1 || echo "absent (expected)"
  ```

- [ ] NOTE: the `/samples/` rule in `.gitignore:24` can stay. Removing it is not required — it's a historical protection that remains correct (if someone creates `samples/` in the future, it stays gitignored unless the rule is removed). Leave the rule as-is to avoid gratuitous churn.

- [ ] **Subsection close-out (05.1)** — MANDATORY before starting 05.2:
  - [ ] `samples/` directory absent
  - [ ] `.gitignore` rule `/samples/` left untouched (not needed to remove, not harmful)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the trivially-simple operation. `rmdir` on an empty directory is
        about as basic as it gets — no tooling gap possible. Document:
        "Retrospective 05.1: trivial operation; no tooling gap."
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.

---

## 05.2 Delete compiler/ori_lsp/.gitkeep Phantom

**File(s):** `compiler/ori_lsp/.gitkeep`

This deletes the phantom `.gitkeep` file but PRESERVES the directory reservation (the directory stays empty but exists in the filesystem as a reminder that `compiler/ori_lsp/` is the canonical future LSP home).

Actually — reconsider. A directory in git is only "tracked" if it has at least one tracked file. Once we `git rm` the `.gitkeep`, the directory becomes untracked-and-empty, and depending on workflow expectations, it might get `rmdir`'d automatically by `git clean` or similar. The "directory reservation" is really a **plan-level claim**, not a filesystem-level claim — the roadmap at `plans/roadmap/section-22-tooling.md:150-156` already documents the intent, so the filesystem directory doesn't need to exist to preserve the reservation.

Decision: delete the `.gitkeep` AND let the directory be untracked. Anyone who wants to populate the canonical LSP home in the future will `mkdir -p compiler/ori_lsp/ && cargo new --lib .` or equivalent. The roadmap claim is the load-bearing reservation, not the empty dir.

- [ ] Verify current state:
  ```bash
  cd /home/eric/projects/ori_lang
  git ls-files compiler/ori_lsp/
  # Expected: compiler/ori_lsp/.gitkeep (just the one file)

  ls -la compiler/ori_lsp/
  # Expected: only .gitkeep
  ```

- [ ] If anything else is tracked under `compiler/ori_lsp/`: STOP. That's unexpected drift — §06 LSP disposition may need to change. Investigate before deletion.

- [ ] Delete the `.gitkeep`:
  ```bash
  git rm compiler/ori_lsp/.gitkeep
  ```

- [ ] Optionally remove the now-empty directory (git does not track empty directories anyway):
  ```bash
  rmdir compiler/ori_lsp/ 2>&1 || true
  # This may fail if the directory contains untracked files; that's fine
  ```

- [ ] Verify:
  ```bash
  git ls-files compiler/ori_lsp/
  # Expected: empty
  ```

- [ ] **Subsection close-out (05.2)** — MANDATORY before starting 05.3:
  - [ ] `.gitkeep` removed from tracking
  - [ ] Directory may or may not exist on disk (either is fine)
  - [ ] §06 LSP disposition section updated if it referenced the `.gitkeep` deletion (cross-section sync — §06 may have been written assuming we keep the phantom directory; verify §06's `compiler/ori_lsp/` treatment is consistent with this subsection's outcome)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on whether the "delete phantom .gitkeep" pattern is something future
        reorgs will encounter. Phantom directories are common enough that a
        `scripts/dev/find-phantom-dirs.sh` helper (find directories whose
        only tracked content is `.gitkeep` or similar placeholders) might
        have value. Is this a one-off or a recurring pattern? Probably
        recurring across any compiler with reserved-but-unpopulated future
        homes. If worth building: commit via `build(scripts): add
        find-phantom-dirs.sh — surfaced by project-reorganization/section-05.2
        retrospective`. If not: "Retrospective 05.2: one-time phantom
        cleanup; no reusable tooling warranted."
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.

---

## 05.3 Delete rebuild-playground.sh

**File(s):** `rebuild-playground.sh`

- [ ] Re-verify zero references (Pass 1 already did this, but we re-verify at execution time per §01.3 discipline):
  ```bash
  cd /home/eric/projects/ori_lang
  rg -n 'rebuild-playground' . ../ori-lang-website/ 2>&1 \
    | grep -v 'plans/project-reorganization/' \
    | grep -v 'rebuild-playground\.sh:'
  # Expected: empty output (no external references)
  ```
  - [ ] No matches outside this plan directory and the script itself
  - [ ] If matches appear: STOP. A reference has been added since Pass 1. Reclassify and DO NOT delete.

- [ ] Quick-read the script to confirm it's the obsolete one (references `./website/playground-wasm/` that no longer exists):
  ```bash
  head -20 rebuild-playground.sh
  ```
  - [ ] Content confirms the `./website/` assumption

- [ ] Delete:
  ```bash
  git rm rebuild-playground.sh
  ```

- [ ] Commit §05 as a single atomic chore commit:
  ```bash
  cd /home/eric/projects/ori_lang
  git commit -m "chore(plan): delete orphan directories and obsolete rebuild-playground.sh

Three orphans removed:
- samples/ (empty, gitignored, leftover placeholder)
- compiler/ori_lsp/.gitkeep (phantom — directory reservation lives
  in plans/roadmap/section-22-tooling.md:150-156, not in the filesystem)
- rebuild-playground.sh (obsolete — 0 references, dead ./website/
  path; ori-lang-website has scripts/rebuild-wasm.sh as canonical)

Pass 1 research confirmed zero active uses for each entry.

tools/ori-lsp/ is NOT deleted here — it has its own atomic-deletion
section at §06 (touches 20+ referencing files across Cargo.toml,
release workflows, design docs, version sync).

Refs: plans/project-reorganization/section-05-orphan-cleanup.md"
  ```

- [ ] Verify the commit:
  ```bash
  git log --oneline -1
  git show --stat HEAD
  # Expected stat: ~2 deletions (rebuild-playground.sh + .gitkeep); samples/ was already gitignored-empty so no git op
  ```

- [ ] Run `./test-all.sh` to prove no regression (rebuild-playground.sh had no references, so test-all cannot know about the deletion — this is a sanity check):
  ```bash
  timeout 150 ./test-all.sh 2>&1 | tail -10
  ```
  - [ ] Exit 0, all phases PASS

- [ ] **Subsection close-out (05.3)** — MANDATORY before 05.R:
  - [ ] `rebuild-playground.sh` absent from git and filesystem
  - [ ] Commit landed atomically
  - [ ] `./test-all.sh` green
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the "verify zero refs → delete" pattern. Is this the same
        pattern §02.3 used (global sweep verification)? If so, and if the
        `plan-ref-sweep.sh` helper from §02.3's retrospective was built,
        is it applicable here? Unify the approach if possible. If no helper
        was built: "Retrospective 05.3: the grep-filter-investigate pattern
        was already documented in 02.3 retrospective; no additional tooling
        needed."
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.

---

## 05.R Third Party Review Findings

<!-- Dual-source /tpr-review iteration 2 (2026-04-11) surfaced a contract contradiction between §05 frontmatter and §05.2's architectural decision. Resolved inline. -->

- [x] `[TPR-XX-004-codex][medium]` `plans/project-reorganization/section-05-orphan-cleanup.md:6` — Reconcile the DRIFT in §05's `compiler/ori_lsp` reservation contract.
  Evidence: §05 gave two incompatible contracts for the `compiler/ori_lsp/.gitkeep` cleanup. The frontmatter goal said "keeping the directory as a reserved canonical future home" (line 6), and the section-level success criterion said "directory itself stays reserved as future canonical LSP home" (line 46). But §05.2's subsection correctly re-evaluated the mechanics (lines 114-149) and concluded that the LSP reservation is a **plan-level claim** at `plans/roadmap/section-22-tooling.md:150-156`, NOT a filesystem artifact — and that deleting `.gitkeep` AND letting the empty directory disappear is the correct architectural choice.
  Impact: The section was internally contradictory. An executor following the subsection's corrected decision would still fail the section's own frontmatter and top-level success criteria.
  Required plan update: Align §05's frontmatter goal, success_criteria, and section-level success criteria with the §05.2 decision (delete `.gitkeep`, allow the empty directory to disappear, preserve the canonical LSP reservation via the roadmap commitment).
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11 iteration 2. Updated §05 frontmatter `goal` to explicitly say "let the now-empty directory disappear from the filesystem since git doesn't track empty dirs; the canonical LSP reservation is preserved by plans/roadmap/section-22-tooling.md:150-156, not by an empty filesystem directory". Updated frontmatter success_criteria row to match. Updated section-level success criterion to say "directory ITSELF may or may not exist on the filesystem after the deletion — both states are acceptable". All three places (frontmatter goal, frontmatter success_criteria, body success_criteria) now tell the same story as §05.2's implementation.

---

## 05.N Completion Checklist

- [ ] `samples/` directory absent (05.1)
- [ ] `compiler/ori_lsp/.gitkeep` removed from git tracking (05.2)
- [ ] `rebuild-playground.sh` removed from git and filesystem (05.3)
- [ ] No references to rebuild-playground.sh anywhere in ori_lang or ori-lang-website
- [ ] `./test-all.sh` green post-cleanup
- [ ] Single atomic commit lands: `chore(plan): delete orphan directories and obsolete rebuild-playground.sh`
- [ ] Plan annotation cleanup: N/A (no `.rs` files modified)
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table for §05 → `Complete`
  - [ ] `00-overview.md` Known Bugs table rows for `rebuild-playground.sh` obsolete + `compiler/ori_lsp/.gitkeep` phantom → `Fixed`
  - [ ] `index.md` updated
  - [ ] `§06 LSP disposition` cross-check: §06 does NOT need to separately delete `.gitkeep` — that's already done here. Verify §06's checklist doesn't duplicate the action.
- [ ] `/tpr-review` passed — trivial section; review brief
- [ ] `/impl-hygiene-review` passed (AFTER TPR clean)
- [ ] `/improve-tooling` **section-close sweep** — per-subsection retrospectives (05.1, 05.2, 05.3) should already be committed. Cross-subsection pattern check: all three deletions share the "verify orphan → delete → verify gone" pattern. Is there a single `scripts/dev/delete-orphan.sh <path>` helper that standardizes the "check-refs, git-rm, verify, commit" sequence? Probably overkill — `git rm + commit` is already minimal. Document: "Section-05 close sweep: per-subsection retrospectives covered everything; git rm is already the minimal tool."
- [ ] `/sync-claude` **section-close doc sync** — verify Claude artifacts across all section commits. Map changed crates to rules files, check CLAUDE.md, canon.md. Fix drift NOW.

**Exit Criteria:** Three orphan entries are gone from the repository. `ls /home/eric/projects/ori_lang/` no longer shows `samples/` or `rebuild-playground.sh`. `git ls-files compiler/ori_lsp/` is empty. Zero references to any deleted entry survive in the repo (verified by post-commit rg). A single `chore(plan):` commit documents the cleanup. `./test-all.sh` green. §06 is unblocked — it can now focus exclusively on `tools/ori-lsp/` without also handling the phantom `.gitkeep`.
