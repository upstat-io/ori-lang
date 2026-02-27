---
section: "05"
title: Verification & Integration
status: not-started
goal: "Test all diagnostic tools, update all documentation, integrate into CI"
depends_on: ["01", "02", "03", "04"]
sections:
  - id: "05.1"
    title: "Script Self-Tests"
    status: not-started
  - id: "05.2"
    title: "Documentation Updates"
    status: not-started
  - id: "05.3"
    title: "CI Integration"
    status: not-started
  - id: "05.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Verification & Integration

**Status:** Not Started
**Goal:** Verify all diagnostic tools work correctly on representative programs, update all documentation to reference the new tools, and optionally integrate into CI.

**Context:** The tools are only useful if people know about them and they're reliable. This section ensures the toolkit is documented in CLAUDE.md, .claude/rules/, and the memory files so that future sessions (and Claude itself) use them automatically when debugging.

**Depends on:** All previous sections (01-04).

---

## 05.1 Script Self-Tests

**File(s):** `diagnostics/self-test.sh` (new script)

Create a self-test script that exercises every diagnostic tool on known-good and known-bad programs, verifying expected output.

- [ ] Create test fixture programs in `diagnostics/fixtures/`:
  - `clean.ori` — program with no issues (all diagnostics should pass)
  - `leaky.ori` — program with a deliberate RC leak (leak check should flag)
  - `chain.ori` — chained COW operations (IR audit should analyze)
  - `simple.ori` — minimal program (baseline for all tools)
- [ ] Create `diagnostics/self-test.sh` that runs each tool on each fixture:
  ```bash
  # Usage: diagnostics/self-test.sh [--verbose]
  # Runs: all diagnostic scripts on fixture programs
  # Verifies: expected output patterns (not exact match)
  ```
- [ ] Verify `ir-dump.sh` produces non-empty IR for all fixtures
- [ ] Verify `ir-diff.sh` shows differences between `simple.ori` and `chain.ori`
- [ ] Verify `rc-stats.sh` shows zero imbalance for `clean.ori`
- [ ] Verify `diagnose-aot.sh` reports pass for `clean.ori`, leak for `leaky.ori`
- [ ] Verify `dual-exec-debug.sh` shows match for `clean.ori`
- [ ] Verify `codegen-audit.sh` reports no issues for `clean.ori`
- [ ] Report summary: `N/N scripts passed on M/M fixtures`

---

## 05.2 Documentation Updates

**File(s):** Multiple documentation files

Update all relevant documentation to reference the new diagnostic tools. This ensures future sessions use the tools automatically.

### CLAUDE.md (root project instructions)

- [ ] **CLAUDE.md** — Add `diagnostics/` to Key Paths section:
  ```
  `diagnostics/` — diagnostic scripts (ir-dump, ir-diff, diagnose-aot, etc.)
  ```
- [ ] **CLAUDE.md** — Add diagnostic commands to Commands section:
  ```
  **Diagnostics**: `diagnostics/ir-dump.sh file.ori` (LLVM IR), `diagnostics/ir-diff.sh a.ori b.ori` (IR diff),
  `diagnostics/diagnose-aot.sh file.ori` (all-in-one AOT check), `diagnostics/rc-stats.sh file.ori` (RC counts),
  `diagnostics/dual-exec-debug.sh file.ori` (interpreter vs AOT), `diagnostics/codegen-audit.sh file.ori` (RC balance)
  ```
- [ ] **CLAUDE.md** — Add new env vars to Tracing/Debugging section:
  ```
  `ORI_TRACE_RC=1` (RC event log), `ORI_RT_DEBUG=1` (runtime assertions),
  `ORI_DUMP_AFTER_PARSE=1` (AST), `ORI_DUMP_AFTER_TYPECK=1` (typed IR),
  `ORI_DUMP_AFTER_ARC=1` (ARC IR), `ORI_DUMP_AFTER_LLVM=1` (annotated LLVM IR)
  ```

### .claude/rules/ files (20 rule files to audit)

All rules files at `.claude/rules/` must reference diagnostic tools where relevant:

- [ ] **`.claude/rules/llvm.md`** — Add diagnostic scripts to Debugging section:
  ```
  ## Diagnostic Scripts (USE FIRST for AOT bugs)
  diagnostics/diagnose-aot.sh <file>                # all-in-one AOT diagnostic
  diagnostics/ir-dump.sh <file>                      # annotated LLVM IR
  diagnostics/ir-diff.sh <working.ori> <broken.ori>  # compare IR
  diagnostics/codegen-audit.sh <file>                # static RC balance analysis
  diagnostics/rc-stats.sh <file>                     # RC operation counts per function
  ```
- [ ] **`.claude/rules/compiler.md`** — Add to Tracing section:
  ```
  ## Phase Dumps (debug builds only)
  ORI_DUMP_AFTER_PARSE=1 ori check file.ori      # AST after parse
  ORI_DUMP_AFTER_TYPECK=1 ori check file.ori     # Typed IR after typeck
  ORI_DUMP_AFTER_ARC=1 ori build file.ori        # ARC IR with RC strategies
  ORI_DUMP_AFTER_LLVM=1 ori build file.ori       # Annotated LLVM IR

  ## Runtime Instrumentation (AOT binaries)
  ORI_TRACE_RC=1 ./binary                         # RC event trace
  ORI_RT_DEBUG=1 ./binary                          # Runtime assertions
  ORI_CHECK_LEAKS=1 ./binary                       # Leak check (with attribution)
  ```
