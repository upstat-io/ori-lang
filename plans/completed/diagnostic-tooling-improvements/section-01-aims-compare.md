---
section: "01"
title: "Remove aims-compare + Create debug-release-compare"
status: complete
reviewed: true
goal: "Replace dead aims-compare.sh (uses non-existent --features aims) with a new debug-release-compare.sh that catches FastISel-only bugs"
success_criteria:
  - "aims-compare.sh, aims-baseline.sh, aims-measure.sh removed from diagnostics/"
  - "New debug-release-compare.sh compiles and runs a program through both debug and release binaries, comparing exit codes and stdout"
  - "self-test.sh updated — new debug-release-compare tests added (no old aims-compare tests exist to remove)"
  - "README.md updated with new script documentation"
inspired_by:
  - "Swift SIL verifier — runs same program through debug and release SIL pipelines"
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-04-09
sections:
  - id: "01.1"
    title: "Remove dead AIMS comparison scripts"
    status: complete
  - id: "01.2"
    title: "Create debug-release-compare.sh"
    status: complete
  - id: "01.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Remove aims-compare + Create debug-release-compare

**Status:** Complete
**Goal:** Replace the dead AIMS comparison scripts with a new debug-vs-release comparison tool that catches FastISel-only bugs and optimization-dependent behavioral divergences.

**Success Criteria:**
- [x] `aims-compare.sh`, `aims-baseline.sh`, `aims-measure.sh` deleted
- [x] New `debug-release-compare.sh` compiles + runs through both `target/debug/ori` and `target/release/ori`, comparing exit codes and stdout
- [x] On mismatch, auto-dumps LLVM IR from both builds for diffing
- [x] `self-test.sh` passes with new debug-release-compare test entries (28/28 passed)
- [x] Satisfies mission criterion: "aims-compare.sh removed; new debug-release-compare.sh functional"

**Context:** `aims-compare.sh` uses `--features aims` (line 177) which no longer exists — the `aims` feature was removed when AIMS became the default pipeline. The script fails immediately on any invocation. Codex verified: `cargo build -p oric --features aims` fails with "package does not contain this feature: aims". Keeping the `aims-compare` name after AIMS is default is DRIFT per impl-hygiene.md. The debug-vs-release capability is genuinely useful since LLVM FastISel (debug) behaves differently from the full optimization pipeline (release).

**Reference implementations:**
- **Swift** verifier: runs same input through debug and release SIL pipelines to catch optimization-dependent bugs

**Depends on:** None.

---

## 01.1 Remove dead AIMS comparison scripts and stale references

**File(s):** `diagnostics/aims-compare.sh`, `diagnostics/aims-baseline.sh`, `diagnostics/aims-measure.sh`, `CLAUDE.md`, `.claude/rules/arc.md`, queued plan files

These three scripts (~900 lines total) are dead code. `aims-compare.sh` (347 lines) fails at line 177 (`--features aims` removed). `aims-baseline.sh` (244 lines) and `aims-measure.sh` (292 lines) are orphaned support scripts only called by `aims-compare.sh`.

**IMPORTANT — Semantic mismatch:** The old `aims-compare.sh` compared **output + RC counts** across AIMS pipeline variants (behavioral + RC parity). The new `debug-release-compare.sh` compares **debug vs release builds** (exit codes + stdout + LLVM IR on mismatch). These are fundamentally different tools answering different questions. References to `aims-compare.sh` must NOT be blindly renamed — each consumer must be audited for whether `debug-release-compare.sh` is the correct replacement or whether the reference should simply be removed.

- [x] Delete `diagnostics/aims-compare.sh` (347 lines)
- [x] Delete `diagnostics/aims-baseline.sh` (244 lines)
- [x] Delete `diagnostics/aims-measure.sh` (292 lines)
- [x] Verify `diagnostics/self-test.sh` contains no aims-compare references (confirmed: none exist)
- [x] Verify `diagnostics/README.md` contains no aims-compare references (confirmed: none exist)
- [x] **Remove** `CLAUDE.md` line 152 aims-compare reference entirely — removed, leaving AIMS lattice description without dead tool reference
- [x] **Remove** `.claude/rules/arc.md` line 174 aims-compare reference — replaced with debug-release-compare.sh reference (accurate semantics: debug vs release comparison, NOT RC comparison)
- [x] Audit and fix stale cross-plan references (both plans are `status: queued`, not active):
  - `plans/locality-representation-unification/section-05-verification.md` lines 19, 91, 233: updated with notes about removal and semantic difference
  - `plans/clang-arc-lessons/section-06-verification.md` line 160: updated with note about removal and interim RC measurement approach
- [x] Verify `diagnostics/self-test.sh` still passes after removal (24/24 passed)

