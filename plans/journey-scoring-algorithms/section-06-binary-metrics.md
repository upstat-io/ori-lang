---
section: "06"
title: "Binary Metrics"
status: complete
goal: "Extract binary quality metrics from exit codes, section sizes, and disassembly"
depends_on: []
sections:
  - id: "06.1"
    title: "Exit Code Verification"
    status: complete
  - id: "06.2"
    title: "Section Size Extraction"
    status: not-started
  - id: "06.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Binary Metrics

**Status:** Not Started
**Goal:** Compute binary quality metrics deterministically from exit codes, ELF section sizes, and disassembly output — no IR parsing needed.

**Context:** Binary metrics are the simplest dimension — they come from running the binary and inspecting it with standard tools. The hard-fail gate (wrong output, crash, eval/AOT mismatch) is the most important binary metric.

**Depends on:** Nothing — this section parses binary analysis output, not IR.

---

## 06.1 Exit Code Verification

**File(s):** `.claude/skills/code-journey/binary_metrics.py`

The most important metric: does the binary produce the correct result?

- [ ] Read eval exit code and AOT exit code from journey temp files
- [ ] Compare against expected value
- [ ] Detect mismatch between eval and AOT (differential testing)
- [ ] Set `hard_fail = True` if: wrong eval output, wrong AOT output, eval/AOT mismatch, or crash (exit code > 128)
- [ ] Handle missing binary (compilation failed): set `aot_exit = None`, `hard_fail = True`, `defects = 3` (crash severity). The binary analysis section (06.2) should be skipped entirely — `binary_size`, `text_size`, `rodata_size` are all `None`.
- [ ] Handle the case where eval succeeds but AOT fails to compile: `eval_correct = True`, `aot_correct = False`, `paths_match = False`, `hard_fail = True`

```python
@dataclass
class BinaryMetrics:
    eval_exit: int
    aot_exit: int
    expected: int
    eval_correct: bool
    aot_correct: bool
    paths_match: bool       # eval == aot
    hard_fail: bool         # Any of the above wrong
    defects: int            # Weighted: crash=3, mismatch=3, wrong=1
    # Section sizes (optional, informational)
    binary_size: int | None
    text_size: int | None
    rodata_size: int | None
```

---

## 06.2 Section Size Extraction

**File(s):** `.claude/skills/code-journey/binary_metrics.py`

Parse output of `size -A binary` to extract section sizes. This is informational (included in narrative) but doesn't affect the score.

- [ ] Parse `size -A` output for `.text`, `.rodata`, `.debug_info` sizes
- [ ] Parse `objdump -d` output to count bytes of user functions (`_ori_*`)
- [ ] Compute user code percentage of `.text`

These are included in the metrics JSON for the AI's narrative but don't feed into `score.py`.

> **Design note:** `binary_metrics.py` parses **pre-collected output** from `size -A` and `objdump -d` (passed as file paths via `--sections-file` and `--disasm-file`). It does NOT invoke these commands itself. The journey runner (or `extract-metrics.py`) is responsible for running the commands and saving output to temp files. This keeps `binary_metrics.py` pure (input → output, no side effects) and testable without a compiled binary.

---

## 06.3 Completion Checklist

- [ ] Journey 1 scores `bin_defects=0, bin_hard_fail=false`
- [ ] Mismatch detection works when eval and AOT differ
- [ ] Crash detection works (exit code > 128)
- [ ] Section size parsing handles `size -A` output format
- [ ] Output matches `score.py` format (`--bin-defects`, `--bin-hard-fail`)

**Exit Criteria:** `compute_binary_metrics(eval_exit=33, aot_exit=33, expected=33)` returns `defects=0, hard_fail=false`. With `eval_exit=33, aot_exit=34, expected=33`, it returns `hard_fail=true` with `defects >= 1` (wrong AOT output = 1, plus eval/AOT mismatch = 3; exact total depends on whether violations are additive or max-of).
