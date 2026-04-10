---
section: "07"
title: "Integration + Polish"
status: in-progress
reviewed: false
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
  status: none
  updated: null
sections:
  - id: "07.1"
    title: "Fix ir-dump.sh DRIFT"
    status: not-started
  - id: "07.2"
    title: "Add diagnostic hints to test-all.sh"
    status: not-started
  - id: "07.3"
    title: "Integrate check-debug-flags.sh"
    status: not-started
  - id: "07.4"
    title: "Update documentation"
    status: in-progress
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Integration + Polish

**Status:** Not Started
**Goal:** Tie all diagnostic improvements together: fix existing DRIFT bugs, integrate diagnostics into the test harness so engineers get actionable suggestions on failure, and ensure all documentation is up to date.

**Success Criteria:**
- [ ] `ir-dump.sh` uses `ORI_DUMP_AFTER_LLVM=1` (not `ORI_DEBUG_LLVM=1`)
- [ ] `test-all.sh` prints diagnostic command suggestions when LLVM/AOT tests fail
- [ ] `check-debug-flags.sh` runs automatically as part of `test-all.sh` or `clippy-all.sh`
- [ ] `diagnostics/README.md` updated with all new scripts and features
- [ ] `CLAUDE.md` §Commands and §Diagnostic scripts reflect all changes
- [ ] Satisfies mission criteria for integration, ir-dump fix, and docs

**Context:** After sections 01-06, the toolkit is expanded but the surrounding infrastructure hasn't caught up. `test-all.sh` still gives raw failure output with no diagnostic guidance. `ir-dump.sh` still uses the legacy `ORI_DEBUG_LLVM` flag (Codex flagged as DRIFT). `check-debug-flags.sh` exists but never runs automatically.

**Depends on:** Sections 01-06 (needs final state of all scripts for accurate documentation).

---

## 07.1 Fix ir-dump.sh DRIFT

**File(s):** `diagnostics/ir-dump.sh`

Codex flagged that `ir-dump.sh` (line 127) uses the legacy `ORI_DEBUG_LLVM=1` environment variable instead of the canonical `ORI_DUMP_AFTER_LLVM=1`. This is DRIFT per impl-hygiene.md — the flag was renamed but the script wasn't updated.

- [ ] Replace `ORI_DEBUG_LLVM=1` with `ORI_DUMP_AFTER_LLVM=1` in `ir-dump.sh`
- [ ] Verify the output format is identical (both should produce LLVM IR between `=== LLVM IR` / `=== END LLVM IR ===` markers)
- [ ] Update the comment at line 122 to reference `ORI_DUMP_AFTER_LLVM`
- [ ] Verify: `diagnostics/ir-dump.sh --raw diagnostics/fixtures/simple.ori` produces non-empty IR
- [ ] Verify: `diagnostics/self-test.sh` still passes

- [ ] **Subsection close-out (07.1)** — MANDATORY before starting 07.2:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 07.2 Add diagnostic hints to test-all.sh

**File(s):** `test-all.sh`

When LLVM or AOT tests fail, print a one-liner suggesting the relevant diagnostic command. The hint should be actionable and specific.

- [ ] After any LLVM/AOT test suite failure, print GENERIC diagnostic hints (not file-specific — `test-all.sh` captures suite-level pass/fail, not individual file paths):
  ```bash
  echo ""
  echo "  Diagnostic hints:"
  echo "    diagnose-aot.sh <file.ori>      — all-in-one AOT diagnostic"
  echo "    dual-exec-debug.sh <file.ori>   — compare interpreter vs AOT"
  echo "    bisect-passes.sh <file.ori>     — identify failing AIMS phase"
  echo "    codegen-audit.sh <file.ori>     — static RC/COW/ABI check"
  ```
  **Note**: File-specific hints would require parsing the test runner's verbose output to extract failing `.ori` paths — this is a future enhancement, not in scope for this plan. Generic hints are still valuable as a reminder of available tools.
- [ ] Keep the hints minimal — 4-5 lines max, appear only on failure
- [ ] Verify: hints appear on LLVM/AOT failure only, not on success or unrelated failures

- [ ] **Subsection close-out (07.2)** — MANDATORY before starting 07.3:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 07.3 Integrate check-debug-flags.sh

**File(s):** `test-all.sh` or `clippy-all.sh`

`check-debug-flags.sh` validates that all `ORI_*` environment variables are documented and consistent. It exists but never runs automatically.

- [ ] Add `check-debug-flags.sh` as a step in `test-all.sh` (or `clippy-all.sh` if it's a better fit)
  - Run after compilation checks, before test execution
  - Non-blocking: log warnings but don't fail the suite (flags may be temporarily out of sync during development)
  - Alternatively: add to `clippy-all.sh` as a "doc lint" step
- [ ] Verify: `./test-all.sh` (or `./clippy-all.sh`) includes the flag check in its output

- [ ] **Subsection close-out (07.3)** — MANDATORY before starting 07.4:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

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

**Remaining (after sections 05-06):**
- [ ] Update `@diagnostic.md` §Diagnostic Scripts with `bisect-passes.sh` (Section 05)
- [ ] Update `diagnostics/README.md` with `bisect-passes.sh` usage section and workflow
- [ ] Update `diagnostics/README.md` fixtures table with all Section 06 fixtures
- [ ] Verify: no stale references to removed scripts remain (`grep -rn "aims-compare\|aims-baseline\|aims-measure" CLAUDE.md .claude/rules/ diagnostics/ plans/`)

- [ ] **Subsection close-out (07.4)** — MANDATORY before starting 07.R:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 07.R Third Party Review Findings

- None.

---

## 07.N Completion Checklist

- [ ] All subsections (07.1-07.4) complete
- [ ] `diagnostics/self-test.sh` passes
- [ ] `timeout 150 ./test-all.sh` green
- [ ] No stale references to removed scripts
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] **`/improve-tooling` section-close sweep**
