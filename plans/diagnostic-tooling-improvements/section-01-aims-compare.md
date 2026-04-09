---
section: "01"
title: "Remove aims-compare + Create debug-release-compare"
status: not-started
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
    status: not-started
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
- [ ] `aims-compare.sh`, `aims-baseline.sh`, `aims-measure.sh` deleted
- [ ] New `debug-release-compare.sh` compiles + runs through both `target/debug/ori` and `target/release/ori`, comparing exit codes and stdout
- [ ] On mismatch, auto-dumps LLVM IR from both builds for diffing
- [ ] `self-test.sh` passes with new debug-release-compare test entries
- [ ] Satisfies mission criterion: "aims-compare.sh removed; new debug-release-compare.sh functional"

**Context:** `aims-compare.sh` uses `--features aims` (line 177) which no longer exists — the `aims` feature was removed when AIMS became the default pipeline. The script fails immediately on any invocation. Codex verified: `cargo build -p oric --features aims` fails with "package does not contain this feature: aims". Keeping the `aims-compare` name after AIMS is default is DRIFT per impl-hygiene.md. The debug-vs-release capability is genuinely useful since LLVM FastISel (debug) behaves differently from the full optimization pipeline (release).

**Reference implementations:**
- **Swift** verifier: runs same input through debug and release SIL pipelines to catch optimization-dependent bugs

**Depends on:** None.

---

## 01.1 Remove dead AIMS comparison scripts

**File(s):** `diagnostics/aims-compare.sh`, `diagnostics/aims-baseline.sh`, `diagnostics/aims-measure.sh`, `diagnostics/self-test.sh`, `diagnostics/README.md`

These three scripts (~900 lines total) are dead code. `aims-compare.sh` (347 lines) fails at line 177. `aims-baseline.sh` (244 lines) and `aims-measure.sh` (292 lines) are support scripts only called by `aims-compare.sh`.

- [ ] Delete `diagnostics/aims-compare.sh` (347 lines)
- [ ] Delete `diagnostics/aims-baseline.sh` (244 lines)
- [ ] Delete `diagnostics/aims-measure.sh` (292 lines)
- [ ] Verify `diagnostics/self-test.sh` contains no aims-compare references (confirmed: none exist as of plan creation — this is a verification step, not a removal)
- [ ] Verify `diagnostics/README.md` contains no aims-compare references (confirmed: none exist as of plan creation — this is a verification step, not a removal)
- [ ] Update `CLAUDE.md` line 152: replace `diagnostics/aims-compare.sh` reference with `diagnostics/debug-release-compare.sh` (cannot leave a stale reference to a deleted script between Section 01 and Section 07)
- [ ] Update `.claude/rules/arc.md` line 174: replace `diagnostics/aims-compare.sh` reference with `diagnostics/debug-release-compare.sh`
- [ ] Update stale cross-plan references (both plans are `status: queued`, not active — fix now to prevent confusion at execution time):
  - `plans/locality-representation-unification/section-05-verification.md` lines 19, 91, 233: replace `aims-compare.sh` with `debug-release-compare.sh`
  - `plans/clang-arc-lessons/section-06-verification.md` line 160: replace `aims-compare.sh` with `debug-release-compare.sh`
- [ ] Verify `diagnostics/self-test.sh` still passes after removal

- [ ] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 01.2 Create debug-release-compare.sh

**File(s):** `diagnostics/debug-release-compare.sh` (new), `diagnostics/self-test.sh`, `diagnostics/README.md`

Create a new script that compiles and runs a program through both debug and release builds, comparing behavioral output. This catches FastISel-only bugs (the >16B aggregate load issue) and optimization-dependent codegen divergences.

- [ ] Create `diagnostics/debug-release-compare.sh` with:
  - `--help`, `--no-color`, `--color` (standard options per `_common.sh` conventions)
  - `--verbose` (include LLVM IR diff on mismatch)
  - Requires both `target/debug/ori` and `target/release/ori` to exist (checked at startup with clear error)
  - Compiles input file with both binaries, runs both, compares exit codes and stdout
  - On mismatch: auto-runs `ir-dump.sh` from both builds via `ORI_BIN` env var override (e.g., `ORI_BIN=target/debug/ori ir-dump.sh file.ori` and `ORI_BIN=target/release/ori ir-dump.sh file.ori`), then shows diff of the two IR outputs
  - Exit codes: 0 = match, 1 = mismatch, 2 = usage/infrastructure error
  - Source `_common.sh` for color/option helpers only. Binary paths are constructed directly (`$ROOT/target/debug/ori`, `$ROOT/target/release/ori`) — NOT via `find_ori_bin()`, which auto-selects a single binary (debug-first) and does not support requesting a specific profile. The dual-binary pattern is unique to this script; extending `_common.sh` with profile selection for a single consumer would be over-engineering.
- [ ] Add self-test entries to `diagnostics/self-test.sh`:
  - `simple.ori` produces matching output from both builds
  - `--help` shows usage
  - Error handling for missing release binary
- [ ] Add documentation to `diagnostics/README.md` with usage examples
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
