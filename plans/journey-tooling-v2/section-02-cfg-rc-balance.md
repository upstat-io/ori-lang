---
section: "02"
title: "CFG-Aware RC Balance Checking"
status: not-started
goal: "Replace per-function linear RC counting with per-SSA-value state tracking through the control flow graph"
inspired_by:
  - "Swift TopDownRefCountState/BottomUpRefCountState (lib/SILOptimizer/ARC/RefCountState.h) — bidirectional RC state machine"
  - "Swift ARCBBState (lib/SILOptimizer/ARC/ARCBBState.h) — per-basic-block RC state"
  - "Swift KnownSafe flag — nested retains guarantee safety"
depends_on: ["01", "04"]
sections:
  - id: "02.1"
    title: "RC State Machine"
    status: not-started
  - id: "02.2"
    title: "Basic Block State Tracking"
    status: not-started
  - id: "02.3"
    title: "CFG Join Semantics"
    status: not-started
  - id: "02.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: CFG-Aware RC Balance Checking

**Status:** Not Started
**Goal:** Track RC state per SSA value through the control flow graph, eliminating false positives from conditional RC paths (SSO gates, null guards, error branches). After this section, conditional `rc_dec` operations are recognized as guarded rather than counted as raw imbalances.

**Context:** The current `arc_metrics.py` sums `rc_inc` and `rc_dec` per function and checks `inc == dec`. This fails when RC operations are behind conditional branches — e.g., in J9, `ori_rc_dec` is only called when the string is heap-allocated (not SSO), so there's a branch where `rc_dec` doesn't execute. The scanner counts the `rc_dec` instruction once but doesn't know it's conditional, creating a false imbalance.

Swift solved this with a bidirectional dataflow analysis: `TopDownRefCountState` walks forward tracking retains, `BottomUpRefCountState` walks backward tracking releases. At CFG joins, states are merged conservatively. A `KnownSafe` flag marks retains/releases that are provably safe (e.g., the object is known retained by an outer scope).

**Reference implementations:**
- **Swift** `lib/SILOptimizer/ARC/RefCountState.h`: State machine with `KnownSafe`, lattice merge at joins
- **Swift** `lib/SILOptimizer/ARC/ARCBBState.h`: Per-basic-block state tracking
- **Swift** `lib/SILOptimizer/ARC/GlobalARCSequenceDataflow.h`: Global CFG dataflow

**Depends on:** Section 01 (effect summaries -- needed to know which calls produce +1/-1 effects) **and** Section 04 (invoke instruction handling in `ir_utils.extract_branch_targets()` -- needed for correct CFG construction when functions use invoke/landingpad for exception handling).

