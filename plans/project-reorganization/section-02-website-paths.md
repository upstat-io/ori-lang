---
section: "02"
title: "Stale Website Path Repair"
status: not-started
reviewed: false
goal: "Fix two stale `website/` path references in `ori_lang/` that assume the sibling website still lives in-repo — `test-all.sh:37` (default JSON output path) and `docs/development/versioning.md:53` (versioning doc) — so that §04 blog migration can proceed against a clean test suite and §09 verification can prove the website boundary is drift-free."
success_criteria:
  - "`test-all.sh:37` no longer writes to `website/public/test-results.json`; new default is `test-all-results.json` at repo root (or another location that exists)"
  - "`docs/development/versioning.md:53` no longer references `website/` paths; updated to reflect the sibling-repo split"
  - "`rg 'website/' --type sh --type md ori_lang/` returns zero matches except documentation explicitly describing the sibling repo (e.g., '...moved to ori-lang-website...')"
  - "`./test-all.sh --json` runs to completion and writes the JSON to the new default location"
  - "Satisfies mission criterion: '**Stale website/ paths repaired**'"
inspired_by:
  - "Swift `utils/update-checkout` — surfaces stale cross-repo path assumptions as build errors, not silent drift"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Fix test-all.sh:37 JSON_PATH Default"
    status: not-started
  - id: "02.2"
    title: "Fix docs/development/versioning.md:53 Website Reference"
    status: not-started
  - id: "02.3"
    title: "Global Sweep Verification"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Stale Website Path Repair

**Status:** Not Started
**Goal:** Repair two confirmed stale `website/` path references in `ori_lang/` that are live drift from the historical split when the website extracted to its own sibling repository. These must be fixed BEFORE §04 (Blog Migration) runs — the blog migration relies on `./test-all.sh` being usable as a regression guard, and a `test-all.sh` that errors on `--json` (because its default output path points at a deleted directory) is not a usable guard.

**Success Criteria:**

- [ ] `test-all.sh:37` updated to a default path that exists on disk (recommendation: `test-all-results.json` at repo root, gitignored by `*.json` or a specific rule)
- [ ] `docs/development/versioning.md:53` updated to remove `website/` reference or rewrite as historical note
- [ ] `rg 'website/' ori_lang/` returns zero active-drift matches (only informational references to the sibling repo survive)
- [ ] `./test-all.sh` runs (post-fix) with `--json` flag and produces valid output at the new path
- [ ] `git diff HEAD~1 -- test-all.sh docs/development/versioning.md` shows the exact expected changes (no accidental rewrites)
- [ ] Satisfies mission criterion: "**Stale website/ paths repaired**"

**Context:** During Pass 1 research, both codex (via grep) and Agent 1 (via repo-wide search) flagged that `test-all.sh:37` contains `JSON_PATH="website/public/test-results.json"` as the default when `--json` is passed without a value. The `./website/` directory no longer exists in `ori_lang/` — the website moved to a sibling repository at some point, but `test-all.sh`'s default was never updated. Similarly, `docs/development/versioning.md:53` references a `website/` path that is no longer accurate. These are not theoretical drift — they are active bugs that would cause `./test-all.sh --json` to fail silently (writing to a path that doesn't exist) or loudly (fs error).

This section is deliberately **small**. It exists to surface-clean two specific line references before §04's cross-repo blog migration begins. Combining it with §04 would entangle the website-extraction cleanup with the content-migration commit, producing a confusing diff.

**Reference implementations:**
- **Swift** (`utils/build-script-impl`): after the LLVM submodule extraction from the Swift monorepo, Swift's build scripts had similar stale path references; Swift's approach was to fix them in small, targeted commits (not bundled with the larger extraction) so post-hoc `git blame` could clearly identify the "extraction cleanup" work.

