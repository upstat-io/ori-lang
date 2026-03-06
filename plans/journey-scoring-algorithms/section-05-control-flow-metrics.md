---
section: "05"
title: "Control Flow Metrics"
status: complete
goal: "Count control flow defects (empty blocks, redundant branches, trivial phi nodes)"
depends_on: ["01"]
sections:
  - id: "05.1"
    title: "Defect Detection"
    status: complete
  - id: "05.2"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Control Flow Metrics

**Status:** Not Started
**Goal:** Deterministically count control flow defects across all user functions, producing the defect count and incorrectness flag for `score.py`.

**Context:** Control flow defects are structural — they can be detected by examining the block/instruction graph without understanding LLVM semantics deeply.

**Depends on:** Section 01 (IR parser provides basic blocks per function) and `ir_utils.py` (shared utilities for redundant branch and trivial phi detection, also used by section 02).

---

## 05.1 Defect Detection

**File(s):** `.claude/skills/code-journey/control_flow_metrics.py`

- [ ] **Empty blocks**: A block whose only instruction is an unconditional `br label %X`. These should be eliminated by merging with the target.
  ```python
  def is_empty_block(block: BasicBlock) -> bool:
      return (len(block.instructions) == 1
              and block.instructions[0].opcode == "br"
              and "i1" not in block.instructions[0].text)  # not conditional
  ```

- [ ] **Redundant unconditional branches**: A `br label %X` where `%X` is the immediately following block in the function layout. These are fall-through jumps that serve no purpose. **Implementation:** Import `is_redundant_unconditional_branch()` from **`ir_utils.py`** (shared with section 02). Do NOT duplicate the implementation. Section 02 counts redundant branches as "unjustified instructions"; section 05 counts them as "control flow defects." This intentional dual-counting is documented in section 02.
  ```python
  def is_redundant_branch(block: BasicBlock, next_block: BasicBlock | None) -> bool:
      if not block.instructions:
          return False
      last = block.instructions[-1]
      if last.opcode != "br" or "i1" in last.text:
          return False  # conditional branch — not redundant
      target = extract_branch_target(last.text)
      return next_block is not None and target == next_block.label
  ```

- [ ] **Trivial phi nodes**: A `phi` instruction with a single incoming value, or all incoming values are the same register. Should be replaced with the value directly. **Implementation:** Import `is_trivial_phi()` from **`ir_utils.py`** (shared with section 02).
  ```python
  # In ir_utils.py:
  def is_trivial_phi(instr: Instruction) -> bool:
      if instr.opcode != "phi":
          return False
      values = extract_phi_values(instr.text)
      return len(values) <= 1 or len(set(values)) == 1
  ```

- [ ] **Semantic incorrectness flag**: Set if any block is unreachable (no predecessors and not the entry block), or if a conditional branch has both targets the same.
> **NOTE:** The `is_incorrect` flag from this section feeds both `--cf-incorrect` (control flow dimension gate) and `--ir-incorrect` (IR quality dimension gate) in `score.py`. Both gates cap their respective dimension at 1 if set. The `extract-metrics.py` integration (section 07) must pass this flag to both CLI arguments.

- [ ] Implement `extract_branch_target(text: str) -> str` in **`ir_utils.py`** — parse `br label %X` to extract `%X`. Must handle both `br label %name` and `br i1 %cond, label %true, label %false` (for conditional, return both targets). Used by `is_redundant_branch()` and predecessor analysis.
- [ ] Implement `extract_phi_values(text: str) -> list[str]` in **`ir_utils.py`** — parse `phi type [ val1, %block1 ], [ val2, %block2 ]` to extract `[val1, val2]`. Used by `is_trivial_phi()`.
- [ ] Handle predecessor analysis for unreachable block detection: scan all branch/switch instructions to build a predecessor map. Blocks with no predecessors (except the entry block) are unreachable.
- [ ] Handle `switch` terminators (not just `br`): `switch i64 %val, label %default [ i64 0, label %case0 i64 1, label %case1 ]` — needed for pattern matching journeys.

```python
@dataclass
class ControlFlowMetrics:
    per_function: list[FunctionCfMetrics]
    total_defects: int      # Sum of all defects
    is_incorrect: bool      # Any semantic incorrectness

@dataclass
class FunctionCfMetrics:
    name: str
    block_count: int
    empty_blocks: int
    redundant_branches: int
    trivial_phis: int
    total_defects: int      # Sum of above
```

---

## 05.2 Completion Checklist

- [ ] Journey 1 (post-block-merge) scores `cf_defects=0, cf_incorrect=false`
- [ ] Synthetic IR with empty block → detected as defect
- [ ] Synthetic IR with redundant branch → detected as defect
- [ ] Output matches `score.py` format: `--cf-defects`, `--cf-incorrect` (control flow dimension) AND `--ir-incorrect` (IR quality dimension, same flag)

**Exit Criteria:** `compute_control_flow_metrics()` on Journey 1's current IR returns zero defects. On IR with injected empty blocks and redundant branches, defect count matches hand-count.
