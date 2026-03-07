---
section: "02"
title: "Drift Fixes (ori_types)"
status: done
goal: "Eliminate duplicated Range<float> iteration method lists"
depends_on: []
sections:
  - id: "02.1"
    title: "Extract Range<float> iteration method constant"
    status: done
  - id: "02.2"
    title: "Completion Checklist"
    status: done
---

# Section 02: Drift Fixes (ori_types)

**Status:** Not Started
**Goal:** Extract the duplicated Range<float> iteration method list into a single constant, eliminating drift risk.

**Context:** The same method list (`"iter"`, `"to_list"`, `"collect"`) appears in 2 locations within `ori_types`, used to identify methods that are invalid on `Range<float>` (because float ranges are not iterable). A third location (`control_flow.rs:607`) handles the same concept but uses `TypeCheckError::range_float_not_iterable` for `for` loops rather than a method name list. If a method is added to one list but not the other, the type checker will emit incorrect errors or miss errors depending on the code path.

---

## 02.1 Extract Range<float> iteration method constant

**File(s):**
- `compiler/ori_types/src/infer/expr/methods/mod.rs:71` — `matches!(method, "iter" | "to_list" | "collect")`
- `compiler/ori_types/src/infer/expr/calls/method_call.rs:380` — `if !matches!(name_str, "iter" | "collect" | "to_list")`

- [ ] Define `const RANGE_FLOAT_ITERATION_METHODS: &[&str] = &["iter", "to_list", "collect"];` in a shared location within `compiler/ori_types/src/infer/expr/` (e.g., at the top of `methods/mod.rs` or in a shared constants module).
- [ ] Replace the `matches!` in `methods/mod.rs:71` with a call to `RANGE_FLOAT_ITERATION_METHODS.contains(&method)`.
- [ ] Replace the `matches!` in `method_call.rs:380` with a reference to the same constant.
- [ ] Verify both paths still correctly reject `Range<float>.iter()`, `.to_list()`, `.collect()`.

---

## 02.2 Completion Checklist

- [ ] Single constant `RANGE_FLOAT_ITERATION_METHODS` exists as the source of truth
- [ ] Both call sites reference the constant (no inline method lists)
- [ ] `cargo test -p ori_types` passes
- [ ] `./test-all.sh` green

**Exit Criteria:** `grep -rn '"iter".*"to_list"\|"to_list".*"iter"' compiler/ori_types/src/infer/expr/` returns only the constant definition, not inline `matches!` patterns.
