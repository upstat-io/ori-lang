---
section: "01"
title: "Remove aims-compare + Create debug-release-compare"
status: in-progress
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
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Remove dead AIMS comparison scripts"
    status: complete
  - id: "01.2"
    title: "Create debug-release-compare.sh"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Remove aims-compare + Create debug-release-compare

**Status:** Not Started
**Goal:** Replace the dead AIMS comparison scripts with a new debug-vs-release comparison tool that catches FastISel-only bugs and optimization-dependent behavioral divergences.

**Success Criteria:**
- [x] `aims-compare.sh`, `aims-baseline.sh`, `aims-measure.sh` deleted
- [ ] New `debug-release-compare.sh` compiles + runs through both `target/debug/ori` and `target/release/ori`, comparing exit codes and stdout
- [ ] On mismatch, auto-dumps LLVM IR from both builds for diffing
- [ ] `self-test.sh` passes with new debug-release-compare test entries
- [ ] Satisfies mission criterion: "aims-compare.sh removed; new debug-release-compare.sh functional"

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

- [ ] Extend `diagnostics/_common.sh` with profile-specific binary resolution:
  - Add `find_ori_bin_profile(profile)` function that returns `$ROOT/target/$profile/ori` with existence check and clear error message (e.g., "Release binary not found — run: cargo b --release")
  - Add `require_both_builds()` helper that validates both debug and release binaries exist (used by this script; Section 02's `diagnose-aot.sh --both-builds` will also consume it)
  - Keep existing `find_ori_bin()` unchanged (auto-selects debug-first for backward compatibility)
  - Rationale: Section 02 (`diagnose-aot.sh --release`, `--both-builds`) explicitly needs profile-specific binary selection (see section-02 line 91). Centralizing in `_common.sh` prevents duplicated path logic across scripts — design principle 1 (canonical surfaces).
- [ ] Create `diagnostics/debug-release-compare.sh` with:
  - `--help`, `--no-color`, `--color` (standard options per `_common.sh` conventions)
  - `--verbose` (include LLVM IR diff on mismatch)
  - Uses `require_both_builds()` from `_common.sh` at startup
  - Uses `find_ori_bin_profile debug` and `find_ori_bin_profile release` for binary paths
  - Compiles input file with both binaries, runs both, compares exit codes and stdout
  - On mismatch: auto-runs `ir-dump.sh` from both builds via `ORI_BIN` env var override (e.g., `ORI_BIN=$(find_ori_bin_profile debug) ir-dump.sh file.ori`), then shows diff of the two IR outputs
  - Exit codes: 0 = match, 1 = mismatch, 2 = usage/infrastructure error
- [ ] Add self-test entries to `diagnostics/self-test.sh`:
  - Setup: ensure release binary exists (`cargo b --release` at self-test start, or skip with clear message if unavailable)
  - `simple.ori` produces matching output from both builds (happy path)
  - `--help` shows usage
  - Error handling for missing release binary: temporarily rename `target/release/ori` → `target/release/ori.bak`, verify error message, restore (or use `ORI_BIN` override to a nonexistent path)
- [ ] Add documentation to `diagnostics/README.md` with usage examples
- [ ] **Add** new reference in `CLAUDE.md` (at line 152, replacing the removed aims-compare line): `diagnostics/debug-release-compare.sh` — describe as "debug vs release behavioral comparison (exit codes + stdout + LLVM IR diff on mismatch)"
- [ ] **Add** new reference in `.claude/rules/arc.md` (at line 174, replacing the removed aims-compare line): `diagnostics/debug-release-compare.sh` — describe accurately as debug-vs-release comparison (NOT RC comparison)
- [ ] Run `diagnostics/self-test.sh --verbose` to verify all tests pass

- [ ] **Subsection close-out (01.2)** — MANDATORY before starting 01.R:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] All subsections (01.1, 01.2) complete
- [ ] `diagnostics/self-test.sh` passes
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] No references to aims-compare remain in active codebase surfaces (grep for `aims-compare`, `aims-baseline`, `aims-measure` across `CLAUDE.md`, `.claude/rules/`, `diagnostics/`, and `plans/`)
- [ ] `/tpr-review` passed — independent third-party review clean
- [ ] `/impl-hygiene-review` passed — after TPR is clean
- [ ] **`/improve-tooling` section-close sweep** — verify both subsection retrospectives ran; add any cross-subsection patterns
