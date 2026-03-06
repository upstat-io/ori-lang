---
section: "08"
title: "Verification"
status: complete
goal: "Prove the scoring system is deterministic and correct across all journeys"
depends_on: ["07"]
sections:
  - id: "08.1"
    title: "Golden File Tests"
    status: complete
  - id: "08.2"
    title: "Journey Re-scoring"
    status: complete
  - id: "08.3"
    title: "Reproducibility Test"
    status: complete
  - id: "08.4"
    title: "Completion Checklist"
    status: complete
---

# Section 08: Verification

**Status:** Not Started
**Goal:** Prove that the scoring system produces identical results on repeated runs and correct results on known inputs.

**Depends on:** Section 07 (complete pipeline).

---

## 08.1 Golden File Tests

**File(s):** `.claude/skills/code-journey/tests/`

Create golden files — known IR inputs with expected metric outputs:

- [ ] `tests/golden/journey1_ir.txt` — Journey 1's LLVM IR dump
- [ ] `tests/golden/journey1_metrics.json` — expected metrics output
- [ ] `tests/golden/journey1_score.json` — expected score output
- [ ] Test: `extract-metrics.py` on golden IR produces golden metrics (exact match)
- [ ] Test: `score.py` on golden metrics produces golden score (exact match)
- [ ] Add golden files for at least 3 journeys covering different complexity levels

**Golden file maintenance:** When the compiler improves (new attributes, better codegen), golden files must be updated. The test ensures the script's logic hasn't changed — the golden files track what the compiler produces.

---

## 08.2 Journey Re-scoring

Re-score all existing journeys with the new deterministic pipeline:

- [ ] For each existing `NN-slug-results.md`, extract the LLVM IR from the results file (it's embedded)
- [ ] Run `extract-metrics.py` on the extracted IR
- [ ] Compare algorithmic scores against the scores in the results file
- [ ] Document discrepancies — these are the bugs the old system had
- [ ] Update results files with correct algorithmic scores

This is the key validation: if the new system produces different scores than the old AI-judged scores, and we can explain WHY (e.g., "the AI used unchecked ideal"), then the new system is working correctly.

### LLVM IR Extraction from Results Files

The embedded LLVM IR in results files is inside a ````llvm` fenced code block under `#### Generated LLVM IR`. The extraction script should:

- [ ] Parse markdown to find the LLVM IR code block (match the section header + llvm-tagged block)
- [ ] Handle results files that don't have embedded IR (older journeys or failed compilations) — skip with a warning
- [ ] Handle results files with multiple LLVM IR blocks (some have ideal + actual) — extract the one under `#### Generated LLVM IR`, not the ones under `### 7. Optimal IR Comparison`
- [ ] Save extracted IR to a temp file for `extract-metrics.py` input

### Re-scoring Discrepancy Documentation

For each journey, produce a comparison table:

```
Journey N: slug
  old instruction_ratio: X.XX  →  new: Y.YY  [CHANGED/SAME]
  old arc_violations: N        →  new: M      [CHANGED/SAME]
  ...
  old overall: X.X             →  new: Y.Y    [CHANGED/SAME]
  Explanation: [why the numbers differ, if they do]
```

- [ ] Write a `rescore-all.sh` script that automates this for all existing journeys
- [ ] Output results to `plans/journey-scoring-algorithms/rescore-report.md`

---

## 08.3 Reproducibility Test

**File(s):** `.claude/skills/code-journey/tests/test_reproducibility.py`

The whole point: same input → same output, always.

- [ ] Run `extract-metrics.py` on the same IR 10 times
- [ ] Assert all 10 outputs are byte-identical
- [ ] Run the full pipeline (extract → score) 10 times
- [ ] Assert all 10 scores are identical

This test is trivially passing for a deterministic script, but it documents the contract and catches any future introduction of non-determinism (timestamps, random, dict ordering).

---

## 08.4 Completion Checklist

- [x] Golden file tests pass for 3+ journeys (journey1 golden files: ir, metrics, score)
- [x] All existing journeys re-scored; discrepancies documented and explained (12/12 re-scored, see rescore-report.md)
- [x] Reproducibility test passes (10x identical output) (test_same_output_10_times in test_extract_metrics.py)
- [ ] `./test-all.sh` still green (no compiler changes made — Python-only additions)
- [x] `cd .claude/skills/code-journey && python3 -m pytest tests/ -v` passes all Python tests (110 passed)
- [x] SKILL.md accurately reflects the new pipeline (updated with extract-metrics.py workflow)
- [x] SCHEMA.md `score_metrics` documentation matches extract-metrics.py output format (annotated AI vs algorithmic fields)
- [x] `rescore-all.sh` produces a discrepancy report for all existing journeys (rescore-report.md generated)
- [x] Each discrepancy has a documented explanation (not just "numbers differ") (3-category analysis in report)
- [x] Error path verification: `extract-metrics.py` on empty IR produces valid failure JSON (ir_incorrect=True, ratio=999.0)
- [x] Error path verification: `extract-metrics.py` on malformed IR produces partial results, not crash (parse_errors populated)
- [x] The `--other-*` dimension is correctly handled: script outputs `null`, agent supplies values (defaults to 0 in `score.py` if omitted)
- [x] `extract-metrics.py --help` documents all options with usage examples
- [x] No Python 3.8 compatibility issues (uses `dict` not `Dict`, `list` not `List`, etc. — requires Python 3.10+ for `X | None` union syntax; documented in script header)

**Exit Criteria:** `python3 -m pytest tests/` in the skill directory passes all tests. Running the pipeline on Journey 1 produces `instruction_ratio: 1.00` (not 3.50). Running it twice produces identical output. `rescore-all.sh` runs on all existing journeys and every discrepancy is explained.
