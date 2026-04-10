---
section: "12"
title: "Verification Dashboard & Regression Tracking"
status: not-started
reviewed: false
goal: "Capture IR baselines for ≥20 key programs, build comparison tooling to detect any IR shape change, integrate llvm-reduce for automatic test case reduction, and track verification trends over time — so that IR regressions are detected at commit time, not after user-visible bugs"
success_criteria:
  - "scripts/ir-baseline.sh --capture creates golden IR baselines for ≥20 key programs"
  - "scripts/ir-baseline.sh --compare detects any IR shape change vs captured baseline"
  - "scripts/ir-baseline.sh --bless updates baselines when changes are intentional"
  - "llvm-reduce integration in diagnostics/reduce-ir.sh reduces failing IR to minimal reproducer"
  - "Trend data (pass counts, verification findings, snapshot diff rates) captured as CI artifacts"
  - "Nightly CI compares against baselines and reports IR regressions"
inspired_by:
  - "Rust perf.rust-lang.org — historical performance tracking across commits with regression detection"
  - "Ori scripts/perf-baseline.sh — existing baseline capture/compare pattern for performance metrics"
  - "LLVM llvm-reduce (llvm-project/llvm/tools/llvm-reduce/) — automatic IR test case reduction"
depends_on: ["11"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "12.1"
    title: "IR Baseline Capture and Comparison"
    status: not-started
  - id: "12.2"
    title: "llvm-reduce Integration"
    status: not-started
  - id: "12.3"
    title: "Trend Tracking and CI Artifacts"
    status: not-started
  - id: "12.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "12.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 12: Verification Dashboard & Regression Tracking

**Status:** Not Started
**Goal:** Build IR baseline infrastructure that captures golden LLVM IR for key programs, detects any IR shape change on subsequent builds, and provides `--bless` mode for intentional updates. Integrate `llvm-reduce` for automatic test case reduction when IR regressions are found. Track verification trends (pass counts, verification findings, snapshot diff rates) over time via CI artifacts. This is the lowest-priority section — the "dashboard" is deliberately minimal, focusing on actionable regression detection rather than a UI.

**Success Criteria:**

- [ ] IR baselines captured for ≥20 key programs — satisfies mission criterion: "IR regression tracking"
- [ ] `--compare` mode detects IR shape changes — satisfies mission criterion: "IR shape regression detection"
- [ ] `llvm-reduce` reduces failing IR — satisfies mission criterion: "automatic test case reduction"
- [ ] Trend data captured in CI artifacts — satisfies mission criterion: "historical comparison"

**Context:** This section complements the FileCheck IR assertions from Section 07. FileCheck tests verify specific IR patterns (e.g., "RC inc/dec pair appears for this function") — they catch pattern violations. IR baselines catch ANY IR change, not just pattern violations. A new optimization that changes the IR shape without violating any CHECK pattern is invisible to FileCheck but caught by baseline comparison. This dual approach — pattern assertions for known-important shapes, full baselines for unexpected changes — provides comprehensive IR regression coverage. The baseline infrastructure follows the pattern already established by `scripts/perf-baseline.sh` (performance baselines) but applies it to IR shape.

**Reference implementations:**
- **Ori** `scripts/perf-baseline.sh`: Existing baseline capture/compare pattern with `--release` flag and structured output. The IR baseline script follows the same UX conventions.
- **Rust** `perf.rust-lang.org`: Historical tracking of metrics across commits with automatic regression detection and bisection.
- **LLVM** `llvm/tools/llvm-reduce/`: Automatic delta-debugging tool that reduces LLVM IR to the minimal reproducing case for a given "interestingness test" script.

**Depends on:** Section 11 (CI produces the artifacts that baselines track; nightly/weekly jobs provide the execution cadence for baseline comparison).

---

## 12.1 IR Baseline Capture and Comparison

**File(s):** `scripts/ir-baseline.sh`, `tests/baselines/` (golden IR directory)

Create the baseline capture/compare infrastructure. This is the core of the section — everything else builds on it.

- [ ] Create `scripts/ir-baseline.sh` following the `perf-baseline.sh` pattern:
  ```bash
  # Usage: scripts/ir-baseline.sh [MODE] [OPTIONS]
  #
  # Modes:
  #   --capture        Capture golden IR baselines for all key programs
  #   --compare        Compare current IR against captured baselines
  #   --bless          Update baselines to match current IR (intentional changes)
  #   --list           List all baselined programs and their status
  #
  # Options:
  #   --release        Use release build (default: debug)
  #   --program FILE   Capture/compare only the specified program
  #   --json           Machine-readable output
  #   --verbose        Show full diff on mismatch
  #   --no-color       Disable color output
  ```

- [ ] Define the key program list (`tests/baselines/programs.txt`). Select ≥20 programs that collectively exercise the major codegen patterns:
  ```
  # Format: <ori_file_path> <description>
  tests/spec/types/int/basic.ori                   # Integer arithmetic
  tests/spec/types/str/concat.ori                  # String operations + RC
  tests/spec/types/list/map_filter.ori             # Collection operations
  tests/spec/traits/iterator/basic.ori             # Iterator protocol
  tests/spec/traits/iterator/chain.ori             # Iterator chaining
  tests/spec/collections/cow/basic_cow.ori         # COW patterns
  tests/spec/types/closures/capture.ori            # Closure codegen
  tests/spec/types/enum/match_exhaustive.ori       # Enum dispatch
  tests/spec/types/struct/nested.ori               # Struct layout + fields
  tests/spec/types/option/map_unwrap.ori           # Option handling
  tests/spec/types/result/error_propagation.ori    # Result + ? operator
  tests/spec/traits/derive/eq_comparable.ori       # Derived trait codegen
  tests/spec/functions/recursive.ori               # Recursion + tail calls
  tests/spec/functions/higher_order.ori            # Higher-order functions
  tests/spec/patterns/nested_match.ori             # Complex pattern matching
  tests/spec/types/tuple/access.ori                # Tuple codegen
  tests/spec/types/map/basic.ori                   # Map operations
  tests/spec/types/set/basic.ori                   # Set operations
  tests/spec/loops/for_yield.ori                   # For-yield comprehensions
  tests/spec/loops/while_break.ori                 # While + break
  ```
  The exact file paths will be determined at implementation time based on what exists. The criteria are: each program exercises a distinct codegen path, and together they cover RC emission, COW, closures, iterators, pattern matching, and control flow.

- [ ] Implement baseline capture (`--capture`):
  1. For each program in the list, compile with `ORI_DUMP_AFTER_LLVM=1 cargo run -- build <file>`
  2. Capture the LLVM IR from stderr
  3. Normalize the IR (see normalization rules below)
  4. Write to `tests/baselines/<program_name>.baseline.ll`

- [ ] Implement IR normalization for stable comparison. Raw LLVM IR contains elements that change across builds without semantic significance:
  - **Metadata IDs**: `!0`, `!1`, ... — strip or renumber sequentially
  - **Unnamed temporaries**: `%0`, `%1`, ... — already sequential in LLVM output, keep as-is
  - **Module-level metadata**: `source_filename`, `target datalayout`, `target triple` — strip (machine-specific)
  - **Debug info**: `!dbg !N` references — strip entirely (debug info is orthogonal to codegen correctness)
  - **Alignment attributes**: Keep (alignment changes are semantically significant)
  - **Function attributes**: Keep attribute groups but strip group numbers (`#0`, `#1`) and renumber

- [ ] Implement baseline comparison (`--compare`):
  1. For each program, capture current IR (same as `--capture`)
  2. Load the golden baseline
  3. Diff the normalized IR
  4. Report mismatches with unified diff output
  5. Exit code: 0 = all match, 1 = mismatches found, 2 = error

- [ ] Implement bless mode (`--bless`):
  1. Capture current IR for all programs (or `--program` target)
  2. Overwrite the golden baselines
  3. Report which baselines were updated

- [ ] **Subsection close-out (12.1)** — MANDATORY before starting 12.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging journey for 12.1 specifically: IR normalization edge cases, baseline path management, diff readability. Implement every accepted improvement NOW and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(scripts): ...`).

---

## 12.2 llvm-reduce Integration

**File(s):** `diagnostics/reduce-ir.sh`, `diagnostics/reduce-tests/` (interestingness test scripts)

Integrate LLVM's `llvm-reduce` tool for automatic test case reduction. When an IR regression is found (via baseline comparison, FileCheck failure, or manual investigation), `llvm-reduce` shrinks the failing LLVM IR to the minimum reproducing case — saving hours of manual reduction.

- [ ] Create `diagnostics/reduce-ir.sh` following diagnostic conventions:
  ```bash
  # Usage: diagnostics/reduce-ir.sh <file.ll> --test <interestingness_test> [OPTIONS]
  #
  # Reduces LLVM IR to the minimal reproducing case using llvm-reduce.
  #
  # Options:
  #   --test SCRIPT     Interestingness test script (REQUIRED)
  #   --output FILE     Output path for reduced IR (default: <input>.reduced.ll)
  #   --timeout SECS    Per-test timeout (default: 30)
  #   --verbose         Show llvm-reduce progress
  #
  # Built-in interestingness tests:
  #   --crash           Reduce to minimal crash reproducer
  #   --wrong-output    Reduce to minimal wrong-output case (requires --expected)
  #   --leak            Reduce to minimal leak case
  ```

- [ ] Create interestingness test templates in `diagnostics/reduce-tests/`:
  - `crash-test.sh` — passes if the input IR crashes the compiler/runtime
  - `wrong-output-test.sh` — passes if the compiled IR produces output different from expected
  - `leak-test.sh` — passes if `ORI_CHECK_LEAKS=1` reports leaks
  - `pattern-test.sh` — passes if a specific IR pattern (grep) is present/absent

- [ ] Verify `llvm-reduce` is available on the system (it ships with LLVM 21):
  ```bash
  if ! command -v llvm-reduce-21 &>/dev/null && ! command -v llvm-reduce &>/dev/null; then
      echo "ERROR: llvm-reduce not found. Install LLVM 21 tools."
      exit 2
  fi
  ```

- [ ] Add the script to `diagnostics/self-test.sh` with a synthetic test case (a known-reducible IR file where the interestingness test checks for a specific pattern).

- [ ] **TPR checkpoint** — `/tpr-review` covering 12.1–12.2 implementation work

- [ ] **Subsection close-out (12.2)** — MANDATORY before starting 12.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 12.1's close-out, scoped to 12.2's debugging journey. Commit improvements separately using a valid conventional-commit type.

---

## 12.3 Trend Tracking and CI Artifacts

**File(s):** `scripts/verification-summary.sh`, `.github/workflows/nightly-verification.yml` (artifact upload)

Capture verification metrics over time so that trends (increasing false positives, decreasing pass counts, growing IR size) are visible. This is the minimal "dashboard" — a structured JSON summary uploaded as a CI artifact, not a web UI.

- [ ] Create `scripts/verification-summary.sh` that aggregates metrics from all verification tools:
  ```bash
  # Usage: scripts/verification-summary.sh [--json] [--verbose]
  #
  # Collects metrics from:
  #   - test-all.sh results (pass/fail/skip counts)
  #   - AIMS snapshot diffs (changed/unchanged counts)
  #   - FileCheck test results (pass/fail counts)
  #   - IR baseline comparison (matched/changed/missing counts)
  #   - Alive2 results (verified/timeout/suppressed/failed counts)
  #   - Fuzzing metrics (corpus size, executions/sec, crashes found)
  #
  # Output: JSON summary suitable for historical comparison
  ```

- [ ] Define the summary JSON format:
  ```json
  {
    "timestamp": "2026-04-10T02:00:00Z",
    "commit": "abc123",
    "metrics": {
      "tests": { "rust_pass": 1234, "rust_fail": 0, "ori_pass": 1677, "ori_fail": 0 },
      "aims_snapshots": { "matched": 15, "changed": 0, "missing": 0 },
      "filecheck": { "pass": 30, "fail": 0 },
      "ir_baselines": { "matched": 20, "changed": 0, "missing": 0 },
      "alive2": { "verified": 15, "timeout": 2, "suppressed": 3, "failed": 0 },
      "fuzzing": { "corpus_size": 5000, "execs_per_sec": 150, "crashes": 0 }
    }
  }
  ```

- [ ] Add summary capture to the nightly CI workflow:
  ```yaml
  - name: Capture verification summary
    run: scripts/verification-summary.sh --json > verification-summary.json

  - uses: actions/upload-artifact@v4
    with:
      name: verification-summary-${{ github.sha }}
      path: verification-summary.json
      retention-days: 365  # Keep for trend analysis
  ```

- [ ] Add IR baseline comparison to the nightly CI:
  ```yaml
  - name: IR baseline comparison
    run: scripts/ir-baseline.sh --compare --json > ir-baseline-results.json

  - uses: actions/upload-artifact@v4
    with:
      name: ir-baselines-${{ github.sha }}
      path: ir-baseline-results.json
  ```

- [ ] Add a simple trend detection check to the nightly job. Compare the current summary against the previous day's summary (downloaded from CI artifacts):
  - If any metric worsened (e.g., `ir_baselines.changed > 0` when yesterday it was 0), add a warning annotation to the workflow run
  - If a critical metric crossed a threshold (e.g., `alive2.failed > 0`), fail the job
  - This is NOT a full dashboard — it is a regression detector

- [ ] **Subsection close-out (12.3)** — MANDATORY before starting 12.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 12.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- all items in this block are marked resolved
- `third_party_review.status` becomes `resolved` or `none`
-->

- None.

---

## 12.N Completion Checklist

- [ ] `scripts/ir-baseline.sh` exists with `--capture`, `--compare`, `--bless`, `--list` modes
- [ ] IR normalization handles metadata, debug info, module-level metadata, attribute groups
- [ ] Golden IR baselines captured for ≥20 key programs in `tests/baselines/`
- [ ] `--compare` detects IR shape changes with clear unified diff output
- [ ] `--bless` updates baselines when changes are intentional
- [ ] `diagnostics/reduce-ir.sh` wraps llvm-reduce with built-in interestingness tests
- [ ] Interestingness test templates for crash, wrong-output, leak, and pattern modes
- [ ] `scripts/verification-summary.sh` aggregates metrics from all verification tools
- [ ] Summary JSON uploaded as nightly CI artifact with 365-day retention
- [ ] IR baseline comparison runs in nightly CI
- [ ] Trend detection warns on metric regression
- [ ] Both new scripts added to `diagnostics/self-test.sh`
- [ ] No existing tests regressed: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 12` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for this section
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** `scripts/ir-baseline.sh --compare` runs against ≥20 golden IR baselines with zero unexpected changes. `--bless` updates baselines cleanly. `diagnostics/reduce-ir.sh` reduces failing IR to minimal reproducers using llvm-reduce with built-in interestingness tests. `scripts/verification-summary.sh` produces a structured JSON summary aggregating all verification metrics. Nightly CI uploads baselines and summaries as artifacts with 365-day retention. Simple trend detection warns on metric regressions. All scripts follow diagnostic conventions and are registered in `diagnostics/self-test.sh`.