- [ ] **`.claude/rules/runtime.md`** — Add debugging env vars:
  ```
  ## Debugging
  - `ORI_TRACE_RC=1` — Logs every RC alloc/inc/dec/free event to stderr
  - `ORI_TRACE_RC=verbose` — Adds backtraces to RC events
  - `ORI_RT_DEBUG=1` — Enables runtime assertions (RC header validation, bounds checks)
  - `ORI_CHECK_LEAKS=1` — Now includes allocation-site attribution for unfreed allocations
  ```
- [ ] **`.claude/rules/tests.md`** — Add debugging test failures section:
  ```
  ## Debugging Test Failures (USE THESE FIRST)
  For AOT test failures: `diagnostics/diagnose-aot.sh test_file.ori`
  For interpreter vs AOT mismatches: `diagnostics/dual-exec-debug.sh test_file.ori`
  For RC leaks: `ORI_TRACE_RC=1 ORI_CHECK_LEAKS=1 ./binary`
  For codegen RC bugs: `diagnostics/codegen-audit.sh test_file.ori`
  ```
- [ ] **`.claude/rules/aot.md`** — Add enhanced LLVM debugging:
  ```
  ## LLVM Debugging (enhanced)
  - `diagnostics/ir-dump.sh file.ori` — Annotated LLVM IR with RC highlighting
  - `diagnostics/ir-diff.sh a.ori b.ori` — Compare IR between programs
  - `diagnostics/codegen-audit.sh file.ori` — Static RC balance analysis
  - `diagnostics/diagnose-aot.sh file.ori` — All-in-one AOT diagnostic
  ```
- [ ] **`.claude/rules/arc.md`** — Add RC debugging tools:
  ```
  ## RC Debugging
  - `ORI_TRACE_RC=1 ./binary` — Event-level RC trace (alloc/inc/dec/free)
  - `diagnostics/rc-stats.sh file.ori` — Count RC ops per function in IR
  - `diagnostics/codegen-audit.sh file.ori` — Detect RC imbalances statically
  - `ORI_DUMP_AFTER_ARC=1 ori build file.ori` — Dump ARC IR with RC decisions
  ```
- [ ] **`.claude/rules/eval.md`** — Add dual-exec debugging reference:
  ```
  ## Debugging Evaluator vs AOT Mismatches
  diagnostics/dual-exec-debug.sh file.ori   # compare interpreter vs AOT output
  ```
- [ ] **`.claude/rules/diagnostic.md`** — Reference new diagnostic infra if applicable
- [ ] **`.claude/rules/ir.md`** — Add phase dump references for IR inspection
- [ ] Audit remaining rules files (`cargo.md`, `code-hygiene.md`, `impl-hygiene.md`, `ori-lang.md`, `ori-syntax.md`, `parse.md`, `patterns.md`, `roadmap.md`, `spec.md`, `typeck.md`, `types.md`) — add diagnostic references where relevant (e.g., `parse.md` should mention `ORI_DUMP_AFTER_PARSE`, `typeck.md` should mention `ORI_DUMP_AFTER_TYPECK`)

### Memory files

- [ ] **Memory file** (`/home/eric/.claude/projects/-home-eric-projects-ori-lang/memory/MEMORY.md`) — Add diagnostic tools quick reference section

---

## 05.3 CI Integration (Optional)

**File(s):** `.github/workflows/` or `test-all.sh`

Optionally integrate diagnostic tools into the CI pipeline or test suite.

- [ ] Add `diagnostics/self-test.sh` to `test-all.sh` as an optional suite (skipped by default, enabled with `--diagnostics`)
- [ ] Add `diagnostics/check-debug-flags.sh` to CI to catch flag drift
- [ ] Consider adding `codegen-audit.sh` in strict mode as a CI gate (catches RC imbalances before merge)
- [ ] Document the CI integration choice in `00-overview.md`

---

## 05.4 Completion Checklist

- [ ] `diagnostics/self-test.sh` passes on all fixture programs
- [ ] CLAUDE.md updated with diagnostics paths, commands, and env vars
- [ ] `.claude/rules/llvm.md` updated with diagnostic script references
- [ ] `.claude/rules/compiler.md` updated with phase dump and runtime instrumentation
- [ ] `.claude/rules/runtime.md` updated with new env vars
- [ ] `.claude/rules/tests.md` updated with diagnostic debugging guidance
- [ ] `.claude/rules/aot.md` updated with enhanced LLVM debugging tools
- [ ] Memory file updated with diagnostic tools reference
- [ ] All scripts have `--help` output
- [ ] `./test-all.sh` green (diagnostic tools don't break anything)

**Exit Criteria:** A developer (or Claude) encountering an AOT bug can find the diagnostic tools within 30 seconds by reading CLAUDE.md or .claude/rules/. Running `diagnostics/diagnose-aot.sh failing_test.ori` produces a comprehensive report. All 6 shell scripts + self-test pass. `check-debug-flags.sh` confirms flag consistency. Documentation is complete across all `.claude/rules/` files.
