---
section: "01"
title: "LCFail Audit & Baseline"
status: not-started
reviewed: true
goal: "Categorize all 3,956 LCFail tests by root cause, fix stale roadmap numbers, and establish the baseline for LCFail elimination tracking."
inspired_by: []
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Fix Stale Roadmap Numbers"
    status: not-started
  - id: "01.2"
    title: "LCFail Root Cause Categorization"
    status: not-started
  - id: "01.3"
    title: "File-Level Failure Analysis"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: LCFail Audit & Baseline

**Status:** Not Started
**Goal:** Every LCFail test is categorized by root cause and mapped to a specific Section 21A subsection. The roadmap reflects accurate current numbers. A tracking mechanism exists to monitor progress toward LCFail=0.

**Context:** The roadmap's Section 21A reports 1,985 LCFail tests, but the actual count is 3,956 — nearly double. This stale data makes it impossible to prioritize effectively. Before reprioritizing (Section 02), we need an accurate inventory of what's failing and why.

**Depends on:** Nothing — this is the starting point.

---

## 01.1 Fix Stale Roadmap Numbers

**File(s):** `plans/roadmap/section-21A-llvm.md`

The "Current Test Results" table at line 72-76 of `section-21A-llvm.md` is wrong:

```markdown
# CURRENT (STALE):
| Ori spec (evaluator)    | 3035 | 0 | 42 | -    | 3077 |
| Ori spec (LLVM backend) | 1082 | 1 |  9 | 1985 | 3077 |
| Rust unit tests (LLVM)  |  527 | 0 | 15 | -    |  542 |
```

- [ ] Update the test results table with actual current numbers:
  ```markdown
  # CORRECTED (2026-03-25):
  | Ori spec (interpreter)  | 4181 | 0 | 42 | -    | 4223 |
  | Ori spec (LLVM backend) |  257 | 0 | 10 | 3956 | 4223 |
  ```
  Note: The evaluator test count grew from 3,035→4,181 (new spec tests added). The LLVM pass count *dropped* from 1,082→257 (likely due to changes in codegen that broke previously-passing files, or test reorganization).

- [ ] Update ALL occurrences of the stale "2,472" `assert_eq` count in Section 21A. Actual count is **3,946** (verified 2026-03-25 via `grep -r "assert_eq" tests/spec/ | wc -l`). Three locations to update:
  - Line 93: `"used in 2,472 test call sites"` → `"used in 3,946 test call sites"`
  - Line 371: `"affects 2,472+ assert_eq call sites"` → `"affects 3,946+ assert_eq call sites"`
  - Line 1084: `"blocks 2,472+ test call sites"` → `"blocks 3,946+ test call sites"`

- [ ] Add a "Last Updated" annotation to the test results table so staleness is visible.

---

## 01.2 LCFail Root Cause Categorization

**File(s):** New tracking table in this section (analysis output, not committed code)

LCFail is a **file-level** failure: when LLVM codegen fails on a module, ALL tests in that file become LCFail. The root cause is always a missing codegen feature. Categorization maps each failing file to its blocking feature.

- [ ] Run the LLVM spec tests with verbose error output to capture the actual compilation error for each failing file:
  ```bash
  ./target/release/ori test --verbose --backend=llvm tests/ 2>&1 | tee lcfail-audit.log
  ```

- [ ] Parse `lcfail-audit.log` to extract per-file error messages. The LLVM backend reports errors in two forms:
  - `"LLVM compilation failed: <msg>"` — from `compile_module_with_tests()` returning `Err` (codegen error with structured message)
  - `"LLVM backend error: <msg>"` — from `catch_unwind` catching a panic (LLVM fatal error, assertion failure, etc.)
  Typical codegen error messages include:
  - Missing codegen for expression types
  - Type lowering failures
  - Function resolution failures (often generic functions skipped by `sig.is_generic()`)
  - IR verification failures (from `fn_val.verify(true)` inside compilation)

- [ ] Categorize each failing file into one of these root cause buckets (mapped to 21A subsections):

  | Category | 21A Subsection | Typical Error |
  |----------|---------------|---------------|
  | Generic monomorphization | 21.7 (Function Sequences & Expressions) | `sig.is_generic()` skips in `declare_all`/`define_all` — any file calling `assert_eq<T>` |
  | Sum type codegen | 21.2 (Type Lowering) | `Ordering` type, prelude `compare()` return type |
  | Lambda/closure ABI | 21.11 (Lambda & Closure Support) | Function type parameters, closure captures |
  | Operator traits + impl blocks | 21.4 (Operator Trait Dispatch) | Struct methods, operator trait dispatch for user types |
  | Built-in functions | 21.12 (Built-in Functions) | Missing prelude function implementations |
  | Control flow | 21.5 (Control Flow) | for-yield, try, catch expressions |
  | Pattern matching | 21.6 (Pattern Matching) | match expressions, destructuring |
  | Collections/iterators | 21.10 (Collections & Iterators) | List/map/set operations, iterator trait |
  | Type lowering | 21.2 (Type Lowering) | Struct layouts, channel types, fixed-capacity lists |
  | Expression codegen | 21.3 (Expression Codegen) | Missing operators, conversions, string ops |
  | Other | various | Capabilities, FFI, concurrency |

- [ ] For each category, count:
  - Number of **files** blocked (primary metric — since LCFail is file-level)
  - Number of **test functions** blocked (secondary metric — total LCFail impact)
  - Whether fixing THIS feature alone would unblock the file, or if the file has multiple blocking features

- [ ] Identify **cascade files**: files that fail due to multiple missing features. These won't be unblocked by any single feature and will be the last to pass. Track them separately.