- [x] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.1: no tooling gaps. Deletion + reference cleanup only; self-test.sh verification sufficient.

---

## 01.2 Create debug-release-compare.sh

**File(s):** `diagnostics/_common.sh` (extend), `diagnostics/debug-release-compare.sh` (new), `diagnostics/self-test.sh`, `diagnostics/README.md`, `CLAUDE.md`, `.claude/rules/arc.md`

Create a new script that compiles and runs a program through both debug and release builds, comparing behavioral output. This catches FastISel-only bugs (the >16B aggregate load issue) and optimization-dependent codegen divergences.

- [x] Extend `diagnostics/_common.sh` with profile-specific binary resolution:
  - Added `find_ori_bin_profile(profile)` function with existence + LLVM check + clear error messages
  - Added `require_both_builds()` helper that sets `ORI_DEBUG` and `ORI_RELEASE`
  - Existing `find_ori_bin()` unchanged
- [x] Create `diagnostics/debug-release-compare.sh` with:
  - `--help`, `--no-color`, `--color`, `--verbose` (standard options)
  - Uses `require_both_builds()` from `_common.sh` at startup
  - Compiles and runs through both builds, compares exit codes and stdout
  - On mismatch: auto-dumps LLVM IR from both builds via `ORI_BIN` override, shows diff
  - `--verbose` also shows RC stats from both builds
  - Exit codes: 0 = match, 1 = mismatch, 2 = usage/infrastructure error
- [x] Add self-test entries to `diagnostics/self-test.sh`:
  - Skip-with-message if release binary unavailable (safer than rename-based testing)
  - `simple.ori` and `clean.ori` matching output (happy path, 2 tests)
  - `--help` shows usage
  - No-args error handling (exit 2)
- [x] Add documentation to `diagnostics/README.md` with usage examples and workflow
- [x] **Add** new reference in `CLAUDE.md` diagnostic scripts section: `debug-release-compare.sh`
- [x] **Add** new reference in `.claude/rules/arc.md` line 174 (done in 01.1 as part of the replacement)
- [x] Run `diagnostics/self-test.sh --verbose` — 28 passed, 0 failed

- [x] **Subsection close-out (01.2)** — MANDATORY before starting 01.R:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.2: no tooling gaps. Script creation followed existing patterns cleanly; self-test infrastructure made test addition straightforward.

---

## 01.R Third Party Review Findings

- [x] `[TPR-01-001-codex][medium]` `diagnostics/debug-release-compare.sh:117` — Return exit code 2 when either build fails.
  Evidence: DRIFT — script header defines exit 1=mismatch, 2=infra error, but compile-failure branches used exit 1. Fresh verification confirmed.
  Resolved: Fixed on 2026-04-09 in commit 337411e7. Both compile-failure branches now exit 2.
- [x] `[TPR-01-002-codex][low]` `diagnostics/self-test.sh:255` — Exercise the infrastructure-error paths in self-test.
  Evidence: GAP — self-test only checked matching fixtures, --help, no-args. No compile-fail test existed.
  Resolved: Fixed on 2026-04-09 in commit 337411e7. Added run_test_exit_code helper and compile-failure test.
- [x] `[TPR-01-003-codex][low]` `plans/diagnostic-tooling-improvements/section-01-aims-compare.md:35` — Synchronize plan status surfaces for Section 01 and Section 02.
  Evidence: DRIFT — Section 01 body said "Not Started" while frontmatter said complete. Overview and index also stale.
  Resolved: Fixed on 2026-04-09 in commit 337411e7. All plan surfaces synced.
- [x] `[TPR-01-001-gemini][informational]` `diagnostics/debug-release-compare.sh:130` — Document omission of stderr comparison.
  Non-actionable observation. Stderr comparison intentionally omitted (exit code catches panics).
- [x] `[TPR-01-002-gemini][informational]` `diagnostics/_common.sh:65` — Address reliance on SCRIPT_DIR convention.
  Non-actionable observation. SCRIPT_DIR convention is the established pattern for all diagnostic scripts.

---

## 01.N Completion Checklist

- [x] All subsections (01.1, 01.2) complete
- [x] `diagnostics/self-test.sh` passes (29/29)
- [x] `timeout 150 ./test-all.sh` green — no regressions (16,927 passed)
- [x] No references to aims-compare remain in active codebase surfaces (`CLAUDE.md`, `.claude/rules/`, `diagnostics/` all clean)
- [x] `/tpr-review` passed — independent third-party review clean
- [x] `/impl-hygiene-review` passed — after TPR is clean
- [x] **`/improve-tooling` section-close sweep** — verify both subsection retrospectives ran; add any cross-subsection patterns
