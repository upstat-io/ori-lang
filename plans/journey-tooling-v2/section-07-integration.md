---
section: "07"
title: "Integration and Re-scoring"
status: not-started
goal: "Wire all improvements together, re-score all 12 journeys, and validate zero false positive regressions"
depends_on: ["01", "02", "03", "04", "06"]  # Excludes 05 (Rust verifier, independent track)
sections:
  - id: "07.1"
    title: "Pipeline Integration"
    status: not-started
  - id: "07.2"
    title: "Re-score All Journeys"
    status: not-started
  - id: "07.3"
    title: "Regression Test Suite"
    status: not-started
  - id: "07.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Integration and Re-scoring

**Status:** Not Started
**Goal:** Integrate all improvements from sections 01-06, re-score all 12 journeys, and build a regression test suite that prevents false positives from returning. All journeys should score ≥7.5 with ≤2 documented false positives total.

**Context:** This section is the validation step — it doesn't add new analysis capability but proves the system works end-to-end. The existing `rescore-report.md` documents the V1 algorithm results; this section produces a V2 rescore report showing the false positive reduction.

**Depends on:** Sections 01 (effect summaries), 02 (CFG balance), 03 (cross-function ownership), 04 (IR parser), 06 (attribute compliance). Section 05 (ARC IR verification) is independent and does not affect Python tool scoring.

> **NOTE (incremental integration):** Integration testing can begin after Phase 0
> (Sections 01 + 04). Re-score J8 and J9 after Phase 0 to validate the effect
> summary and quoted-name fixes before proceeding to Phase 1. This provides an
> early gate and reduces risk of discovering Phase 0 issues only at Phase 3.

---

## 07.1 Pipeline Integration

**File(s):** `.claude/skills/code-journey/extract-metrics.py`

- [ ] Update `extract-metrics.py` imports to include new modules:
  ```python
  from effect_summaries import RUNTIME_EFFECTS  # Section 01
  from rc_state import analyze_function_cfg      # Section 02
  from arc_metrics import compute_arc_metrics    # Updated with 01 + 03
  ```

- [ ] Verify `compute_arc_metrics()` calls `get_effect()` for each callee (Section 01) and computes `module_balanced` (Section 03)
- [ ] Add `--verbose` flag to output per-function detail including:
  - Effect summary matches (which runtime functions were recognized)
  - Conditional RC annotations (which operations are behind branches)
  - Ownership transfer annotations (which imbalances are cross-function)

- [ ] Ensure JSON output includes new fields:
  ```json
  {
    "arc_module_balanced": true,
    "arc_ownership_transfers": 3,
    "arc_conditional_ops": 2,
    "arc_effect_matches": ["ori_str_from_raw: +1", "ori_rc_dec: -1"],
    "attr_not_applicable_count": 5
  }
  ```

- [ ] **Update attribute compliance calculation**: When Section 06 marks attributes as not-applicable for closure functions, `total_applicable` decreases (excluded from denominator). This improves the compliance percentage. Verify that `extract-metrics.py` passes through the updated `attr_applicable` count.
- [ ] **Per-function attribute detail**: Include `not_applicable_reason` in per-function output for debugging:
  ```python
  for fm in atm.per_function:
      detail = {"checks": len(fm.checks)}
      if fm.not_applicable_reason:
          detail["not_applicable_reason"] = fm.not_applicable_reason
      per_function.setdefault(fm.name, {})["attribute"] = detail
  ```

---

## 07.2 Re-score All Journeys

**File(s):** `.claude/skills/code-journey/rescore-v2.sh` (new, or manual process)

- [ ] **Extract IR from existing results**: Use `extract_ir_from_results.py` to pull LLVM IR from each journey's `*-results.md` file (journey IR is embedded inside results files, not standalone):
  ```bash
  for results_file in plans/code-journeys/j*/results.md; do
      journey=$(basename $(dirname "$results_file"))
      python3 extract_ir_from_results.py "$results_file" -o "/tmp/journey_ir/${journey}.ll"
  done
  ```
- [ ] Handle missing IR gracefully (some journeys may not have `#### Generated LLVM IR`)
- [ ] Re-run `extract-metrics.py` on all 12 journey IR files
- [ ] Compare V1 vs V2 scores:

  | Journey | V1 Score | V2 Score (target) | V1 Violations | V2 Violations (target) |
  |---------|----------|-------------------|---------------|------------------------|
  | J1 | 9.8 | ≥ 9.8 | 0 | 0 |
  | J5 | 6.7 | ≥ 8.5 | 14 | ≤ 3 |
  | J9 | 7.4 | ≥ 8.5 | 9 | ≤ 2 |
  | J10 | 5.2 | ≥ 7.5 | 15 | ≤ 5 |

