---
section: "06"
title: "Attribute Compliance Improvements"
status: not-started
goal: "Reduce false attribute violations by teaching the checker about closures, indirect calls, and structural patterns"
depends_on: []
sections:
  - id: "06.1"
    title: "Closure-Aware Attribute Rules"
    status: not-started
  - id: "06.2"
    title: "Memory Attribute Analysis"
    status: not-started
  - id: "06.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Attribute Compliance Improvements

**Status:** Not Started
**Goal:** Reduce false attribute deductions by teaching `attribute_metrics.py` about closures, indirect calls, and memory effects. After this section, J5's attribute compliance rises from 60% to ≥80%.

**Context:** J5 (closures) scores 60% attribute compliance (12/38 applicable correct) — the worst of all journeys. The root cause: closure-related functions involve indirect calls through function pointers, which cannot carry calling convention or `nounwind` annotations in LLVM IR. The attribute checker doesn't know this and penalizes the function for "missing" attributes that are structurally impossible.

Similarly, `memory(read)` / `memory(none)` annotations require interprocedural analysis that the current checker doesn't attempt. Functions that only read memory but call other functions are flagged as missing `memory(read)` even when LLVM can't know they're read-only without full analysis.

**Depends on:** Nothing (independent fix).

**Current attribute rules** (from `_ATTRIBUTE_RULES` in `attribute_metrics.py`):

| Attribute | Applicable When | Description |
|-----------|----------------|-------------|
| `fastcc` | User function, not entry-called (`@_ori_main`) | Internal calling convention |
| `nounwind` | All defined functions | Panic paths end in unreachable |
| `uwtable` | Defined functions except `@main` wrapper | Unwind table for stack traces |
| `noundef` (return) | Defined functions with non-void return | Ori values are always defined |
| `noreturn` | Runtime decls in `NORETURN_FUNCTIONS` | `ori_panic_cstr`, `ori_panic` |
| `cold` | Runtime decls in `COLD_FUNCTIONS` | `ori_panic_cstr`, `ori_panic` |
| `noundef` (per-param) | Defined functions, per parameter | Each param is always defined |

Additionally, `_detect_wrong_attributes()` checks:
- `noreturn` present on a function that has `ret` instructions
- `nounwind` present on `MUST_NOT_BE_NOUNWIND` runtime declarations

---

## 06.1 Closure-Aware Attribute Rules

**File(s):** `.claude/skills/code-journey/attribute_metrics.py`

- [ ] Detect closure-related functions (patterns):
  ```python
  def is_closure_function(func: Function) -> bool:
      """Detect functions related to closure implementation.

      Patterns:
      - Name contains "$lambda" or "$closure"
      - Function takes a closure env parameter (ptr to { ptr, ptr } struct)
      - Function body contains indirect calls (call ptr %reg)
      """
  ```

- [ ] Reduce or skip attribute applicability for closure functions:
  - `fastcc` not applicable (indirect calls go through C calling convention)
  - `nounwind` not applicable (indirect call target may unwind)
  - `noundef` on closure env pointer: still applicable
  - `uwtable` still applicable (needed for unwinding stack traces)

- [ ] **Detect indirect calls** in function body via regex matching on instruction text (needed for closure detection):
  ```python
  _INDIRECT_CALL_RE = re.compile(r'call\b[^@]*%\w+\s*\(')  # call ptr %reg(...)
  _INDIRECT_INVOKE_RE = re.compile(r'invoke\b[^@]*%\w+\s*\(')

  def has_indirect_calls(func: Function) -> bool:
      """True if function contains any indirect call/invoke instructions."""
      return any(
          _INDIRECT_CALL_RE.search(instr.text) or _INDIRECT_INVOKE_RE.search(instr.text)
          for block in func.blocks
          for instr in block.instructions
      )
  ```

- [ ] Add `not_applicable_reason: str | None` to the existing `FunctionAttributeMetrics` data class:
  ```python
  @dataclass
  class FunctionAttributeMetrics:
      name: str
      checks: list[AttributeCheck]          # existing field
      not_applicable_reason: str | None = None  # new: e.g., "closure (indirect calls)"
  ```

---

## 06.2 Memory Attribute Analysis

**File(s):** `.claude/skills/code-journey/attribute_metrics.py`

- [ ] Only check `memory(...)` attributes on leaf functions (no calls to other user functions):
  ```python
  def is_leaf_function(func: Function) -> bool:
      """True if this function makes no calls to other user functions.

      Calls to runtime functions (ori_*) and LLVM intrinsics (llvm.*)
      don't count — they have known memory effects.
      """
  ```

- [ ] For non-leaf functions, `memory(...)` is not applicable (requires interprocedural analysis)
- [ ] For leaf functions with only `load` instructions (no `store`, no calls), flag missing `memory(read)`

---

## 06.3 Completion Checklist

- [ ] Closure functions detected and attribute applicability adjusted
- [ ] Memory attributes only checked on leaf functions
- [ ] J5 (closures): attribute compliance ≥ 80% (up from 60%)
- [ ] J1-J4 (non-closure): results unchanged
- [ ] Per-function output includes `not_applicable_reason` when relevant
- [ ] Tests cover: closure function detection, leaf function detection, mixed module
- [ ] `python3 -m pytest tests/test_attribute_metrics.py` passes

**Exit Criteria:** J5's attribute compliance rises to ≥80% by correctly excluding attributes that are structurally not applicable to closure functions. The remaining deductions are genuine missing attributes (not structural impossibilities).
