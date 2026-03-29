---
section: "06"
title: "Verification"
status: not-started
reviewed: false
goal: "Prove measurable improvement across all parser frontend benchmarks with zero regressions."
inspired_by:
  - "Ori .claude/skills/create-plan/plan-schema.md — Verification Section Schema"
depends_on: ["01", "02", "03", "04", "05"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Benchmark Comparison"
    status: not-started
  - id: "06.2"
    title: "Regression Testing"
    status: not-started
  - id: "06.3"
    title: "Documentation"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Verification

**Status:** Not Started
**Goal:** All parser frontend benchmarks show measurable improvement compared to Section 01 baselines. All test suites green. Findings documented for future reference.

**Context:** This section re-measures all benchmarks established in Section 01 after all optimization work (Sections 02-05) is complete. The before/after comparison proves the plan delivered measurable value and identifies which optimizations had the most impact.

**Depends on:** All prior sections (01-05).

---

## 06.1 Benchmark Comparison

**File(s):** `plans/parser-perf/baselines.json`

Re-run the complete benchmark suite and compare against Section 01 baselines.

- [ ] Run all benchmarks with `--baseline parser-perf-v0` to compare against saved baselines:
  ```bash
  cargo bench -p oric --bench lexer_core -- --baseline parser-perf-v0
  cargo bench -p oric --bench lexer -- --baseline parser-perf-v0
  cargo bench -p oric --bench parser -- --baseline parser-perf-v0
  ```

- [ ] Record final metrics in `baselines.json` alongside original baselines:
  ```json
  {
    "final_results": {
      "date": "YYYY-MM-DD",
      "commit": "<hash>",
      "lexer_core_raw_throughput_mib_s": null,
      "lexer_cooked_throughput_mib_s": null,
      "parser_raw_throughput_mib_s": null,
      "parser_salsa_throughput_mib_s": null,
      "salsa_overhead_percent": null
    },
    "improvement": {
      "lexer_cooked_percent": null,
      "parser_raw_percent": null,
      "parser_salsa_percent": null,
      "salsa_overhead_delta": null
    }
  }
  ```

- [ ] Verify new workloads from Section 01.2 are included in the final run:
  - `parser/salsa_overhead/raw` and `parser/salsa_overhead/via_salsa` (Salsa isolation)
  - String-heavy workload (in `lexer.rs`)
  - Expression-heavy workload (in `parser.rs`)

- [ ] Identify which optimizations had the most impact:
  - Cross-crate inlining (Sections 03.1, 04.1)
  - Arena pre-allocation (Sections 03.2, 04.2)
  - Cooker/parser micro-optimizations (Sections 03.3, 04.4)
  - Incremental parsing (Section 05.2)

- [ ] Verify no benchmark regressions: every benchmark must be equal or better than baseline.

---

## 06.2 Regression Testing

Run the complete test suite to verify zero behavioral regressions.

- [ ] `timeout 150 ./test-all.sh` — all tests pass
- [ ] `./clippy-all.sh` — no warnings
- [ ] `./fmt-all.sh` — no formatting changes needed
- [ ] `timeout 150 cargo st` — all Ori spec tests pass
- [ ] `timeout 150 cargo t -p ori_lexer` — all lexer tests pass
- [ ] `timeout 150 cargo t -p ori_parse` — all parser tests pass
- [ ] `timeout 150 cargo t -p oric` — all Salsa integration tests pass
- [ ] Verify debug AND release builds:
  ```bash
  timeout 150 cargo t
  cargo b --release  # build first (no timeout on compilation)
  timeout 150 cargo t --release  # test execution only, respects mandatory 150s timeout
  ```

---

## 06.3 Documentation

Update project documentation to reflect changes and record findings.

- [ ] Update CLAUDE.md `Parser Performance` memory section with new baseline numbers.
- [ ] Update memory file `/home/eric/.claude/projects/-home-eric-projects-ori-lang/memory/MEMORY.md` with new benchmark results.
- [ ] If file splits changed crate structure, update `.claude/rules/parse.md` key files section.
- [ ] If incremental parsing was activated, document the `ParseCache` / `parse_cache()` pattern in `.claude/rules/compiler.md`.
- [ ] If incremental parsing was activated, document `compute_text_change()` in `.claude/rules/parse.md` incremental section.

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] All benchmarks re-measured against Section 01 baselines
- [ ] Improvement percentages recorded in `baselines.json`
- [ ] No benchmark regressions detected
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `./fmt-all.sh` clean
- [ ] Debug and release builds pass all tests
- [ ] CLAUDE.md and memory updated with new baselines
- [ ] Plan documentation updated
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

**Exit Criteria:** `baselines.json` contains populated before/after results showing measurable improvement. All test suites green in both debug and release modes. Documentation updated. `parser_raw_throughput_mib_s` final > `parser_raw_throughput_mib_s` baseline.