- [ ] Record the categorization as a table:
  ```markdown
  | Category | Files Blocked | Tests Blocked | 21A Subsection | Cascade? |
  |----------|--------------|---------------|----------------|----------|
  | Generic monomorphization | ??? | ??? | 21.7 | Many files ALSO need this |
  | Sum types | ??? | ??? | 21.2 | ... |
  | ... | ... | ... | ... | ... |
  ```

### Test Strategy

- **Matrix**: This section is analysis, not code. Verification is by checking that every LCFail file appears in exactly one primary category.
- **Semantic pin**: The categorization table itself is the deliverable. It becomes stale if not updated, so Section 02 creates a tracking mechanism.

---

## 01.3 File-Level Failure Analysis

**File(s):** Analysis of `compiler/oric/src/test/runner/llvm_backend.rs`

Understanding HOW LCFail works is essential for tracking. Key facts from `llvm_backend.rs`:

1. The JIT test runner (`run_file_llvm`) compiles all functions in a file into a single JIT module via `compile_module_with_tests()`.
2. Compilation is wrapped in `std::panic::catch_unwind`. If compilation returns `Err(e)` (codegen error) OR panics (LLVM fatal error), ALL tests in the file become `TestOutcome::LlvmCompileFail(msg)`.
3. The error message is the codegen error's `.message` field (for `Err`) or the panic payload string (for panics). There is no direct `LLVMVerifyModule` call in the test runner — verification happens inside `compile_module_with_tests` via `fn_val.verify(true)`.
4. LCFail does NOT count as a test failure — `exit_code()` returns 0 even with LCFail tests (they are filtered out of `has_failures()`).

- [ ] Verify that the current LCFail mechanism correctly reports the blocking error per file (not just "compilation failed" but the specific LLVM error).

- [ ] Check whether the current error messages are specific enough to categorize root causes. If error messages are too generic (e.g., just "LLVM compilation failed"), we may need to enhance the error reporting in `llvm_backend.rs` to include which function/expression caused the failure.

- [ ] Document the LCFail tracking infrastructure:
  - Where LCFail is defined: `compiler/oric/src/test/result/mod.rs:20` (`TestOutcome::LlvmCompileFail(String)`)
  - Where LCFail is produced: `compiler/oric/src/test/runner/llvm_backend.rs` (lines 272-314, in `Ok(Err(e))` and `Err(panic_info)` arms of `compile_result` match)
  - Where LCFail is counted: `FileSummary.llvm_compile_fail` (line 121 of `result/mod.rs`), `TestSummary.llvm_compile_fail` (line 178 of `result/mod.rs`)
  - Where LCFail is displayed: `compiler/oric/src/commands/test.rs` (lines 171-172 for test count, lines 179/201-202 for file count)
  - How LCFail interacts with exit codes: `TestSummary::exit_code()` (line 226 of `result/mod.rs`) — LCFail does NOT cause exit code 1 (failure); `has_failures()` only checks `failed > 0 || error_files > 0`. However, if the ONLY results are LCFail (total passed+failed+skipped == 0 AND error_files == 0), `exit_code()` returns 2 ("no tests found") because the `total() == 0` guard fires first — note the guard also checks `llvm_compile_fail == 0 && llvm_compile_fail_files == 0`, so a fully-LCFail suite returns exit code 0, NOT 2. Only truly empty runs return 2.

- [ ] **Tracking mechanism decision**: For automated LCFail tracking, choose one:
  - **(a) Parse existing verbose output** (recommended for now): The `--verbose` flag already shows per-file errors. A shell script can parse `./target/release/ori test --verbose --backend=llvm tests/` output to extract per-file error messages and counts. No code changes needed.
  - **(b) Add `--lcfail-details` flag** (better long-term): Machine-readable JSON output of per-file LCFail errors. Requires changes to the test runner. Only pursue if (a) proves insufficient for Section 02's milestone tracking.

  Implement option (a) as a script in `scripts/lcfail-report.sh`:
  ```bash
  #!/bin/bash
  # scripts/lcfail-report.sh — LCFail tracking and regression detection
  # Usage: ./scripts/lcfail-report.sh [--check] [--update-baseline]
  #
  # Default: prints current LCFail count and per-category breakdown
  # --check: compare against test-baselines/lcfail-count.txt, exit 1 if regression
  # --update-baseline: write current count to test-baselines/lcfail-count.txt
  ```
  The script runs `./target/release/ori test --backend=llvm tests/` (requires pre-built release binary), parses the summary line for the LCFail count, and optionally parses `--verbose` output for per-file error categories. Exit 0 for informational mode, exit 1 for `--check` mode when count increased.
  Section 02.2 and Section 06.1 will use this script for tracking.

### Test Strategy

- **Matrix**: N/A — this subsection is documentation of existing infrastructure.
- **TDD**: For the `scripts/lcfail-report.sh` script:
  - Script produces a count that matches the summary line's LCFail number
  - Script lists per-file error categories
  - Script exits 0 (informational, not a test gate)

---

## 01.R Third Party Review Findings

- None.

---

## 01.4 Completion Checklist

- [ ] Section 21A test results table updated with correct numbers (4,181/257/3,956)
- [ ] Every LCFail file categorized into exactly one primary root cause bucket
- [ ] Categorization table includes file counts and test function counts per category
- [ ] Cascade files (multiple blockers) identified and tracked separately
- [ ] LCFail infrastructure documented (where defined, produced, counted, displayed)
- [ ] `scripts/lcfail-report.sh` implemented and produces counts matching the test runner's summary line
- [ ] `./test-all.sh` green (no regressions from roadmap updates)

**Exit Criteria:** A complete categorization table exists mapping all ~188 LCFail files to their primary root cause bucket, with counts per category. The roadmap reflects accurate test numbers. The categorization data is sufficient for Section 02 to create an impact-ordered implementation sequence.
