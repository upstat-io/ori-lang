---
section: "08"
title: "Script Consolidation & Reference Sweep"
status: in-progress
reviewed: false
goal: "Execute the Option C hybrid script consolidation — move 4 cleanly-moved scripts (clippy-all, llvm-build, llvm-clippy, llvm-test) to `scripts/dev/`, install permanent thin-exec wrappers at root for the 4 hot-path scripts (test-all, full-check, fmt-all, build-all) while their real implementations live in `scripts/dev/`, move `diagnostics/` to `scripts/diagnostics/`, and atomically sweep every affected script reference (747 matches / 230 files per Pass 1 refreshed census) across CLAUDE.md, CONTRIBUTING.md, all .claude/rules/*.md, all .claude/skills/*/SKILL.md, all .claude/commands/*.md, .codex/skills/tp-help/SKILL.md, plans/**, docs/compiler/design/appendices/E-coding-guidelines.md, compiler/ori_llvm/tests/aot/util/aot.rs, scripts/release.sh, and .claude/rules/diagnostic.md — all in a single atomic commit."
success_criteria:
  - "`scripts/dev/` exists and contains 8 dev script files (test-all.sh, full-check.sh, fmt-all.sh, build-all.sh, clippy-all.sh, llvm-build.sh, llvm-clippy.sh, llvm-test.sh)"
  - "`scripts/diagnostics/` exists and contains the 16 diagnostic scripts previously in `diagnostics/` PLUS the new `_repo-root.sh` canonical helper (Finding 1 fix)"
  - "`diagnostics/` directory absent"
  - "Root still contains 4 thin exec wrappers: test-all.sh, full-check.sh, fmt-all.sh, build-all.sh (each ~5 lines, with `readlink -f` symlink-safety)"
  - "Root does NOT contain: clippy-all.sh, llvm-build.sh, llvm-clippy.sh, llvm-test.sh"
  - "Root still contains: install.sh, setup.sh (unchanged per Option C)"
  - "Running `./test-all.sh` at repo root execs `scripts/dev/test-all.sh` via the wrapper and produces identical output to direct invocation"
  - "`lefthook run pre-commit` passes (uses `./fmt-all.sh` and `./full-check.sh` which are wrappers)"
  - "Reference sweep verification: `rg '\\./(clippy-all|llvm-build|llvm-clippy|llvm-test)\\.sh'` returns ONLY `scripts/dev/` paths in the result (no root-form references remaining)"
  - "Hot-path references (test-all.sh, full-check.sh, fmt-all.sh, build-all.sh) may still appear as `./<name>.sh` because the wrappers preserve that path — these are expected and correct"
  - "`.claude/rules/diagnostic.md` §Diagnostic Scripts section updated to reference `scripts/diagnostics/` instead of `diagnostics/`"
  - "CLAUDE.md §Commands section lines 133, 134, 151, 152 updated"
  - "CONTRIBUTING.md script references updated"
  - "`compiler/ori_llvm/tests/aot/util/aot.rs` test-code reference updated"
  - "`compiler/oric/src/llvm_dump/mod.rs:8` comment updated to reference `scripts/diagnostics/ir-dump.sh` (Pattern E Step E.4 manual edit)"
  - "All 9 broken `$(cd \"$SCRIPT_DIR/..\" && pwd)` patterns in diagnostic scripts replaced with `$REPO_ROOT` from `_repo-root.sh` (Finding 1 fix)"
  - "Pattern E sweep used explicit allow-list (CLAUDE.md, CONTRIBUTING.md, .claude/, .codex/, plans/ except plans/completed/, docs/, lefthook.yml) — NOT deny-list (Finding 2 fix)"
  - "`git diff --name-only HEAD~1 HEAD | grep '^compiler/'` returns ONLY `compiler/oric/src/llvm_dump/mod.rs` + `compiler/ori_llvm/tests/aot/util/aot.rs` (Finding 2 negative-pin)"
  - "`git diff --name-only HEAD~1 HEAD | grep '^library/'` returns empty (Finding 2 negative-pin)"
  - "Invocation matrix (§08.7) all 33 cells (29 exercisable + 4 PATH SKIP) PASS, SKIP (documented), or fail as documented limitation (C3 PWD assumption for cross-dir wrappers; C9 SKIP for PATH invocation)"
  - "`§08.8` atomic commit uses EXPLICIT path staging (no `git add -A`) with worktree cleanliness gate (Finding 4 fix)"
  - "`./test-all.sh` green post-sweep (validates hot-path wrappers work end-to-end)"
  - "Single atomic commit lands: `refactor(plan): consolidate dev scripts into scripts/dev/ + atomic reference sweep`"
  - "Rollback strategy (§08.N) documented for catastrophic failure modes (Finding 6 fix)"
  - "Satisfies mission criteria: '**Zero stale root references**', '**Lefthook pre-commit passes**', '**`./test-all.sh` green**', '**Hot-path 4 muscle memory preserved**'"
inspired_by:
  - "rustc `x.py` — single entry point principle (adapted as stable root facades)"
  - "swift `utils/` — consolidated developer scripts"
  - "gleam `bin/` + Makefile — root-level commands forwarding to internal scripts"
depends_on: ["02", "03", "05", "06", "07"]
third_party_review:
  status: resolved
  updated: 2026-04-11
sections:
  - id: "08.1"
    title: "Create scripts/dev/ and scripts/diagnostics/ Directories"
    status: not-started
  - id: "08.2"
    title: "Move Cleanly-Moved Scripts (clippy-all, llvm-*)"
    status: not-started
  - id: "08.3"
    title: "Move Hot-Path Scripts & Install Root Wrappers"
    status: not-started
  - id: "08.4"
    title: "Move diagnostics/ to scripts/diagnostics/ (with repo-root resolution fix)"
    status: not-started
  - id: "08.5"
    title: "Atomic Reference Sweep (747 matches / 230 files)"
    status: not-started
  - id: "08.6"
    title: "End-to-End Validation"
    status: not-started
  - id: "08.7"
    title: "Invocation Matrix Verification (script × context)"
    status: not-started
  - id: "08.8"
    title: "Atomic Commit"
    status: not-started
  - id: "08.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "08.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Script Consolidation & Reference Sweep

