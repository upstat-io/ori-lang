---
section: "06"
title: "Verification"
status: not-started
reviewed: false
goal: "Confirm LCFail tracking infrastructure is in place and the 30s performance target is met, with regression guards to prevent backsliding."
inspired_by:
  - "Rust perf.rust-lang.org — continuous performance regression detection"
depends_on: ["01", "02", "03", "04", "05"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "LCFail Tracking Verification"
    status: not-started
  - id: "06.2"
    title: "Performance Target Verification"
    status: not-started
  - id: "06.3"
    title: "Regression Guards"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Verification

**Status:** Not Started
**Goal:** Both plan objectives are measurably achieved: LCFail tracking is in place with accurate data and milestones, and `cargo t` wall time is ≤30s. Regression guards prevent backsliding on both fronts.

**Context:** This section is the gate — it verifies that all prior sections achieved their objectives and installs mechanisms to prevent regression.

**Depends on:** All prior sections (01-05).

---

## 06.1 LCFail Tracking Verification

Verify that the LCFail audit and roadmap reprioritization are complete and accurate.

- [ ] Run the LLVM spec tests and verify the count matches what's recorded in the updated Section 21A:
  ```bash
  # Requires release binary: cargo build --release
  ./target/release/ori test --verbose --backend=llvm tests/ 2>&1 | tail -5
  ```
  Expected output includes a summary line with the current LCFail count. The `--backend=llvm` flag routes test execution through the JIT backend in `compiler/oric/src/test/runner/llvm_backend.rs`.

- [ ] Verify Section 21A's test results table has been updated (from Section 01.1):
  - Old stale number (1,985) does NOT appear anywhere in the roadmap
  - Old stale `assert_eq` count (2,472) does NOT appear anywhere in the roadmap (3 occurrences existed at lines 93, 371, 1084)
  - New numbers match reality (run `./scripts/lcfail-report.sh` and `grep -r "assert_eq" tests/spec/ | wc -l`)
  - "Last Updated" annotation present

- [ ] Verify Section 21A has priority annotations (from Section 02.1):
  - Each subsection (21.1-21.19) has a `LCFail Priority: P{N}` annotation
  - Priority ordering matches the impact analysis from Section 01.2

- [ ] Verify LCFail milestones exist (from Section 02.2):
  - Milestone table with targets: <1500, <1000, <500, <200, <50, 0
  - Gate tests defined for each milestone
  - Tracking section with current count and update instructions

- [ ] Verify the categorization data from Section 01.2 is recorded and accessible.

---

## 06.2 Performance Target Verification

Verify that the 30s target is met.

- [ ] Run the canonical measurement from Section 03.1:
  ```bash
  ./scripts/bench-tests.sh --runs 5
  ```
  Record: wall time mean, user time mean, system time mean, stddev.

- [ ] Compare with the baseline from Section 03.1:
  ```
  Before: ???s +/- ???
  After:  ???s +/- ???
  Improvement: ???%
  Target: 30s
  Met: yes/no
  ```

- [ ] If the 30s target is NOT met, document:
  - What was achieved (actual wall time)
  - What the remaining bottleneck is (from profiling)
  - What further optimization would be needed
  - Whether the target should be revised

- [ ] Run per-crate timing and compare with baseline:
  | Crate | Before | After | Improvement |
  |-------|--------|-------|-------------|
  | ori_llvm (AOT) | 35.6s | ??? | ??? |
  | ori_arc | 3.4s | ??? | ??? |
  | ... | ... | ... | ... |

- [ ] Verify the improvements are real (not measurement noise):
  - Each improvement exceeds 2x the stddev
  - Results are reproducible across multiple runs

---

## 06.3 Regression Guards

Install mechanisms to prevent both LCFail count and test performance from regressing.

- [ ] **LCFail regression guard**: Extend `scripts/lcfail-report.sh` (from Section 01.3) with a `--check` mode that:
  - Reads current LCFail count (via LLVM backend spec tests)
  - Compares against stored baseline (`test-baselines/lcfail-count.txt`)
  - Returns non-zero exit if LCFail count INCREASED (regression detected)
  - Optionally add a call to `scripts/lcfail-report.sh --check` in `test-all.sh`'s summary phase

- [ ] **Performance regression guard**: Add to `scripts/bench-tests.sh`:
  - Compare current wall time against stored baseline
  - Warn if wall time increases by more than 15% from baseline
  - Implementation: store baseline in `test-baselines/perf-baseline.json`

- [ ] **Create `test-baselines/` directory** (does not currently exist — verified 2026-03-25):
  ```bash
  mkdir -p test-baselines
  ```
  Populate with:
  - `test-baselines/lcfail-count.txt` — single integer: the current LCFail count (e.g., `3956`)
  - `test-baselines/perf-baseline.json` — JSON: `{"wall_time_s": 59.0, "user_time_s": 323.0, "system_time_s": 537.0, "date": "2026-03-25", "runs": 5}`
  - `test-baselines/README.md` — update instructions (see below)
  Add `test-baselines/` to `.gitignore`? **No** — baselines should be committed so CI and other developers can compare.

- [ ] **Document the update process** in `test-baselines/README.md`:
  - When to update: after intentional changes that affect LCFail count (new codegen features) or timing (performance optimizations)
  - How to update LCFail: `./scripts/lcfail-report.sh --update-baseline`
  - How to update perf: `./scripts/bench-tests.sh --update-baseline`
  - How to check: `./scripts/lcfail-report.sh --check` and `./scripts/bench-tests.sh --check`
  - **Format**: Keep `lcfail-count.txt` as a single integer for easy `cat` + comparison. Keep `perf-baseline.json` as structured JSON for programmatic access.

### Test Strategy

- **TDD ordering**: The regression guard scripts are new code that must be tested:
  - [ ] Write the regression guard scripts with `--check` mode BEFORE creating the baselines
  - [ ] Verify `--check` returns non-zero when no baseline file exists (expected: fails gracefully with helpful message)
  - [ ] Create baseline files
  - [ ] Verify `--check` returns 0 with matching baselines
- **Matrix**: The regression guards are tested across these scenarios:
  - LCFail guard: (a) baseline matches reality: exit 0, (b) count increased: exit 1, (c) count decreased: exit 0 with "improved" message, (d) no baseline file: exit 1 with instructions
  - Performance guard: (a) within 15%: exit 0, (b) regression >15%: exit 0 with warning, (c) no baseline file: exit 0 with "no baseline" note
- **Semantic pin**: Intentionally increase LCFail count (temporarily break a test) and verify guard fires. Revert and verify guard stops firing. This is the semantic pin for the LCFail guard.
- **LCFail guard behavior**: The LCFail regression guard MUST fail (non-zero exit) if LCFail count increases from baseline. This is blocking -- an LCFail regression means a previously-compiling codegen path broke. The guard script (`scripts/lcfail-report.sh`) returns non-zero on regression.
- **Performance guard behavior**: The performance regression guard warns (exit 0) if wall time increases by >15%. Performance regressions have legitimate causes (new tests, new features) and require human judgment. The guard produces a clear warning message but does not block.
- **Baseline update**: Both guards support `--update-baseline` to record a new baseline after intentional changes.
- **Debug and release**: `timeout 150 cargo t` (debug) AND `timeout 150 cargo t --release` (release) must pass after all verification section changes. `./test-all.sh` must also pass.

---

## 06.R Third Party Review Findings

- None.

---

## 06.4 Completion Checklist

- [ ] LCFail tracking verified: roadmap numbers correct, milestones in place, categorization recorded
- [ ] Performance target measured: `cargo t` wall time ≤30s (or documented gap with rationale)
- [ ] Per-component improvements documented (table with before/after/improvement per crate)
- [ ] LCFail regression guard installed and tested
- [ ] Performance regression guard installed and tested
- [ ] Baselines directory created with current values
- [ ] Update process documented
- [ ] `timeout 150 cargo t` green
- [ ] `./test-all.sh` green
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

**Exit Criteria:** Running `./scripts/bench-tests.sh` reports wall time ≤30s with consistent results. The LCFail count is accurately tracked in the roadmap with a clear path to zero. Regression guards will alert if either metric backslides. The plan is complete.
