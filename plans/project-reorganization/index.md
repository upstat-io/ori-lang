# Project Reorganization Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Plan goal:** Bring the `ori_lang/` repository root into alignment with first-class compiler conventions (rustc, swift, zig, gleam, roc, lean4) while preserving every existing reference (lefthook, CI, 747 script references across 230 files, cross-repo globs from `ori-lang-website`).

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Census & Classification Baseline
**File:** `section-01-census.md` | **Status:** Not Started

```
root-entry classification, 46 visible entries, 37 tracked
blast radius, 747 matches, 230 files, per-script breakdown
reference audit, lefthook.yml, CLAUDE.md:131-152, CONTRIBUTING.md
ls /home/eric/projects/ori_lang, git ls-files, rg blast radius
PUBLIC_API, INTERNAL_DEV, ORPHAN, TRACKED_ARTIFACT, LOCAL_ARTIFACT
first-class compiler comparison, rustc, swift, zig, gleam, roc
```

---

### Section 02: Stale Website Path Repair
**File:** `section-02-website-paths.md` | **Status:** Not Started

```
test-all.sh:37, website/public/test-results.json, stale JSON_PATH
docs/development/versioning.md:53, website/ references
rebuild-playground.sh, ori-lang-website/scripts/rebuild-wasm.sh
website boundary drift, pre-blog-migration repair
DRIFT under SSOT, Phase-1 cleanup before blog move
```

---

### Section 03: Tracked Artifact Removal & .gitignore Drift Fix
**File:** `section-03-tracked-artifacts.md` | **Status:** Not Started

```
build/debug/simplest_crash.ll, build/debug/test_simple, tracked leak
.gitignore drift bug, **/build/ vs /build/ root-anchor
git rm --cached, git check-ignore -v, gitignore /build/ rule
profile.json.gz (INTENTIONAL — keep), 365K performance reference
negative rule interaction, compiler/oric/src/commands/build/
```

---

### Section 04: Blog Migration (Cross-Repo, Atomic)
**File:** `section-04-blog-migration.md` | **Status:** Not Started

```
blog/building-ori-from-scratch.md, cross-compilation-nightmare.md
three-weeks-of-compiler-plumbing.md, Astro content collection
ori-lang-website/src/content.config.ts:85-92, blog loader base
'../ori_lang/blog' → './content/blog', Astro loader relocation
AskUserQuestion per-file approval, projects/CLAUDE.md rule
cross-repo sequenced commit, git mv, frontmatter preserved
```

---

### Section 05: Orphan Cleanup
**File:** `section-05-orphan-cleanup.md` | **Status:** Not Started

```
samples/ empty directory, gitignored, delete
compiler/ori_lsp/.gitkeep, phantom stub, reserved canonical home
rebuild-playground.sh, obsolete, no references, dead website/ path
git rm, orphan elimination, zero references verification
```

---

### Section 06: LSP Disposition (tools/ori-lsp Atomic Deletion)
**File:** `section-06-lsp-disposition.md` | **Status:** Not Started

```
tools/ori-lsp, unrevivable, cargo check E0432 E0433 E0425
oric::ast, oric::lexer, oric::parser::Parser, oric::type_check
oric::format::format(), removed public APIs
Cargo.toml:47-50 workspace exclude (REQUIRES USER PERMISSION — cargo.md)
scripts/sync-version.sh:187-188, .github/workflows/auto-release.yml:106-107
docs/tooling/lsp/design/index.md, 02-architecture/index.md, wasm.md:33
docs/tooling/formatter/design/index.md:126, 130, integration.md:70, 135
compiler/oric/src/lib.rs:96 re-exports comment, docs/development/versioning.md:67
plans/roadmap/section-22-tooling.md:150-156, mcp-semantic-tooling-proposal.md:48
editors/vscode-ori UNAFFECTED, no LSP client configured
tier-1 critical, tier-2 stale, tier-3 docs rewrite, tier-4 phantom
§06.6 deletion: git rm -rf + rm -rf (two-step to remove untracked target/ residue,
  Finding 3 fix). Commit uses explicit path staging, NOT git add -A (Finding 4).
```

---

### Section 07: Scratchpad Triage & Migration
**File:** `section-07-scratchpad-triage.md` | **Status:** Not Started

