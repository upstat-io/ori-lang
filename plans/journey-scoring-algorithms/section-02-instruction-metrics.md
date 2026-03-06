---
section: "02"
title: "Instruction Efficiency Metrics"
status: complete
goal: "Algorithmically compute instruction ratio by recognizing justified overhead patterns in IR"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Justified Overhead Patterns"
    status: complete
  - id: "02.2"
    title: "Ideal Computation Algorithm"
    status: not-started
  - id: "02.3"
    title: "Ratio Calculation"
    status: not-started
  - id: "02.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Instruction Efficiency Metrics

**Status:** Not Started
**Goal:** Compute the instruction efficiency ratio (`actual / ideal`) deterministically by algorithmically identifying justified overhead patterns in the IR, so that "ideal" has exactly one definition regardless of who runs the tool.

**Context:** This is the metric that caused the score regression. The AI defined "ideal" two different ways on successive runs. The fix: define "ideal" as `actual - unjustified_overhead`, where "justified" and "unjustified" are determined by pattern matching, not judgment.

**Key insight:** Ori's codegen emits specific, recognizable patterns for overflow checking, panic paths, and ABI compliance. Each pattern has a fixed instruction cost. We can count these patterns, compute the "core logic" instructions, and derive the ratio algorithmically.

**Depends on:** Section 01 (IR parser provides `Module` with per-function instruction lists) and `ir_utils.py` (shared utilities for `is_redundant_unconditional_branch()` and `is_trivial_phi()`, also used by section 05).

---

## 02.1 Justified Overhead Patterns

**File(s):** `.claude/skills/code-journey/instruction_metrics.py`

Define the set of patterns that are **justified** — mandatory overhead from Ori's semantics that should not penalize the score. Each pattern has a fixed instruction count.

### Pattern: Overflow-Checked Arithmetic (6 instructions per operation)

Actual Ori codegen for one checked arithmetic operation (verified from `ORI_DUMP_AFTER_LLVM=1`):

```llvm
; Pattern: one checked arithmetic operation = 6 instructions
; Hot path (4 instructions):
%add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %0, i64 %1)               ; 1
%add.val = extractvalue { i64, i1 } %add, 0                                         ; 2
%add.ovf = extractvalue { i64, i1 } %add, 1                                         ; 3
br i1 %add.ovf, label %add.ovf_panic, label %add.ok                                 ; 4
; Cold panic block (2 instructions):
call void @ori_panic_cstr(ptr @ovf.msg)                                              ; 5
unreachable                                                                          ; 6
```

That's 6 instructions per checked operation: 4 in the hot path block and 2 in the cold panic block.

**Note:** The panic function is `@ori_panic_cstr` (NOT `@ori_panic_overflow` — that function does not exist). It takes a `ptr` to a C string constant like `@ovf.msg`. The block labels follow the pattern `%name.ok` and `%name.ovf_panic` (e.g., `add.ok`, `add.ovf_panic`).

**For ideal computation:** The ideal IR for `a + b` in Ori IS the overflow-checked pattern. So the pattern cost is part of ideal, not overhead above it.

- [ ] Define `OVERFLOW_CHECK_PATTERN`: regex matching `call.*@llvm\.s(add|sub|mul)\.with\.overflow`
- [ ] Define `PANIC_CALL_PATTERN`: regex matching `call.*@ori_panic_cstr`
- [ ] Count overflow-checked operations per function
- [ ] Each operation contributes: 4 hot-path instructions + 2 cold-path instructions = 6 total

### Pattern: ABI Wrapper (entry point)

```llvm
; @main wrapper: always 3 instructions (actual codegen output)
; Function Attrs: nounwind
define i32 @main() #N {
entry:
  %ori_main_result = call i64 @_ori_main()   ; 1
  %exit_code = trunc i64 %ori_main_result to i32  ; 2
  ret i32 %exit_code                         ; 3
}
```

**Note:** `@main` has `nounwind` but NOT `fastcc` (C ABI entry point). It calls `@_ori_main` without `fastcc` because `@_ori_main` itself uses default calling convention (it is the entry-called function).

