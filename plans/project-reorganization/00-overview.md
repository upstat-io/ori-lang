---
plan: "project-reorganization"
title: "Project Root Reorganization: Exhaustive Implementation Plan"
status: not-started
supersedes: []
references:
  - "CLAUDE.md"
  - ".claude/rules/impl-hygiene.md"
  - ".claude/rules/cargo.md"
  - ".claude/rules/tests.md"
  - ".claude/rules/diagnostic.md"
  - "../../ori-lang-website/src/content.config.ts"
---

# Project Root Reorganization: Exhaustive Implementation Plan

## Mission

Bring the `ori_lang/` repository root into alignment with first-class compiler conventions (rustc, swift, zig, gleam, roc, lean4) — by moving internal dev scripts into a consolidated `scripts/dev/` home, merging `diagnostics/` into `scripts/diagnostics/`, migrating `blog/` out to `ori-lang-website/`, deleting the provably-dead `tools/ori-lsp` prototype and its phantom `compiler/ori_lsp/.gitkeep` placeholder, triaging the 704K `scratchpad/` corpus into proper `docs/` homes, removing the `build/debug/*` gitignore leak and associated tracked artifacts, and sweeping all 747 script references across 230 files atomically — while preserving the `install.sh` / `setup.sh` public API, the `full-check.sh` → `clippy-all.sh` → `test-all.sh` internal call chain, the `ori-lang-website` cross-repo glob couplings, and the `lefthook.yml` pre-commit contract.

## Mission Success Criteria

The mission is complete when ALL of these are true.

- [ ] **Root entry inventory** — `ls /home/eric/projects/ori_lang/` produces a root layout where every remaining entry has a justified classification (STANDARD, PUBLIC_API, or INFRA) per the census taxonomy in §01; no ORPHAN or TRACKED_ARTIFACT category entries remain. *(Delivered by: §02, §03, §04, §05, §06, §07, §08.)*
- [ ] **Zero stale root references** — `rg -n '\./(test-all|clippy-all|build-all|fmt-all|full-check|llvm-test|llvm-build|llvm-clippy)\.sh' | grep -v 'scripts/dev/' | grep -v '^scripts/dev/' | grep -v '^(test-all|full-check|fmt-all|build-all)\.sh:'` returns ZERO results for the four cleanly-moved scripts (`clippy-all`, `llvm-test`, `llvm-build`, `llvm-clippy`). Hot-path wrappers are the only root-level script references that survive. *(Delivered by: §08.)*
- [ ] **Lefthook pre-commit passes** — `lefthook run pre-commit` executes `scripts/dev/fmt-all.sh` and `scripts/dev/full-check.sh` via wrapper forwarding and completes with exit 0. *(Delivered by: §08.)*
- [ ] **CI green** — a test PR to master runs `.github/workflows/ci.yml` successfully with no path errors (CI already uses `./scripts/sync-version.sh` / `./scripts/bump-build.sh` which are unchanged). *(Delivered by: §08, §09.)*
- [ ] **`./test-all.sh` green** — the root wrapper forwards to `scripts/dev/test-all.sh` and the full test matrix passes in both debug and release; `ORI_CHECK_LEAKS=1 ./test-all.sh` reports zero leaks. *(Delivered by: §08, §09.)*
- [ ] **Blog is served from website** — `ori-lang-website/src/content/blog/` contains the 3 migrated posts with original frontmatter preserved, `ori-lang-website/src/content.config.ts:85-92` loader base is `'./content/blog'` (local), and `ori_lang/blog/` no longer exists in `git ls-files`. A local Astro `bun run dev` build in the website repo renders the 3 blog posts without 404s. *(Delivered by: §04.)*
- [ ] **LSP deletion is atomic** — `tools/ori-lsp/` absent, `compiler/ori_lsp/.gitkeep` absent, and no reference to either path survives in `Cargo.toml`, `scripts/sync-version.sh`, `.github/workflows/auto-release.yml`, `.claude/rules/cargo.md`, `docs/development/versioning.md`, `compiler/oric/src/lib.rs`, `docs/tooling/lsp/design/**`, `docs/tooling/formatter/**`, `plans/roadmap/section-22-tooling.md`, or the proposals docs. *(Delivered by: §06.)*
- [ ] **Scratchpad is empty or absent** — all 12 migrated files live in their approved destinations (11 with Astro `docsSchema`-compliant frontmatter — `title`, `order` — where required by website cross-repo globs; 1 with body-format proposal schema per `ori-lang-website/src/loaders/proposal-loader.ts:22-59`); the 18 deletions are gone; `scratchpad/` either does not exist or contains zero files. *(Delivered by: §07.)*
- [ ] **Tracked build/debug artifacts removed** — `git ls-files build/` returns empty; `git check-ignore -v --no-index build/debug/simplest_crash.ll` matches `.gitignore:5:**/build/` (empirical proof from TPR iteration 3 that the existing rule ALREADY covers root-level `build/`); no `.gitignore` edit is needed (the iteration 1 premise that `**/build/` doesn't match root-level was factually wrong — git's `**` path glob includes the zero-component case); the existing negative rules for `compiler/oric/src/commands/build/` continue to work unchanged. *(Delivered by: §03.)*
- [ ] **Stale website/ paths repaired** — `test-all.sh:37` no longer writes to `website/public/test-results.json`; `docs/development/versioning.md:53` no longer references `website/`; `rebuild-playground.sh` is deleted. `rg 'website/' ori_lang/` returns only legitimate references (e.g., in documentation describing the sibling repo). *(Delivered by: §02, §05.)*
- [ ] **`./fmt-all.sh`, `./clippy-all.sh`, `./test-all.sh` muscle memory preserved (hot-path 4)** — `./test-all.sh` (root wrapper), `./full-check.sh`, `./fmt-all.sh`, `./build-all.sh` all resolve and produce identical output to direct `scripts/dev/*.sh` invocation. Developers do NOT need to re-learn daily commands. *(Delivered by: §08.)*
- [ ] **`/tpr-review` clean** — independent dual-source review (Codex + Gemini) finds no critical or major findings against the full plan execution (or all findings are triaged and resolved). *(Delivered by: §09.)*
- [ ] **`/impl-hygiene-review` clean** — no SSOT/DRIFT/LEAK/BLOAT/WASTE/EXPOSURE findings against the reorganized state. MUST run AFTER `/tpr-review` clean. *(Delivered by: §09.)*
- [ ] **`./test-all.sh` green after all sections complete** — final regression guard. *(Delivered by: §09.)*

