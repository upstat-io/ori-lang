---
section: "07"
title: "Integration"
status: complete
goal: "Wire all metric extractors into a single extract-metrics.py that feeds score.py"
depends_on: ["01", "02", "03", "04", "05", "06"]
sections:
  - id: "07.1"
    title: "Unified extract-metrics.py"
    status: complete
  - id: "07.2"
    title: "Pipeline Integration"
    status: not-started
  - id: "07.2b"
    title: "Migration Strategy"
    status: not-started
  - id: "07.3"
    title: "SKILL.md Update"
    status: not-started
  - id: "07.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Integration

**Status:** Not Started
**Goal:** Combine all metric extractors into a single `extract-metrics.py` script that reads journey artifacts and outputs the complete set of metrics for `score.py`, then update SKILL.md to mandate its use.

**Context:** The individual metric extractors (sections 02-06) each compute one dimension. This section wires them together so the journey runner invokes one command that produces all metrics.

**Depends on:** Sections 01-06 (all metric extractors).

---

## 07.1 Unified extract-metrics.py

**File(s):** `.claude/skills/code-journey/extract-metrics.py`

Single entry point that reads journey artifacts and produces a metrics JSON:

```python
#!/usr/bin/env python3
"""Extract all journey metrics deterministically from compiler artifacts.

Usage:
    python3 extract-metrics.py \
      --ir-file /tmp/journey_N/llvm_ir.txt \
      --eval-exit 33 \
      --aot-exit 33 \
      --expected 33 \
      [--binary /tmp/journey_N/binary] \
      [--sections-file /tmp/journey_N/sections.txt] \
      [--disasm-file /tmp/journey_N/disasm.txt]

Output: JSON with all metrics ready for score.py input.
"""
```

- [ ] Accept IR dump file path (required), exit codes (required), binary analysis files (optional)
- [ ] Call `parse_module()` then `compute_instruction_metrics()`, `compute_arc_metrics()`, `compute_attribute_metrics()`, `compute_control_flow_metrics()`, `compute_binary_metrics()` (the 5 metric extractors are independent of each other and can run in any order after parsing)
- [ ] Map extractor outputs to score.py's 7 dimensions (note that 7 score dimensions come from 5 extractors + 1 AI dimension):
  - `instruction_metrics.avg_ratio` -> `--instruction-ratio`
  - `instruction_metrics.max_ratio` -> `--instruction-ratio-max`
  - `instruction_metrics.total_unjustified` -> `--ir-unjustified` (IR Quality dimension)
  - `arc_metrics.total_violations` -> `--arc-violations`
  - `arc_metrics.has_unbalanced` -> `--arc-has-unbalanced` (gate flag)
  - `arc_metrics.has_scalar_rc` -> `--arc-has-scalar-rc` (gate flag)
  - `attribute_metrics.total_applicable` -> `--attr-applicable`
  - `attribute_metrics.total_correct` -> `--attr-correct`
  - `attribute_metrics.has_wrong` -> `--attr-has-wrong` (gate flag)
  - `control_flow_metrics.total_defects` -> `--cf-defects`
  - `control_flow_metrics.is_incorrect` -> `--cf-incorrect` AND `--ir-incorrect` (both gates)
  - `binary_metrics.defects` -> `--bin-defects`
  - `binary_metrics.hard_fail` -> `--bin-hard-fail`
  - `other_*` -> AI-determined (see section 07.2)
- [ ] Output JSON:
  ```json
  {
    "instruction_ratio": 1.0,
    "instruction_ratio_max": 1.0,
    "arc_violations": 0,
    "arc_has_unbalanced": false,
    "arc_has_scalar_rc": false,
    "attr_applicable": 5,
    "attr_correct": 5,
    "attr_has_wrong": false,
    "cf_defects": 0,
    "cf_incorrect": false,
    "ir_unjustified": 0,
    "ir_incorrect": false,
    "bin_defects": 0,
    "bin_hard_fail": false,
    "other_critical": null,
    "other_high": null,
    "other_low": null,
    "per_function": { ... },
    "binary_info": { ... }
  }
  ```
- [ ] Also output `score.py` command line to stderr for easy copy-paste:
  ```
  python3 score.py --instruction-ratio 1.00 --instruction-ratio-max 1.00 ...
  ```

### Error Handling