- [ ] Recognize `@main` wrapper by: name is `@main`, contains exactly one `call @_ori_main`, one `trunc`, one `ret`
- [ ] Wrapper is excluded from user function scoring (it's boilerplate)

### Pattern: Function Preamble

Every function has at minimum:
- `ret` — 1 instruction (always present)

Non-overhead. This is part of core logic.

- [ ] Document which patterns are NOT justified overhead (and therefore count as bloat if extra):
  - `alloca` + `store` + `load` chains for values that could be SSA
  - `br label %next` unconditional branches between consecutive blocks
  - Redundant `bitcast` or `getelementptr` chains
  - Dead code after `unreachable` or `ret`

---

## 02.2 Ideal Computation Algorithm

**File(s):** `.claude/skills/code-journey/instruction_metrics.py`

The ideal instruction count for a function is computed algorithmically — NOT by writing ideal IR by hand.

**Algorithm:**

```
ideal(function) = actual(function) - unjustified(function)
```

Where `unjustified` counts instructions that are present in the actual IR but serve no purpose:

```python
def compute_unjustified(func: Function) -> int:
    """Count instructions that shouldn't be in the IR."""
    unjustified = 0
    for idx, block in enumerate(func.blocks):
        next_block = func.blocks[idx + 1] if idx + 1 < len(func.blocks) else None
        # Block-level checks:
        # Unconditional branch to the immediately following block (1 wasted instruction)
        if is_redundant_branch(block, next_block):  # from ir_utils.py
            unjustified += 1
        # Per-instruction checks:
        for i, instr in enumerate(block.instructions):
            # alloca/store/load chain for a value used exactly once (should be SSA)
            if is_unnecessary_alloca_chain(instr, block, func):
                unjustified += 1
            # Dead code after noreturn call or unreachable
            if is_dead_after_noreturn(instr, i, block):
                unjustified += 1
            # Trivial phi with single predecessor or all-same values
            if is_trivial_phi(instr):  # from ir_utils.py
                unjustified += 1
    return unjustified
```

**Key property:** `ideal >= 1` always (at minimum `ret`). If `unjustified == 0`, the function is OPTIMAL and `ideal == actual`, giving ratio 1.00x.

### Functions With No Arithmetic (Pure Control Flow)

Functions like `@_ori_main` in Journey 1 have no overflow-checked arithmetic — just a call and a return. For these functions, the ideal computation is straightforward: every instruction is either core logic or unjustified overhead. No overflow-checking patterns to classify.

**Edge cases:**
- **Empty function** (only `ret void`): `actual=1, ideal=1, ratio=1.00x`
- **Pure call function** (call + ret): `actual=2, ideal=2, ratio=1.00x` (no overhead expected)
- **Identity function** (ret param): `actual=1, ideal=1, ratio=1.00x`
- **Function with only control flow** (branches, phis, no arithmetic): ideal = actual - unjustified (same algorithm; unjustified still counts redundant branches, trivial phis, etc.)

The algorithm handles these naturally because `compute_unjustified()` examines structural waste, not arithmetic patterns. No arithmetic means no overflow checking to classify — but also no arithmetic-specific waste.

- [ ] Implement `compute_unjustified(func)` with the patterns above
- [ ] Implement `is_redundant_branch(block, next_block)` in **`ir_utils.py`** (shared with section 05) — returns `True` if the block's last instruction is `br label %X` where `%X` is `next_block.label`. Both `instruction_metrics.py` and `control_flow_metrics.py` import from `ir_utils.py`.
- [ ] Implement `is_unnecessary_alloca_chain()` — conservative: only flag `alloca` that has exactly one `store` and one `load` with no other uses
- [ ] Implement `is_dead_after_noreturn()` — instructions after `call @noreturn_func` + `unreachable` in the same block
- [ ] Implement `is_trivial_phi()` in **`ir_utils.py`** (shared with section 05) — `phi` with one incoming value or all incoming values the same

> **WARNING (complexity):** `is_unnecessary_alloca_chain()` is the hardest pattern to detect reliably. It requires tracking which `alloca` values are stored to and loaded from exactly once, which means building a use-def chain across the entire function. For the initial implementation, consider a conservative heuristic: flag only `alloca` + immediate `store` + immediate `load` in the same block with no intervening uses. Full use-def analysis can be added later if needed.

**Design decision: why `ideal = actual - unjustified` instead of writing ideal IR:**

Option (a) **Subtractive** (recommended): `ideal = actual - unjustified`. Deterministic — the algorithm examines actual IR and identifies waste. No ambiguity about what "ideal" looks like. If the codegen improves, ideal automatically adjusts.

Option (b) **Constructive**: write "ideal IR" per-function and count. This is what the AI was doing — and it failed because different runs wrote different ideals. A script would need to synthesize ideal IR from the actual IR, which is equivalent to option (a) but harder.

Option (c) **Template-based**: define per-operation instruction costs and sum them. E.g., "checked add = 6, call = 1, ret = 1." Problem: doesn't account for SSA flow between operations, phi nodes at join points, etc. Too rigid for complex functions.

**Recommended:** Option (a). The instruction ratio measures codegen quality — "how much waste is in this IR?" Counting waste directly answers that question.

---

## 02.3 Ratio Calculation

**File(s):** `.claude/skills/code-journey/instruction_metrics.py`

- [ ] Compute per-function: `actual`, `ideal`, `ratio = actual / ideal`, `verdict`
- [ ] Compute module-level weighted average: `sum(actual_i) / sum(ideal_i)` across user functions
- [ ] Compute module-level max ratio: `max(actual_i / ideal_i)`
- [ ] Exclude the `@main` ABI wrapper from scoring
- [ ] Handle division by zero in ratio: if `ideal == 0` (impossible since `ideal >= 1`), but guard defensively — clamp to `ideal = max(1, actual - unjustified)`

```python
@dataclass
class FunctionMetrics:
    name: str
    actual: int           # Total instructions in function
    unjustified: int      # Instructions flagged as waste
    ideal: int            # actual - unjustified
    ratio: float          # actual / ideal (>= 1.0)
    verdict: str          # OPTIMAL, NEAR-OPTIMAL, ACCEPTABLE, BLOATED, WASTEFUL

@dataclass
class InstructionMetrics:
    per_function: list[FunctionMetrics]
    avg_ratio: float      # Weighted average (feeds score.py --instruction-ratio)
    max_ratio: float      # Worst function (feeds score.py --instruction-ratio-max)
    total_unjustified: int  # Sum of unjustified across all user functions (feeds --ir-unjustified)
```

Verdict thresholds (matching SCHEMA.md):
- OPTIMAL: 1.0x
- NEAR-OPTIMAL: 1.01x–1.50x
- ACCEPTABLE: 1.51x–2.50x
- BLOATED: 2.51x–5.00x
- WASTEFUL: >5.00x

- [ ] Unit test: Journey 1's IR produces ratio 1.00x (zero unjustified instructions)
- [ ] Unit test: artificially inject a redundant `br label` → ratio increases
- [ ] Unit test: function with only `ret void` → ratio 1.00x, ideal=1, actual=1
- [ ] Unit test: function with only `call` + `ret` (no arithmetic) → ratio 1.00x
- [ ] Unit test: function with `alloca`+`store`+`load`+`ret` for single-use value → unjustified=3, ratio > 1.00x

---

### Relationship to IR Quality (`ir_unjustified`)

Both the **Instruction Efficiency** dimension (this section, 15% weight) and the **IR Quality** dimension (20% weight, `score.py --ir-unjustified`) use the concept of "unjustified instructions." They are the SAME count used in two different scoring dimensions:

- **Instruction Efficiency** (15%): `ratio = actual / (actual - unjustified)` -- measures the ratio
- **IR Quality** (20%): `unjustified` count directly -- measures absolute waste

The `compute_unjustified()` function is computed ONCE by this section and feeds both dimensions. The `extract-metrics.py` integration (section 07) must pass this single count to both `--instruction-ratio` (as the denominator ingredient) and `--ir-unjustified` (as the direct value).

> **NOTE:** There is no separate "IR Quality" section in this plan. The IR Quality scoring dimension is fully derived from section 02's `compute_unjustified()` output plus section 05's `is_incorrect` flag (which feeds `--ir-incorrect`). The `ir_incorrect` flag is set to `true` if section 05 detects semantic incorrectness (unreachable blocks, same-target conditional branches).

The overlap between section 05's control flow defects and section 02's unjustified instructions is intentional:
- [ ] Define clear ownership: redundant branches are counted as CF defects (section 05) AND as unjustified instructions (section 02). This is intentional -- they affect both dimensions.
- [ ] Document that this is not double-counting: the two dimensions measure different things (ratio vs absolute count) and have different weights (15% vs 20%).

## 02.4 Completion Checklist

- [ ] `compute_instruction_metrics(module)` returns correct ratios for Journey 1
- [ ] Journey 1 scores `instruction_ratio: 1.00` (not 3.50)
- [ ] Unjustified pattern detectors tested individually
- [ ] `InstructionMetrics.avg_ratio` maps to `score.py --instruction-ratio` (float >= 1.0); `InstructionMetrics.max_ratio` maps to `--instruction-ratio-max` (float >= avg_ratio); `InstructionMetrics.total_unjustified` maps to `--ir-unjustified` (int >= 0)
- [ ] Verdict labels match SCHEMA.md thresholds exactly

**Exit Criteria:** `compute_instruction_metrics()` on Journey 1's IR returns `avg_ratio=1.0, max_ratio=1.0` with zero unjustified instructions. On a deliberately bloated IR (with injected redundant branches), it returns a ratio > 1.0 with the correct unjustified count.