**Depends on:** §01 (the baseline must be frozen so §02's changes produce a clean `git diff` against a known-good starting state).

---

## 02.1 Fix test-all.sh:37 JSON_PATH Default

**File(s):** `test-all.sh`

The issue is at line 37 (inside the `--json` flag handler, in the case statement's `--json)` arm):

```bash
--json)
    EMIT_JSON=1
    JSON_PATH="website/public/test-results.json"
    ;;
```

The directory `website/public/` no longer exists in `ori_lang/`. When a developer or CI script passes `--json` without a value, `test-all.sh` attempts to write to a nonexistent path, which either silently drops output (if the tee can tolerate it) or errors out.

- [ ] Read the current `test-all.sh:30-45` to confirm line 37's content and surrounding context:
  ```bash
  sed -n '30,45p' test-all.sh
  ```
  - [ ] Verify: line 37 still contains `JSON_PATH="website/public/test-results.json"`. If it has already been fixed, skip the rest of this subsection and mark 02.1 complete.

- [ ] Decide the new default. Options:
  - **(a) Repo root** — `JSON_PATH="test-all-results.json"`. Simplest. Gitignored by the new `/test-all-results.json` rule we add in §03 (or covered by `*.log`-adjacent rules). Recommended.
  - **(b) Target dir** — `JSON_PATH="target/test-all-results.json"`. Uses existing Cargo build dir which is gitignored. Slightly cleaner since `target/` is already gitignored.
  - **(c) Scripts/dev/output** — `JSON_PATH="scripts/dev/test-all-results.json"`. Internal to dev-tooling hierarchy. Only makes sense AFTER §08 creates scripts/dev/.

  **Recommendation: Option (b) — `target/test-all-results.json`.** `target/` is already gitignored (`/target/` at `.gitignore:2`), so no new gitignore rule is needed. The file lives alongside other Cargo build outputs, which is semantically appropriate for a test-run summary.

- [ ] Apply the fix with a single atomic edit:
  ```bash
  # Use Edit tool on test-all.sh
  # old_string: '            JSON_PATH="website/public/test-results.json"'
  # new_string: '            JSON_PATH="target/test-all-results.json"'
  ```

- [ ] Verify the fix:
  ```bash
  grep -n 'JSON_PATH=' test-all.sh
  # Expected: one line matching target/test-all-results.json (and possibly the --json=* form which uses ${arg#--json=})
  ```
  - [ ] No surviving `website/public/` reference
  - [ ] The `--json=PATH` form (line 41) still works for explicit paths — we only changed the default

- [ ] Smoke-test the fix (non-blocking; just verify the path is writable and the script starts without immediate error):
  ```bash
  mkdir -p target  # should already exist but be safe
  timeout 10 ./test-all.sh --json 2>&1 | head -5 || true
  # Not waiting for full test suite; just confirming the initial "LOGGING ALL OUTPUT TO..." line appears
  ```
  - Note: DO NOT run the full `./test-all.sh --json` in this subsection. The full-suite verification belongs in 02.3.

- [ ] **Subsection close-out (02.1)** — MANDATORY before starting 02.2:
  - [ ] Line 37 of `test-all.sh` no longer contains `website/public/`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the fix journey: was it hard to find line 37 without line numbers?
        Would `rg --line-number 'website/' test-all.sh` have been faster? Is
        there a reusable pattern for "find and fix stale cross-repo paths in
        shell scripts" that would help future splits? If `test-all.sh` is
        about to move to `scripts/dev/` in §08, is there friction in editing
        it now at the root and then re-editing its moved form? (No — the
        Edit tool follows the file; the post-§08 file at
        `scripts/dev/test-all.sh` will carry this fix forward via git mv.)
        If tooling gap found, add and commit via `build(diagnostics): ... —
        surfaced by project-reorganization/section-02.1 retrospective`.
        Otherwise document: "Retrospective 02.1: no tooling gaps — direct
        Edit of a single line was the right tool."
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.

---

## 02.2 Fix docs/development/versioning.md:53 Website Reference

**File(s):** `docs/development/versioning.md`

Pass 1 research identified that `docs/development/versioning.md:53` references `website/` in a way that assumes the in-repo layout. The exact line needs to be read fresh in this subsection (rather than relying on the Pass 1 recollection) because the file may have been edited between research and execution.

- [ ] Read the current line 53 and surrounding context:
  ```bash
  sed -n '48,60p' docs/development/versioning.md
  ```
  - [ ] Identify the `website/` reference exactly. Is it:
    - A file path assumption (e.g., `website/public/version.json`)?
    - A narrative reference (e.g., "the website reads BUILD_NUMBER from...")?
    - A build-process claim (e.g., "the Astro build writes...")?
  - [ ] Document what the current line says in this checklist before editing, so the fix is deliberate:

    **Current (line 53):** `{fill in from the read above}`

    **Fix approach:**
    - If it's a file path assumption → rewrite to reference the sibling repo path (`../ori-lang-website/...`) or remove if stale
    - If it's a narrative reference → update to reflect that the website is now a sibling repo
    - If it's a build-process claim → update or remove as appropriate

- [ ] Apply the fix with a single Edit (the exact new_string depends on what the line contains):
  ```bash
  # Use Edit tool
  # old_string: (exact line 53 content)
  # new_string: (corrected content)
  ```

- [ ] Verify:
  ```bash
  grep -n 'website/' docs/development/versioning.md
  # Expected: either zero matches, or only matches that explicitly say "sibling repo" / "ori-lang-website"
  ```

- [ ] **Subsection close-out (02.2)** — MANDATORY before starting 02.3:
  - [ ] The `website/` reference at line 53 is gone OR rewritten as an explicit cross-repo reference
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on whether the Pass 1 research's "line 53" pointer was accurate, or
        whether the file had drifted between research and execution. If line
        numbers routinely drift between research and fix, is there a
        `scripts/dev/verify-plan-references.sh <plan-dir>` helper that would
        re-check every `file:line` reference in the plan before execution?
        That would be broadly valuable for every future plan. If yes, add
        now and commit via `build(scripts): add verify-plan-references.sh —
        surfaced by project-reorganization/section-02.2 retrospective`.
        Otherwise: "Retrospective 02.2: no tooling gaps — line 53 was
        accurate; one-line edit sufficient."
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.

---

## 02.3 Global Sweep Verification

**File(s):** No edits — verification only

Prove that no other stale `website/` references exist in `ori_lang/` beyond the two explicitly fixed above. This is the regression guard that catches drift the Pass 1 research missed.

- [ ] Global sweep for `website/` references:
  ```bash
  cd /home/eric/projects/ori_lang
  rg 'website/' --type sh --type md --type toml --type yaml 2>&1 \
    | grep -v 'plans/project-reorganization/' \
    | grep -v 'ori-lang-website' \
    | grep -v 'playground-wasm'
  ```
  Every remaining match must be either (a) a documentation reference explicitly describing the sibling repo, or (b) a legitimate reference to `ori-lang-website/` via relative path.

- [ ] Classify each remaining match:
  - [ ] **Acceptable**: "the sibling repo at `ori-lang-website/`" (documentation)
  - [ ] **Acceptable**: "see ori-lang-website/scripts/rebuild-wasm.sh" (cross-reference)
  - [ ] **NOT acceptable**: any file path that assumes `./website/` exists in `ori_lang/`. If found: add a sub-task to fix it immediately in this section (zero deferral per CLAUDE.md). Do NOT defer to §09 — that section is verification, not fixing.

- [ ] Run `./test-all.sh` with the fix in place to prove no regression:
  ```bash
  timeout 150 ./test-all.sh 2>&1 | tail -20
  echo "Exit: $?"
  ```
  - [ ] Exit code is 0
  - [ ] Tail shows clean PASS status for all test phases

- [ ] Run `./test-all.sh --json` to prove the new default path works:
  ```bash
  timeout 150 ./test-all.sh --json 2>&1 | head -5
  # Check that JSON was written
  ls -la target/test-all-results.json && echo "JSON written"
  ```

- [ ] Commit all §02 changes in a single atomic commit:
  ```bash
  cd /home/eric/projects/ori_lang
  git add test-all.sh docs/development/versioning.md
  git commit -m "fix(plan): repair stale website/ path references

- test-all.sh:37: JSON_PATH default was website/public/test-results.json
  (nonexistent since website extraction); new default: target/test-all-results.json
- docs/development/versioning.md:53: website/ reference updated to reflect
  sibling-repo split

Pre-blog-migration cleanup. Discovered via Pass 1 research agents during
project-reorganization plan creation.

Refs: plans/project-reorganization/section-02-website-paths.md"
  ```

- [ ] **Subsection close-out (02.3)** — MANDATORY before 02.R:
  - [ ] Global sweep returns zero unacceptable `website/` references
  - [ ] `./test-all.sh` green post-fix
  - [ ] Commit landed
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        global-sweep command is one I'll want again for §08's reference sweep
        (747 refs across 230 files). Is there a reusable
        `scripts/dev/plan-ref-sweep.sh <pattern> <exclude-paths...>` helper
        that would standardize the "filter legitimate from stale" logic?
        Strongly consider adding it NOW since §08 will need the exact same
        capability at much larger scale. If added: commit via
        `build(scripts): add plan-ref-sweep.sh — surfaced by
        project-reorganization/section-02.3 retrospective`.
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether code
        changes invalidated any CLAUDE.md, `.claude/rules/*.md`, or
        `canon.md` claims. If no API/command/phase changes, document
        briefly. Fix any drift NOW.

---

## 02.R Third Party Review Findings

<!-- Reserved for the dual-source `/tpr-review` (Codex + Gemini) and other external reviewers. -->

- None.

---

## 02.N Completion Checklist

- [ ] `test-all.sh:37` JSON_PATH default fixed (02.1)
- [ ] `docs/development/versioning.md:53` website reference fixed (02.2)
- [ ] Global sweep returns zero stale `website/` references (02.3)
- [ ] `./test-all.sh` green post-fix
- [ ] `./test-all.sh --json` writes to new default path successfully
- [ ] Single atomic commit landed: `fix(plan): repair stale website/ path references`
- [ ] Plan annotation cleanup: N/A (no `.rs` files modified)
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table for §02 → `Complete`
  - [ ] `00-overview.md` mission success criterion "Stale website/ paths repaired" → checked
  - [ ] `00-overview.md` Known Bugs table rows for §02-scoped bugs → `Fixed`
  - [ ] `index.md` Quick Reference table updated
- [ ] `/tpr-review` passed — low-risk section; review should be brief
- [ ] `/impl-hygiene-review` passed (AFTER TPR clean)
- [ ] `/improve-tooling` **section-close sweep** — per-subsection retrospectives (02.1, 02.2, 02.3) should already be committed. Cross-subsection pattern check: 02.1 and 02.2 are both "find a specific line and apply a one-line fix" — was there friction in locating the exact lines from Pass 1 research? If so, the `verify-plan-references.sh` helper from 02.2's retrospective may subsume this. Verify the helper (if built) covers both cases. If no cross-cutting gap: document "Section-02 close sweep: per-subsection retrospectives covered everything; no cross-subsection patterns required new tooling."
- [ ] `/sync-claude` **section-close doc sync** — verify Claude artifacts across all section commits. Map changed crates to rules files, check CLAUDE.md, canon.md. Fix drift NOW.

**Exit Criteria:** `rg 'website/' ori_lang/ --type sh --type md --type toml --type yaml` returns only documentation references to the sibling repo. `./test-all.sh` green, `./test-all.sh --json` writes to `target/test-all-results.json` successfully. A single atomic commit (`fix(plan): repair stale website/ path references`) contains exactly two file edits. Subsequent sections (§04 blog migration, §09 verification) can cite this commit as proof that the website boundary is no longer drifting within ori_lang.