- [ ] **Empty IR file**: If `--ir-file` points to an empty file (compilation failed, no output), `extract-metrics.py` must:
  - Set `ir_incorrect = true` (gate: IR quality capped at 1)
  - Set all IR-derived metrics to worst-case: `instruction_ratio = 999.0`, `arc_violations = 0`, `attr_applicable = 0`, `attr_correct = 0`, `cf_defects = 0`
  - Output a clear error message to stderr: `"ERROR: IR file is empty (compilation likely failed). IR-derived metrics set to failure defaults."`
  - Exit 0 (produce valid JSON with failure metrics — don't block the pipeline)
- [ ] **Malformed IR**: If the IR parser encounters lines it can't parse, collect parse errors in `Module.parse_errors` and continue. The metrics should be computed from whatever was successfully parsed. Include `parse_errors` in the output JSON for diagnostic purposes.
- [ ] **Missing binary files**: If `--binary`, `--sections-file`, or `--disasm-file` are omitted, binary metrics (06.2) are skipped — `binary_size`, `text_size`, `rodata_size` are `null` in the output JSON. This is not an error.
- [ ] **IR file not found**: Exit 1 with error message to stderr. Do NOT produce partial JSON.
- [ ] **Exit codes**: Exit 0 on success (even with failure metrics). Exit 1 on invalid arguments or missing required files. Exit 2 on internal errors (unexpected exceptions).

---

## 07.2 Pipeline Integration

**File(s):** `.claude/skills/code-journey/SKILL.md`

Update the journey execution pipeline so the background agent uses the scripts:

**Current flow (broken):**
```
AI reads IR → AI counts things → AI feeds counts to score.py
```

**New flow:**
```
extract-metrics.py reads IR → outputs metrics.json → score.py reads metrics → outputs scores
AI reads metrics.json for narrative context
```

- [ ] Add step in SKILL.md Step 3 (background agent): run `extract-metrics.py` before analysis
- [ ] Background agent reads `metrics.json` for its narrative (instruction counts, etc.)
- [ ] Background agent runs `score.py` with metrics from the JSON (or piped directly)
- [ ] Background agent MUST NOT override any metric from extract-metrics.py
- [ ] The `--other-critical`, `--other-high`, `--other-low` args remain AI-determined (these are from journey-specific categories that can't be algorithmically scored)

### The "Other Findings" Dimension (7th Scoring Dimension)

The "Other Findings" dimension (15% weight) is the ONE dimension that remains AI-determined. It scores findings from journey-specific categories (Cat 8+) that don't map to the 6 algorithmic dimensions.

**How it works in the new pipeline:**
1. `extract-metrics.py` computes 6 dimensions algorithmically and outputs them in `metrics.json`
2. The `metrics.json` output includes `"other_critical": null, "other_high": null, "other_low": null` — signaling that these values must be supplied by the background agent
3. The background agent reads `metrics.json`, performs its journey-specific analysis (Cat 8+), and determines the `--other-*` counts based on what it finds
4. The background agent passes ALL metrics (6 from script + 3 other counts from its analysis) to `score.py`

**Why `null` instead of `0`:** Outputting `null` makes it explicit that the script does not compute this dimension — the agent must. If the agent passes `null` values through to `score.py`, `score.py`'s `--other-*` defaults to 0 (no penalty), which is the safe fallback.

- [ ] `extract-metrics.py` outputs `other_critical`, `other_high`, `other_low` as `null` in JSON (not 0), signaling the agent must supply these values
- [ ] Document in `extract-metrics.py --help` that `null` means "AI-determined, not yet supplied"
- [ ] Pipeline integration: agent reads `null` fields, replaces with its analysis counts, passes to `score.py` (which defaults omitted `--other-*` args to 0)

---

## 07.2b Migration Strategy

Existing journey results were scored with the old AI-judged pipeline. The transition must handle:

### Backward Compatibility

- [ ] Existing `*-results.md` files contain `score_metrics` frontmatter with the raw inputs to `score.py`. These are the AI's counts — they may differ from the algorithmic counts.
- [ ] The `score_metrics` block in frontmatter is already structured for `score.py` input — no format change needed.
- [ ] Existing journeys do NOT need their embedded LLVM IR re-extracted — the IR is already in the results files under `#### Generated LLVM IR`.

### Migration Steps

- [ ] For each existing journey: extract the LLVM IR from the results file, run `extract-metrics.py`, compare against the `score_metrics` in frontmatter
- [ ] Document every discrepancy as a validation finding (e.g., "Journey 1: AI scored instruction_ratio as 3.50, script computes 1.00 — AI was using unchecked ideal")
- [ ] Update `score_metrics` in frontmatter to match the script's output
- [ ] Re-run `score.py` with the updated metrics to get corrected scores
- [ ] Update `score` and `score_breakdown` in frontmatter to match

### Handling Missing IR

Some journey results may not have embedded LLVM IR (if the journey predates the schema or if compilation failed). For these:
- [ ] Re-run the journey code through the compiler to regenerate the IR
- [ ] If the code no longer compiles (compiler changes), note in the results file that re-scoring is blocked and why

### SKILL.md Pipeline Transition

- [ ] The old pipeline (AI counts → `score.py`) must continue to work during the transition — `score.py` CLI interface is unchanged
- [ ] The new pipeline (`extract-metrics.py` → `score.py`) is strictly additive — it doesn't break the old flow
- [ ] Once all journeys are re-scored and verified, the SKILL.md instructions switch to mandate the new pipeline

## 07.3 SKILL.md Update

**File(s):** `.claude/skills/code-journey/SKILL.md`

Update the "Scoring" section to reflect the new pipeline:

- [ ] Add "CRITICAL: Use extract-metrics.py" section before "How to Score"
- [ ] Document the exact command to run
- [ ] Make clear: 6 of 7 score dimensions come from the script, only "Other Findings" is AI-determined
- [ ] Update the background agent prompt template to include the extract-metrics step
- [ ] Add: "If extract-metrics.py produces different numbers than your manual analysis, the SCRIPT IS CORRECT. Investigate why your analysis differs."

### Sync Points — Files Created or Modified

| File | Action | Section |
|------|--------|---------|
| `.claude/skills/code-journey/ir_parser.py` | **CREATE** | 01 |
| `.claude/skills/code-journey/ir_utils.py` | **CREATE** | 02, 05 (shared) |
| `.claude/skills/code-journey/instruction_metrics.py` | **CREATE** | 02 |
| `.claude/skills/code-journey/arc_metrics.py` | **CREATE** | 03 |
| `.claude/skills/code-journey/attribute_metrics.py` | **CREATE** | 04 |
| `.claude/skills/code-journey/control_flow_metrics.py` | **CREATE** | 05 |
| `.claude/skills/code-journey/binary_metrics.py` | **CREATE** | 06 |
| `.claude/skills/code-journey/extract-metrics.py` | **CREATE** | 07 |
| `.claude/skills/code-journey/SKILL.md` | **MODIFY** | 07 |
| `.claude/skills/code-journey/tests/` | **CREATE** (directory) | 01 |
| `.claude/skills/code-journey/tests/test_ir_parser.py` | **CREATE** | 01 |
| `.claude/skills/code-journey/tests/test_instruction_metrics.py` | **CREATE** | 02 |
| `.claude/skills/code-journey/tests/test_arc_metrics.py` | **CREATE** | 03 |
| `.claude/skills/code-journey/tests/test_attribute_metrics.py` | **CREATE** | 04 |
| `.claude/skills/code-journey/tests/test_control_flow_metrics.py` | **CREATE** | 05 |
| `.claude/skills/code-journey/tests/test_binary_metrics.py` | **CREATE** | 06 |
| `.claude/skills/code-journey/tests/test_extract_metrics.py` | **CREATE** | 07 |
| `.claude/skills/code-journey/tests/golden/` | **CREATE** (directory) | 08 |
| `.claude/skills/code-journey/tests/golden/journey1_ir.txt` | **CREATE** | 08 |
| `.claude/skills/code-journey/tests/golden/journey1_metrics.json` | **CREATE** | 08 |
| `.claude/skills/code-journey/tests/golden/journey1_score.json` | **CREATE** | 08 |
| `.claude/skills/code-journey/tests/test_reproducibility.py` | **CREATE** | 08 |
| `plans/journey-scoring-algorithms/rescore-report.md` | **CREATE** | 08 |

**NOT modified:** `score.py` (its CLI interface is unchanged), `SCHEMA.md` (format spec unchanged, though `score_metrics` documentation may get a clarifying note about `other_*` fields).

---

## 07.4 Completion Checklist

- [ ] `extract-metrics.py` runs on Journey 1 artifacts and produces correct JSON
- [ ] `extract-metrics.py | score.py` pipeline produces the correct score
- [ ] SKILL.md updated with new pipeline instructions
- [ ] Background agent prompt template includes extract-metrics step
- [ ] End-to-end: re-run Journey 1 with updated skill → score matches expected
- [ ] `extract-metrics.py` on empty IR file produces valid JSON with failure defaults (does not crash)
- [ ] `extract-metrics.py` on malformed IR produces partial results with `parse_errors` in JSON
- [ ] `extract-metrics.py` with missing `--binary` arg produces JSON with `null` binary metrics
- [ ] Existing journey results re-scored; discrepancies documented
- [ ] Add a comment in SCHEMA.md's `score_metrics` example noting that `other_critical/high/low` are AI-determined (the only 3 fields not produced by `extract-metrics.py`)

**Exit Criteria:** Running `python3 extract-metrics.py --ir-file journey1_ir.txt --eval-exit 33 --aot-exit 33 --expected 33 | python3 score.py [args from output]` produces `overall: 10.0` (or the correct score given current compiler output). Running with an empty `--ir-file` produces a valid JSON with `ir_incorrect: true` and does not crash.