## Architecture

### Current state — where the drift lives

```
ori_lang/                                     [46 visible, 37 tracked, 747 script refs]
│
├── STABLE (23 entries)                       ✔ untouched — matches first-class compilers
│   13 root files (README, LICENSE×2, Cargo×2, CLAUDE.md, etc.)
│   8 directories (compiler/, library/, tests/, docs/, plans/, editors/, docker/, examples/)
│   + 2 public scripts (install.sh, setup.sh)
│
├── FRAGMENTATION (13 entries)                ⚙ consolidate into scripts/
│   8 internal dev scripts at root (test-all, clippy-all, build-all, fmt-all,
│   full-check, llvm-build, llvm-clippy, llvm-test)
│   diagnostics/ (16 debug scripts)
│   scripts/ (13 utility scripts — already well-placed)
│   tools/ (contains ori-lsp stub)
│   + compiler/ori_lsp/.gitkeep phantom
│
├── ORPHANS (4 entries)                       ☠ delete or migrate
│   blog/              → ori-lang-website/src/content/blog/
│   samples/           → git rm (empty)
│   scratchpad/        → triage to docs/ (12 files), delete (18 files); live count 30
│   tools/ori-lsp/     → git rm (provably unrevivable)
│
├── TRACKED ARTIFACTS (3 entries)             ☠ remove + fix gitignore
│   build/debug/simplest_crash.ll             ← tracked BEFORE **/build/ was added (iteration 3 correction: the rule ALREADY matches root-level build/; these files just need `git rm --cached`)
│   build/debug/test_simple
│   profile.json.gz                           ← INTENTIONAL, keep
│
└── STALE DRIFT (3 locations)                 🔧 repair in place
    test-all.sh:37              JSON_PATH="website/public/test-results.json"
    docs/development/versioning.md:53         "./website/..."
    rebuild-playground.sh (whole file)        Cross-repo path to deleted website/
```

### Target state — after reorganization

```
ori_lang/                                     [~32 tracked, clean canonical homes]
│
├── 13 stable root files                      ✔ unchanged (frontmatter-only edits to CLAUDE.md, CONTRIBUTING.md)
│
├── 8 standard directories                    ✔ compiler/, library/, tests/, docs/, plans/, editors/, docker/, examples/
│   └── compiler/ori_lsp/.gitkeep             ☠ DELETED (phantom)
│
├── 2 public-API scripts                      🌐 install.sh, setup.sh (unchanged)
│
├── 4 hot-path thin wrappers                  📎 Option C hybrid — muscle memory preserved
│   test-all.sh      ← exec scripts/dev/test-all.sh "$@"
│   full-check.sh    ← exec scripts/dev/full-check.sh "$@"
│   fmt-all.sh       ← exec scripts/dev/fmt-all.sh "$@"
│   build-all.sh     ← exec scripts/dev/build-all.sh "$@"
│
└── scripts/                                  📦 consolidated tooling tree
    ├── (existing CI-used scripts unchanged)
    │   sync-version.sh, bump-build.sh, release.sh, cache-doctor.sh,
    │   cow-benchmark.sh, perf-baseline.sh, pgo-*.sh, generate-release-notes.py, ...
    ├── dev/                                   ⬅ NEW — internal dev scripts
    │   test-all.sh, full-check.sh, fmt-all.sh, build-all.sh,  ← wrapper targets
    │   clippy-all.sh, llvm-build.sh, llvm-clippy.sh, llvm-test.sh  ← cleanly moved
    └── diagnostics/                           ⬅ MOVED from root diagnostics/
        arc-dump.sh, bisect-passes.sh, check-debug-flags.sh, codegen-audit.sh,
        debug-release-compare.sh, diagnose-aot.sh, disasm-ori.sh,
        dual-exec-debug.sh, dual-exec-verify.sh, ir-diff.sh, ir-dump.sh,
        rc-stats.sh, self-test.sh, valgrind-aot.sh, _common.sh, README.md,
        fixtures/
```