- [ ] Document any remaining violations as genuine issues (not false positives)
- [ ] Write `plans/journey-tooling-v2/rescore-v2-report.md` with full results
- [ ] Update `plans/code-journeys/overview.md` with V2 scores

---

## 07.3 Regression Test Suite

**File(s):** `.claude/skills/code-journey/tests/`

Build a test suite that prevents false positives from returning:

- [ ] **Golden file tests**: Store expected `extract-metrics.py` output for each journey:
  ```
  tests/golden/
  ├── j01_metrics.json
  ├── j05_metrics.json  # closure ownership transfer
  ├── j09_metrics.json  # string effect summaries
  └── j10_metrics.json  # list effect summaries
  ```

- [ ] **Synthetic IR tests**: Test each false positive category in isolation (uses `ori_list_alloc_data` which returns `ptr` directly, unlike `ori_str_from_raw` which uses sret):
  ```python
  def test_list_alloc_data_counted_as_allocation():
      """ori_list_alloc_data should count as +1 for RC balance."""
      ir = """
      define fastcc void @_ori_build_list() {
      entry:
        %data = call ptr @ori_list_alloc_data(i64 4, i64 8)
        call void @ori_buffer_rc_dec(ptr %data, i64 0, i64 4, i64 8, ptr null)
        ret void
      }
      declare ptr @ori_list_alloc_data(i64, i64)
      declare void @ori_buffer_rc_dec(ptr, i64, i64, i64, ptr)
      """
      module = parse_module(ir)
      metrics = compute_arc_metrics(module)
      assert metrics.has_unbalanced is False  # +1 from list_alloc_data, -1 from buffer_rc_dec
  ```

- [ ] **Regression guard**: test that J5, J9, J10 produce `arc_has_unbalanced: false` with the new tooling

- [ ] **Full pipeline test**: A single pytest test that:
  1. Takes a known IR text (from a golden file or inline)
  2. Runs `extract_metrics()` (the Python function, not CLI)
  3. Asserts all output fields match expected values
  4. Asserts `parse_errors` is `None` or empty

- [ ] **`score.py` compatibility check**: Verify that `score.py` can consume the new output fields without breaking. The new JSON fields (`arc_module_balanced`, `arc_ownership_transfers`, `arc_conditional_ops`, `arc_effect_matches`) are informational -- `score.py` reads JSON by key and ignores unknown fields. The **semantic change** is that `arc_violations` and `arc_has_unbalanced` values change (fewer violations). Verify that `score.py`'s scoring formulas produce correct results with the reduced violation counts -- this is the intended improvement, not a regression.

- [ ] **Rescore using existing `rescore-all.sh`**: The existing `rescore-all.sh` already handles:
  1. Finding `*-results.md` files
  2. Extracting IR via `extract_ir_from_results.py`
  3. Running `extract-metrics.py`
  4. Running `score.py`
  5. Generating a comparison report

  The V2 rescore should either:
  - Run the existing script as-is (it will automatically use the updated Python modules), OR
  - Create `rescore-v2.sh` that adds the `--output-dir` for V2-specific reporting

  The report output goes to `plans/journey-scoring-algorithms/rescore-report.md` — the V2 report should go to `plans/journey-tooling-v2/rescore-v2-report.md`.

---

## 07.4 Completion Checklist

- [ ] `extract-metrics.py` integrates all improvements from sections 01-06
- [ ] All 12 journeys re-scored with V2 algorithms
- [ ] All journeys score ≥ 7.5
- [ ] ≤ 2 documented false positives across all journeys total
- [ ] Golden file tests for J5, J9, J10 (the previously affected journeys)
- [ ] Synthetic IR tests for each false positive category
- [ ] Full pipeline end-to-end test (IR -> parse -> metrics -> score)
- [ ] `score.py` compatibility verified (new JSON fields ignored, reduced `arc_violations` produces higher scores)
- [ ] `rescore-v2.sh` script created and tested
- [ ] IR extraction from `*-results.md` files works for all 12 journeys
- [ ] `rescore-v2-report.md` written with full comparison
- [ ] `python3 -m pytest tests/` passes (all test files)

**Exit Criteria:** Running `extract-metrics.py` on all 12 journey IR files produces scores where every journey is ≥7.5, with J5 ≥8.5, J9 ≥8.5, and J10 ≥7.5. The `rescore-v2-report.md` shows the V1→V2 improvement for each journey and documents any remaining violations as genuine codegen issues.
