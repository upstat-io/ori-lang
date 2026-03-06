---
section: "03"
title: "ARC Metrics"
status: complete
goal: "Count RC operations, detect violations, and compute weighted ARC violation score"
depends_on: ["01"]
sections:
  - id: "03.1"
    title: "RC Operation Counting"
    status: complete
  - id: "03.2"
    title: "Violation Detection"
    status: not-started
  - id: "03.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: ARC Metrics

**Status:** Not Started
**Goal:** Deterministically count RC operations per function, detect violations (unbalanced pairs, scalar RC, wasted pairs), and produce the weighted violation count for `score.py`.

**Context:** ARC metrics are relatively straightforward — RC operations are explicit function calls in the IR (`ori_rc_inc`, `ori_rc_dec`, `ori_buffer_rc_dec`, etc.). The algorithm counts calls and checks for violations.

**Depends on:** Section 01 (IR parser).

---

## 03.1 RC Operation Counting

**File(s):** `.claude/skills/code-journey/arc_metrics.py`

- [ ] Define RC operation patterns (verified against `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`):
  ```python
  RC_INC_PATTERNS = [
      r"call.*@ori_rc_inc",           # (ptr) — generic RC inc
      r"call.*@ori_list_rc_inc",      # (ptr, i64) — slice-aware list/set RC inc
  ]
  RC_DEC_PATTERNS = [
      r"call.*@ori_rc_dec",           # (ptr, ptr) — generic RC dec (data_ptr, drop_fn)
      r"call.*@ori_rc_free",          # (ptr, i64, i64) — direct free (no element cleanup)
      r"call.*@ori_buffer_rc_dec",    # (ptr, i64, i64, i64, ptr) — collection buffer dec
      r"call.*@ori_buffer_drop_unique",  # same sig as buffer_rc_dec — unique-path fast drop
      r"call.*@ori_map_buffer_drop_unique",  # (ptr, i64, i64, i64, i64, ptr, ptr) — map buffer drop
  ]
  ```

  **Note on `invoke` vs `call`:** In LLVM IR, calls to functions that may unwind use `invoke` instead of `call`. Since `ori_panic_cstr` is NOT `nounwind`, functions that call it may use `invoke` in some codegen paths. The RC patterns above must also match `invoke.*@ori_rc_inc` (not just `call.*@ori_rc_inc`). However, since RC functions (`ori_rc_inc`, `ori_rc_dec`) ARE `nounwind`, they should always appear as `call` — if they appear as `invoke`, that itself is a finding.

  **Note:** `ori_str_rc_inc`, `ori_str_rc_dec`, `ori_map_rc_inc`, and `ori_map_rc_dec` do NOT exist in the runtime. Strings use SSO (inline for <=23 bytes) or share the generic `ori_rc_inc`/`ori_rc_dec`. Maps and sets use `ori_buffer_rc_dec` for their data buffers.
- [ ] Count `rc_inc` and `rc_dec` per function
- [ ] Check balance: `rc_inc == rc_dec` per function (simple heuristic — full flow analysis is out of scope)
> **WARNING (balance heuristic):** Per-function `rc_inc == rc_dec` is a coarse check that will produce false positives in real programs. Common legitimate imbalances: (1) A function that allocates and returns a value has `rc_inc` with no matching `rc_dec` (caller owns it). (2) A function that consumes an argument may have `rc_dec` with no matching `rc_inc`. (3) `ori_buffer_drop_unique` is a dec-equivalent but counts separately from `ori_rc_dec`. The balance check should count ALL dec-like operations (including `ori_rc_free`, `ori_buffer_drop_unique`, `ori_map_buffer_drop_unique`) and document that cross-function ownership transfers cause expected imbalances. Consider making the balance check module-level (sum across all functions) in addition to per-function.
- [ ] Detect scalar RC: RC calls on values that are `i64`, `double`, `i1` (examine the argument type in the call instruction)
  **Note on scalar RC detection:** In LLVM IR, `ori_rc_inc` takes a `ptr` argument. To detect scalar RC, you must trace the pointer back to its origin — if it was produced by `inttoptr` from an `i64`, or if the original Ori value is known to be a scalar type, that's scalar RC. In practice, the ARC pipeline should never emit RC on scalars, so this is a regression guard. A simpler heuristic: if `ori_rc_inc`/`ori_rc_dec` appears in a function whose only parameters are `i64`/`double`/`i1` and the function has no `alloca` of pointer-containing structs, flag it for review.

```python
@dataclass
class FunctionArcMetrics:
    name: str
    rc_inc: int
    rc_dec: int
    balanced: bool          # rc_inc == rc_dec (simple check)
    has_scalar_rc: bool     # RC on i64/double/i1

@dataclass
class ArcMetrics:
    per_function: list[FunctionArcMetrics]
    total_violations: int    # Weighted count for score.py
    has_unbalanced: bool     # Any function unbalanced
    has_scalar_rc: bool      # Any scalar RC detected
```

---

## 03.2 Violation Detection

**File(s):** `.claude/skills/code-journey/arc_metrics.py`

Apply severity multipliers per SKILL.md:

| Violation | Multiplier | Detection |
|-----------|-----------|-----------|
| Unbalanced RC pair | x3 | `rc_inc != rc_dec` per function |
| RC on scalar type | x5 | Argument to RC call is `i64`/`double`/`i1` |
| Wasted pair | x1 | `rc_inc` immediately followed by `rc_dec` on same value (same block, consecutive instructions) |

- [ ] Implement `detect_unbalanced(func)` — compare inc/dec counts
- [ ] Implement `detect_scalar_rc(func)` — check argument types of RC calls
- [ ] Implement `detect_wasted_pairs(func)` — consecutive inc+dec on same register
- [ ] Compute `total_violations = sum(unbalanced * 3 + scalar * 5 + wasted * 1)`

**Note:** Borrow elision and move semantics detection require deeper analysis (data flow, use-after-inc). For now, these are flagged only if the AI identifies them in narrative — not scored algorithmically. This is a known limitation that can be improved later.

### Zero-RC Programs

Programs with no heap-allocated values (all scalars) should have zero RC operations. Per SKILL.md: "Programs with no heap-allocated values that correctly emit zero RC operations score 10." The algorithm must handle this correctly:
- If `total_rc_inc == 0` AND `total_rc_dec == 0`: `balanced=true, violations=0, score=10`
- If `total_rc_inc == 0` AND `total_rc_dec > 0` (or vice versa): `balanced=false` — this is a violation even in an otherwise scalar-only program

---

## 03.3 Completion Checklist

- [ ] Journey 1 (all-scalar) produces `total_violations=0, has_unbalanced=false, has_scalar_rc=false`
- [ ] Test with synthetic IR containing RC operations produces correct counts
- [ ] Wasted pair detection works on consecutive inc+dec
- [ ] Output matches `score.py` format (`--arc-violations`, `--arc-has-unbalanced`, `--arc-has-scalar-rc`)

**Exit Criteria:** `compute_arc_metrics()` on Journey 1 returns all zeros. On synthetic IR with deliberate violations, it returns correct weighted counts matching hand-calculation.