### Cross-repo touch points (ori-lang-website)

```
ori-lang-website/                             (per-file AskUserQuestion approval per projects/CLAUDE.md)
├── src/
│   ├── content.config.ts:85-92               ✏ update blog loader base:
│   │                                            from: '../ori_lang/blog'
│   │                                            to:   './content/blog'
│   └── content/
│       └── blog/                              ⬅ NEW directory
│           ├── building-ori-from-scratch.md
│           ├── cross-compilation-nightmare.md
│           └── three-weeks-of-compiler-plumbing.md
│                    (frontmatter already conforms — no rewriting)
├── package.json                              ✔ unchanged (install.sh coupling preserved)
└── scripts/rebuild-wasm.sh                   ✔ unchanged (renders rebuild-playground.sh obsolete)
```

## Design Principles

### 1. Permanent canonical homes, not temporary shims

`impl-hygiene.md` §API Stability: "No deprecation for internal compiler code." When we move internal dev scripts to `scripts/dev/`, we do NOT leave shims that get deleted in a later phase (no `§09 shim removal`). Either a script is PUBLIC (permanent at root: `install.sh`, `setup.sh`) or it's INTERNAL (permanent at `scripts/dev/`). The "hot-path 4" wrappers at root ARE permanent public-ish entry points — they exist because developers type `./test-all.sh` 50 times a day and breaking that muscle memory has a real cost — but they are PERMANENT facades, not shims with a deletion timeline. Codex's position: "stable root facades, canonical implementations underneath." Gemini's position: "no shims, atomic rewrite." We synthesize: the wrappers are not shims because they're permanent.

### 2. Atomic updates across coupling boundaries

Every coupling between a moved/deleted file and its references must be updated in the same commit as the move. This is non-negotiable and applies at three scales:

- **Script move + reference sweep** — `§08` moves `clippy-all.sh` (and the other 3 clean-moved scripts) in the same commit that rewrites all references to `./scripts/dev/clippy-all.sh` across CLAUDE.md, .claude/rules/, .claude/skills/, .claude/commands/, CONTRIBUTING.md, plans/, docs/, and test code. Gemini's position: "big-bang atomic rewrite, `sed` + `rg` are the right tools." Codex verified: `test-all.sh` has 503 refs, `clippy-all.sh` has 175 — the mechanical sweep is tractable.
- **LSP deletion + tier-1/2 atomic updates** — `§06` deletes `tools/ori-lsp/` in the same commit that updates `scripts/sync-version.sh:187-188` (else release exits 1), `.github/workflows/auto-release.yml:106-107` (else `git add` errors), and `Cargo.toml:47-50` workspace exclude (requires user permission per `cargo.md`).
- **Blog move + website loader update** — `§04` changes `ori-lang-website/src/content.config.ts:85-92` loader base in the same sequence as the file move; the cross-repo glob coupling means the website and the blog files cannot drift apart even for a single commit (else the website build errors).

### 3. Preserve internal call chains

Script-to-script internal calls use `$SCRIPT_DIR` relative resolution. When all dependencies move together to the same directory (`scripts/dev/`), the `$SCRIPT_DIR` relative paths continue to resolve correctly with no code changes needed. Specifically:

- `full-check.sh` (hot-path wrapper) calls `$SCRIPT_DIR/clippy-all.sh` and `$SCRIPT_DIR/test-all.sh`. The wrapper at root execs into `scripts/dev/full-check.sh`, where `$SCRIPT_DIR` resolves to `scripts/dev/`, and `clippy-all.sh` (cleanly moved) + `test-all.sh` (wrapper target) are siblings in that same directory. The chain works without modification to internal `SCRIPT_DIR` logic.
- `llvm-*.sh` call `./docker/llvm/run.sh` which is repo-root-relative (not `$SCRIPT_DIR`-relative). When the llvm scripts move to `scripts/dev/`, they still run from the repo root (cwd-relative), so `./docker/llvm/run.sh` still resolves correctly from wherever the caller invoked them — no change needed.

## Section Dependency Graph

```
                              §01 Census
                                  │
              ┌───────────────────┼───────────────────┐
              │                   │                   │
          §02 Stale        §03 Artifacts &       §05 Orphans
         website paths       gitignore fix    (samples, .gitkeep,
              │                   │            rebuild-playground)
              │                   │                   │
              │                   │                   │
              ▼                   ▼                   ▼
         §04 Blog migration (cross-repo, sequenced)
              │
              ├───── Website loader update (ori-lang-website)
              ├───── Content files created (ori-lang-website) — AskUserQuestion ×3
              └───── ori_lang/blog/ git rm
                                  │
                                  ▼
                          §06 LSP disposition
                   (tools/ori-lsp, docs rewrite, tier-1/2/3 atomic)
                                  │
                                  ▼
                        §07 Scratchpad triage
                  (12 migrations with frontmatter, 18 deletes; 30 total)
                                  │
                                  ▼
                §08 Script consolidation + reference sweep
              (Option C hybrid, 747 refs swept atomically,
              lefthook.yml, CLAUDE.md, diagnostic.md updates)
                                  │
                                  ▼
                        §09 Final verification
               (test-all green, lefthook green, CI green)
```

