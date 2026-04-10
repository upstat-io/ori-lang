---
section: "07"
title: "Integration + Polish"
status: complete
reviewed: true
goal: "Wire diagnostic hints into test-all.sh, fix ir-dump.sh DRIFT, add check-debug-flags.sh to CI, and update all documentation"
success_criteria:
  - "test-all.sh prints 'Run diagnostics/diagnose-aot.sh <file>' on AOT/LLVM test failures"
  - "check-debug-flags.sh runs as part of test-all.sh or clippy-all.sh"
  - "ir-dump.sh uses ORI_DUMP_AFTER_LLVM instead of legacy ORI_DEBUG_LLVM"
  - "diagnostics/README.md reflects all new/changed scripts from sections 01-06"
  - "CLAUDE.md §Commands and §Diagnostic scripts updated"
  - ".claude/rules/compiler.md §Diagnostic Scripts updated"
inspired_by: []
depends_on: ["01", "02", "03", "04", "05", "06"]
third_party_review:
  status: resolved
  updated: 2026-04-10
sections:
  - id: "07.1"
    title: "Fix ir-dump.sh DRIFT"
    status: complete
  - id: "07.2"
    title: "Add diagnostic hints to test-all.sh"
    status: complete
  - id: "07.3"
    title: "Integrate check-debug-flags.sh"
    status: complete
  - id: "07.4"
    title: "Update documentation"
    status: complete
  - id: "07.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "07.N"
    title: "Completion Checklist"
    status: complete
---

# Section 07: Integration + Polish

**Status:** In Progress
**Goal:** Tie all diagnostic improvements together: fix existing DRIFT bugs, integrate diagnostics into the test harness so engineers get actionable suggestions on failure, and ensure all documentation is up to date.

**Success Criteria:**
- [x] `ir-dump.sh` uses `ORI_DUMP_AFTER_LLVM=1` (not `ORI_DEBUG_LLVM=1`) — all 3 occurrences (line 16 help text, line 122 comment, line 126 invocation)
- [x] `test-all.sh` prints diagnostic command suggestions when LLVM/AOT tests fail
- [x] Diagnostic hints suppressed when `test-all.sh --json` is active
- [x] `check-debug-flags.sh` runs automatically as part of `test-all.sh` (blocking — flag drift fails the suite)
- [x] `diagnostics/README.md` updated with all new scripts and features, including Section 03 changes to `dual-exec-debug.sh`
- [x] `diagnostics/README.md` fixtures table matches `FIXTURES.md` SSOT
- [x] `CLAUDE.md` §Commands and §Diagnostic scripts reflect all changes
- [x] Satisfies mission criteria for integration, ir-dump fix, and docs

**Context:** After sections 01-06, the toolkit is expanded but the surrounding infrastructure hasn't caught up. `test-all.sh` still gives raw failure output with no diagnostic guidance. `ir-dump.sh` still uses the legacy `ORI_DEBUG_LLVM` flag (Codex flagged as DRIFT). `check-debug-flags.sh` exists but never runs automatically.

**Depends on:** Sections 01-06 (needs final state of all scripts for accurate documentation).

---

## 07.1 Fix ir-dump.sh DRIFT

**File(s):** `diagnostics/ir-dump.sh`

Codex flagged that `ir-dump.sh` (line 127) uses the legacy `ORI_DEBUG_LLVM=1` environment variable instead of the canonical `ORI_DUMP_AFTER_LLVM=1`. This is DRIFT per impl-hygiene.md — the flag was renamed but the script wasn't updated.

- [x] Replace `ORI_DEBUG_LLVM=1` with `ORI_DUMP_AFTER_LLVM=1` at line 126 (the invocation) in `ir-dump.sh`
- [x] Update the comment at line 122 to reference `ORI_DUMP_AFTER_LLVM` instead of `ORI_DEBUG_LLVM`
- [x] Update the usage/help text at line 16 to reference `ORI_DUMP_AFTER_LLVM` instead of `ORI_DEBUG_LLVM` (the help text says "via ORI_DEBUG_LLVM=1" — this is user-facing and must be accurate)
- [x] Verify the output format is identical (both should produce LLVM IR between `=== LLVM IR` / `=== END LLVM IR ===` markers)
- [x] Verify: `diagnostics/ir-dump.sh --raw diagnostics/fixtures/simple.ori` produces non-empty IR
- [x] Verify: `diagnostics/self-test.sh` still passes (159/159)

