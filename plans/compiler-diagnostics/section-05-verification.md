---
section: "05"
title: Verification & Integration
status: complete
goal: "Test all diagnostic tools, update all documentation, integrate into CI"
depends_on: ["01", "02", "03", "04"]
sections:
  - id: "05.1"
    title: "Script Self-Tests"
    status: complete
  - id: "05.2"
    title: "Documentation Updates"
    status: complete
  - id: "05.3"
    title: "CI Integration"
    status: not-started
  - id: "05.4"
    title: "Completion Checklist"
    status: complete
---

# Section 05: Verification & Integration

**Status:** Complete (05.3 CI Integration deferred — marked optional in plan)
**Goal:** Verify all diagnostic tools work correctly on representative programs, update all documentation to reference the new tools, and optionally integrate into CI.

**Context:** The tools are only useful if people know about them and they're reliable. This section ensures the toolkit is documented in CLAUDE.md, .claude/rules/, and the memory files so that future sessions (and Claude itself) use them automatically when debugging.

**Depends on:** All previous sections (01-04).

---

## 05.1 Script Self-Tests

**File(s):** `diagnostics/self-test.sh` (new script)

Create a self-test script that exercises every diagnostic tool on known-good and known-bad programs, verifying expected output.

- [x] Create test fixture programs in `diagnostics/fixtures/`:
  - `clean.ori` — program with no issues (all diagnostics should pass)
  - `leaky.ori` — program with a deliberate RC leak (leak check should flag)
  - `chain.ori` — chained COW operations (IR audit should analyze)
  - `simple.ori` — minimal program (baseline for all tools)
- [x] Create `diagnostics/self-test.sh` that runs each tool on each fixture:
  ```bash
  # Usage: diagnostics/self-test.sh [--verbose]
  # Runs: all diagnostic scripts on fixture programs
  # Verifies: expected output patterns (not exact match)
  ```
- [x] Verify `ir-dump.sh` produces non-empty IR for all fixtures
- [x] Verify `ir-diff.sh` shows differences between `simple.ori` and `chain.ori`
- [x] Verify `rc-stats.sh` shows zero imbalance for `clean.ori`
- [x] Verify `diagnose-aot.sh` reports pass for `clean.ori`, leak for `leaky.ori`
- [x] Verify `dual-exec-debug.sh` shows match for `clean.ori`
- [x] Verify `codegen-audit.sh` reports no issues for `clean.ori`
- [x] Report summary: `N/N scripts passed on M/M fixtures`

---

## 05.2 Documentation Updates

**File(s):** Multiple documentation files

Update all relevant documentation to reference the new diagnostic tools. This ensures future sessions use the tools automatically.

### CLAUDE.md (root project instructions)

- [x] **CLAUDE.md** — Add `diagnostics/` to Key Paths section
- [x] **CLAUDE.md** — Add diagnostic commands to Commands section
- [x] **CLAUDE.md** — Add new env vars to Tracing/Debugging section

### .claude/rules/ files (20 rule files to audit)

All rules files at `.claude/rules/` must reference diagnostic tools where relevant:

- [x] **`.claude/rules/llvm.md`** — Added diagnostic scripts and `ORI_AUDIT_CODEGEN` docs
- [x] **`.claude/rules/compiler.md`** — Added phase dumps and runtime instrumentation
- [x] **`.claude/rules/runtime.md`** — Added `ORI_TRACE_RC`, `ORI_RT_DEBUG`, `ORI_CHECK_LEAKS` env vars
- [x] **`.claude/rules/tests.md`** — Added AOT test failure diagnostic guidance
- [x] **`.claude/rules/aot.md`** — Added `ir-diff`, `codegen-audit`, `disasm-ori` references
- [x] **`.claude/rules/arc.md`** — Added `ORI_TRACE_RC` and `codegen-audit.sh` references
- [x] **`.claude/rules/eval.md`** — Added dual-exec debugging reference
- [x] **`.claude/rules/diagnostic.md`** — Added `ORI_AUDIT_CODEGEN` and phase dump reference
- [x] **`.claude/rules/ir.md`** — Added all 4 phase dump flags (`ORI_DUMP_AFTER_*`)
- [x] Audit remaining rules files — added `ORI_DUMP_AFTER_PARSE` to `parse.md`, `ORI_DUMP_AFTER_TYPECK` to `typeck.md` and `types.md`, clarified `cargo bl`/`blr` in `cargo.md`

### Memory files

- [x] **Memory file** — Skipped (diagnostic tools well-documented in CLAUDE.md and rules files; memory file tracks cross-session patterns/gotchas, not tool listings)

---

## 05.3 CI Integration (Optional)

**File(s):** `.github/workflows/` or `test-all.sh`

Optionally integrate diagnostic tools into the CI pipeline or test suite.

CI integration deferred — diagnostic scripts work standalone and are documented in CLAUDE.md.
Future improvements tracked in roadmap if needed:
- Add `diagnostics/self-test.sh` to `test-all.sh` (optional `--diagnostics` flag)
- Add `diagnostics/check-debug-flags.sh` to CI for flag drift detection
- Consider `codegen-audit.sh` in strict mode as a CI gate
- Document CI integration choice in `00-overview.md`

---

## 05.4 Completion Checklist

- [x] `diagnostics/self-test.sh` passes on all fixture programs (21/21)
- [x] CLAUDE.md updated with diagnostics paths, commands, and env vars
- [x] `.claude/rules/llvm.md` updated with diagnostic script references
- [x] `.claude/rules/compiler.md` updated with phase dump and runtime instrumentation
- [x] `.claude/rules/runtime.md` updated with new env vars
- [x] `.claude/rules/tests.md` updated with diagnostic debugging guidance
- [x] `.claude/rules/aot.md` updated with enhanced LLVM debugging tools
- [x] Memory file — skipped (tools documented in CLAUDE.md and rules files)
- [x] All scripts have `--help` output
- [x] `./test-all.sh` green (diagnostic tools don't break anything)

**Exit Criteria:** A developer (or Claude) encountering an AOT bug can find the diagnostic tools within 30 seconds by reading CLAUDE.md or .claude/rules/. Running `diagnostics/diagnose-aot.sh failing_test.ori` produces a comprehensive report. All 6 shell scripts + self-test pass. `check-debug-flags.sh` confirms flag consistency. Documentation is complete across all `.claude/rules/` files.