**Independent sections** (can be worked in parallel after §01):
- §02, §03, §05 — pure state-observation and deletion; no cross-dependencies

**Serial dependencies**:
- §04 depends on §02 (stale `website/` paths must be gone from `test-all.sh` before the blog migration — the test suite must still run after the migration)
- §06 depends on §01 (LSP touch surface is a research output of the census)
- §07 depends on §06 (scratchpad migrations land in `docs/tooling/lsp/design/` which is being rewritten in §06 — order matters to avoid merge churn)
- §08 depends on EVERYTHING before it (the 747-reference sweep must cover everything that's been moved, renamed, or deleted; running it before §06/§07 would miss references in the LSP docs and scratchpad-migrated files)
- §09 depends on all of §01-§08 (it's the final gate)

**Critical-path warning**: §08 is the single biggest section by reference count (~747 sed replacements across 230 files) and carries the most drift risk if ANY prior section leaves a stale reference that §08's sweep then writes into a final committed state.

**Cross-section interactions (must be co-implemented within a single commit)**:

- **§06 + `Cargo.toml` edit** — deleting `tools/ori-lsp` requires removing the workspace exclude in `Cargo.toml:47-50`. Per `.claude/rules/cargo.md`, Cargo.toml edits require **explicit user permission**. This section must use `AskUserQuestion` to confirm the Cargo.toml edit before proceeding.
- **§06 tier-1 files** — `Cargo.toml`, `scripts/sync-version.sh:187-188`, `.github/workflows/auto-release.yml:106-107` MUST land in the same commit as the `git rm -rf tools/ori-lsp/`. If they split, release workflow and version sync both break.
- **§04 blog migration + `ori-lang-website/src/content.config.ts`** — the website's cross-repo glob pointer must change in lockstep with the file move. If the content moves but the loader still points at `../ori_lang/blog`, the website build errors immediately.
- **§08 moves + `lefthook.yml:12,16`** — lefthook's `./fmt-all.sh` and `./full-check.sh` hooks are the primary non-CI runtime dependency on the hot-path wrappers. If lefthook isn't updated in the same commit (even though wrappers preserve the paths, a wrapper bug could cascade), the pre-commit hook fails for every contributor.

## Execution Prerequisites — GNU userland required (Phase 2 Finding 7, 2026-04-11)

The plan's shell commands assume **GNU userland** — specifically GNU `sed`, GNU `grep`, GNU `timeout`, and GNU `find`. This is the default on Linux but NOT on macOS (which ships BSD variants).

**Required tools and their GNU behavior:**
- `sed -i` (in-place edit) — GNU accepts `sed -i 'expr' file`; BSD requires an extra empty suffix `sed -i '' 'expr' file`. All §08 sweep commands use the GNU form.
- `timeout <seconds> <cmd>` — not present in macOS base install; provided by `coreutils` as `gtimeout`.
- `grep -P` (Perl-compatible regex) and ERE `-E` — both in GNU grep; BSD grep lacks `-P`.
- `find -printf` and similar — GNU-only flags.

**macOS setup (one-time, before plan execution):**
```bash
brew install gnu-sed coreutils grep findutils
# Then either:
#   (a) use the g-prefixed variants (gsed, gtimeout, ggrep, gfind), OR
#   (b) prepend the GNU bin to PATH:
#       export PATH="/opt/homebrew/opt/gnu-sed/libexec/gnubin:$PATH"
#       export PATH="/opt/homebrew/opt/coreutils/libexec/gnubin:$PATH"
#       export PATH="/opt/homebrew/opt/grep/libexec/gnubin:$PATH"
#       export PATH="/opt/homebrew/opt/findutils/libexec/gnubin:$PATH"
# Option (b) is preferred — it means the plan's commands work verbatim without
# per-command `g`-prefix substitution.
```

**Linux systems:** no setup needed — GNU is the default.

**Verification before plan execution:**
```bash
sed --version | head -1          # should say "sed (GNU sed)"
timeout --version | head -1      # should say "timeout (GNU coreutils)"
grep --version | head -1         # should say "grep (GNU grep)"
```
- [ ] All three print "GNU ..." — OK to proceed
- [ ] If any prints a BSD variant: install GNU userland per macOS setup above before starting §02

## Implementation Sequence

```
Phase 0 — Census and baseline  (§01)
  Foundation for everything. Refreshed from Pass 1 research agents:
  confirmed 747/230 blast radius, classified all 46 root entries,
  captured LSP touch surface, verified gitignore drift bug.

Phase 1 — Cleanup that unlocks everything else  (§02, §03, §05 — parallel)
  §02 Stale website path repair (test-all.sh:37, versioning.md:53)
  §03 Tracked artifact removal + /build/ gitignore fix
  §05 Orphan cleanup (samples/, compiler/ori_lsp/.gitkeep, rebuild-playground.sh)
  Gate: ./test-all.sh still green (proves no accidental breakage)
        git status clean (proves commits landed atomically)

Phase 2 — Cross-repo blog migration  (§04)
  Website loader update + 3 AskUserQuestion-approved file creates in
  ori-lang-website/src/content/blog/ + ori_lang git rm.
  Gate: Local `bun run dev` in ori-lang-website renders 3 blog posts
        ori_lang/blog/ not in git ls-files
        ./test-all.sh still green

Phase 3 — LSP disposition  (§06)
  tools/ori-lsp deletion is atomic across tier-1 (critical), tier-2
  (stale comments), tier-3 (doc rewrite). Requires user permission
  to edit Cargo.toml:47-50 per cargo.md rule.
  Gate: cargo check --workspace still works (Cargo.toml valid)
        ./scripts/sync-version.sh --check returns clean
        No references to tools/ori-lsp in any tier-1/2 file

Phase 4 — Scratchpad triage  (§07)
  12 migrations with Astro frontmatter compliance + 18 deletions (30 total, reconciled from live count).
  docs/tooling/lsp/design/ must already be rewritten (§06) before
  scratchpad content lands near it.
  Gate: All docs/ additions satisfy docsSchema (title + order)
        Website cross-repo glob would pick up new files without error
        scratchpad/ either absent or empty

Phase 5 — Script consolidation (the big atomic rewrite)  (§08)
  Option C hybrid: move clippy-all + llvm-* to scripts/dev/, install
  wrappers for hot-path 4 at root, move diagnostics/ to scripts/diagnostics/,
  sweep 747 refs across 230 files atomically, update lefthook.yml,
  CLAUDE.md, CONTRIBUTING.md, .claude/rules/diagnostic.md.
  Gate: lefthook pre-commit passes
        ./test-all.sh (via wrapper) green
        ./scripts/dev/test-all.sh (direct) green
        rg hot-path references returns only wrapper + scripts/dev/ matches
        CI dry-run green

Phase 6 — Final verification  (§09)
  test-all, lefthook, CI, cargo c/cl/b/t/st all green.
  /tpr-review clean, /impl-hygiene-review clean.
```

**Why this order:**

- **Phase 1 items are independent**: §02/§03/§05 each touch disjoint file sets (website paths, gitignore + build/debug, orphan directories). They can land in any order or in parallel.
- **Phase 2 must follow Phase 1 §02**: if the blog migration is done while `test-all.sh:37` still points at a dead `website/public/` path, the test suite can't be used to prove the migration didn't break anything.
- **Phase 3 must follow Phase 2**: the LSP docs rewrite in `docs/tooling/lsp/design/` is a large multi-file edit; doing it before the blog migration would risk merge churn with the blog-related docs sweep.
- **Phase 4 must follow Phase 3**: scratchpad migrations add files to `docs/tooling/lsp/design/` and other docs paths — those directories must be in their final state first.
- **Phase 5 is gated on everything**: §08's 747-reference sweep must be the final state-changing operation; running it earlier would require re-running it after every subsequent section's changes.
- **Phase 6 is the single-point exit gate**: the plan doesn't complete until test-all, lefthook, CI are all green AND both reviewers signed off.

**Known failing tests (expected until plan completion):**

- **None at plan start**. The current state has `./test-all.sh` passing. The plan must MAINTAIN this invariant throughout — no section is permitted to leave the tree in a state where `./test-all.sh` fails.

## Metrics (Current State)

### Root entry inventory (from §01 census)

| Category | Count | Disposition |
|----------|-------|-------------|
| STANDARD files (LICENSE, README, Cargo.toml, clippy.toml, etc.) | 13 | unchanged |
| STANDARD directories (compiler/, library/, tests/, docs/, plans/, editors/, docker/, examples/) | 8 | unchanged internal |
| PUBLIC_API scripts (install.sh, setup.sh) | 2 | stay at root |
| INTERNAL_DEV scripts (test-all, full-check, fmt-all, build-all, clippy-all, llvm-build, llvm-clippy, llvm-test) | 8 | 4 wrapped + 4 moved |
| INTERNAL_DEV directories (scripts/, diagnostics/) | 2 | diagnostics/ → scripts/diagnostics/; scripts/ gets scripts/dev/ subdir |
| ORPHANS (blog/, samples/, scratchpad/, tools/ori-lsp/) | 4 | migrate/delete |
| TRACKED_ARTIFACTS (profile.json.gz, build/debug leaks) | 3 | 1 keep (perf ref), 2 delete |
| Phantom stubs (compiler/ori_lsp/.gitkeep) | 1 | delete |
| LOCAL_ARTIFACTS (gitignored on disk — target/, target-llvm/, test_cast*, *.log, .pytest_cache) | 8 | unchanged (not tracked) |
| INFRA (hidden dotfiles — .git, .github, .claude, etc.) | 10 | unchanged |
| Standalone (rebuild-playground.sh) | 1 | delete (obsolete) |
| **Total visible entries** | **46** | |
| **Total tracked entries** | **37** | |

### Reference blast radius (from §01 census, verified with `rg`)

| Script | Matches | Files | Notes |
|--------|---------|-------|-------|
| `test-all.sh` | 503 | 224 | 68% of total; dominates the sweep |
| `clippy-all.sh` | 175 | 108 | #2 by frequency |
| `llvm-test.sh` | 30 | 16 | |
| `fmt-all.sh` | 29 | 20 | |
| `full-check.sh` | 7 | 4 | |
| `build-all.sh` | 2 | 2 | |
| `setup.sh` | 1 | 1 | |
| `install.sh`, `llvm-build.sh`, `llvm-clippy.sh`, `rebuild-playground.sh` | 0 | 0 | |
| **Total** | **747** | **230** | |

### Reference distribution by directory

| Location | Matches | Files |
|----------|---------|-------|
| `plans/` (including completed/) | 597 | ~220 |
| `.claude/skills/` | 14 | 7 |
| `CLAUDE.md` | 7 | 1 |
| `.claude/rules/` | 6 | 4 |
| `CONTRIBUTING.md` | 5 | 1 |
| `.claude/commands/` | 4 | 2 |
| `docs/compiler/design/appendices/` | 2 | 1 |
| `lefthook.yml` | 2 | 1 |
| `.codex/skills/` | 1 | 1 |
| `compiler/ori_llvm/tests/aot/util/aot.rs` | 1 | 1 |
| `scripts/release.sh` | 1 | 1 |
| `.github/workflows/` | 0 | 0 |

**Top 5 most-affected individual files**:
1. `plans/repr-opt/section-07-enum-repr.md` — 24 matches
2. `plans/completed/iter-rc-contract/section-06-verification.md` — 20 matches
3. `plans/completed/jit-exception-handling/section-06-lcfail-resolution.md` — 19 matches
4. `plans/repr-opt/section-01-repr-ir.md` — 18 matches
5. `plans/llvm-worker-isolation/section-03-verification.md` — 14 matches

### Scratchpad corpus (from §07 triage)

| Destination | Count | Notes |
|-------------|-------|-------|
| `docs/ori_lang/v2026/design/` | 7 | Language design philosophy + syntax rationale |
| `docs/compiler/design/` | 3 | AOT test gaps, lexer architecture, error UX |
| `docs/tooling/` | 1 | ori-fmt code review plan |
| `docs/ori_lang/proposals/drafts/` | 1 | Package system proposal (iteration 6 only) |
| **DELETE** | **18** | Superseded, stale, or duplicated by existing rules (includes `07-modern-lang-repos.md`) |
| **Total files** | **30** | Reconciled from live `find scratchpad -type f` count |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| §01 Census & Classification Baseline | ~200 | Low | — |
| §02 Stale Website Path Repair | ~150 | Low | §01 |
| §03 Tracked Artifact + Gitignore Fix | ~180 | Low-Medium | §01 |
| §04 Blog Migration (Cross-Repo) | ~250 | Medium | §02 |
| §05 Orphan Cleanup | ~150 | Low | §01 |
| §06 LSP Disposition | ~400 | High | §01 (user permission gate) |
| §07 Scratchpad Triage | ~500 | Medium | §06 |
| §08 Script Consolidation + Reference Sweep | ~600 | High | §02–§07 |
| §09 Final Verification | ~200 | Low | §01–§08 |
| **Total plan** | **~2630 lines** | | |
| **Code deleted** | ~1200 lines (tools/ori-lsp + 18 scratchpad files + leaked artifacts) | | |

## Known Bugs (Pre-existing — discovered during Pass 1 research and Phase 2 dual-source review, ALL tracked here per CLAUDE.md §Zero Deferral)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| `build/debug/simplest_crash.ll` + `build/debug/test_simple` tracked despite `**/build/` gitignore | Files were committed BEFORE the `**/build/` rule was added (or via `git add -f`); once in the index, gitignore rules don't apply. Iteration 3 correction: the rule DOES match root-level `build/` (verified via `git check-ignore -v --no-index`). The iteration 1 claim that `**/build/` has a root-level pattern gap was factually wrong. Fix: `git rm --cached` only, no gitignore edit. | §03 | Not Started |
| `test-all.sh:37` writes to `website/public/test-results.json` which no longer exists after website extraction | Stale default when `--json` is passed without value; website/ dir removed when sibling repo was created but test-all.sh default wasn't updated | §02 | Not Started |
| `docs/development/versioning.md:53` references `website/` path | Same historical split — doc not updated when website moved | §02 | Not Started |
| `rebuild-playground.sh` is a dead script with zero references and assumes `./website/playground-wasm/` which no longer exists | Leftover from pre-split ori_lang repo; ori-lang-website has its own `scripts/rebuild-wasm.sh` now | §05 | Not Started |
| `tools/ori-lsp` is excluded from Cargo workspace and provably uncompilable — imports `oric::ast`, `oric::lexer`, `oric::parser::Parser`, `oric::type_check()`, `oric::format::format()` — none of these exist in current oric public API | Prototype was built against old oric API; was never updated when the new query-based compiler replaced the old flat-module API; `cargo check` emits E0432/E0433/E0425 errors | §06 | Not Started |
| `compiler/ori_lsp/.gitkeep` exists as phantom directory placeholder for "future canonical LSP home" | Directory was reserved but never populated; the canonical-home reservation is mentioned in `plans/roadmap/section-22-tooling.md:150-156` and several design docs as future intent | §06 | Not Started (directory reservation stays as plan commitment; only `.gitkeep` file deletes) |
| `scratchpad/aot-llvm-findings.md` duplicates content already captured in `plans/aot_codegen_pipeline/` | Original debugging journal later formalized into plan; scratchpad version is stale | §07 | Not Started |
| `scratchpad/design-ideas/03-parser-patterns.md`, `04-type-system-design.md`, `05-interpreter-patterns.md`, `06-codegen-patterns.md`, `09-testing-tooling.md` — all general reference material duplicated by `.claude/rules/*.md` | Educational notes that were superseded by formal Ori-specific rule files | §07 | Not Started |
| **Phase 2 Finding 1 (CRITICAL):** 9 diagnostic scripts derive repo root via `$(cd "$SCRIPT_DIR/.." && pwd)` which breaks at the new `scripts/diagnostics/` depth — `_common.sh:29,70,113`, `check-debug-flags.sh:29`, `self-test.sh:22`, `dual-exec-verify.sh:33`, `valgrind-aot.sh:118,136`, `fixtures/mismatch-wrapper.sh:25` | The prior §08.4 claimed "most diagnostic scripts use `$(dirname "$0")` patterns which work unchanged" — wrong for scripts that climb to repo root via `SCRIPT_DIR/..`. At new depth, parent is `scripts/`, not repo root, so `target/debug/ori`, `compiler/oric/src/debug_flags.rs`, `CLAUDE.md`, `tests/valgrind/`, `plans/code-journeys/` lookups silently fail. | §08.4 (canonical `scripts/diagnostics/_repo-root.sh` helper using `git rev-parse --show-toplevel`, plus edit of 9 patterns across 6 files) | Fixed by §08.4 in this review pass (2026-04-11) |
| **Phase 2 Finding 2 (CRITICAL):** Pattern E sweep (`diagnostics/` → `scripts/diagnostics/`) in §08.5 had no scope restriction — would corrupt real Rust modules `compiler/ori_eval/src/diagnostics/`, `compiler/ori_patterns/src/errors/diagnostics/`, and Rust source comment `compiler/oric/src/llvm_dump/mod.rs:8` | The prior Pattern E used deny-list (`grep -v '^target/'`) which does not exclude in-repo Rust source. `diagnostics/` is a legitimate Rust module name. Blind sed would rewrite Rust source literals and comments. | §08.5 (explicit allow-list: CLAUDE.md, CONTRIBUTING.md, .claude/, .codex/, plans/ except plans/completed/, docs/, lefthook.yml; compiler/oric/src/llvm_dump/mod.rs:8 handled as separate manual Edit; negative-pin verifies `compiler/`, `library/`, `tests/` untouched) | Fixed by §08.5 in this review pass (2026-04-11) |
| **Phase 2 Finding 3 (CRITICAL):** `git rm -rf tools/ori-lsp/` leaves disk residue — untracked `target/` inside the LSP dir survives. §06.N success criterion `ls tools/ori-lsp` returns "No such file or directory" would FAIL | `git rm` only removes tracked files from the index. The prototype has `target/` (untracked cargo build output) and `Cargo.lock` (potentially modified). The directory survives with untracked contents. | §06.6 (two-step deletion: `git rm -rf tools/ori-lsp/` followed by `rm -rf tools/ori-lsp/`) | Fixed by §06.6 in this review pass (2026-04-11) |
| **Phase 2 Finding 4 (LEAK):** §06.6 and §08.8 used `git add -A` which stages unrelated concurrent edits into the atomic commit | `git add -A` is indiscriminate — it absorbs any dirty file in the working tree, including concurrent contributor work, editor scratch files, or in-progress plan edits from other sections. The atomic commit becomes non-atomic. | §06.6 + §08.8 (explicit per-path `git add` with clean-worktree gate that allow-lists expected dirty paths and aborts on unexpected dirt) | Fixed by §06.6 + §08.8 in this review pass (2026-04-11) |
| **Phase 2 Finding 5 (MAJOR):** §08 had no explicit test matrix per `tests.md` §Matrix Testing Rule — missing cells for symlink invocation, cross-directory invocation, negative-pin exit propagation, test-code reference, internal chain re-entry | §08 is a reorg, not a compiler change, but reorgs ARE behavior changes (invocation paths). The matrix dimension is `script × invocation-context`. Unclamped cells are future regressions when developers use unusual invocation paths. | §08.7 (new subsection — 29-cell matrix across 8 scripts × 8 contexts, with wrapper template updated to `readlink -f` for symlink-safety) | Fixed by §08.7 in this review pass (2026-04-11) |
| **Phase 2 Finding 6 (MAJOR):** §08 had no rollback strategy — if the 230-file sweep corrupts compiler source, "fix forward" is not acceptable at that scale | Per CLAUDE.md §Stabilization Discipline, interference is handled by reorder-don't-skip. Mass corruption requires clean revert, not patches on top of a broken base. | §08.N (explicit rollback procedure: `git reset --hard HEAD~1`, `git clean -fd`, baseline re-verification, bug filing, root-cause investigation before retry) | Fixed by §08.N in this review pass (2026-04-11) |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Census & Classification Baseline | `section-01-census.md` | Not Started |
| 02 | Stale Website Path Repair | `section-02-website-paths.md` | Not Started |
| 03 | Tracked Artifact Removal & .gitignore Drift Fix | `section-03-tracked-artifacts.md` | Not Started |
| 04 | Blog Migration (Cross-Repo, Atomic) | `section-04-blog-migration.md` | Not Started |
| 05 | Orphan Cleanup | `section-05-orphan-cleanup.md` | Not Started |
| 06 | LSP Disposition | `section-06-lsp-disposition.md` | Not Started |
| 07 | Scratchpad Triage & Migration | `section-07-scratchpad-triage.md` | Not Started |
| 08 | Script Consolidation & Reference Sweep | `section-08-script-consolidation.md` | Not Started |
| 09 | Final Verification | `section-09-verification.md` | Not Started |

## Rules woven in (applies to every section)

These rules apply pervasively and MUST be reflected in each section's checklist items:

- **CLAUDE.md §The One Rule (Correctness Above All)** — No workarounds. If the correct fix touches 10 files across 3 crates, that IS the fix. The size of the atomic sweep in §08 is the assignment, not a reason to scope down.
- **CLAUDE.md §Zero Deferral on Bugs** — Every bug found during research is tracked in the Known Bugs table above. New bugs discovered during execution must be added to the table (or filed via `/add-bug` if out of scope) before continuing.
- **CLAUDE.md §Stabilization Discipline** — Narrow the front. Complete one section fully before starting another. No partial state left between sections.
- **CLAUDE.md §MANDATORY TEST TIMEOUTS** — Every test command in this plan's checklists uses `timeout 150` or `--timeout 150000`. Review/agent tasks (`/tpr-review`, `/tp-help`) are NOT test commands — no timeout prefix.
- **`.claude/rules/cargo.md`** — **ANY edit to `Cargo.toml` requires explicit user permission via `AskUserQuestion`.** This applies specifically to `§06` (workspace exclude removal). The section MUST gate its `Cargo.toml` edit on user approval.
- **`projects/CLAUDE.md` (sibling-repo approval rule)** — **Every individual file edit to `ori-lang-website/` requires `AskUserQuestion` approval.** This applies specifically to `§04` (blog migration) which touches 4 website files (content.config.ts + 3 new blog posts) — 4 separate approvals.
- **`.claude/rules/impl-hygiene.md` §SSOT** — No duplicated logic. Canonical homes for every moved/created file. No "future cleanup" phase.
- **`.claude/rules/impl-hygiene.md` §No Side Logic** — No shim mechanisms. Hot-path 4 wrappers are permanent public-ish facades, not shims.
- **`.claude/rules/tests.md` §Matrix Testing Rule** — Every section that modifies behavior must declare its test matrix dimensions. For file-move sections, the matrix is (moved file × call context: root / lefthook / CI / script-internal / test code). For the gitignore fix, it's (file × tracked-state × ignored-state).
- **`.claude/rules/impl-hygiene.md` §Test Function Naming** — No new test functions are added by this plan (it's a reorganization, not a code change), but any validation test harness scripts written for §09 follow the `<subject>_<scenario>_<expected>` shape.

## Reference implementations studied

- **rustc** (`reference_repos/lang_repos/rust/x.py`) — single-entry-point dev dispatcher. Considered for Option C but ultimately rejected: rustc's `x.py` replaces `make` / `configure` entirely; Ori's users already have muscle memory for `./test-all.sh` style. We adopted rustc's **spirit** (canonical implementation underneath a stable interface) without copying its specific dispatcher form.
- **swift** (`reference_repos/lang_repos/swift/utils/`) — consolidated developer utilities directory. Inspired the `scripts/dev/` layout.
- **zig** (`reference_repos/lang_repos/zig/build.zig`) — single build entry point. Not directly applicable because Ori uses Cargo, but the principle of "one canonical entry point per concern" influenced our decision to not fragment script locations.
- **gleam** (`reference_repos/lang_repos/gleam/Makefile` + `bin/`) — Makefile dispatcher + bin/ for utility scripts. Analogous to our `scripts/dev/` + wrapper pattern at repo root.
- **koka** (`reference_repos/lang_repos/koka/util/`, `koka/support/`) — simple consolidation under `util/`. Matches the "one place for dev tooling" principle.

## External consultation history

- **Step 1D consensus loop (2026-04-11)** — dual-source `/tp-help` consultation with Codex + Gemini on mission scope, script model, shim strategy, LSP disposition, and scratchpad routing. **Key disagreement resolved**: Codex argued for permanent root facades, Gemini argued for atomic rewrite with no shims. Synthesized by discovering (via Codex's grep) that `install.sh` is externally surfaced (copied into `ori-lang-website/package.json:7`), which proves SOME root scripts must be permanent — the clean "delete everything" position cannot survive the cross-repo coupling fact. Resolution: **Option C hybrid** — public scripts stay permanently at root, internal dev scripts move permanently to `scripts/dev/`, the hot-path 4 get permanent wrapper facades (not shims) to preserve muscle memory.
- **Codex numerical correction** — my Pass 0 claim of "738 references / 246 files" was wrong; Codex re-counted and found 747/230. Verified by Pass 1 Agent 1 with exact ripgrep.
- **Codex externally-surfaced discovery** — `install.sh` is referenced in `README.md:182`, `docs/guide/01-getting-started.md:20`, `compiler/ori_llvm/src/aot/runtime.rs:168` (error message), and `ori-lang-website/package.json:7`. This fact is load-bearing for the script consolidation design.
- **User decisions** — Full reorganization scope, blog migration handled end-to-end by Claude with per-file AskUserQuestion approval, LSP investigated-and-decided (result: delete), scratchpad audited with useful content migrated, Option C hybrid script model.