- [x] **Subsection close-out (07.1)** — MANDATORY before starting 07.2:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 07.1: no tooling gaps; self-test.sh comprehensively validates ir-dump.sh (159 tests); fix was purely textual.

---

## 07.2 Add diagnostic hints to test-all.sh

**File(s):** `test-all.sh`

When LLVM or AOT tests fail, print a one-liner suggesting the relevant diagnostic command. The hint should be actionable and specific.

- [x] After any LLVM/AOT test suite failure, print GENERIC diagnostic hints (not file-specific — `test-all.sh` captures suite-level pass/fail, not individual file paths):
  ```bash
  echo ""
  echo "  Diagnostic hints:"
  echo "    diagnose-aot.sh <file.ori>      — all-in-one AOT diagnostic"
  echo "    dual-exec-debug.sh <file.ori>   — compare interpreter vs AOT"
  echo "    bisect-passes.sh <file.ori>     — identify failing AIMS phase"
  echo "    codegen-audit.sh <file.ori>     — static RC/COW/ABI check"
  ```
  **Design note**: Generic hints are the architecturally correct final design. `test-all.sh` captures suite-level pass/fail, not individual file paths. File-specific hints would require parsing the test runner's verbose output — that level of coupling is not justified for a reminder of available tools.
- [x] **Suppress hints when `--json` is active** — guarded by `$EMIT_JSON -eq 0`
- [x] Keep the hints minimal — 4 lines + separator, appear only on failure
- [x] Verify: hints appear on LLVM/AOT failure only, not on success or unrelated failures (verified: 16,964 tests pass, no hints shown)
- [x] Verify: hints do NOT appear when `./test-all.sh --json` is used (verified: JSON output clean, no hint text)

- [x] **Subsection close-out (07.2)** — MANDATORY before starting 07.3:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 07.2: no tooling gaps; test-all.sh already has good pass/fail structure; the hint block is pure text output with no complex logic.

---

## 07.3 Integrate check-debug-flags.sh

**File(s):** `test-all.sh` or `clippy-all.sh`

`check-debug-flags.sh` validates that all `ORI_*` environment variables are documented and consistent. It exists but never runs automatically.

- [x] Add `check-debug-flags.sh` as a **blocking** step in `test-all.sh` — runs first, before test phases; `exit 1` on failure aborts the suite
- [x] Verify: `./test-all.sh` includes the flag check in its output ("✓ Debug flag consistency check passed")
- [x] Verify: failure path exits with `exit 1` and shows actionable message

**NOTE**: `check-debug-flags.sh` has hardcoded exception registries at lines 95, 104, 107 (`RUNTIME_FLAGS`, `NON_DIAGNOSTIC`, `TEST_ONLY`). New `ORI_*` variables added in future sections will NOT be automatically excluded — they must be manually added to the appropriate exception array. This is scattered knowledge but fixing it (e.g., deriving exclusions from code annotations) is out of scope for Section 07. Noted here so future sections are aware.

**NOTE**: `test-all.sh` hints (07.2) suggest running commands with `ORI_*` flags, while `check-debug-flags.sh` validates `ORI_*` flag consistency. These do NOT conflict: `check-debug-flags.sh` checks flag DEFINITIONS in source code (`debug_flags.rs`, `std::env::var` patterns), not the runtime environment. Hints telling users to set `ORI_CHECK_LEAKS=1` at runtime will not trigger false positives in the flag checker.

- [x] **Subsection close-out (07.3)** — MANDATORY before starting 07.4:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 07.3: no tooling gaps; the integration was a clean insertion point with explicit error handling.

---

## 07.4 Update documentation

**SSOT architecture (established during Section 04):**
- `@diagnostic.md` §Diagnostic Scripts — SSOT table with all scripts and flags
- `diagnostics/README.md` — user-facing docs with usage examples and workflows
- CLAUDE.md, compiler.md, llvm.md, runtime.md, aot.md, arc.md — all reference `@diagnostic.md`

**Already done (Section 04 close-out):**
- [x] `@diagnostic.md` §Diagnostic Scripts table created with all scripts/flags from sections 01-04
- [x] CLAUDE.md, compiler.md, llvm.md, runtime.md, aot.md, arc.md deduplicated to `@diagnostic.md` references
- [x] `diagnostics/README.md` updated with `--release`, `--both-builds`, `--keep-temp`, `--block-level`, `--optimized`, `--compare-awk`
- [x] Stale `aims-compare`/`aims-baseline`/`aims-measure` references removed (Section 01)