```
scratchpad/ audit, 30 files, 12 migrate, 18 delete, 704K total
docs/ori_lang/v2026/design/ (7 files): language-design-principles,
  syntax-design-principles, sigil-syntax-evaluation, language-beauty,
  syntax-improvements, let-mut-evaluation, block-expression-syntax
docs/compiler/design/ (3 files): aot-test-gaps, lexer-architecture,
  error-handling-ux
docs/tooling/ (1): ori-fmt-code-review
docs/ori_lang/proposals/drafts/ (1): package-system iteration-6
DELETE: aot-llvm-findings (already in plans), all 4 .ori experiments,
  parser/type/interp/codegen patterns (not Ori-specific),
  iteration-1-5 (superseded), formatter-architecture (stale),
  spec-to-architecture-mapping (spec-captured)
Astro content.config.ts docsSchema: title + order frontmatter required
website cross-repo glob auto-publishes
```

---

### Section 08: Script Consolidation (Option C Hybrid + Reference Sweep)
**File:** `section-08-script-consolidation.md` | **Status:** Not Started

```
Option C hybrid, hot-path wrappers, clean move for rest
test-all.sh, full-check.sh, fmt-all.sh, build-all.sh (ROOT WRAPPERS)
clippy-all.sh, llvm-build.sh, llvm-clippy.sh, llvm-test.sh (CLEAN MOVE)
scripts/dev/ new home, scripts/diagnostics/ from root diagnostics/
thin exec wrappers, readlink -f SCRIPT_PATH, symlink-safe, cwd=repo-root assumption
lefthook.yml:12,16 update, ./scripts/dev/fmt-all.sh, ./scripts/dev/full-check.sh
CLAUDE.md:133-152 commands section, CONTRIBUTING.md script refs
.claude/rules/diagnostic.md §Diagnostic Scripts table update
.claude/rules/{compiler,tests,llvm,registry}.md, 6 matches
.claude/skills/**/*.md 14 matches across 7 skills
.claude/commands/{verify-roadmap,commit-push}.md 4 matches
.codex/skills/tp-help/SKILL.md:21
compiler/ori_llvm/tests/aot/util/aot.rs:168 test code
compiler/oric/src/llvm_dump/mod.rs:8 comment (Pattern E Step E.4 manual)
plans/** 597 matches, EXCEPT plans/completed/ (archival), atomic sed sweep
full-check.sh internal calls: $SCRIPT_DIR/clippy-all.sh, test-all.sh
747 references atomic rewrite, explicit allow-list, no shim removal phase
install.sh (PUBLIC API — stays), setup.sh (stays), rebuild-playground.sh (deleted)

§08 subsections: 08.1 create dirs, 08.2 clean-move 4 scripts, 08.3 wrap 4 hot-path
  (with readlink -f symlink-safety and cwd=repo-root doc), 08.4 move diagnostics/
  WITH canonical _repo-root.sh helper (Finding 1 fix — 9 broken $SCRIPT_DIR/..
  patterns replaced), 08.5 sweep A/B/C/D/E with Pattern E explicit allow-list
  (Finding 2 fix — negative-pin guards compiler/ library/ tests/), 08.6 smoke
  validation, 08.7 NEW invocation matrix (33 cells: 29 exercisable + 4 PATH SKIP —
  Finding 5 fix), 08.8 NEW atomic commit with explicit path staging
  (Finding 4 fix — no git add -A), 08.N rollback strategy (Finding 6 fix)
```

---

### Section 09: Final Verification
**File:** `section-09-verification.md` | **Status:** Not Started

```
./test-all.sh green, ./full-check.sh green (via wrapper)
lefthook pre-commit runs clean, fmt + full-check
CI test PR, .github/workflows/ci.yml green, all jobs pass
cargo c, cargo cl, cargo b, cargo t, cargo st — all green
git status clean, no stale references via global grep
grep -rn './test-all.sh\|./build-all.sh\|...' returns only:
  - scripts/dev/*.sh (the moved files themselves)
  - hot-path wrappers at root (test-all.sh, full-check.sh, fmt-all.sh, build-all.sh)
  - scripts/dev/full-check.sh internal $SCRIPT_DIR calls
No website/ references in ori_lang except compiler/oric commands
Auto-release.yml git add list verified
Version sync runs clean with ori-lsp removed
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Census & Classification Baseline | `section-01-census.md` |
| 02 | Stale Website Path Repair | `section-02-website-paths.md` |
| 03 | Tracked Artifact Removal & .gitignore Drift Fix | `section-03-tracked-artifacts.md` |
| 04 | Blog Migration (Cross-Repo, Atomic) | `section-04-blog-migration.md` |
| 05 | Orphan Cleanup | `section-05-orphan-cleanup.md` |
| 06 | LSP Disposition | `section-06-lsp-disposition.md` |
| 07 | Scratchpad Triage & Migration | `section-07-scratchpad-triage.md` |
| 08 | Script Consolidation & Reference Sweep | `section-08-script-consolidation.md` |
| 09 | Final Verification | `section-09-verification.md` |