**Status:** In Progress (pre-execution) — frontmatter auto-flipped by the plan-audit linter when iteration-1 TPR findings landed with `[x]` resolutions in §08.R. The section's IMPLEMENTATION work (§08.1 through §08.8) has NOT begun; this status reflects "has TPR findings being resolved pre-execution", not "section is being implemented". Subsection statuses remain `not-started` accordingly. §09.6 plan-close handles both `not-started→complete` and `in-progress→complete` transitions.
**Goal:** The load-bearing section for the entire reorganization plan. Execute the Option C hybrid script model: consolidate internal dev scripts under `scripts/dev/`, merge `diagnostics/` into `scripts/diagnostics/`, install permanent exec wrappers at root for the 4 daily-use scripts (so developer muscle memory is preserved), and atomically sweep 747 references across 230 files. This is the biggest section by both reference count (68% of all script references point at `test-all.sh` alone) and by number of touched file categories (CLAUDE.md, CONTRIBUTING.md, all .claude/* rule files and skills, all plan files, test code, doc appendices).

**Success Criteria:** (see frontmatter above — extensive; summarized here)

- [ ] `scripts/dev/` contains 8 files; `scripts/diagnostics/` contains 16 files
- [ ] `diagnostics/` absent; hot-path 4 wrappers live at root
- [ ] Reference sweep complete: no `./clippy-all.sh` / `./llvm-*.sh` references remain outside `scripts/dev/` in the repo
- [ ] Hot-path references (`./test-all.sh` etc.) may remain since wrappers preserve those paths
- [ ] `./test-all.sh` green via wrapper; `scripts/dev/test-all.sh` green direct
- [ ] `lefthook run pre-commit` green
- [ ] Single atomic commit lands
- [ ] Mission criteria #2 (zero stale root references), #3 (lefthook passes), #5 (test-all green), #11 (muscle memory preserved) all satisfied

**Context:** Pass 1 Agent 1 verified the blast radius as 747 matches across 230 files, with `test-all.sh` dominating at 503 matches (68% of total). The Option C decision (user-approved during Step 9 review) splits the 8 internal dev scripts into two groups:

- **Cleanly moved (4 scripts)**: `clippy-all.sh`, `llvm-build.sh`, `llvm-clippy.sh`, `llvm-test.sh` — moved to `scripts/dev/` with ALL references updated to the new path. Zero trace at root.
- **Wrapper-preserved (4 hot-path scripts)**: `test-all.sh`, `full-check.sh`, `fmt-all.sh`, `build-all.sh` — real implementation moves to `scripts/dev/`, but a permanent thin exec wrapper is installed at root. Existing references to `./test-all.sh` etc. continue to resolve correctly without modification.

**Critical ordering insight:** The hot-path wrappers let us AVOID rewriting 503 + 7 + 29 + 2 = 541 references. Only the cleanly-moved scripts' references need sweeping: 175 (clippy-all) + 30 (llvm-test) + 0 (llvm-build, llvm-clippy) = ~205 references. That's a ~73% reduction in sweep work compared to rewriting everything.

**However:** CLAUDE.md line 133 explicitly lists `./clippy-all.sh` as "Primary", and CLAUDE.md line 134 lists `./llvm-test.sh`. These specific CLAUDE.md references are among the 205 that need updating. Similarly, `.claude/rules/*.md`, `.claude/skills/*.md`, CONTRIBUTING.md line 33, docs appendices, and test code.

**Script internal call chain (recap from overview)**: `full-check.sh` calls `$SCRIPT_DIR/clippy-all.sh` + `$SCRIPT_DIR/test-all.sh`. When all three land in `scripts/dev/`, `$SCRIPT_DIR` resolves to `scripts/dev/`, making them siblings. The root wrapper for `full-check.sh` execs into `scripts/dev/full-check.sh`, which then uses its local `$SCRIPT_DIR` to find `clippy-all.sh` (cleanly moved, sibling) and `test-all.sh` (wrapper target, also in `scripts/dev/`). Chain works unchanged.

**Reference implementations:**
- **rustc** (`x.py` + `src/bootstrap/`): the root `x.py` is a 100-line Python dispatcher forwarding to `src/bootstrap/`. Rustc chose single-entry-point; we adopt the dispatch pattern via 4 specific wrappers instead of a single `x.sh` because Ori's `./test-all.sh` muscle memory is already in 503 places.
- **gleam** (`bin/gleam` + Makefile): gleam has a Makefile at root that delegates to `bin/` scripts. Same pattern as our wrappers.
- **swift** (`utils/build-script` + `utils/`): swift's utilities live in a single consolidated dir; we adopt the consolidation but keep stable root entry points.

**Depends on:** §02 (website paths repaired), §03 (gitignore clean), §05 (orphans gone), §06 (LSP gone), §07 (scratchpad gone). §08 is the terminal state-changing section — after it, only §09 verification runs.

---

## 08.1 Create scripts/dev/ and scripts/diagnostics/ Directories

**File(s):** `scripts/dev/` (new), `scripts/diagnostics/` (new), `target/` (must exist as a persistent workspace for sweep-pattern file lists)

- [ ] Create the new directories:
  ```bash
  cd /home/eric/projects/ori_lang
  mkdir -p scripts/dev scripts/diagnostics
  # target/ already exists (cargo build dir, gitignored). Ensure the sweep-pattern
  # file lists can be written there — TPR-XX-001-gemini iteration 2 fix moved them
  # from /tmp/ (volatile) to target/ (persistent-until-cargo-clean, gitignored).
  mkdir -p target
  ```

- [ ] Verify:
  ```bash
  ls scripts/dev/ scripts/diagnostics/
  # Expected: both exist, both empty
  test -d target && echo "target/ exists (sweep-pattern file list destination)"
  ```

- [ ] **Enumerate pre-move fixture files** (TPR-XX-001-codex iteration 2 fix — explicit per-file fixture staging):
  ```bash
  find diagnostics/fixtures -type f > target/sweep-pattern-fixtures-files.txt
  wc -l target/sweep-pattern-fixtures-files.txt
  # Expected: ~1-5 fixture files (current count as of 2026-04-11: mismatch-wrapper.sh and others)
  ```
  - [ ] Fixture file list captured. This list is used by §08.8 to stage each fixture file individually (pre-move paths) and by the same subsection to stage each file at its post-move path (`scripts/diagnostics/fixtures/*`). Eliminates the `git add -u diagnostics/fixtures/` directory-level add that the iteration 1 fix overlooked.

- [ ] Note: empty directories are not tracked by git. The first `git mv` in 08.2 will populate them and make them git-visible.

- [ ] **Subsection close-out (08.1)** — MANDATORY before starting 08.2:
  - [ ] Both directories exist
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — trivial
        mkdir. No tooling gap. Document: "Retrospective 08.1: no tooling
        gap — mkdir is adequate."

---

## 08.2 Move Cleanly-Moved Scripts (clippy-all, llvm-*)

**File(s):** `clippy-all.sh`, `llvm-build.sh`, `llvm-clippy.sh`, `llvm-test.sh` — moved via `git mv` from root to `scripts/dev/`

These 4 scripts are cleanly moved with no root wrappers. All references in other files will be updated in §08.5.

- [ ] Move each via `git mv`:
  ```bash
  cd /home/eric/projects/ori_lang
  git mv clippy-all.sh scripts/dev/clippy-all.sh
  git mv llvm-build.sh scripts/dev/llvm-build.sh
  git mv llvm-clippy.sh scripts/dev/llvm-clippy.sh
  git mv llvm-test.sh scripts/dev/llvm-test.sh
  ```

- [ ] Verify:
  ```bash
  ls scripts/dev/
  # Expected: clippy-all.sh llvm-build.sh llvm-clippy.sh llvm-test.sh
  ls *.sh 2>&1 | grep -E '(clippy-all|llvm-)'
  # Expected: no matches (all root-level copies gone)
  ```

- [ ] Verify git status shows the renames:
  ```bash
  git status --short
  # Expected: R  clippy-all.sh -> scripts/dev/clippy-all.sh (etc.)
  ```

- [ ] Verify the scripts still work from their new location:
  ```bash
  # Non-blocking smoke test — just start them and kill immediately
  timeout 3 bash scripts/dev/clippy-all.sh 2>&1 | head -3 || true
  timeout 3 bash scripts/dev/llvm-test.sh 2>&1 | head -3 || true
  ```
  - [ ] Both scripts load without immediate syntax errors

- [ ] **Subsection close-out (08.2)** — MANDATORY before starting 08.3:
  - [ ] 4 scripts moved via `git mv`
  - [ ] Root-level copies gone
  - [ ] Scripts still parse from new location
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — `git
        mv` is the canonical tool; no gap. Document: "Retrospective 08.2:
        no tooling gap — git mv handled the rename preservation."

---

## 08.3 Move Hot-Path Scripts & Install Root Wrappers

**File(s):** `test-all.sh`, `full-check.sh`, `fmt-all.sh`, `build-all.sh` — moved via `git mv` to `scripts/dev/`, then NEW thin wrappers written at root

The hot-path 4 are special: their real content moves to `scripts/dev/`, but a NEW file with the same name is created at root containing a 3-line exec wrapper that forwards to the real implementation.

### Documented invocation assumptions (Phase 2 Findings 4 and 8, 2026-04-11)

The wrappers (and the inner scripts at `scripts/dev/`) inherit the pre-move behavior of the hot-path scripts — specifically:

1. **PWD assumption — wrappers require cwd = repo root (Finding 8).** The inner implementations (`scripts/dev/test-all.sh` etc.) resolve many paths as cwd-relative: `test-all.sh` writes logs to `test-all.log`, invokes `./target/debug/ori`, and writes JSON results to `target/test-all-results.json`. If invoked from a subdirectory (e.g., `cd compiler && ../../test-all.sh`), these paths break silently — output lands in `compiler/target/` or `compiler/test-all.log` instead of the repo root. This is a **pre-existing behavior** inherited from the old root-level scripts; the wrappers do not introduce new breakage, but they also do not fix it.

   **Decision (Finding 8 Option A):** Document the assumption, do NOT change the wrapper template to add `cd "$(git rev-parse --show-toplevel)"`. Rationale:
   - The pre-existing muscle-memory pattern is `./test-all.sh` from the repo root — every existing invocation site already satisfies cwd=root
   - Adding a cd-anchor to the wrapper changes behavior for edge cases that aren't covered by the 747 existing references
   - The matrix verification (§08.7 C3) exercises cross-dir invocation and captures the documented limitation
   - A future plan can revisit and add the cd-anchor if the limitation proves painful; the wrapper template is exactly 1 line of change away

   **User-facing documentation:** Add a one-line note to the wrapper template explaining the assumption. Users who invoke from subdirectories will see the note on the first line of the wrapper file and understand the constraint.

2. **Symlink assumption — wrappers assume invocation via the real path, not a symlink (Finding 4 / §08.7 C4).** The wrapper uses `$(dirname "$0")` which does NOT resolve through symlinks. If a user symlinks `~/bin/test-all -> /repo/test-all.sh` and runs `test-all`, `$(dirname "$0")` returns `~/bin`, not `/repo`, and the exec target path is wrong.

   **Decision:** Use `readlink -f` to resolve the symlink at wrapper-entry time. This costs one extra line per wrapper but makes the wrappers robust against symlink aliasing (a legitimate and common developer pattern). The §08.7 C4 matrix cell verifies this.

### Wrapper template (all 4 wrappers share this shape)

```bash
#!/usr/bin/env bash
# Permanent root facade for <script>.sh — real implementation at scripts/dev/<script>.sh.
# Preserves `./<script>.sh` muscle memory while consolidating implementation under scripts/dev/.
# Do NOT treat this as a temporary shim — it is a permanent public-ish entry point.
#
# Assumption: invocation cwd = repo root (the inner script writes logs/artifacts as
# cwd-relative paths). Invocation from a subdirectory is UNSUPPORTED — it will break
# silently as output lands in the wrong directory. Matrix cell §08.7 C3 captures this.
# To invoke from a subdirectory, use `./test-all.sh` from the repo root instead.
#
# Symlink-safe: uses readlink -f to resolve through symlinks when $0 is a real path
# (direct invocation `./test-all.sh` or absolute path). `ln -s $PWD/test-all.sh
# ~/bin/test-all && test-all` works correctly IF `~/bin/test-all` was launched via
# an absolute resolved path (matrix cell §08.7 C4).
#
# **PATH invocation is UNSUPPORTED.** When `$0` is just a bare name (e.g. the user
# added the repo root to PATH and runs `test-all.sh` without `./` from any cwd),
# `readlink -f "$0"` treats the bare name as cwd-relative and returns
# `$PWD/test-all.sh`, which will NOT exist outside the repo root. The wrapper will
# then try to exec `$PWD/scripts/dev/test-all.sh` and fail with "No such file".
# Matrix cell §08.7 C9 explicitly documents this as a SKIP (unsupported). Rationale:
# adding PATH resolution (`command -v` + `readlink -f`) would add complexity for an
# edge case developers don't hit — aliases to `./test-all.sh` work fine, and the
# typical `cargo`-style dev workflow uses `./test-all.sh` directly from repo root.
SCRIPT_PATH="$(readlink -f "$0")"
exec "$(dirname "$SCRIPT_PATH")/scripts/dev/<script>.sh" "$@"
```

- [ ] Move the 4 hot-path scripts:
  ```bash
  git mv test-all.sh scripts/dev/test-all.sh
  git mv full-check.sh scripts/dev/full-check.sh
  git mv fmt-all.sh scripts/dev/fmt-all.sh
  git mv build-all.sh scripts/dev/build-all.sh
  ```

- [ ] Verify internal call chain still works in `scripts/dev/full-check.sh`:
  ```bash
  grep -n '"\$SCRIPT_DIR/' scripts/dev/full-check.sh
  # Expected: lines 28 and 39 reference $SCRIPT_DIR/clippy-all.sh and $SCRIPT_DIR/test-all.sh
  ```
  - [ ] Both references are still present (unchanged by the move)
  - [ ] These will resolve correctly at runtime because `$SCRIPT_DIR` now evaluates to `scripts/dev/` (the new home of all three files)

- [ ] Verify `scripts/dev/full-check.sh` discovers its siblings:
  ```bash
  # Trace the $SCRIPT_DIR resolution manually
  cd /home/eric/projects/ori_lang
  SCRIPT_DIR="$(dirname "$(readlink -f "scripts/dev/full-check.sh")")"
  echo "$SCRIPT_DIR"
  # Expected: /home/eric/projects/ori_lang/scripts/dev
  ls "$SCRIPT_DIR/clippy-all.sh" "$SCRIPT_DIR/test-all.sh"
  # Both should exist
  ```

- [ ] **Create the 4 root wrappers.** For each, use `Write` tool to create the new root file. All 4 use `readlink -f` for symlink-safety (§08.7 C4) and document the cwd=repo-root assumption (§08.7 C3):

  **test-all.sh wrapper**:
  ```bash
  #!/usr/bin/env bash
  # Permanent root facade for test-all.sh — real implementation at scripts/dev/test-all.sh.
  # Preserves `./test-all.sh` muscle memory (503+ references across plans and docs) while
  # consolidating implementation under scripts/dev/. Do NOT treat as a temporary shim —
  # this wrapper is a permanent public-ish entry point per the Option C hybrid model.
  #
  # Assumption: invocation cwd = repo root (inner script writes cwd-relative artifacts).
  # Invocation from a subdirectory is UNSUPPORTED — see §08.7 C3. Symlink-safe via readlink -f.
  SCRIPT_PATH="$(readlink -f "$0")"
  exec "$(dirname "$SCRIPT_PATH")/scripts/dev/test-all.sh" "$@"
  ```

  **full-check.sh wrapper**:
  ```bash
  #!/usr/bin/env bash
  # Permanent root facade for full-check.sh — real implementation at scripts/dev/full-check.sh.
  # Hot-path wrapper per Option C hybrid script model — used by lefthook.yml pre-commit.
  #
  # Assumption: cwd = repo root. Symlink-safe via readlink -f.
  SCRIPT_PATH="$(readlink -f "$0")"
  exec "$(dirname "$SCRIPT_PATH")/scripts/dev/full-check.sh" "$@"
  ```

  **fmt-all.sh wrapper**:
  ```bash
  #!/usr/bin/env bash
  # Permanent root facade for fmt-all.sh — real implementation at scripts/dev/fmt-all.sh.
  # Hot-path wrapper per Option C hybrid script model — used by lefthook.yml pre-commit.
  #
  # Assumption: cwd = repo root. Symlink-safe via readlink -f.
  SCRIPT_PATH="$(readlink -f "$0")"
  exec "$(dirname "$SCRIPT_PATH")/scripts/dev/fmt-all.sh" "$@"
  ```

  **build-all.sh wrapper**:
  ```bash
  #!/usr/bin/env bash
  # Permanent root facade for build-all.sh — real implementation at scripts/dev/build-all.sh.
  # Hot-path wrapper per Option C hybrid script model.
  #
  # Assumption: cwd = repo root. Symlink-safe via readlink -f.
  SCRIPT_PATH="$(readlink -f "$0")"
  exec "$(dirname "$SCRIPT_PATH")/scripts/dev/build-all.sh" "$@"
  ```

- [ ] Make all 4 wrappers executable:
  ```bash
  chmod +x test-all.sh full-check.sh fmt-all.sh build-all.sh
  ```

- [ ] Add the 4 NEW wrapper files to git:
  ```bash
  git add test-all.sh full-check.sh fmt-all.sh build-all.sh
  ```

- [ ] Verify git status:
  ```bash
  git status --short | grep -E '(test-all|full-check|fmt-all|build-all)\.sh'
  # Expected: 4 renames (R  old -> scripts/dev/...) + 4 new files (A  test-all.sh, etc.)
  ```

- [ ] Smoke test a wrapper — run `./test-all.sh --help` or similar non-blocking invocation:
  ```bash
  # If test-all.sh supports --help, use it. Otherwise run with a short timeout and kill.
  timeout 5 ./test-all.sh 2>&1 | head -10 || true
  ```
  - [ ] Output shows the wrapper-forwarding is working (the real script's initial lines appear)

- [ ] **Subsection close-out (08.3)** — MANDATORY before starting 08.4:
  - [ ] 4 hot-path scripts moved to scripts/dev/
  - [ ] 4 root wrappers created and executable
  - [ ] Smoke test proves wrapper forwarding works
  - [ ] Internal call chain in full-check.sh still resolves correctly
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        wrapper template was repeated 4 times with minor variations. Is a
        `scripts/dev/generate-wrapper.sh <script-name>` helper worth
        building? Probably not — wrappers are one-time infrastructure.
        Document: "Retrospective 08.3: 4-time repetition; helper not
        justified for one-time facade creation."

---

## 08.4 Move diagnostics/ to scripts/diagnostics/ (with repo-root resolution fix)

**File(s):**
- `diagnostics/` → `scripts/diagnostics/` (git mv of entire directory)
- **NEW**: `scripts/diagnostics/_repo-root.sh` (new helper)
- **EDITED** (repo-root resolution): `scripts/diagnostics/_common.sh` (lines 29, 70, 113), `scripts/diagnostics/check-debug-flags.sh` (line 29), `scripts/diagnostics/self-test.sh` (line 22), `scripts/diagnostics/dual-exec-verify.sh` (line 33), `scripts/diagnostics/valgrind-aot.sh` (lines 118, 136), `scripts/diagnostics/fixtures/mismatch-wrapper.sh` (line 25)

**CRITICAL FINDING (Phase 2 dual-source review, 2026-04-11):** Seven diagnostic scripts derive the repo root via `ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"` (or the inlined equivalent). This resolves correctly at the current depth (`/repo/diagnostics/<script>.sh` → parent = `/repo/`) but becomes WRONG after the move to `scripts/diagnostics/` (`/repo/scripts/diagnostics/<script>.sh` → parent = `/repo/scripts/`). Every affected script uses `ROOT_DIR` / `root_dir` to locate `target/debug/ori`, `target/release/ori`, `compiler/oric/src/debug_flags.rs`, `CLAUDE.md`, `plans/code-journeys/`, `tests/valgrind/`, the `build/` JSON output path, and grep roots for the compiler tree — each a broken-at-the-new-depth lookup.

**The false claim removed:** The prior version of this subsection said "Most diagnostic scripts use `$(dirname "$0")` patterns which work unchanged". That is **wrong** for the scripts that use `$SCRIPT_DIR/..` to find the repo root — they break silently (run from wrong root → can't find binary → error message that misleads debugging) at the new depth.

**The fix (architecturally correct per CLAUDE.md §The One Rule):** Add a canonical repo-root resolver helper `scripts/diagnostics/_repo-root.sh` that uses `git rev-parse --show-toplevel` (depth-independent and symlink-safe), and have every script source it and use `$REPO_ROOT` instead of re-deriving from `$SCRIPT_DIR/..`. This eliminates the depth coupling entirely — future reorgs won't break the scripts again.

**Why Option B (helper) over Option A (per-script s|/..|/../..|):**
1. Depth-independent: `git rev-parse --show-toplevel` returns the repo root regardless of where the script lives in the tree; survives any future reorg
2. Symlink-safe: `git rev-parse` resolves through symlinks correctly, where `cd ... && pwd` does not
3. SSOT: one canonical resolver instead of 9 instances of `$(cd "$SCRIPT_DIR/.." && pwd)` scattered across scripts
4. Matches CLAUDE.md "Continuous improvement — fix at the source, never work around a problem"
5. No per-script bookkeeping for depth; the helper is a one-time cost

**Fallback strategy (non-git checkouts):** If `git rev-parse` fails (script run from a tarball or non-git checkout — unlikely but possible), fall back to a depth-aware `$SCRIPT_DIR/../..` resolution with a sanity check (verify `Cargo.toml` exists at the computed root). This preserves backward compatibility.

---

### 08.4 steps

- [ ] **Step 1: Move the directory and its contents via `git mv`.** Per-file to preserve git rename tracking:
  ```bash
  cd /home/eric/projects/ori_lang
  git mv diagnostics/*.sh scripts/diagnostics/
  git mv diagnostics/*.md scripts/diagnostics/
  git mv diagnostics/fixtures scripts/diagnostics/fixtures
  ```
  Fallback per-file loop if the globs misbehave:
  ```bash
  cd diagnostics
  for f in *.sh *.md; do
    git mv "$f" "../scripts/diagnostics/$f"
  done
  cd ..
  git mv diagnostics/fixtures scripts/diagnostics/fixtures
  ```

- [ ] **Step 2: Remove the now-empty `diagnostics/` directory:**
  ```bash
  rmdir diagnostics 2>&1 || ls diagnostics
  ```

- [ ] **Step 3: Verify move:**
  ```bash
  ls scripts/diagnostics/
  # Expected: all 16 diagnostic scripts + README.md + _common.sh + fixtures/
  ls diagnostics 2>&1 || echo "absent (expected)"
  ```

- [ ] **Step 4: Enumerate the repo-root resolution pattern in the moved scripts** — this is the finding from Phase 2. The broken pattern is `$(cd "$SCRIPT_DIR/.." && pwd)` used to derive repo root:
  ```bash
  cd /home/eric/projects/ori_lang
  grep -rn '"\$SCRIPT_DIR/\.\." && pwd"' scripts/diagnostics/ 2>&1
  grep -rn 'cd "\$SCRIPT_DIR/\.\."' scripts/diagnostics/ 2>&1
  grep -rn '"\$SCRIPT_DIR/\.\."' scripts/diagnostics/ 2>&1
  ```
  Expected matches (verified against working tree 2026-04-11):
  | File | Line | Pattern |
  |------|------|---------|
  | `scripts/diagnostics/_common.sh` | 29 | `root_dir="$(cd "$SCRIPT_DIR/.." && pwd)"` (inside `find_ori_bin`) |
  | `scripts/diagnostics/_common.sh` | 70 | `root_dir="$(cd "$SCRIPT_DIR/.." && pwd)"` (inside `find_ori_bin_profile`) |
  | `scripts/diagnostics/_common.sh` | 113 | `root_dir="$(cd "$SCRIPT_DIR/.." && pwd)"` (inside `find_any_ori_bin`) |
  | `scripts/diagnostics/check-debug-flags.sh` | 29 | `ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"` |
  | `scripts/diagnostics/self-test.sh` | 22 | `ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"` |
  | `scripts/diagnostics/dual-exec-verify.sh` | 33 | `ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"` |
  | `scripts/diagnostics/valgrind-aot.sh` | 118 | `ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"` (inside case branch) |
  | `scripts/diagnostics/valgrind-aot.sh` | 136 | `ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"` (inside case branch) |
  | `scripts/diagnostics/fixtures/mismatch-wrapper.sh` | 25 | `"$SCRIPT_DIR/../../target/debug/ori"` (inlined sibling-parent climb) |
  - [ ] The match count must match this table exactly. If `grep` reports fewer/more, the working tree drifted — investigate before proceeding.

- [ ] **Step 5: Create the canonical repo-root resolver helper** at `scripts/diagnostics/_repo-root.sh`. Use the `Write` tool:

  **File: `scripts/diagnostics/_repo-root.sh`**
  ```bash
  #!/bin/bash
  # Canonical repo-root resolver for diagnostic scripts.
  #
  # Sourced by diagnostic scripts to set REPO_ROOT to the ori_lang repository root,
  # regardless of where the script lives in the tree. Depth-independent and
  # symlink-safe. Replaces the older `$(cd "$SCRIPT_DIR/.." && pwd)` pattern that
  # broke when diagnostics/ moved to scripts/diagnostics/ (see
  # plans/project-reorganization/section-08-script-consolidation.md §08.4).
  #
  # Usage:
  #   SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  #   source "$SCRIPT_DIR/_repo-root.sh"
  #   # REPO_ROOT is now set
  #
  # Strategy:
  #   1. Prefer `git rev-parse --show-toplevel` — depth-independent, symlink-safe
  #   2. Fallback: walk up from $SCRIPT_DIR until a directory containing Cargo.toml
  #      AND CLAUDE.md is found (sanity check that it's the ori_lang root, not a
  #      crate root that happens to have Cargo.toml)
  #   3. Hard failure if neither strategy works — exit 2 with a clear error
  
  _resolve_repo_root() {
      local candidate
  
      # Strategy 1: git rev-parse (preferred)
      if command -v git >/dev/null 2>&1; then
          candidate="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null || true)"
          if [[ -n "$candidate" && -f "$candidate/Cargo.toml" && -f "$candidate/CLAUDE.md" ]]; then
              echo "$candidate"
              return 0
          fi
      fi
  
      # Strategy 2: walk-up from SCRIPT_DIR
      candidate="$SCRIPT_DIR"
      while [[ "$candidate" != "/" && -n "$candidate" ]]; do
          if [[ -f "$candidate/Cargo.toml" && -f "$candidate/CLAUDE.md" ]]; then
              echo "$candidate"
              return 0
          fi
          candidate="$(dirname "$candidate")"
      done
  
      # Both strategies failed
      echo "Error: could not resolve ori_lang repo root from $SCRIPT_DIR" >&2
      echo "Tried: git rev-parse --show-toplevel and walk-up from SCRIPT_DIR" >&2
      return 2
  }
  
  REPO_ROOT="$(_resolve_repo_root)" || exit 2
  export REPO_ROOT
  ```

- [ ] **Step 6: Update `scripts/diagnostics/_common.sh`** — replace all three `$(cd "$SCRIPT_DIR/.." && pwd)` occurrences with `$REPO_ROOT`, and add the `source` line near the top. Use `Edit` tool:

  Edit 1 — add sourcing after the shebang/docs, before any function definition. The sourcing must appear BEFORE any function that uses `$REPO_ROOT`:
  ```
  old_string:
  # Test whether a binary has LLVM support.
  
  new_string:
  # Source the canonical repo-root resolver (required for REPO_ROOT)
  # shellcheck disable=SC1091
  source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/_repo-root.sh"
  
  # Test whether a binary has LLVM support.
  ```
  Note: `${BASH_SOURCE[0]}` is used instead of `$0` because `_common.sh` is sourced, not executed directly — `$0` would be the parent script's name, not `_common.sh`.

  Edit 2 (line 29, in `find_ori_bin`):
  ```
  old_string:
      local root_dir
      root_dir="$(cd "$SCRIPT_DIR/.." && pwd)"
  
      if [[ -n "${ORI_BIN:-}" ]]; then
  
  new_string:
      local root_dir="$REPO_ROOT"
  
      if [[ -n "${ORI_BIN:-}" ]]; then
  ```

  Edit 3 (line 70, in `find_ori_bin_profile`):
  ```
  old_string:
      local profile="$1"
      local root_dir
      root_dir="$(cd "$SCRIPT_DIR/.." && pwd)"
      local bin="$root_dir/target/$profile/ori"
  
  new_string:
      local profile="$1"
      local root_dir="$REPO_ROOT"
      local bin="$root_dir/target/$profile/ori"
  ```

  Edit 4 (line 113, in `find_any_ori_bin`):
  ```
  old_string:
  # Locate any ori binary (no LLVM requirement).
  # Tries: $ORI_BIN (env), target/debug/ori, target/release/ori, `ori` (PATH).
  # Sets ORI_INTERP to the first working candidate.
  # Exits with code 2 if no binary is found.
  find_any_ori_bin() {
      local root_dir
      root_dir="$(cd "$SCRIPT_DIR/.." && pwd)"
  
  new_string:
  # Locate any ori binary (no LLVM requirement).
  # Tries: $ORI_BIN (env), target/debug/ori, target/release/ori, `ori` (PATH).
  # Sets ORI_INTERP to the first working candidate.
  # Exits with code 2 if no binary is found.
  find_any_ori_bin() {
      local root_dir="$REPO_ROOT"
  ```

- [ ] **Step 7: Update `scripts/diagnostics/check-debug-flags.sh`** (line 29):
  ```
  old_string:
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
  
  new_string:
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  # shellcheck disable=SC1091
  source "$SCRIPT_DIR/_repo-root.sh"
  ROOT_DIR="$REPO_ROOT"
  ```

- [ ] **Step 8: Update `scripts/diagnostics/self-test.sh`** (line 22):
  ```
  old_string:
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
  FIXTURES_DIR="$SCRIPT_DIR/fixtures"
  
  new_string:
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  # shellcheck disable=SC1091
  source "$SCRIPT_DIR/_repo-root.sh"
  ROOT_DIR="$REPO_ROOT"
  FIXTURES_DIR="$SCRIPT_DIR/fixtures"
  ```

- [ ] **Step 9: Update `scripts/diagnostics/dual-exec-verify.sh`** (line 33):
  ```
  old_string:
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
  source "$SCRIPT_DIR/_common.sh"
  
  new_string:
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  # shellcheck disable=SC1091
  source "$SCRIPT_DIR/_repo-root.sh"
  ROOT_DIR="$REPO_ROOT"
  source "$SCRIPT_DIR/_common.sh"
  ```
  Note: `_common.sh` also now sources `_repo-root.sh`, but sourcing it twice is harmless (the file only assigns variables; it has no side effects on re-source).

- [ ] **Step 10: Update `scripts/diagnostics/valgrind-aot.sh`** (both occurrences at lines 118 and 136). These are inside `case` branches, so the simplest fix is to replace the `$(cd "$SCRIPT_DIR/.." && pwd)` with `$REPO_ROOT` directly — but the file must first source `_repo-root.sh` near the top. Add the sourcing near line 34 after `source "$SCRIPT_DIR/_common.sh"` (since `_common.sh` will already source `_repo-root.sh`, we can use `$REPO_ROOT` directly without a second source in valgrind-aot.sh — but being explicit is clearer):
  ```
  old_string (line ~34):
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  source "$SCRIPT_DIR/_common.sh"
  
  new_string:
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  # shellcheck disable=SC1091
  source "$SCRIPT_DIR/_repo-root.sh"
  source "$SCRIPT_DIR/_common.sh"
  ```

  Then replace both `ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"` occurrences inside the case branches:
  ```bash
  sed -i 's|ROOT_DIR="$(cd "\$SCRIPT_DIR/\.\." && pwd)"|ROOT_DIR="$REPO_ROOT"|g' scripts/diagnostics/valgrind-aot.sh
  ```
  Verify:
  ```bash
  grep -n 'ROOT_DIR' scripts/diagnostics/valgrind-aot.sh
  # Expected: both lines now say ROOT_DIR="$REPO_ROOT"
  ```

- [ ] **Step 11: Update `scripts/diagnostics/fixtures/mismatch-wrapper.sh`** (line 25). This fixture script uses an inlined `$SCRIPT_DIR/../../target/debug/ori` at depth-2, which assumes it lives at `diagnostics/fixtures/*.sh`. After the move it lives at `scripts/diagnostics/fixtures/*.sh`, so the depth-2 climb now lands at `scripts/` instead of repo root. Fix by sourcing `_repo-root.sh` from the sibling parent dir and using `$REPO_ROOT/target/debug/ori`:
  ```
  old_string:
      SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
      ...
          "$SCRIPT_DIR/../../target/debug/ori" \
  
  new_string:
      SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
      # shellcheck disable=SC1091
      source "$SCRIPT_DIR/../_repo-root.sh"
      ...
          "$REPO_ROOT/target/debug/ori" \
  ```
  (Read the file first to get the exact `old_string` context; the `...` above represents the lines between that are unchanged.)

- [ ] **Step 12: Verify NO remaining broken `$SCRIPT_DIR/..` repo-root resolution patterns in moved scripts:**
  ```bash
  cd /home/eric/projects/ori_lang
  grep -rn '\$SCRIPT_DIR/\.\.' scripts/diagnostics/ 2>&1 | grep -v '_repo-root.sh'
  # Expected: only references inside comments/docs or inside _repo-root.sh itself (which doesn't exist in this grep since excluded)
  # If any other hit: investigate
  ```
  ```bash
  grep -rn 'cd "\$SCRIPT_DIR/\.\." && pwd' scripts/diagnostics/ 2>&1
  # Expected: zero matches (all patterns replaced)
  ```

- [ ] **Step 13: Verify each repaired diagnostic script still finds the repo root correctly from the new location.** This is the regression guard for Finding 1:
  ```bash
  cd /home/eric/projects/ori_lang
  
  # Test 13a: _common.sh sourcing produces correct REPO_ROOT
  (
    SCRIPT_DIR="$(cd scripts/diagnostics && pwd)"
    # shellcheck disable=SC1091
    source "$SCRIPT_DIR/_repo-root.sh"
    echo "REPO_ROOT from _repo-root.sh: $REPO_ROOT"
    test "$REPO_ROOT" = "/home/eric/projects/ori_lang" || { echo "FAIL: wrong root"; exit 1; }
  )
  
  # Test 13b: check-debug-flags.sh runs and succeeds
  scripts/diagnostics/check-debug-flags.sh 2>&1 | tail -5
  echo "check-debug-flags exit: $?"
  # Expected: exit 0 (or 1 if there are pre-existing ORI_* inconsistencies — but NOT path-related errors)
  
  # Test 13c: self-test.sh --help loads without repo-root resolution errors
  scripts/diagnostics/self-test.sh --help 2>&1 | head -10
  
  # Test 13d: dual-exec-verify.sh --help loads without repo-root resolution errors
  scripts/diagnostics/dual-exec-verify.sh --help 2>&1 | head -10
  
  # Test 13e: _common.sh via find_ori_bin
  (
    set -u
    SCRIPT_DIR="$(cd scripts/diagnostics && pwd)"
    # shellcheck disable=SC1091
    source "$SCRIPT_DIR/_common.sh"
    find_ori_bin
    echo "ORI=$ORI"
    test -x "$ORI" || { echo "FAIL: ORI not executable"; exit 1; }
  )
  # Expected: ORI points to target/debug/ori or target/release/ori
  
  # Test 13f: fixture wrapper resolves correctly
  bash scripts/diagnostics/fixtures/mismatch-wrapper.sh --help 2>&1 | head -5 || true
  # Expected: no "No such file or directory" errors related to target/debug/ori path
  ```
  - [ ] All 6 sub-tests show correct repo-root resolution
  - [ ] If ANY sub-test fails with "No such file or directory" related to `target/debug/ori`, `compiler/oric/src/debug_flags.rs`, or similar repo-rooted paths: STOP, investigate, fix the source before proceeding

- [ ] **Step 14: Verify cross-script path references are still valid** (scripts that reference each other via `$SCRIPT_DIR`):
  ```bash
  grep -rn '\$SCRIPT_DIR/' scripts/diagnostics/ 2>&1 | head -20
  ```
  These are cross-script references (e.g., `diagnose-aot.sh` calls `$SCRIPT_DIR/rc-stats.sh`) and are correctly unchanged by the move because all scripts are still siblings in the same directory.

- [ ] **Step 15: Run the diagnostics self-test from the new location** (the canonical validation):
  ```bash
  timeout 120 bash scripts/diagnostics/self-test.sh 2>&1 | tail -20
  ```
  - [ ] Exit 0 or the self-test's own defined acceptable exit codes
  - [ ] No path-related errors in the output

- [ ] **Subsection close-out (08.4)** — MANDATORY before starting 08.5:
  - [ ] `diagnostics/` absent, `scripts/diagnostics/` contains all 16+ files plus the new `_repo-root.sh`
  - [ ] All 9 listed repo-root patterns replaced with `$REPO_ROOT` (verified via grep)
  - [ ] Sub-tests 13a–13f all pass
  - [ ] `self-test.sh` runs cleanly from the new location
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        canonical `_repo-root.sh` resolver is a broadly-reusable pattern.
        Consider whether `scripts/dev/*.sh` and other non-diagnostic scripts
        should adopt the same pattern. `scripts/perf-baseline.sh`,
        `scripts/cow-benchmark.sh`, `scripts/cache-doctor.sh`, and others
        likely have their own ad-hoc repo-root derivation. File a new bug
        via `/add-bug` if the grep finds similar patterns elsewhere:
        `grep -rn '"\$SCRIPT_DIR/\.\." && pwd"' scripts/ | grep -v
        scripts/diagnostics/`. If hits exist: file the bug and track in
        bug-tracker; do NOT fix in §08.4's commit (scope boundary — this
        section is already large). Document:
        "Retrospective 08.4: canonical repo-root resolver pattern should be
        promoted to scripts/ subtree. Filed as BUG-XX-NNN for follow-up."

---

## 08.5 Atomic Reference Sweep (747 matches / 230 files)

**File(s):** CLAUDE.md, CONTRIBUTING.md, all `.claude/rules/*.md`, all `.claude/skills/*/SKILL.md`, all `.claude/commands/*.md`, `.codex/skills/tp-help/SKILL.md`, `plans/**/*.md`, `docs/compiler/design/appendices/E-coding-guidelines.md`, `compiler/ori_llvm/tests/aot/util/aot.rs`, `scripts/release.sh`, `.claude/rules/diagnostic.md`, `lefthook.yml` (verify — likely no change needed)

The sweep is mechanical but high-risk: a single sed-pattern error could corrupt hundreds of files. The strategy is:
1. **Only update cleanly-moved script references** (`./clippy-all.sh`, `./llvm-*.sh`). The hot-path 4 wrappers mean `./test-all.sh` etc. continue to work unchanged — leave those references alone.
2. **Update `diagnostics/` path prefix to `scripts/diagnostics/`** in documentation (CLAUDE.md line 152, `.claude/rules/diagnostic.md` table, anywhere else).
3. **Use a carefully-scoped `rg` + `sed -i`** approach, with verification after each pattern.

**Sweep patterns in order**:

| Pattern | Old | New | Expected matches |
|---------|-----|-----|------------------|
| A | `./clippy-all.sh` | `./scripts/dev/clippy-all.sh` | ~175 |
| B | `./llvm-build.sh` | `./scripts/dev/llvm-build.sh` | ~0 |
| C | `./llvm-clippy.sh` | `./scripts/dev/llvm-clippy.sh` | ~0 |
| D | `./llvm-test.sh` | `./scripts/dev/llvm-test.sh` | ~30 |
| E | `diagnostics/` | `scripts/diagnostics/` | ~30 (CLAUDE.md + diagnostic.md + related) |

Note: Pattern E is riskier because `diagnostics/` is a common word. Scope must be tight.

- [ ] **Pattern A: `./clippy-all.sh` → `./scripts/dev/clippy-all.sh`** (~175 matches)

  First, enumerate every match with rg:
  ```bash
  cd /home/eric/projects/ori_lang
  rg -l '\./clippy-all\.sh' > target/sweep-pattern-a-files.txt
  wc -l target/sweep-pattern-a-files.txt
  # Expected: ~108 files
  ```

  Verify the list doesn't include unintended files:
  ```bash
  # Expected paths: plans/**, .claude/rules/*.md, .claude/skills/*.md, .claude/commands/*.md, CLAUDE.md, CONTRIBUTING.md, docs/**, scripts/**, compiler/**
  # Unexpected: anything outside these (e.g., target/, .git/, etc.)
  grep -E '^(target|\.git|scripts/dev/clippy-all\.sh)' target/sweep-pattern-a-files.txt || echo "no unexpected files"
  ```
  - [ ] Note: `scripts/dev/clippy-all.sh` (self-reference) — if this appears in the list, handle carefully. The clippy-all.sh script itself doesn't use `./clippy-all.sh` in its own body (verify with `grep -n '\./clippy-all\.sh' scripts/dev/clippy-all.sh`), but `full-check.sh` calls `$SCRIPT_DIR/clippy-all.sh` which is NOT matched by the `./clippy-all.sh` pattern — safe.

  Apply the sed substitution:
  ```bash
  while read -r file; do
    sed -i 's|\./clippy-all\.sh|\./scripts/dev/clippy-all\.sh|g' "$file"
  done < target/sweep-pattern-a-files.txt
  ```

  Verify:
  ```bash
  # Old pattern should be gone
  rg '\./clippy-all\.sh' 2>&1 | grep -v '/scripts/dev/' || echo "clean"
  # New pattern should be present in the expected places
  rg '\./scripts/dev/clippy-all\.sh' | head -10
  ```

- [ ] **Pattern B: `./llvm-build.sh` → `./scripts/dev/llvm-build.sh`** (likely 0 matches)

  ```bash
  rg -l '\./llvm-build\.sh' > target/sweep-pattern-b-files.txt
  wc -l target/sweep-pattern-b-files.txt
  ```
  - [ ] If 0 matches: skip the sed step; pattern B is no-op
  - [ ] If >0 matches: apply same sed substitution

- [ ] **Pattern C: `./llvm-clippy.sh` → `./scripts/dev/llvm-clippy.sh`** (likely 0 matches)

  Same treatment as Pattern B.

- [ ] **Pattern D: `./llvm-test.sh` → `./scripts/dev/llvm-test.sh`** (~30 matches)

  ```bash
  rg -l '\./llvm-test\.sh' > target/sweep-pattern-d-files.txt
  wc -l target/sweep-pattern-d-files.txt
  # Expected: ~16 files

  while read -r file; do
    sed -i 's|\./llvm-test\.sh|\./scripts/dev/llvm-test\.sh|g' "$file"
  done < target/sweep-pattern-d-files.txt

  rg '\./llvm-test\.sh' | grep -v '/scripts/dev/' || echo "clean"
  ```

- [ ] **Pattern E: `diagnostics/` → `scripts/diagnostics/`** (explicit allow-list scoped; DO NOT use deny-list)

  **CRITICAL FINDING (Phase 2 dual-source review, 2026-04-11):** The previous version of this pattern used a deny-list (`grep -v '^target/'`, etc.) to exclude unintended files. That is **unsafe** because the repo contains REAL Rust modules named `diagnostics/` that a blind sweep would corrupt:
  - `compiler/ori_eval/src/diagnostics/` (actual Rust module — `mod diagnostics;` + `mod.rs`)
  - `compiler/ori_patterns/src/errors/diagnostics/` (actual Rust module)
  - `compiler/oric/src/llvm_dump/mod.rs:8` (Rust comment: `//! This is the in-compiler equivalent of \`diagnostics/ir-dump.sh\`...`) — this is a legitimate Rust source comment that happens to mention the path; it is in scope for the sweep BUT the deny-list would not catch it and the corruption risk is that the sed pattern applied to a `.rs` file might interact poorly with other mentions of `diagnostics`

  Pattern E **MUST** use an explicit **allow-list** — only these paths are eligible for the substitution:
  - `CLAUDE.md`
  - `CONTRIBUTING.md` (verify first — may be unchanged)
  - `.claude/` (all `.md` files — skills, rules, commands)
  - `.codex/` (all `.md` files — reviewer skills)
  - `.gemini/` (if present)
  - `plans/` — **EXCEPT** `plans/completed/` (archival per §09.1 exclusion rule)
  - `docs/` — all `.md` files (any diagnostic references in compiler/appendix docs)
  - `lefthook.yml` (verify first — probably unchanged)
  - **`scripts/` top-level `*.sh` and `*.py` files** (Phase 4 TPR-XX-001-gemini fix — defensive inclusion) — ONLY files directly under `scripts/`, **EXCLUDING** `scripts/dev/` (the moved internal scripts don't reference `diagnostics/`) and **EXCLUDING** `scripts/diagnostics/` (self-references, handled in Step E.9 below). Current known consumers in this scope: `scripts/release.sh`, `scripts/cache-doctor.sh`, `scripts/perf-baseline.sh`, `scripts/cow-benchmark.sh`, `scripts/pgo-build.sh`, `scripts/pgo-train.sh`, `scripts/bump-build.sh`, `scripts/sync-version.sh`, `scripts/*.py`. Pre-sweep verification (`rg -n 'diagnostics/' scripts/*.sh scripts/*.py 2>/dev/null`) confirmed zero current references at 2026-04-11 TPR iteration 1, but the allow-list includes them defensively so any future addition is automatically swept.

  Pattern E **MUST NEVER** touch:
  - `compiler/` — contains real Rust `diagnostics` modules + comment references
  - `library/` — stdlib
  - `tests/` — test data, spec tests
  - `target/`, `target-llvm/` — build artifacts
  - `.git/` — git internals
  - `scripts/dev/` — newly-moved hot-path/cleanly-moved scripts (zero `diagnostics/` refs expected; sweep skip defensive)
  - `scripts/diagnostics/` — the newly-moved diagnostic scripts themselves. **Self-references within these files are handled separately in Step E.9**, NOT via Pattern E (TPR-XX-004-codex fix). The reason: Pattern E's allow-list operates on reference paths like `diagnostics/ir-dump.sh` (outside the directory), but the moved scripts' `--help` text and README contain the same substring as self-references (`diagnostics/diagnose-aot.sh file.ori` as an example command). A post-move scoped sweep over `scripts/diagnostics/**` updates them cleanly without risking Pattern E's allow-list expansion.

  **Step E.1 — Safety pre-check: count and verify zero compiler/library/tests matches are eligible for the sweep:**
  ```bash
  cd /home/eric/projects/ori_lang
  # These counts MUST be zero for Pattern E to proceed safely.
  # If any are non-zero, Pattern E must NOT run on these paths — the deny-list
  # is rejected in favor of an explicit allow-list.
  compiler_hits=$(rg -l 'diagnostics/' compiler/ 2>/dev/null | wc -l)
  library_hits=$(rg -l 'diagnostics/' library/ 2>/dev/null | wc -l)
  tests_hits=$(rg -l 'diagnostics/' tests/ 2>/dev/null | wc -l)
  echo "compiler/: $compiler_hits files (MUST NOT be swept — these are real Rust modules + comments)"
  echo "library/:  $library_hits files (MUST NOT be swept)"
  echo "tests/:    $tests_hits files (MUST NOT be swept)"
  ```
  - [ ] Document the counts. Expected (from 2026-04-11 grep verification): `compiler/ = 1 file` (`compiler/oric/src/llvm_dump/mod.rs:8` — a Rust doc comment), `library/ = 0 files`, `tests/ = 0 files`.
  - [ ] **These files MUST NOT be in the allow-list.** The single `compiler/oric/src/llvm_dump/mod.rs:8` reference is a Rust source comment that happens to describe the equivalent shell script. It must be updated separately in Step E.4 below as a manual `Edit` tool call — it is NOT in Pattern E's scope.

  **Step E.2 — Build the explicit allow-list of files to sweep:**
  ```bash
  cd /home/eric/projects/ori_lang
  # Enumerate the allow-list scope. Note: `rg` respects .gitignore so target/ is
  # already excluded; the explicit scope makes safety load-bearing not incidental.
  {
    # CLAUDE.md / CONTRIBUTING.md / lefthook.yml at root
    test -f CLAUDE.md && echo CLAUDE.md
    test -f CONTRIBUTING.md && echo CONTRIBUTING.md
    test -f lefthook.yml && echo lefthook.yml
    # .claude/ .codex/ .gemini/ — all .md files
    find .claude -type f -name '*.md' 2>/dev/null
    find .codex -type f -name '*.md' 2>/dev/null
    find .gemini -type f -name '*.md' 2>/dev/null
    # plans/ EXCEPT plans/completed/
    find plans -type f -name '*.md' -not -path 'plans/completed/*' 2>/dev/null
    # docs/ all .md
    find docs -type f -name '*.md' 2>/dev/null
    # scripts/ top-level .sh and .py (NOT scripts/dev/ or scripts/diagnostics/)
    # TPR-XX-001-gemini fix — defensive inclusion so future additions auto-sweep
    find scripts -maxdepth 1 -type f \( -name '*.sh' -o -name '*.py' \) 2>/dev/null
  } > target/sweep-pattern-e-allowlist.txt
  wc -l target/sweep-pattern-e-allowlist.txt
  ```
  - [ ] Allow-list file count captured.
  - [ ] Manually spot-check the allow-list — `head -50 target/sweep-pattern-e-allowlist.txt` — to verify no compiler/library/tests files leaked in AND no `scripts/dev/` or `scripts/diagnostics/` files leaked in.
  - [ ] **Defensive check**: `grep -E '^(compiler|library|tests|scripts/dev|scripts/diagnostics)/' target/sweep-pattern-e-allowlist.txt` — must return empty. If any match: STOP, fix the allow-list generation.

  **Step E.3 — Narrow the allow-list to files that actually contain `diagnostics/` (avoid touching files that don't need changes):**
  ```bash
  # Filter: only files whose content contains "diagnostics/" in a path-ish context
  xargs -a target/sweep-pattern-e-allowlist.txt rg -l 'diagnostics/' 2>/dev/null \
    | sort -u > target/sweep-pattern-e-files.txt
  wc -l target/sweep-pattern-e-files.txt
  ```
  - [ ] Review the final list manually:
    ```bash
    cat target/sweep-pattern-e-files.txt
    ```
  - [ ] Expected entries (from 2026-04-11 verification): `CLAUDE.md`, `.claude/rules/diagnostic.md`, `.claude/rules/arc.md`, `.claude/rules/eval.md`, `.claude/skills/fix-bug/SKILL.md`, `.claude/skills/improve-tooling/SKILL.md`, `.claude/skills/dual-tpr/envelope-format.md`, `.claude/skills/create-plan/plan-schema.md`, `.claude/skills/continue-roadmap/SKILL.md`, various `plans/*/section-*.md`, `docs/compiler/design/appendices/D-debugging.md`, and similar. NO `compiler/`, `library/`, `tests/` entries.
  - [ ] **If ANY file in `target/sweep-pattern-e-files.txt` is under `compiler/`, `library/`, or `tests/`: STOP. The allow-list is leaking. Investigate before proceeding.**

  **Step E.4 — Manual Rust comment fix (outside Pattern E scope):**
  Update `compiler/oric/src/llvm_dump/mod.rs:8` separately with the `Edit` tool — it is a Rust doc comment that references `diagnostics/ir-dump.sh`:
  ```bash
  grep -n 'diagnostics/' compiler/oric/src/llvm_dump/mod.rs
  ```
  Expected: line 8 reads `//! This is the in-compiler equivalent of \`diagnostics/ir-dump.sh\`, triggered via` (or similar). Use `Edit` tool:
  ```
  old_string: //! This is the in-compiler equivalent of `diagnostics/ir-dump.sh`, triggered via
  new_string: //! This is the in-compiler equivalent of `scripts/diagnostics/ir-dump.sh`, triggered via
  ```
  - [ ] Verify: `grep -n 'diagnostics/' compiler/oric/src/llvm_dump/mod.rs` shows only the new form.
  - [ ] Verify `cargo check -p oric` still passes (comment change cannot affect compilation, but sanity-check).
  - [ ] This single manual edit covers the ONLY in-compiler reference and keeps Pattern E's scope clean.

  **Step E.5 — Apply the sed substitution to the allow-listed files ONLY.** Use multiple scoped patterns rather than a single catch-all to minimize false-positive risk:
  ```bash
  while read -r file; do
    # Backtick-wrapped path: `diagnostics/...` → `scripts/diagnostics/...`
    sed -i 's|`diagnostics/|`scripts/diagnostics/|g' "$file"
    # Explicit ./diagnostics/ → ./scripts/diagnostics/
    sed -i 's|\./diagnostics/|./scripts/diagnostics/|g' "$file"
    # Whitespace-prefixed diagnostics/ (command-line context)
    sed -i 's| diagnostics/| scripts/diagnostics/|g' "$file"
    # Start-of-line diagnostics/ (rare, but possible in code fences)
    sed -i 's|^diagnostics/|scripts/diagnostics/|g' "$file"
    # Double-quoted path: "diagnostics/..." → "scripts/diagnostics/..."
    # (TPR-XX-002-gemini iteration 2 fix — JSON fixtures, schema examples, code
    # blocks with quoted path literals would otherwise escape the sweep.)
    sed -i 's|"diagnostics/|"scripts/diagnostics/|g' "$file"
  done < target/sweep-pattern-e-files.txt
  ```

  **Step E.6 — Post-sweep safety verification.** The sweep MUST NOT have touched `compiler/`, `library/`, or `tests/`. This is verified at commit time (§08.6) via `git diff --stat`, but also check directly here:
  ```bash
  # Any Rust file modified? If any, something leaked.
  git diff --name-only | grep -E '\.(rs|toml)$' | grep -v '^plans/'
  # Expected (post Step E.4): only compiler/oric/src/llvm_dump/mod.rs (from the manual Edit)
  # If any OTHER .rs file appears: STOP, inspect, revert
  ```
  - [ ] Only `compiler/oric/src/llvm_dump/mod.rs` is modified in `compiler/` (from the manual Edit in Step E.4)
  - [ ] No files in `library/` are modified
  - [ ] No files in `tests/` are modified
  - [ ] `plans/completed/` files are NOT modified (archival exclusion)

  **Step E.7 — Verify the substitution landed correctly:**
  ```bash
  # Remaining "diagnostics/" matches in the allow-listed files should only be:
  # - The new "scripts/diagnostics/" form
  # - Prose references that weren't scoped to a path (e.g., "the diagnostics directory without a trailing path")
  while read -r file; do
    grep -n 'diagnostics/' "$file" 2>/dev/null
  done < target/sweep-pattern-e-files.txt \
    | grep -v 'scripts/diagnostics/' \
    | head -20
  ```
  - [ ] Every remaining match is either prose (fine) or needs an additional targeted fix
  - [ ] If any residual `./diagnostics/` or backtick-wrapped `diagnostics/script.sh` form remains, apply a targeted fix via `Edit`

  **Step E.8 — Negative-pin: verify compiler source is untouched by Pattern E.** This is the regression guard for Finding 2:
  ```bash
  # Verify no compiler/ file other than llvm_dump/mod.rs was touched
  git status --porcelain compiler/ 2>&1 \
    | grep -v ' compiler/oric/src/llvm_dump/mod.rs$' \
    | grep -v '^$' \
    || echo "CLEAN — Pattern E did not touch compiler/ beyond the manual llvm_dump/mod.rs edit"
  # Expected: "CLEAN — ..." line printed
  ```
  - [ ] Output shows CLEAN
  - [ ] If any other `compiler/` file is modified: STOP, revert the sweep, investigate why the allow-list leaked

  **Step E.9 — Post-move sweep of `scripts/diagnostics/` self-references.** (TPR-XX-004-codex fix — 2026-04-11.) The moved diagnostic scripts and `README.md` contain internal self-references that say `diagnostics/ir-dump.sh ...` etc. Pattern E's allow-list deliberately EXCLUDES `scripts/diagnostics/` (to avoid scope creep), so those self-references survive unchanged after §08.4's move. This creates a DRIFT finding: the canonical home is `scripts/diagnostics/` but the scripts' own help text still instructs users to run `diagnostics/...`. Fix with a scoped post-move sweep that ONLY touches `scripts/diagnostics/**`.

  Known self-reference locations (from 2026-04-11 TPR verification):
  - `scripts/diagnostics/README.md` lines 30-35, 43-58, 165-183 — usage examples
  - `scripts/diagnostics/ir-dump.sh` lines 5 and 86 — usage header
  - `scripts/diagnostics/self-test.sh` lines 5 and 11 — usage header
  - Any other `.sh` or `.md` in `scripts/diagnostics/` containing `diagnostics/`

  Step E.9 implementation:
  ```bash
  cd /home/eric/projects/ori_lang
  # Enumerate ONLY scripts/diagnostics/ self-refs (post-move state)
  rg -l 'diagnostics/' scripts/diagnostics/ > target/sweep-pattern-e9-files.txt
  wc -l target/sweep-pattern-e9-files.txt
  cat target/sweep-pattern-e9-files.txt
  ```
  - [ ] Review the list. Every entry must be under `scripts/diagnostics/`.
  - [ ] Apply a scoped sed substitution to each file. The pattern is different from Pattern E because we're rewriting `diagnostics/` (internal self-reference) → `scripts/diagnostics/` (canonical new path). NOTE: this is NOT the same as Pattern E's sed — Pattern E replaces `diagnostics/` anywhere in the allow-listed files, while E.9 replaces `diagnostics/` specifically in the moved files' own content (not cross-references FROM elsewhere INTO these files).

  ```bash
  while read -r file; do
    # Match diagnostics/ in path-like contexts: after space, backtick, double-quote,
    # ./ prefix, or start-of-line. Same scoping as Pattern E to avoid rewriting prose.
    # Also match double-quoted paths per TPR-XX-002-gemini iteration 2 fix.
    sed -i 's|`diagnostics/|`scripts/diagnostics/|g; s|\./diagnostics/|./scripts/diagnostics/|g; s| diagnostics/| scripts/diagnostics/|g; s|^diagnostics/|scripts/diagnostics/|g; s|"diagnostics/|"scripts/diagnostics/|g' "$file"
  done < target/sweep-pattern-e9-files.txt
  ```
  - [ ] Verify the substitutions:
    ```bash
    rg -n 'diagnostics/' scripts/diagnostics/ 2>&1 \
      | grep -v '^scripts/diagnostics/' \
      || echo "CLEAN — no surviving stale self-references"
    # Any surviving reference that's NOT already scripts/diagnostics/ is a miss
    rg -n '\bdiagnostics/' scripts/diagnostics/ 2>&1 \
      | grep -v 'scripts/diagnostics/' \
      || echo "CLEAN"
    ```
  - [ ] Spot-check updated files:
    ```bash
    head -35 scripts/diagnostics/README.md | tail -15
    head -10 scripts/diagnostics/ir-dump.sh
    head -15 scripts/diagnostics/self-test.sh
    ```
    - [ ] Each shows `scripts/diagnostics/...` rather than `diagnostics/...` in usage examples

  **Step E.10 — Negative-pin for Step E.9 scope containment.** Verify Step E.9's sweep did NOT escape `scripts/diagnostics/`:
  ```bash
  git diff --name-only 2>&1 | grep -v '^scripts/diagnostics/' \
    | grep -v '^plans/project-reorganization/' \
    | head -20
  # Expected: only files from OTHER Pattern E/A/D sweeps (e.g. CLAUDE.md, .claude/ files)
  # — NOT any new compiler/, library/, tests/ entries from a Step E.9 scope leak
  ```
  - [ ] No compiler/, library/, tests/ entries added by Step E.9

- [ ] **.claude/rules/diagnostic.md — specific section header + table update**:
  ```bash
  grep -n '## Diagnostic Scripts' .claude/rules/diagnostic.md
  # Expected: line 47-ish "## Diagnostic Scripts (`diagnostics/`)"
  ```
  After Pattern E, the header should read "## Diagnostic Scripts (`scripts/diagnostics/`)". Verify.

- [ ] **CLAUDE.md line 152 specific check**:
  ```bash
  sed -n '150,155p' CLAUDE.md
  # Expected: the line 152 diagnostics references are now scripts/diagnostics/
  ```

- [ ] **lefthook.yml verification** (should NOT need changes — hot-path wrappers preserve paths):
  ```bash
  grep -n '\./' lefthook.yml
  # Expected: ./fmt-all.sh and ./full-check.sh (unchanged because they're root wrappers now)
  # Also: ./scripts/sync-version.sh (unchanged, already in scripts/)
  ```
  - [ ] NO changes to lefthook.yml are required, because the wrappers preserve `./fmt-all.sh` and `./full-check.sh` at root

- [ ] **compiler/ori_llvm/tests/aot/util/aot.rs** — test code has a script reference at line 168:
  ```bash
  grep -n '\./llvm-test\.sh\|\./test-all\.sh' compiler/ori_llvm/tests/aot/util/aot.rs
  ```
  - [ ] If pattern D caught this file, `./llvm-test.sh` is already updated. If the reference was `./test-all.sh`, it's unchanged (wrapper preserves path) and needs no action.
  - [ ] If the reference is something else: investigate and fix manually

- [ ] **scripts/release.sh** — internal reference at line 130:
  ```bash
  grep -n '\./test-all\.sh\|\./scripts/dev/' scripts/release.sh
  ```
  - [ ] The reference to `./test-all.sh` is unchanged (wrapper preserves)
  - [ ] Note: if release.sh also references `./clippy-all.sh` or similar, pattern A caught it

- [ ] Global post-sweep verification — ZERO cleanly-moved script references at root form remain:
  ```bash
  rg '\./(clippy-all|llvm-build|llvm-clippy|llvm-test)\.sh' 2>&1 \
    | grep -v 'scripts/dev/' \
    | grep -v '^plans/project-reorganization/section-08-script-consolidation\.md' \
    || echo "CLEAN — zero stale cleanly-moved references"
  ```
  - [ ] Output: "CLEAN — zero stale cleanly-moved references" (the grep -v excludes this very plan file which mentions the old paths for context)

- [ ] Hot-path references may still appear as `./test-all.sh` etc. — this is EXPECTED and correct (wrappers preserve those paths):
  ```bash
  rg -c '\./test-all\.sh' 2>&1 | head -5
  # Expected: most references preserved (wrappers mean the root path still resolves)
  ```

- [ ] **Subsection close-out (08.5)** — MANDATORY before starting 08.6:
  - [ ] All 5 sweep patterns applied
  - [ ] Zero stale cleanly-moved references remain
  - [ ] `diagnostics/` path references updated to `scripts/diagnostics/`
  - [ ] CLAUDE.md line 152 updated; `.claude/rules/diagnostic.md` header updated
  - [ ] lefthook.yml verified unchanged (wrappers preserve paths)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — this is
        THE section where tooling matters most. The 5-pattern sweep with
        per-pattern verification was mechanical but error-prone. Is there
        a reusable `scripts/dev/atomic-sweep.sh <old> <new> [scope-exclude]`
        helper that: (1) enumerates matching files, (2) shows a preview,
        (3) asks for confirmation, (4) applies sed, (5) verifies the old
        pattern is gone and the new pattern is present in expected count?
        This would be broadly useful for every future reorg. Build it NOW
        and commit via `build(scripts): add atomic-sweep.sh — surfaced
        by project-reorganization/section-08.5 retrospective`. High
        confidence this helper has reuse value beyond this one plan.

---

## 08.6 End-to-End Validation

**File(s):** No edits — smoke-test invocation only

This subsection runs per-script smoke tests. The full invocation matrix lives in §08.7; the atomic commit lives in §08.8. Keeping validation, matrix, and commit in separate subsections lets us rollback cleanly if a late check fails (per §08.N rollback strategy).

- [ ] Verify the wrapper-forwarding works end-to-end for each hot-path script:
  ```bash
  cd /home/eric/projects/ori_lang
  # test-all.sh via wrapper
  timeout 150 ./test-all.sh 2>&1 | tail -15
  # Expected: exit 0, all phases PASS
  ```

- [ ] Verify `./test-all.sh --json` writes to the correct location (should be target/test-all-results.json per §02 fix):
  ```bash
  ls target/test-all-results.json && echo "JSON artifact created"
  ```

- [ ] Verify full-check wrapper → internal chain works:
  ```bash
  timeout 150 ./full-check.sh 2>&1 | tail -15
  # Expected: calls clippy-all (now scripts/dev/) then test-all (wrapper → scripts/dev/)
  # Exit 0
  ```

- [ ] Verify fmt wrapper:
  ```bash
  timeout 60 ./fmt-all.sh 2>&1 | tail -10
  # Expected: exit 0 or fmt diff output
  ```

- [ ] Verify build wrapper:
  ```bash
  timeout 150 ./build-all.sh 2>&1 | tail -10
  # Expected: exit 0 (build succeeds)
  ```

- [ ] Verify cleanly-moved scripts work from `scripts/dev/`:
  ```bash
  timeout 60 ./scripts/dev/clippy-all.sh 2>&1 | tail -10
  ```

- [ ] Verify diagnostic scripts work from `scripts/diagnostics/`:
  ```bash
  bash scripts/diagnostics/self-test.sh 2>&1 | tail -10
  ```

- [ ] **Verify lefthook pre-commit hook passes**:
  ```bash
  # Simulate a pre-commit run (requires lefthook installed)
  lefthook run pre-commit 2>&1 | tail -20 || echo "lefthook not installed locally — skip"
  ```
  - [ ] If lefthook is installed: exit 0, all hooks pass
  - [ ] If not installed locally: skip (will be validated by the commit itself — lefthook runs as the pre-commit hook on `git commit`)

- [ ] **Subsection close-out (08.6)** — MANDATORY before starting 08.7:
  - [ ] All 7 smoke tests passed
  - [ ] No commit yet — that happens in §08.8
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        end-to-end validation sequence (test-all, full-check, fmt, build,
        cleanly-moved, diagnostics, lefthook) was 7 separate verification
        commands. §09 re-runs similar commands; no separate helper
        warranted here. Document: "Retrospective 08.6: verification
        overlaps with §09; no separate tooling warranted."

---

## 08.7 Invocation Matrix Verification (script × context)

**File(s):** No edits — matrix verification only

**Phase 2 finding (MAJOR, 2026-04-11):** Per `.claude/rules/tests.md` §Matrix Testing Rule, file-reorg sections must declare their test matrix dimensions. For §08, the matrix is `script × invocation-context`. The prior version of §08.6 tested each script once in the "direct-root" context but did not exercise the wrapper under alternate invocation paths (symlink, cross-directory cwd, inner-chain re-entry, test-code reference, lefthook context). Missing matrix cells are future regressions.

### Matrix dimensions

**Dimension 1 — scripts (8 internal dev scripts):**
| Script | Type | Home |
|--------|------|------|
| `test-all.sh` | Wrapper + impl | root + scripts/dev/ |
| `full-check.sh` | Wrapper + impl | root + scripts/dev/ |
| `fmt-all.sh` | Wrapper + impl | root + scripts/dev/ |
| `build-all.sh` | Wrapper + impl | root + scripts/dev/ |
| `clippy-all.sh` | Cleanly-moved | scripts/dev/ |
| `llvm-build.sh` | Cleanly-moved | scripts/dev/ |
| `llvm-clippy.sh` | Cleanly-moved | scripts/dev/ |
| `llvm-test.sh` | Cleanly-moved | scripts/dev/ |

**Dimension 2 — invocation contexts:**
| ID | Context | Tests |
|----|---------|-------|
| C1 | direct-root (`./<script>.sh`) | Wrapper exec forwarding from root cwd |
| C2 | direct-scripts-dev (`./scripts/dev/<script>.sh`) | Direct invocation of real impl |
| C3 | cross-dir (`cd compiler && ../<script>.sh`) | PWD assumption (scripts that assume cwd = repo root) |
| C4 | symlink (`ln -s $PWD/test-all.sh /tmp/t && /tmp/t`) | Symlink-safety of wrapper dirname resolution |
| C5 | lefthook-context (`lefthook run pre-commit`) | Wrappers invoked from git hook runner |
| C6 | internal-chain (`full-check` → `clippy-all` + `test-all` via `$SCRIPT_DIR`) | Inner-script sibling discovery |
| C7 | test-code error-message string literal (`compiler/ori_llvm/tests/aot/util/aot.rs:168`) | Pattern-D sweep correctness in a Rust string literal (NOT a runtime invocation — the string is part of a panic message suggesting `./llvm-test.sh` to the user) |
| C8 | negative-pin (deliberate test failure — wrapper must propagate non-zero exit) | Wrapper exit-code transparency — applies to wrapper scripts ONLY (the test is exercising the `exec` forwarding of the thin wrapper, not anything specific to cleanly-moved scripts) |
| C9 | PATH invocation (bare-name from outside repo root) | UNSUPPORTED — documented in §08.3 wrapper comment |

**Cells that apply per script type (TPR iteration 2 fix — reconciled against actual implementation):**
- **Wrapper scripts (test-all, full-check, fmt-all, build-all)**: C1, C2, C3, C4, C5 (lefthook only runs fmt-all + full-check), C8 (1 cell on test-all wrapper, exercises wrapper exit-code forwarding — the wrapper template is identical across all 4 so 1 test is load-bearing for all). C6 for **full-check only**. C9 SKIP (all 4).
- **Cleanly-moved scripts (clippy-all, llvm-*)**: C1-negative (MUST NOT exist at root), C2 (direct invocation). C3 does NOT apply (these scripts are not invoked via wrapper forwarding; any cwd assumption they have is not a wrapper concern). C4/C5 don't apply. C6 applies to clippy-all only (called by `full-check` via `$SCRIPT_DIR`). C7 applies to `llvm-test.sh` specifically (Pattern D target for the aot.rs:168 string literal). C8 does NOT apply (no wrapper to test). C9 does NOT apply (no wrapper).

### Matrix cells and expected outcomes

- [ ] **C1 — direct-root wrapper forwarding (4 cells)**
  ```bash
  cd /home/eric/projects/ori_lang
  timeout 150 ./test-all.sh 2>&1 | tail -5; echo "test-all exit: $?"
  timeout 150 ./full-check.sh 2>&1 | tail -5; echo "full-check exit: $?"
  timeout 60  ./fmt-all.sh 2>&1 | tail -5; echo "fmt-all exit: $?"
  timeout 150 ./build-all.sh 2>&1 | tail -5; echo "build-all exit: $?"
  ```
  - [ ] All 4 exit 0

- [ ] **C1-negative — cleanly-moved scripts MUST NOT exist at root (4 negative-pin cells)**
  ```bash
  test -f clippy-all.sh && echo "FAIL: clippy-all.sh still at root" || echo "OK: absent"
  test -f llvm-build.sh && echo "FAIL: llvm-build.sh still at root" || echo "OK: absent"
  test -f llvm-clippy.sh && echo "FAIL: llvm-clippy.sh still at root" || echo "OK: absent"
  test -f llvm-test.sh && echo "FAIL: llvm-test.sh still at root" || echo "OK: absent"
  ```
  - [ ] All 4 print "OK: absent"

- [ ] **C2 — direct invocation of real impl at scripts/dev/ (8 cells)**
  ```bash
  timeout 150 ./scripts/dev/test-all.sh 2>&1 | tail -5; echo "exit: $?"
  timeout 150 ./scripts/dev/full-check.sh 2>&1 | tail -5; echo "exit: $?"
  timeout 60  ./scripts/dev/fmt-all.sh 2>&1 | tail -5; echo "exit: $?"
  timeout 150 ./scripts/dev/build-all.sh 2>&1 | tail -5; echo "exit: $?"
  timeout 60  ./scripts/dev/clippy-all.sh 2>&1 | tail -5; echo "exit: $?"
  timeout 30  ./scripts/dev/llvm-build.sh 2>&1 | tail -5; echo "exit: $?"
  timeout 30  ./scripts/dev/llvm-clippy.sh 2>&1 | tail -5; echo "exit: $?"
  timeout 150 ./scripts/dev/llvm-test.sh 2>&1 | tail -5; echo "exit: $?"
  ```
  - [ ] All 8 exit 0 (or the script's expected non-blocking exit code)

- [ ] **C3 — cross-directory invocation (PWD assumption test, 4 cells — wrappers only)**

  Wrappers assume cwd = repo root (test-all.sh writes `target/test-all-results.json` as a relative path, etc.). Test invocation from a subdirectory for ALL 4 wrappers (TPR-XX-003-codex iteration 2 fix — previous version only exercised test-all and fmt-all):
  ```bash
  cd /home/eric/projects/ori_lang/compiler
  timeout 150 ../test-all.sh 2>&1 | tail -10;    echo "test-all exit: $?"
  timeout 150 ../full-check.sh 2>&1 | tail -10;  echo "full-check exit: $?"
  timeout 30  ../fmt-all.sh 2>&1 | tail -10;     echo "fmt-all exit: $?"
  timeout 150 ../build-all.sh 2>&1 | tail -10;   echo "build-all exit: $?"
  cd /home/eric/projects/ori_lang
  ```
  - [ ] **Expected behavior decision:** wrappers invoked from non-repo-root cwd may FAIL (e.g., writing JSON to `compiler/target/test-all-results.json` instead of repo-root target/). Per Finding 8, this is an **intentional limitation** — wrappers are documented as requiring cwd=repo-root. If the test fails with a path resolution error from a subdirectory, that is EXPECTED and does not fail §08.
  - [ ] Document the actual behavior observed — pass/fail — and capture as a known limitation.
  - [ ] Option: if the invocation from compiler/ fails, this is the documented limitation (Finding 8 chose documentation over anchoring). Proceed.
  - [ ] Option: if a future plan decides to support cross-dir invocation, the fix is to add `cd "$(git rev-parse --show-toplevel)"` to the wrapper template — **but not in this section**; file as `/add-bug` for follow-up if the limitation is judged too painful to live with.

- [ ] **C4 — symlink invocation (wrapper symlink-safety, 4 cells)**

  The wrapper template (per §08.3) uses `readlink -f "$0"` to resolve through symlinks before computing `dirname`. This means `ln -s $PWD/test-all.sh /tmp/t && /tmp/t` works correctly because `readlink -f` returns the real path in the repo. Test:
  ```bash
  cd /home/eric/projects/ori_lang
  ln -sf "$PWD/test-all.sh" /tmp/ori-test-all-symlink
  timeout 150 /tmp/ori-test-all-symlink 2>&1 | tail -10; echo "test-all symlink exit: $?"
  rm /tmp/ori-test-all-symlink
  
  ln -sf "$PWD/full-check.sh" /tmp/ori-full-check-symlink
  timeout 150 /tmp/ori-full-check-symlink 2>&1 | tail -10; echo "full-check symlink exit: $?"
  rm /tmp/ori-full-check-symlink
  
  ln -sf "$PWD/fmt-all.sh" /tmp/ori-fmt-all-symlink
  timeout 60 /tmp/ori-fmt-all-symlink 2>&1 | tail -10; echo "fmt-all symlink exit: $?"
  rm /tmp/ori-fmt-all-symlink
  
  ln -sf "$PWD/build-all.sh" /tmp/ori-build-all-symlink
  timeout 150 /tmp/ori-build-all-symlink 2>&1 | tail -10; echo "build-all symlink exit: $?"
  rm /tmp/ori-build-all-symlink
  ```
  - [ ] All 4 symlink invocations exit 0 (the wrapper's `readlink -f` makes symlink invocation transparent)
  - [ ] **If any C4 cell FAILS:** the wrapper template lost its `readlink -f` resolution somewhere in §08.3 — STOP, inspect the 4 wrapper files, verify they contain `SCRIPT_PATH="$(readlink -f "$0")"` and `exec "$(dirname "$SCRIPT_PATH")/scripts/dev/<script>.sh"`. Fix via `Edit` and re-run C4.
  - [ ] **Platform note:** `readlink -f` is GNU-specific. On macOS with BSD `readlink`, the wrapper would fall back to the non-symlink-resolving form and C4 would fail. The Execution Prerequisites in `00-overview.md` require GNU userland — this cell is one concrete reason that requirement exists.

- [ ] **C5 — lefthook pre-commit context (2 cells — fmt-all, full-check)**
  ```bash
  lefthook run pre-commit 2>&1 | tail -20; echo "lefthook exit: $?"
  ```
  - [ ] Exit 0 — both `./fmt-all.sh` and `./full-check.sh` are invoked by lefthook.yml and succeed
  - [ ] If lefthook not installed: skip with note (will run at actual commit time anyway)

- [ ] **C6 — internal-chain resolution (1 cell — full-check → clippy-all + test-all via $SCRIPT_DIR)**

  Already covered by §08.6 full-check smoke test, but verify explicitly:
  ```bash
  # full-check.sh at scripts/dev/full-check.sh calls:
  #   $SCRIPT_DIR/clippy-all.sh  (sibling — cleanly-moved, in scripts/dev/)
  #   $SCRIPT_DIR/test-all.sh    (sibling — wrapper target, in scripts/dev/)
  # Both must resolve when invoked via the root wrapper.
  timeout 150 ./full-check.sh 2>&1 | grep -E '(clippy-all|test-all)' | head -5
  echo "exit: $?"
  ```
  - [ ] Output shows both chain steps executing
  - [ ] Exit 0

- [ ] **C7 — test-code error-message string literal (1 cell — compiler/ori_llvm/tests/aot/util/aot.rs:168)**

  **TPR iteration 2 correction**: The earlier claim that C7 exercises "live test-code invocation" was WRONG. `compiler/ori_llvm/tests/aot/util/aot.rs:168` is inside a panic message string literal that says (pre-sweep): `"or use `./llvm-test.sh` which builds automatically."` — a user-facing error hint, NOT a runtime `std::process::Command` invocation. The test code doesn't actually execute the shell script; it only references the name in help text.

  C7 is therefore a **Pattern-D sweep-correctness** check, not a runtime invocation test. Verify the Rust file's string literal was correctly rewritten by Pattern D:
  ```bash
  grep -n '\./llvm-test\.sh\|scripts/dev/llvm-test\.sh' compiler/ori_llvm/tests/aot/util/aot.rs
  # Expected POST-sweep: single hit showing "./scripts/dev/llvm-test.sh" in the error message string
  # NOT expected: "./llvm-test.sh" (the pre-sweep form) — if found, Pattern D missed it
  ```
  - [ ] Single post-sweep reference present in the panic message
  - [ ] No pre-sweep `./llvm-test.sh` form remains in the file
  - [ ] The Rust file still parses (compiler hasn't broken syntax):
    ```bash
    cargo check -p ori_llvm 2>&1 | tail -5
    ```
  - [ ] Exit 0 from `cargo check`

- [ ] **C8 — negative-pin (wrapper exit-code propagation, 1 cell)**

  Introduce a deliberate failure and verify the wrapper propagates the non-zero exit:
  ```bash
  cd /home/eric/projects/ori_lang
  # Create a temporary failing test at the very tail of a test run by corrupting
  # a test input (reversible via git checkout). The safest way is to have
  # scripts/dev/test-all.sh exit non-zero via an explicit injection:
  
  # Option A (safer): directly run a failing inner command via the wrapper
  # Use `false` as a sentinel — shells exec forwarding is tested by any failing exit.
  # Create a temporary wrapper that execs `false` and verify exit propagation:
  cat > /tmp/ori-negative-wrapper.sh <<'EOF'
  #!/usr/bin/env bash
  exec "$(dirname "$0")/scripts/dev/non-existent-script.sh" "$@"
  EOF
  chmod +x /tmp/ori-negative-wrapper.sh
  /tmp/ori-negative-wrapper.sh
  actual_exit=$?
  test "$actual_exit" != 0 && echo "OK: wrapper propagated non-zero exit ($actual_exit)" \
    || echo "FAIL: wrapper did NOT propagate non-zero exit"
  rm /tmp/ori-negative-wrapper.sh
  ```
  - [ ] Output shows "OK: wrapper propagated non-zero exit"
  - [ ] (Optional, if §07 / §06 work allows time) also exercise the real wrapper pattern with a known-failing test injected into `scripts/dev/test-all.sh` — reversible via git checkout

- [ ] **C9 — PATH invocation (4 cells, documented SKIP per TPR-XX-002-codex fix, 2026-04-11)**

  Verify the wrapper's behavior under PATH invocation (user adds the repo root to PATH and runs `test-all.sh` without `./` from any cwd). Per the §08.3 wrapper documentation, this invocation pattern is **UNSUPPORTED** because `readlink -f "$0"` treats a bare name as cwd-relative.

  ```bash
  cd /home/eric/projects/ori_lang
  # Simulate PATH invocation from a subdirectory
  PATH="$PWD:$PATH" bash -c 'cd /tmp && test-all.sh --help 2>&1 | head -5'
  # Expected: wrapper fails to find scripts/dev/test-all.sh (because readlink -f "test-all.sh"
  # from /tmp resolves to /tmp/test-all.sh which doesn't exist, and the exec target
  # becomes /tmp/scripts/dev/test-all.sh which also doesn't exist).
  # This cell MUST be marked SKIP (unsupported) rather than FAIL.
  ```
  - [ ] Behavior confirmed: wrapper fails when invoked via PATH from outside the repo
  - [ ] Mark all 4 C9 cells (test-all, full-check, fmt-all, build-all via PATH) as **SKIP — unsupported** per §08.3 wrapper documentation
  - [ ] Document in the retrospective: "C9 PATH invocation is an explicit non-goal of Option C. Aliases to `./test-all.sh` from the repo root work fine; developers who want PATH-style invocation should use an alias like `alias test-all='./test-all.sh'` rather than putting the repo root on PATH."

- [ ] **Matrix completeness check:** total cells = 4 (C1 wrapper exec) + 4 (C1-negative cleanly-moved absent) + 8 (C2 direct scripts/dev) + 4 (C3 cross-dir wrappers — ALL 4 wrappers now per TPR-XX-003-codex iteration 2 fix) + 4 (C4 symlink wrappers) + 2 (C5 lefthook — fmt-all + full-check) + 1 (C6 full-check internal chain) + 1 (C7 pattern-check, not invocation, per TPR iteration 2 reclassification) + 1 (C8 wrapper exit-code — 1 representative test, not per-script) + 4 (C9 PATH SKIP unsupported) = **33 cells**.
  ```bash
  echo "Matrix cells enumerated: 33 (29 exercisable + 4 SKIP)"
  echo "Breakdown: C1=4, C1-neg=4, C2=8, C3=4, C4=4, C5=2, C6=1, C7=1, C8=1, C9=4 (SKIP)"
  echo "Any cell SKIPPED must be documented in the retrospective with reason"
  ```
  - [ ] Document every SKIPPED cell. Acceptable skips: C5 partial when lefthook not locally installed (will run at commit time), C9 PATH invocation (all 4 cells — unsupported per TPR-XX-002-codex fix). C7 is a pattern-check (not invocation) and is always exercisable via grep + cargo check.
  - [ ] Any FAILED cell that is NOT a documented limitation (C3 per Finding 8 on cross-dir PWD assumption, C9 per TPR-XX-002-codex on PATH unsupported) blocks §08.8 commit.

- [ ] **Subsection close-out (08.7)** — MANDATORY before starting 08.8:
  - [ ] All applicable matrix cells either PASS or are documented SKIPS or are documented limitations (C3)
  - [ ] C4 symlink result captured; if failed, wrapper template updated + re-tested OR bug filed
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        matrix had **33 cells** total (29 exercisable + 4 PATH SKIP). A
        `scripts/dev/verify-invocation-matrix.sh` helper that runs all 33
        cells (including validating the 4 C9 SKIP cells fail-closed with
        the correct unsupported-PATH-invocation error rather than
        accidentally succeeding) and reports pass/fail/skip per cell
        would be broadly useful for every future script-layout reorg.
        Build if time permits; otherwise file as `/add-bug` for follow-up.
        Document outcome.

---

## 08.8 Atomic Commit

**File(s):** No edits — commit only

The atomic commit lands here, AFTER the full validation (§08.6) and matrix verification (§08.7) have passed. Splitting validation from commit gives us a clean rollback point: if §08.8 encounters a last-minute issue (e.g., pre-commit hook catches a new problem), we can rollback via §08.N's rollback strategy without losing §08.6/§08.7 work.

- [ ] **Pre-commit worktree cleanliness gate (Phase 2 LEAK finding, 2026-04-11):** `git add -A` is banned for this commit — the §08 sweep is broad enough (230+ files) that an indiscriminate stage would absorb any concurrent edit and bury it in the reorganization commit. Before staging, verify the working tree contains ONLY §08's expected edits:
  ```bash
  cd /home/eric/projects/ori_lang
  
  # Expected dirty paths for §08:
  # - 8 renamed scripts (R  clippy-all.sh -> scripts/dev/clippy-all.sh, etc.)
  # - 4 new root wrappers (A  test-all.sh, full-check.sh, fmt-all.sh, build-all.sh)
  # - 16+ moved diagnostic scripts (R  diagnostics/*.sh -> scripts/diagnostics/*.sh)
  # - 1 new diagnostic helper (A  scripts/diagnostics/_repo-root.sh)
  # - 7 edited diagnostic scripts (M  scripts/diagnostics/_common.sh, check-debug-flags.sh, etc.)
  # - 1 edited compiler source comment (M  compiler/oric/src/llvm_dump/mod.rs)
  # - Pattern A/D/E sweep edits: CLAUDE.md, CONTRIBUTING.md, .claude/**, .codex/**,
  #   plans/**, docs/**, .claude/rules/diagnostic.md, lefthook.yml, scripts/release.sh
  # - compiler/ori_llvm/tests/aot/util/aot.rs (Pattern D target)
  # - plans/project-reorganization/section-08-script-consolidation.md (plan sync)
  # - plans/project-reorganization/00-overview.md (status updates)
  # - plans/project-reorganization/index.md (status updates)
  
  git status --porcelain > /tmp/section-08-git-status.txt
  wc -l /tmp/section-08-git-status.txt
  # Expected: ~240+ lines (huge sweep)
  
  # Negative check — ensure NO files outside the expected scope are dirty.
  # The allow-list for §08 is very broad, so the check is: no unexpected
  # compiler/ changes beyond llvm_dump/mod.rs, no library/ changes, no tests/
  # changes beyond compiler/ori_llvm/tests/aot/util/aot.rs, no .git/ or target/.
  git status --porcelain \
    | grep -E '^.. compiler/' \
    | grep -v ' compiler/oric/src/llvm_dump/mod\.rs$' \
    | grep -v ' compiler/ori_llvm/tests/aot/util/aot\.rs$' \
    || echo "CLEAN — compiler/ changes are scoped to the expected 2 files"
  
  git status --porcelain | grep -E '^.. library/' \
    || echo "CLEAN — library/ untouched"
  
  git status --porcelain | grep -E '^.. tests/' \
    | grep -v ' tests/.*/(cow/)?(baseline\.json|valgrind|spec)' \
    || echo "CLEAN — tests/ untouched (or only pre-existing baselines)"
  ```
  - [ ] All three checks produce "CLEAN — ..." messages
  - [ ] If any unexpected file: STOP, investigate, decide whether it's (a) a legitimate sweep target missed in enumeration (update allow-list) or (b) a concurrent edit (shelve before committing §08)

- [ ] **Commit everything in §08 atomically with EXPLICIT per-file path staging (no `git add -A`, no `git add <directory>/`).**

  **Why explicit per-file staging (TPR-XX-001-codex + TPR-XX-002-gemini fix, 2026-04-11):** Both directory-wide adds (`git add .claude/`) and awk-based glob adds absorb unrelated concurrent work. The correct allow-list is the set of files §08.5's sweep patterns ACTUALLY touched — which we already captured in `target/sweep-pattern-a-files.txt`, `target/sweep-pattern-d-files.txt`, and `target/sweep-pattern-e-files.txt` during Step E.2/E.3. Reuse those files as the staging source of truth. Any file NOT in those lists is either (a) unchanged, or (b) concurrent dirt that must NOT be absorbed into the §08 commit.

  ```bash
  cd /home/eric/projects/ori_lang

  # 1. Script moves — already staged by git mv in 08.2, 08.3, 08.4 (explicit, no globs)
  git add -u clippy-all.sh llvm-build.sh llvm-clippy.sh llvm-test.sh \
              test-all.sh full-check.sh fmt-all.sh build-all.sh 2>/dev/null || true

  # 2. New content — scripts/dev/ (wrapper targets + cleanly-moved files)
  git add scripts/dev/clippy-all.sh scripts/dev/llvm-build.sh \
          scripts/dev/llvm-clippy.sh scripts/dev/llvm-test.sh \
          scripts/dev/test-all.sh scripts/dev/full-check.sh \
          scripts/dev/fmt-all.sh scripts/dev/build-all.sh

  # 3. New root wrappers (from 08.3)
  git add test-all.sh full-check.sh fmt-all.sh build-all.sh

  # 4. Diagnostic moves (from 08.4 — explicit per-file, no glob, no directory-wide add).
  # TPR-XX-001-codex iteration 2 fix — fixtures/ sub-adds were the last remaining
  # directory-level stage. Generate an explicit fixture file list from the pre-move
  # state and stage per-file.
  git add -u diagnostics/_common.sh diagnostics/arc-dump.sh diagnostics/bisect-passes.sh \
              diagnostics/check-debug-flags.sh diagnostics/codegen-audit.sh \
              diagnostics/debug-release-compare.sh diagnostics/diagnose-aot.sh \
              diagnostics/disasm-ori.sh diagnostics/dual-exec-debug.sh \
              diagnostics/dual-exec-verify.sh diagnostics/ir-diff.sh diagnostics/ir-dump.sh \
              diagnostics/rc-stats.sh diagnostics/self-test.sh diagnostics/valgrind-aot.sh \
              diagnostics/README.md 2>/dev/null || true
  # fixtures: enumerate each file explicitly (no git add -u diagnostics/fixtures/)
  # Generate the list ONCE at §08.1 via `find diagnostics/fixtures -type f > target/sweep-pattern-fixtures-files.txt`
  if [ -s target/sweep-pattern-fixtures-files.txt ]; then
    xargs -a target/sweep-pattern-fixtures-files.txt git add -u -- 2>/dev/null || true
  fi
  # Staged new locations (post-move) — explicit per-file
  git add scripts/diagnostics/_common.sh scripts/diagnostics/arc-dump.sh \
          scripts/diagnostics/bisect-passes.sh scripts/diagnostics/check-debug-flags.sh \
          scripts/diagnostics/codegen-audit.sh scripts/diagnostics/debug-release-compare.sh \
          scripts/diagnostics/diagnose-aot.sh scripts/diagnostics/disasm-ori.sh \
          scripts/diagnostics/dual-exec-debug.sh scripts/diagnostics/dual-exec-verify.sh \
          scripts/diagnostics/ir-diff.sh scripts/diagnostics/ir-dump.sh \
          scripts/diagnostics/rc-stats.sh scripts/diagnostics/self-test.sh \
          scripts/diagnostics/valgrind-aot.sh scripts/diagnostics/README.md \
          scripts/diagnostics/_repo-root.sh
  # fixtures new locations (post-move) — rewrite the fixture list paths to their
  # new scripts/diagnostics/ prefix and stage
  if [ -s target/sweep-pattern-fixtures-files.txt ]; then
    sed 's|^diagnostics/fixtures/|scripts/diagnostics/fixtures/|' target/sweep-pattern-fixtures-files.txt \
      | xargs git add --
  fi

  # 5. Pattern A/D/E sweep targets — stage ONLY files the sweep actually touched,
  #    using the per-pattern file lists generated in §08.5 as the authoritative source.
  #    This is the LEAK fix: no glob adds, no directory adds, no awk scans.
  if [ -s target/sweep-pattern-a-files.txt ]; then
    xargs -a target/sweep-pattern-a-files.txt git add --
  fi
  if [ -s target/sweep-pattern-d-files.txt ]; then
    xargs -a target/sweep-pattern-d-files.txt git add --
  fi
  if [ -s target/sweep-pattern-e-files.txt ]; then
    xargs -a target/sweep-pattern-e-files.txt git add --
  fi
  if [ -s target/sweep-pattern-e9-files.txt ]; then
    xargs -a target/sweep-pattern-e9-files.txt git add --
  fi

  # 6. Step E.4 manual Rust comment edit (not captured by any pattern file)
  git add compiler/oric/src/llvm_dump/mod.rs

  # 7. Plan sync edits — explicit per-file
  git add plans/project-reorganization/00-overview.md \
          plans/project-reorganization/index.md \
          plans/project-reorganization/section-08-script-consolidation.md

  # Sanity check — show staged count
  git status --short | wc -l
  git status --short | head -40
  ```
  - [ ] Staged count matches expected scope (~240+ files from the sweep, plus scripts/moves/wrappers/helper + plan sync)

- [ ] **Post-stage catch-all gate (TPR-XX-002-gemini fix, 2026-04-11).** Verify NO unstaged changes remain — if any appear, they're either a missed sweep target (must be added to staging) or concurrent dirt (must be shelved before commit). This guards the gap where Pattern A/D could touch a file in `.github/`, `docker/`, `editors/`, or other directory NOT anticipated by explicit staging:
  ```bash
  unstaged=$(git diff --name-only)
  if [ -n "$unstaged" ]; then
    echo "FAIL: unstaged changes remain after explicit staging:"
    echo "$unstaged"
    echo ""
    echo "Classify each:"
    echo "  - If it's a missed sweep target (rg pattern hit but not in target/sweep-pattern-*.txt):"
    echo "    → add to the corresponding pattern file and re-stage"
    echo "  - If it's concurrent dirt (unrelated to §08 work):"
    echo "    → git stash or git restore to defer it before committing §08"
    exit 1
  else
    echo "OK: catch-all gate PASS — zero unstaged changes remain"
  fi
  ```
  - [ ] Catch-all gate prints "OK: catch-all gate PASS"
  - [ ] If FAIL: do NOT proceed to commit. Resolve the unstaged changes first per the classify protocol above.

- [ ] **Commit the atomic §08 change:**
  ```bash
  
  git commit -m "refactor(plan): consolidate dev scripts into scripts/dev/ + atomic reference sweep

Option C hybrid script model (per 00-overview.md Design Principle 1):

Cleanly moved (4 scripts, root → scripts/dev/):
- clippy-all.sh, llvm-build.sh, llvm-clippy.sh, llvm-test.sh

Hot-path wrapped (4 scripts, real impl in scripts/dev/ + 3-line wrapper at root):
- test-all.sh, full-check.sh, fmt-all.sh, build-all.sh
  (wrappers are PERMANENT — they preserve muscle memory for daily-use
  scripts while consolidating implementation. They are NOT temporary shims.)

Diagnostics moved (diagnostics/ → scripts/diagnostics/):
- 16 debug scripts + fixtures/ + README.md

Preserved at root (unchanged):
- install.sh (PUBLIC_API — codex-verified external surface)
- setup.sh (semi-public, CONTRIBUTING.md:14)

Atomic reference sweep across the blast radius:
- Pattern A: 175 refs to ./clippy-all.sh → ./scripts/dev/clippy-all.sh
- Pattern D: 30 refs to ./llvm-test.sh → ./scripts/dev/llvm-test.sh
- Pattern E: ~30 refs to diagnostics/ → scripts/diagnostics/
- Hot-path 4 refs (503+7+29+2=541) unchanged — wrappers preserve the paths

Files touched: CLAUDE.md, CONTRIBUTING.md, all .claude/rules/*.md,
all .claude/skills/*/SKILL.md, .claude/commands/*.md, .codex/skills/,
plans/** (597+ refs), docs/compiler/design/appendices/,
compiler/ori_llvm/tests/aot/util/aot.rs, scripts/release.sh,
.claude/rules/diagnostic.md (§Diagnostic Scripts table + header).

Verified: ./test-all.sh green (via wrapper), lefthook pre-commit passes,
full-check chain (wrapper → scripts/dev/full-check → \$SCRIPT_DIR/clippy-all
+ test-all) resolves correctly.

Refs: plans/project-reorganization/section-08-script-consolidation.md"
  ```

- [ ] **Verify commit landed**:
  ```bash
  git log --oneline -1
  git show --stat HEAD | head -60
  ```
  - [ ] ~240+ file changes in one commit (8 script moves + 4 new wrappers + ~225 sweep edits)

- [ ] **Final sanity check — everything still works**:
  ```bash
  timeout 150 ./test-all.sh 2>&1 | tail -10
  ```
  - [ ] Exit 0, all phases PASS

- [ ] **Subsection close-out (08.8)** — MANDATORY before 08.R:
  - [ ] Atomic commit landed with ~240+ file changes
  - [ ] Post-commit test-all.sh green
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        explicit-path staging pattern was mechanical but error-prone. A
        `scripts/dev/stage-section-paths.sh <section-file>` helper that
        parses the section's "File(s):" frontmatter and stages only those
        paths would be broadly useful for every future large-atomic-commit
        section. Build if time permits; otherwise file as `/add-bug`.
        Document: "Retrospective 08.8: explicit path staging is mandatory
        per LEAK finding — helper would automate it; filed for follow-up."

---

## 08.R Third Party Review Findings

<!-- Dual-source /tpr-review findings (iteration 1, 2026-04-11). Both reviewers read the plan AND the live source tree. All findings verified against live source before fix. Five actionable findings (2 HIGH, 3 medium) were surfaced and fixed. -->

- [x] `[TPR-XX-001-codex][high]` `plans/project-reorganization/section-08-script-consolidation.md:1341` — Remove the LEAK in atomic staging scope.
  Evidence: The cleanliness gate at lines 1276-1316 only excluded unexpected changes under `compiler/`, `library/`, `tests/`, but the staging block at lines 1341-1356 used `git add .claude/` + `git add .codex/` + `git add .gemini/` (directory-wide adds) plus awk-based globs for plans/ and docs/. Live `git status --short` showed unrelated dirty files at `.claude/skills/continue-roadmap/SKILL.md`, `plans/llvm-verification-tooling/section-03-aims-snapshots.md`, `plans/test-suite-health/section-03-profiling-infrastructure.md` — all would have been absorbed into the §08 atomic commit.
  Impact: The "atomic" commit would not actually be atomic — it could pull in concurrent contributor work, recreating the LEAK that Phase 3 was trying to eliminate.
  Required plan update: Narrow staging to an explicit per-file allow-list derived from the paths Pattern A/D/E actually touched. Use `target/sweep-pattern-{a,d,e,e9}-files.txt` as the staging source of truth.
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11. Rewrote §08.8 staging to use `xargs -a target/sweep-pattern-a-files.txt git add --` (and similarly for Pattern D/E/E9). Removed all `git add .claude/` / `git add .codex/` / `git add .gemini/` directory adds. Removed the awk-based globs for plans/ and docs/. Every staged path is now an explicit filename derived from §08.5's own sweep file lists.

- [x] `[TPR-XX-002-gemini][high]` `plans/project-reorganization/section-08-script-consolidation.md:1318` — Stage unbounded Pattern A and D sweep targets in atomic commit.
  Evidence: Pattern A and D run globally via `rg -l`. If they modify files in `.github/`, `docker/`, `editors/`, or any other directory outside the §08.8 explicit staging list, those files remain unstaged. The negative check only guarded `compiler/`, `library/`, and `tests/`, silently leaving unstaged changes in other scopes.
  Impact: Unstaged sweep changes would be excluded from the atomic commit, breaking both the atomicity and the post-commit worktree cleanliness guarantee.
  Required plan update: Add a post-stage catch-all gate (`git diff --name-only | grep . && echo 'FAIL: unstaged changes remain'`).
  Basis: inference. Confidence: high.
  Resolved: Fixed on 2026-04-11. Added a post-stage catch-all gate to §08.8 that runs `git diff --name-only` after explicit staging and fails if ANY unstaged changes remain. The gate prints a classification protocol: missed sweep target → add to pattern file and re-stage; concurrent dirt → `git stash` before commit. This also composes with the TPR-XX-001-codex fix: together they eliminate both sides of the LEAK (over-broad staging + missed staging).

- [x] `[TPR-XX-002-codex][medium]` `plans/project-reorganization/section-08-script-consolidation.md:224` — Close the GAP in wrapper PATH invocation.
  Evidence: The wrapper template at line 224 uses `SCRIPT_PATH="$(readlink -f "$0")"`. This resolves symlinks when `$0` is a real path, but does NOT resolve through PATH when `$0` is a bare name. Fresh probes showed `bash -c 'readlink -f "$0"' test-all.sh` from `compiler/` resolves to `/home/eric/projects/ori_lang/compiler/test-all.sh` (nonexistent). §08.7's matrix had no PATH cell.
  Impact: The wrapper recipe was not robust against PATH-based invocation (aliases, PATH-exported dev paths). Any PATH-style launch from outside the repo root would exec a nonexistent `scripts/dev/...` target.
  Required plan update: Either declare PATH invocation unsupported and document the limitation, or change the wrapper to resolve the executable via PATH before calling `readlink -f`. Add a PATH cell to §08.7.
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11. Added a block comment to §08.3 wrapper template explicitly declaring PATH invocation as UNSUPPORTED with the rationale (adding PATH resolution would add complexity for an edge case; `./test-all.sh` from repo root is the canonical invocation pattern). Added a new matrix cell §08.7 C9 for PATH invocation (4 cells: test-all, full-check, fmt-all, build-all) marked as **SKIP — unsupported** per the documented limitation, with a verification command that confirms the wrapper fails when invoked via PATH from outside the repo root. Matrix completeness updated to 33 cells (29 exercisable + 4 SKIP).

- [x] `[TPR-XX-001-gemini][high]` `plans/project-reorganization/section-08-script-consolidation.md:834` — Add scripts/ directory to Pattern E allow-list.
  Evidence: The explicit allow-list for Pattern E omitted `scripts/` entirely. If any existing script outside `diagnostics/` (e.g. `scripts/release.sh` or `scripts/perf-baseline.sh`) invoked a diagnostic script, the reference would not have been updated and the script would break after §08.4's move.
  Impact: Missed references in `scripts/` would cause those scripts to fail when attempting to invoke moved diagnostic scripts.
  Required plan update: Add `find scripts -maxdepth 1 -type f \( -name '*.sh' -o -name '*.py' \)` to the Pattern E allow-list generation in Step E.2.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-11. Added `scripts/` top-level `*.sh` and `*.py` files to the Pattern E allow-list (Step E.2 generation command), with EXCLUSION of `scripts/dev/` and `scripts/diagnostics/` (the former has no `diagnostics/` refs by construction; the latter is handled by the new Step E.9 for self-references). Added a defensive check that the allow-list does NOT leak `scripts/dev/` or `scripts/diagnostics/` paths. Current verification (2026-04-11) showed zero `diagnostics/` references in `scripts/*.sh` / `scripts/*.py`, but the defensive inclusion means any future addition auto-sweeps.

- [x] `[TPR-XX-004-codex][medium]` `plans/project-reorganization/section-08-script-consolidation.md:800` — Sweep the remaining DRIFT in diagnostics self-references.
  Evidence: Pattern E's allow-list explicitly excluded `scripts/diagnostics/` to avoid scope creep, so moved diagnostic scripts and `README.md` would never be swept. The current files contain many legitimate self-references that would go stale: `diagnostics/README.md:30-35, 43-58, 165-183`, `diagnostics/ir-dump.sh:5 and 86`, `diagnostics/self-test.sh:5 and 11`.
  Impact: After §08, the canonical diagnostics home would change to `scripts/diagnostics/` but the moved scripts and README would still instruct users to run `diagnostics/...`. Documentation and `--help` surfaces would be out of sync with the new canonical path.
  Required plan update: Add a separate post-move sweep scoped to `scripts/diagnostics/**`.
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11. Added Step E.9 (`Post-move sweep of scripts/diagnostics/ self-references`) to §08.5 with explicit scoping to `scripts/diagnostics/**`, the same regex patterns as Pattern E, a negative-pin (Step E.10) that verifies the E.9 sweep did NOT escape `scripts/diagnostics/`. The `target/sweep-pattern-e9-files.txt` output is used as an explicit staging source in §08.8's per-file add loop.

<!-- Iteration 2 (2026-04-11) surfaced 5 additional findings: 2 HIGH, 3 medium. 2 of them were follow-ons to iteration 1 fixes (the staging LEAK wasn't fully fixed; the sweep file path was fragile); 3 were new architectural issues iteration 1 didn't catch (scripts/ in Pattern E allow-list gap, double-quoted sed pattern gap, §08.7 matrix vs implementation mismatch). All 5 fixed inline. -->

- [x] `[TPR-XX-001-codex][high]` (iteration 2) `plans/project-reorganization/section-08-script-consolidation.md:1441,1451` — Remove the remaining LEAK in §08.8 directory staging.
  Evidence: Iteration 1's staging fix was incomplete. §08.8 still had `git add -u diagnostics/fixtures/` (line 1441) and `git add scripts/diagnostics/fixtures/` (line 1451) — two directory-level adds that iteration 1 missed because they handle the `fixtures/` subdirectory. Both violated the "no `git add <directory>/`" contract that iteration 1 established.
  Impact: The §08 atomic commit could still absorb concurrent or accidental changes from `diagnostics/fixtures/` during the execution window — the original LEAK risk survived through a narrower but still non-canonical staging path.
  Required plan update: Replace both directory adds with explicit per-file staging via a new `target/sweep-pattern-fixtures-files.txt` enumeration generated at §08.1.
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11 iteration 2. Updated §08.1 to generate `target/sweep-pattern-fixtures-files.txt` via `find diagnostics/fixtures -type f > target/sweep-pattern-fixtures-files.txt` as part of the directory-setup step. Updated §08.8 staging to read the list with `xargs -a ... git add -u --` (pre-move paths) AND a `sed` pass that rewrites `diagnostics/fixtures/` to `scripts/diagnostics/fixtures/` for the post-move add. Every fixture is now staged per-file.

- [x] `[TPR-XX-001-gemini][high]` (iteration 2) `plans/project-reorganization/section-08-script-consolidation.md:multiple` — Store sweep file lists in the repository instead of the temporary directory.
  Evidence: Step 08.5 wrote sweep targets to `/tmp/sweep-pattern-a-files.txt` etc. and Step 08.8 read them for git staging. Between those two steps, manual verification (08.6 and 08.7) happens. If the system reboots, `/tmp` is cleared, or another process cleans `/tmp` during manual verification, the sweep file lists are permanently lost — but the sed rewrites have already happened. Step 08.8 would then silently skip staging those files (empty xargs input), resulting in a broken half-applied state that's hard to recover from.
  Impact: Fragile cross-step state. A hardware reboot during manual verification turns a successful sweep into a permanently broken plan execution.
  Required plan update: Move all `/tmp/sweep-pattern-*.txt` files to a persistent repo-scoped path that survives reboot. `target/` is gitignored and persistent until `cargo clean` — the natural home.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-11 iteration 2. Used `sed -i 's|/tmp/sweep-pattern-|target/sweep-pattern-|g'` to replace all 37 references across §08.5, §08.8, and §08.9 from `/tmp/sweep-pattern-*.txt` to `target/sweep-pattern-*.txt`. Updated §08.1 to ensure `target/` exists before §08.5 writes to it (cargo build dir already exists in practice, but the explicit `mkdir -p target` makes the dependency load-bearing).

- [x] `[TPR-XX-002-gemini][medium]` (iteration 2) `plans/project-reorganization/section-08-script-consolidation.md:908` — Update Pattern E to match paths enclosed in double quotes.
  Evidence: Iteration 1's Pattern E Step E.5 had sed patterns for backtick (`` ` ``), `./`, space-prefix, and start-of-line boundaries. Missing: double-quoted path literals like `"diagnostics/arc-dump.sh"` which appear in JSON fixtures, code blocks with quoted path arguments, and schema examples. These would have been left untouched by Pattern E's sweep and drift from the canonical `scripts/diagnostics/` path.
  Impact: Documentation examples and JSON fixtures containing `"diagnostics/"` would drift silently from the canonical paths, causing confusion for developers reading TPR schema examples or test fixtures.
  Required plan update: Add a fifth sed pattern for double-quoted paths.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-11 iteration 2. Added `sed -i 's|"diagnostics/|"scripts/diagnostics/|g' "$file"` as the 5th sed pattern in Step E.5. Also added the same double-quoted pattern to Step E.9's scoped self-reference sweep (`scripts/diagnostics/**`) so JSON fixtures inside the moved scripts are handled consistently.

- [x] `[TPR-XX-003-codex][medium]` + `[TPR-XX-003-gemini][medium]` (iteration 2) `plans/project-reorganization/section-08-script-consolidation.md:1120,1220` — Close the GAP between §08.7 matrix claims and executable verification.
  Evidence (codex): §08.7 claimed C3 is a 4-cell wrapper matrix, but the commands only exercised `../test-all.sh` and `../fmt-all.sh` (lines 1225-1226), not `full-check.sh` or `build-all.sh`. It also defined C7 as "live test-code invocation" at `compiler/ori_llvm/tests/aot/util/aot.rs:168`, but the actual source at that line is a panic **message string literal** that mentions `./llvm-test.sh` as a help hint, NOT a runtime script invocation. The matrix verification was overstated and partially non-runnable.
  Evidence (gemini): The "Cells that apply per script type" rule stated C8 applies to both wrapper and cleanly-moved scripts (implying 8 cells total), and C3 applies to both. But the actual C3 implementation was "4 cells — wrappers only" and C8 implementation was "wrapper exit-code propagation, 1 cell" — the documentation claimed coverage the implementation didn't deliver.
  Impact: §08 could not actually prove the invocation surface it claimed to clamp. Executors would either skip the missing cells (undercount) or stall trying to execute nonsensical cells (cleanly-moved scripts have no wrapper to test exit-code forwarding through).
  Required plan update: Reconcile matrix documentation with implementation. Add missing C3 cells for full-check and build-all; reclassify C7 as a pattern-check (not invocation); fix the "applies to both" rule for C8 to say "wrappers only, 1 representative test".
  Basis: fresh_verification (codex) + direct_file_inspection (gemini). Confidence: high.
  Resolved: Fixed on 2026-04-11 iteration 2. (a) Added the 2 missing C3 cells for `../full-check.sh` and `../build-all.sh` — C3 now exercises all 4 wrappers from a subdirectory. (b) Reclassified C7 from "live test-code invocation" to "test-code error-message string literal" with updated verification (grep for Pattern D's post-sweep form + `cargo check` to verify syntax) — the Rust file at aot.rs:168 is a panic-message string, verified by reading the actual source. (c) Rewrote the "Cells that apply per script type" rule block to match implementation reality: C3 applies to wrappers only (C3 does NOT apply to cleanly-moved scripts); C8 applies to wrappers only and is 1 representative test (the thin wrapper template is identical across all 4, so one test is load-bearing for all). Updated the matrix completeness totals description with the full breakdown (C1=4, C1-neg=4, C2=8, C3=4, C4=4, C5=2, C6=1, C7=1, C8=1, C9=4 SKIP = 33 total).

<!-- Iteration 3 (2026-04-11) surfaced 1 metadata propagation miss: frontmatter success_criteria, C3 context row, and completion checklist still said "29 cells" or used the wrong `cd compiler && ../../<script>.sh` pattern. Fixed inline. -->

- [x] `[TPR-XX-004-codex][medium]` (iteration 3) `plans/project-reorganization/section-08-script-consolidation.md:27,1193,1690` — DRIFT: Propagate the 33-cell matrix contract through §08 metadata.
  Evidence: Iteration 2 fixed the executable matrix body to 33 cells at §08.7 ("33 total", "29 exercisable + 4 SKIP"), but the frontmatter success criterion at line 27 still said "all 29 cells", the C3 context row at line 1193 still showed `cd compiler && ../../<script>.sh` even though the actual commands use `../<script>.sh`, and the completion checklist at line 1690 still said "all 29 cells". The section contract, matrix legend, and close-out checklist no longer agreed with the implementation.
  Impact: Reviewers couldn't tell whether a 33-cell run was the intended end state or whether C3 was being exercised correctly.
  Required plan update: Update the frontmatter, the C3 context row, and §08.N to the final matrix shape: 33 total cells, 29 exercisable plus 4 PATH SKIPs, with C3 documented as `cd compiler && ../<script>.sh`.
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11 iteration 3. Updated frontmatter success_criteria line 27 to "all 33 cells (29 exercisable + 4 PATH SKIP)" with the C3 + C9 skip rationale. Updated C3 context row in the matrix legend from `cd compiler && ../../<script>.sh` to `cd compiler && ../<script>.sh` to match the actual §08.7 C3 implementation. Updated §08.N completion checklist line to match the 33-cell shape. All three metadata locations now agree with the §08.7 body.

---

## 08.N Completion Checklist

- [ ] 4 cleanly-moved scripts in `scripts/dev/` (08.2)
- [ ] 4 hot-path scripts in `scripts/dev/` with wrappers at root (08.3)
- [ ] `diagnostics/` → `scripts/diagnostics/` (08.4) with canonical `_repo-root.sh` helper + 9 repo-root resolution patches (Finding 1 fix)
- [ ] Reference sweep patterns A, B, C, D, E all applied (08.5) with explicit allow-list for Pattern E (Finding 2 fix)
- [ ] Hot-path refs preserved (wrappers work)
- [ ] Cleanly-moved refs updated (zero stale)
- [ ] `diagnostics/` path refs updated to `scripts/diagnostics/`
- [ ] `compiler/oric/src/llvm_dump/mod.rs:8` comment updated (manual edit, Pattern E Step E.4)
- [ ] CLAUDE.md §Commands section accurate
- [ ] CONTRIBUTING.md accurate
- [ ] `.claude/rules/diagnostic.md` §Diagnostic Scripts accurate
- [ ] compiler/ori_llvm/tests/aot/util/aot.rs updated if needed
- [ ] Invocation matrix (08.7) complete — all 33 cells (29 exercisable + 4 PATH SKIP) PASS, SKIP (documented), or fail as documented limitation (C3 PWD for cross-dir wrappers; C9 unsupported for PATH)
- [ ] `./test-all.sh` green post-sweep (via wrapper)
- [ ] `./scripts/dev/test-all.sh` green (direct)
- [ ] `./full-check.sh` green (wrapper → internal chain)
- [ ] `lefthook run pre-commit` green
- [ ] Single atomic commit lands: `refactor(plan): consolidate dev scripts into scripts/dev/ + atomic reference sweep`
- [ ] `git diff --name-only HEAD~1 HEAD | grep '^compiler/'` shows ONLY `compiler/oric/src/llvm_dump/mod.rs` and `compiler/ori_llvm/tests/aot/util/aot.rs` — NO other `compiler/` files touched by the sweep (Finding 2 negative-pin regression guard)
- [ ] Plan annotation cleanup: N/A (no `.rs` files modified beyond the two explicit edits)
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table for §08 → `Complete`
  - [ ] `00-overview.md` mission success criteria "Zero stale root references", "Lefthook pre-commit passes", "`./test-all.sh` green", "Hot-path 4 muscle memory preserved" all → checked
  - [ ] `index.md` updated
- [ ] `/tpr-review` passed — CRITICAL PATH section; review should check: (a) all wrappers exec correctly, (b) no sweep pattern corrupted unintended files (Pattern E allow-list held), (c) lefthook still works, (d) internal $SCRIPT_DIR chain resolves, (e) hot-path preservation works (no regression in daily-use workflows), (f) CI still passes (but CI doesn't use root scripts so this is guaranteed), (g) diagnostic scripts find repo root correctly from new location
- [ ] `/impl-hygiene-review` passed (AFTER TPR clean) — SSOT (each script has exactly one canonical implementation; `_repo-root.sh` is the single source for repo-root derivation), No Side Logic (wrappers are thin exec-forwarders with no logic), DRIFT (sweep caught everything)
- [ ] `/improve-tooling` **section-close sweep** — per-subsection retrospectives (08.1-08.8) should already be committed. Cross-subsection pattern check: the `atomic-sweep.sh` helper from 08.5 is the most valuable contribution. Verify it was built. Also check: did 08.3's wrapper template benefit from generating the 4 wrappers programmatically? Probably not — 4 is few enough to hand-write. The big-ticket tooling wins from §08 are: `_repo-root.sh` (canonical repo-root resolver, broadly reusable across scripts/), `atomic-sweep.sh` (from 08.5), `verify-invocation-matrix.sh` (from 08.7). If not built: STOP and build them now.

### Rollback strategy (Phase 2 Finding 6, 2026-04-11)

§08 touches 230+ files and is the largest single commit in the plan. If post-commit verification fails catastrophically (compiler source corrupted, `cargo check` broken, `./test-all.sh` fails in ways that don't have obvious 2-minute fixes), "fix forward" is **NOT** acceptable — per CLAUDE.md §Stabilization Discipline, interference is best handled by reorder-don't-skip, and mass-corruption is the strongest interference signal.

**Rollback triggers (any one of these):**
1. `cargo check --workspace` fails with errors in any crate that was not expected to be touched
2. `cargo clippy --workspace --all-targets` fails with new errors
3. `./test-all.sh` fails with errors that trace to §08's edits (path resolution, missing files, script-not-found)
4. `git diff compiler/ --stat` shows any file modified other than the two pre-approved (`llvm_dump/mod.rs`, `aot/util/aot.rs`)
5. `git diff library/ --stat` is non-empty
6. `lefthook run pre-commit` fails in a way that's not a pre-existing issue

**Rollback procedure:**
```bash
cd /home/eric/projects/ori_lang

# 1. Revert the §08 commit — the commit is the atomic unit, so HEAD~1 is pre-§08 state
git reset --hard HEAD~1

# 2. Clean any untracked files the sweep created (e.g., scripts/diagnostics/_repo-root.sh)
git clean -fd

# 3. Verify baseline state restored
./test-all.sh 2>&1 | tail -10
# Expected: green (same as pre-§08 state)

cargo check --workspace 2>&1 | tail -5
# Expected: compiles cleanly

ls scripts/dev/ 2>&1 || echo "no scripts/dev/ (expected — §08 rolled back)"
ls diagnostics/ 2>&1 | head -5
# Expected: diagnostics/ restored to pre-§08 location

# 4. File a bug for the failure mode — this bug BLOCKS re-running §08 until fixed
# Run /add-bug with:
#   title: "§08 reorg failure mode: <specific symptom>"
#   severity: high
#   repro: "re-apply §08 per plan → reproduce the exact failure observed"
#   subsystem: project-reorganization

# 5. Investigate root cause — which step of §08 produced the corruption?
#    Common causes:
#    - Pattern E allow-list leaked (fix: tighten allow-list in §08.5)
#    - Diagnostic repo-root resolver broken (fix: audit _repo-root.sh logic)
#    - Wrapper template broken (fix: update template in §08.3)
#    - Concurrent edit absorbed (fix: stronger worktree gate in §08.8)

# 6. Update §08 with the lesson learned, amend the plan, then retry from the
#    appropriate subsection (NOT §08.1 — only the failed subsection needs redo)
```

**Rollback is NOT acceptable for:** minor cosmetic issues, test timeout due to slow machine, rg/grep output formatting differences, known-pre-existing test failures. Rollback is the nuclear option — use it only for actual corruption.

**After rollback:** §08 must be retried from the beginning of the affected subsection. Do NOT attempt to "fix the partial commit in place" — the whole point of atomic commits is that they are either fully landed or fully reverted.

**Exit Criteria:** `ls /home/eric/projects/ori_lang/` shows exactly 2 public scripts at root (install.sh, setup.sh) plus the 4 hot-path wrappers (test-all.sh, full-check.sh, fmt-all.sh, build-all.sh) — 6 .sh files total, down from 11. `scripts/dev/` contains 8 internal scripts. `scripts/diagnostics/` contains 16+ debug scripts plus `_repo-root.sh`. `diagnostics/` absent. All 747 references either correctly point to new paths (cleanly-moved) or correctly preserved via wrappers (hot-path). `./test-all.sh`, `./full-check.sh`, `lefthook run pre-commit` all green. Single atomic commit documents the entire reorganization's final-form. Mission success criteria #2, #3, #5, #11 satisfied. Rollback strategy documented for catastrophic failure modes.