> **WARNING (complexity):** This section is the highest-risk item in the plan.
> Full bidirectional dataflow analysis (Swift's approach) is ~2000 lines of
> hardened C++ in production. A Python implementation will be fragile and slow
> on large IR files. **Recommended phased approach:**
>
> 1. **MVP (Phase 1a):** Forward-only analysis with conservative merge at joins.
>    This handles the SSO gate pattern (paired conditional +1/-1) and covers
>    80% of the false positive reduction. Skip backward analysis, `KnownSafe`,
>    and fixpoint iteration. Estimated ~150 lines.
>
> 2. **Full (Phase 1b, optional):** Add backward analysis and `KnownSafe` flag
>    only if the MVP leaves significant false positives. Most Ori codegen
>    produces simple branching patterns; the full Swift machinery may be
>    unnecessary.
>
> Target `rc_state.py` at ~250-300 lines for the MVP. If it exceeds 400 lines,
> split into `rc_state.py` (state machine + transitions) and `cfg_analysis.py`
> (CFG construction + traversal).

---

## 02.1 RC State Machine

**File(s):** `.claude/skills/code-journey/rc_state.py` (new)

Define a per-SSA-value state machine inspired by Swift's `RefCountState`:

- [ ] Define RC state enum and per-value tracker:
  ```python
  class RcState(Enum):
      """State of a single RC-managed value."""
      UNKNOWN = "unknown"       # Not yet seen
      LIVE = "live"             # Known to have RC ≥ 1 (allocated or incremented)
      INCREMENTED = "incremented"  # Explicitly incremented (may be redundant)
      DECREMENTED = "decremented"  # Explicitly decremented
      CONSUMED = "consumed"     # Passed to a consuming function (ownership transferred)
      KNOWN_SAFE = "known_safe" # Provably safe — outer scope holds retain

  @dataclass
  class ValueState:
      """RC state for a single SSA value."""
      state: RcState
      inc_count: int = 0
      dec_count: int = 0
      is_conditional: bool = False  # True if any RC op is behind a branch
      known_safe: bool = False      # True if outer retain guarantees safety
  ```

- [ ] Implement state transition rules:
  - `UNKNOWN + alloc → LIVE`
  - `UNKNOWN + inc → INCREMENTED` (may be param)
  - `LIVE + inc → INCREMENTED` (redundant? or needed for sharing)
  - `LIVE + dec → DECREMENTED`
  - `INCREMENTED + dec → LIVE` (inc/dec pair cancel)
  - `DECREMENTED + dec → ERROR` (double dec)
  - Any state + `consume` → `CONSUMED`

- [ ] Implement `merge_states()` for CFG join points (see 02.3)

---

## 02.2 Basic Block State Tracking

**File(s):** `.claude/skills/code-journey/rc_state.py`

Walk instructions per basic block, tracking state changes. This replaces the flat `_count_rc_ops()` in `arc_metrics.py`.

- [ ] Build a basic block graph from the parsed `Function`:
  ```python
  def build_cfg(func: Function) -> dict[str, list[str]]:
      """Build successor map from basic blocks.

      Parses terminator instructions (br, switch, invoke, ret) to find
      successors for each block.
      """
  ```

- [ ] **Handle `invoke` instructions in CFG construction**: `invoke` is NOT a terminator in the same sense as `br` — it appears mid-block but creates two successor edges (`to %normal unwind %cleanup`). The CFG builder must:
  1. Parse `invoke ... to label %normal unwind label %cleanup` to extract both targets
  2. Add both `%normal` and `%cleanup` as successors of the containing block
  3. Recognize that `landingpad` blocks are reachable only via invoke unwind edges
  ```python
  _INVOKE_TARGETS_RE = re.compile(
      r'invoke\b.*to\s+label\s+%(\S+)\s+unwind\s+label\s+%(\S+)'
  )
  ```
- [ ] **Update `ir_utils.extract_branch_targets()`** to also check for `invoke` patterns, or add a separate `extract_invoke_targets()` helper
- [ ] **RC operations in landing pads**: `landingpad` blocks typically contain cleanup RC decrements. These are conditional on exception flow — the state tracker must mark them as `is_conditional = True`

- [ ] Walk each block, updating per-SSA-value state:
  ```python
  def analyze_block(
      block: BasicBlock,
      entry_state: dict[str, ValueState],
      effects: EffectSummaries,
  ) -> dict[str, ValueState]:
      """Process one basic block, returning exit state."""
  ```

- [ ] Track which blocks contain RC operations to identify conditional paths:
  - If a `rc_dec` appears in only one successor of a branch → `is_conditional = True`
  - This directly addresses the SSO gate false positive

- [ ] **SSO gate pattern recognition**: The SSO gate creates paired conditional +1/-1 (both the allocation and the deallocation are behind the same SSO branch):
  ```
  entry:
    %str = call @ori_str_from_raw(...)    ; conditional +1 (SSO or heap)
    %data = extractvalue %str, 2          ; get data ptr
    %is_heap = icmp ne ptr %data, null    ; SSO check
    br i1 %is_heap, label %heap, label %sso

  heap:
    call @ori_buffer_rc_dec(ptr %data, ...) ; conditional -1
    br label %merge

  sso:
    br label %merge

  merge:
    ; balanced: conditional +1 paired with conditional -1
  ```
  The state tracker should recognize this as balanced, not as an imbalance.

---

## 02.3 CFG Join Semantics

**File(s):** `.claude/skills/code-journey/rc_state.py`

At CFG join points (blocks with multiple predecessors), merge states conservatively.

- [ ] Implement merge semantics (inspired by Swift's lattice):
  ```python
  def merge_states(states: list[dict[str, ValueState]]) -> dict[str, ValueState]:
      """Merge RC states from multiple predecessor blocks.

      Conservative merge:
      - If all predecessors agree → keep state
      - If any predecessor has DECREMENTED and another doesn't → CONDITIONAL
      - If any predecessor has CONSUMED → CONSUMED (irreversible)
      - KNOWN_SAFE dominates (if known safe on any path, safe on all)
      """
  ```

- [ ] Handle phi nodes: when a phi node selects between two pointers, track both source values
- [ ] Handle loop headers: use fixpoint iteration (iterate until state converges)
  - Limit to 10 iterations to avoid infinite loops on pathological IR

- [ ] **Traversal order**: Walk blocks in reverse-postorder (RPO) to ensure predecessors are visited before their successors (except loop back-edges). This is standard for forward dataflow and guarantees convergence in O(n * depth) iterations instead of arbitrary.
  ```python
  def compute_rpo(cfg: dict[str, list[str]], entry: str) -> list[str]:
      """Compute reverse-postorder traversal of CFG."""
  ```

- [ ] **Build predecessor map** alongside successor map: needed for detecting join points (blocks with multiple predecessors where state merge is required)
  ```python
  def build_predecessor_map(cfg: dict[str, list[str]]) -> dict[str, list[str]]:
      """Invert successor map to get predecessors."""
  ```

---

## 02.4 Completion Checklist

- [ ] `rc_state.py` implements per-SSA-value state machine
- [ ] CFG built from parsed basic blocks
- [ ] State merge at join points handles conditional RC paths
- [ ] J9's SSO-guarded `rc_dec` recognized as conditional (not counted as imbalance)
- [ ] Straight-line RC code (J1-J4, J11) produces identical results to current scanner
- [ ] Tests cover: unconditional balance, conditional balance, double-dec detection, loop RC
- [ ] `python3 -m pytest tests/test_rc_state.py` passes

**Exit Criteria:** J9's `cf_defects` related to SSO gating drops from 5 to 0. Conditional RC operations are annotated as `is_conditional: true` in the per-function detail, and the balance check accounts for them. No regressions on scalar-only journeys.
