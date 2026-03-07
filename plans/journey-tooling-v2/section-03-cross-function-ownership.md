---
section: "03"
title: "Cross-Function Ownership Tracking"
status: not-started
goal: "Track RC ownership across function boundaries to eliminate false positives from allocation/consumption in different functions"
inspired_by:
  - "Swift RCIdentityAnalysis (include/swift/SILOptimizer/Analysis/RCIdentityAnalysis.h) — RC identity through projections and across calls"
  - "Lean 4 Borrow.lean (Compiler/IR/Borrow.lean) — parameter ownership inference"
  - "Ori AnnotatedSig (ori_arc/src/ownership/mod.rs) — per-parameter Owned/Borrowed via AnnotatedParam"
depends_on: ["01"]
sections:
  - id: "03.1"
    title: "Module-Level Balance Check"
    status: not-started
  - id: "03.2"
    title: "Ownership Convention Inference"
    status: not-started
  - id: "03.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Cross-Function Ownership Tracking

**Status:** Not Started
**Goal:** Eliminate false positive ARC violations from cross-function ownership transfers. After this section, J5's `arc_violations` should drop from 14 to ≤3.

**Context:** J5 (closures) has 14 ARC violations because closure environments are allocated in one function (the closure constructor) and consumed in another (the closure body or destructor). Per-function balance will always flag this — the constructor has `+1` with no matching `-1`, and the body has `-1` with no matching `+1`. This is correct behavior (ownership transfer), not a bug.

Swift's `RCIdentityAnalysis` handles this by tracing RC identity through projections and across function calls. Ori's own `AnnotatedSig` already has per-parameter `Owned`/`Borrowed` annotations from borrow inference. The Python tooling needs to use this information.

**Reference implementations:**
- **Swift** `include/swift/SILOptimizer/Analysis/RCIdentityAnalysis.h`: Traces RC identity through `struct_extract`, across function boundaries
- **Lean 4** `Compiler/IR/Borrow.lean`: Borrow inference determines which parameters transfer ownership
- **Ori** `ori_arc/src/ownership/`: `AnnotatedSig` with `Owned`/`Borrowed` per parameter — the compiler already has this info

**Depends on:** Section 01 (effect summaries provide the building blocks).

---

## 03.1 Module-Level Balance Check

**File(s):** `.claude/skills/code-journey/arc_metrics.py`

The simplest fix: complement per-function balance with module-level balance. If the sum of all incs and decs across the entire module balances, individual per-function imbalances are likely ownership transfers rather than bugs.

- [ ] Add module-level balance to `compute_arc_metrics()`:
  ```python
  def compute_arc_metrics(module: Module) -> ArcMetrics:
      # ... existing per-function logic ...

      # Module-level balance check
      total_inc = sum(f.rc_inc for f in results)
      total_dec = sum(f.rc_dec for f in results)
      module_balanced = total_inc == total_dec

      # If module is balanced, downgrade per-function imbalances from
      # violations to notes (ownership transfers, not bugs)
      if module_balanced:
          for fm in results:
              if not fm.balanced:
                  # This function has an imbalance, but the module as a
                  # whole is balanced — likely an ownership transfer
                  fm.is_ownership_transfer = True  # new field
  ```

- [ ] Add `is_ownership_transfer: bool = False` field to `FunctionArcMetrics`
- [ ] Reduce violation weight for ownership transfers: `violations += 1` instead of `abs(inc - dec) * 3`
- [ ] Add `module_balanced: bool` to `ArcMetrics` output

> **WARNING (false negative risk):** Module-level balance alone is a blunt
> instrument. If function A leaks (+1 unmatched) and function B double-frees
> (-1 unmatched), the module "balances" but both are bugs. The paired-function
> validation (03.1 below) is **not optional** — it is the mechanism that
> prevents this. Do not ship module-level balance without paired-function
> validation.

**Known limitations of module-level balance:**

- [ ] **Single-module assumption**: Document that module-level balance only works when all functions are in the same LLVM module. This is true for journey scoring (single `.ori` file) but not in general. Add an assertion or warning if the module contains `declare` (extern) functions that participate in RC.
- [ ] **Paired-function validation**: Instead of blindly downgrading all imbalances when module balances, validate that each +N function has a corresponding -N function that is called with the same pointer type. This avoids masking leak+double-free pairs.
  ```python
  def _find_ownership_pairs(
      results: list[FunctionArcMetrics], module: Module
  ) -> list[tuple[str, str]]:
      """Find (producer, consumer) pairs where producer has +N and consumer has -N."""
  ```
- [ ] **Closure pattern detection**: Specifically detect the closure constructor/body pattern:
  - Constructor: name contains `$lambda` or `$closure`, has RC inc but no matching dec
  - Body: takes env_ptr parameter, has RC dec but no matching inc

---

## 03.2 Ownership Convention Inference

**File(s):** `.claude/skills/code-journey/ownership_inference.py` (new)

For more precise tracking, infer ownership conventions from LLVM IR patterns.

- [ ] Detect common ownership patterns:
  ```python
  def infer_ownership_convention(func: Function, module: Module) -> OwnershipInfo:
      """Infer whether a function produces (+1) or consumes (-1) RC values.

      Patterns:
      1. Returns a pointer from ori_rc_alloc/ori_str_from_raw → produces +1
      2. Calls ori_rc_dec on a parameter → consumes that parameter
      3. Calls ori_rc_inc on a parameter and returns it → borrows (inc to
         keep alive, will be dec'd elsewhere)
      4. No RC ops and passes pointer to callee → transparent pass-through
      """
  ```

- [ ] Build a call graph to trace ownership across calls:
  ```python
  def build_call_graph(module: Module) -> dict[str, list[str]]:
      """Map each function to the functions it calls."""
  ```

- [ ] For each imbalanced function, check if its imbalance is compensated by its callers/callees:
  - Constructor (+1, no -1): check if callers always dec the return value
  - Destructor (-1, no +1): check if callers always inc before calling
  - Pass-through (0): no concern

---

## 03.3 Completion Checklist

- [ ] Module-level balance check added to `arc_metrics.py`
- [ ] Per-function imbalances downgraded when module is balanced (ownership transfer)
- [ ] `is_ownership_transfer` field added to `FunctionArcMetrics` dataclass
- [ ] `module_balanced` field added to `ArcMetrics` dataclass
- [ ] `extract-metrics.py` updated to include `arc_module_balanced` and `arc_ownership_transfers` in JSON output
- [ ] J5 (closures): `arc_violations` ≤ 3 (down from 14)
- [ ] J1-J4 (scalar): results unchanged (already balanced)
- [ ] Tests cover: balanced module with unbalanced functions, genuinely unbalanced module, module with mixed real violations and ownership transfers
- [ ] `python3 -m pytest tests/` passes

**Exit Criteria:** J5's closure environment allocation/consumption is recognized as ownership transfer, reducing `arc_violations` from 14 to ≤3. The remaining violations (if any) are genuine issues with the closure codegen, not scanner blind spots. Module-level `module_balanced: true` is reported for all 12 journeys.