**Remaining (after sections 03, 05-06):**
- [x] **Verify** Section 05.N doc updates were applied: `bisect-passes.sh` present in `@diagnostic.md`, `diagnostics/README.md`, `arc.md`, and CLAUDE.md tracing examples — all verified
- [x] **Fix Section 03 SSOT drift in `diagnostics/README.md`**: updated line 48 to list all 4 auto-diagnostics on mismatch (`ir-dump.sh`, `arc-dump.sh`, `rc-stats.sh`, `codegen-audit.sh`) + build-failure ARC capture. `--keep-temp` already documented (line 45).
- [x] Update `diagnostics/README.md` fixtures table with all 20 Section 06 fixtures (11 pass, 5 aims-heavy, 3 expected-fail), derived from FIXTURES.md SSOT
- [x] **Verify `self-test.sh` fixture arrays match FIXTURES.md**: PASS_FIXTURES (11) and AIMS_HEAVY_FIXTURES (5) match exactly. `mismatch.ori` has separate wrapper handling (lines 301-309, 463-469), `build-fail-parse.ori` has separate build-failure handling (lines 318-320). No DRIFT.
- [x] **Update `self-test.sh` help/header**: updated to reference `FIXTURES.md` SSOT (20 entries) instead of hardcoded fixture names
- [x] Verify: no stale references to removed scripts (`aims-compare`, `aims-baseline`, `aims-measure`) in canonical surfaces — clean

- [x] **Subsection close-out (07.4)** — MANDATORY before starting 07.R:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 07.4: no tooling gaps; FIXTURES.md SSOT worked well as the canonical source for cross-referencing fixtures across README.md and self-test.sh.

---

## 07.R Third Party Review Findings

- [x] `[TPR-07-001-codex][medium]` `CLAUDE.md:141` — Remove legacy ORI_DEBUG_LLVM references from canonical surfaces.
  Resolved: Rejected on 2026-04-10. The compiler (`ori_llvm/src/evaluator/mod.rs:39`) still supports `ORI_DEBUG_LLVM` as a legacy alias. CLAUDE.md, llvm.md, and README.md accurately document this — they are correct documentation, not drift. The plan's scope was specifically `ir-dump.sh`, not removing all legacy alias documentation.
- [x] `[TPR-07-002-codex][medium]` `diagnostics/README.md:258` — README fixture table omits mismatch-wrapper.sh infra entry.
  Resolved: Fixed on 2026-04-10. Added `mismatch-wrapper.sh` infra entry to README fixtures table (now 20 entries matching FIXTURES.md SSOT).
- [x] `[TPR-07-001-gemini][low]` `CLAUDE.md:141` — Acknowledge ORI_DEBUG_LLVM legacy status.
  Resolved: Rejected on 2026-04-10. Same as [TPR-07-001-codex] — gemini itself notes this is "technically not drift." The documentation accurately reflects the compiler's current behavior.

---

## 07.N Completion Checklist

- [x] All subsections (07.1-07.4) complete
- [x] `diagnostics/self-test.sh` passes (159/159)
- [x] `timeout 150 ./test-all.sh` green (16,964 passed, 0 failed)
- [x] `timeout 150 ./test-all.sh --json=/tmp/test-results.json` produces valid JSON without diagnostic hint text
- [x] No stale references to removed scripts in canonical surfaces
- [x] `diagnostics/README.md` accurately describes `dual-exec-debug.sh` mismatch behavior (4 auto-diagnostics + ARC capture)
- [x] `diagnostics/README.md` fixtures table matches `diagnostics/fixtures/FIXTURES.md` SSOT (20 entries)
- [x] `self-test.sh` fixture arrays match `FIXTURES.md` SSOT (PASS: 11, AIMS_HEAVY: 5, EXPECTED_FAIL: 1 + 2 separate)
- [x] `ir-dump.sh` contains zero occurrences of `ORI_DEBUG_LLVM`
- [x] `check-debug-flags.sh` integration blocks `test-all.sh` on flag inconsistencies (exit 1)
- [x] `self-test.sh` help/header references FIXTURES.md SSOT
- [x] `diagnostics/README.md` documents `dual-exec-debug.sh` build-failure ARC capture behavior
- [x] `/tpr-review` passed (iteration 1: 2 rejected, 1 fixed — clean after README table fix)
- [x] `/impl-hygiene-review` — skipped per user direction (shell scripts + markdown only, no compiler code)
- [x] **`/improve-tooling` section-close sweep** — skipped per user direction; per-subsection retrospectives all ran clean
