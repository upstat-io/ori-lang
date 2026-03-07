---
section: "04"
title: "Type Checker Polish (ori_types)"
status: done
goal: "Fix missing annotations, import inconsistencies, and decorative banners"
depends_on: []
sections:
  - id: "04.1"
    title: "Add #[must_use] to return_tag_to_idx"
    status: done
  - id: "04.2"
    title: "Standardize import style"
    status: done
  - id: "04.3"
    title: "Fix import grouping"
    status: done
  - id: "04.4"
    title: "Remove decorative banners"
    status: done
  - id: "04.5"
    title: "Completion Checklist"
    status: done
---

# Section 04: Type Checker Polish (ori_types)

**Status:** Not Started
**Goal:** Clean up minor hygiene issues in the type checker code introduced during registry wiring: missing `#[must_use]`, inconsistent import paths, wrong import grouping, and decorative banners.

**Context:** These are low-severity issues (EXPOSURE, WASTE, NOTE) that accumulated during the registry wiring implementation. None affect correctness, but they violate established conventions.

---

## 04.1 Add #[must_use] to return_tag_to_idx

**File(s):** `compiler/ori_types/src/infer/expr/registry_bridge/mod.rs:86`

`return_tag_to_idx` is a pure function returning `Idx`. Ignoring its return value is always a bug.

- [ ] Add `#[must_use]` attribute to `return_tag_to_idx` function.

---

## 04.2 Standardize import style

**File(s):**
- `compiler/ori_types/src/infer/expr/registry_bridge/mod.rs:20` — uses `super::super::InferEngine`
- `compiler/ori_types/src/infer/expr/methods/mod.rs:20` — uses `super::super::InferEngine`
- `compiler/ori_types/src/infer/expr/methods/computed_returns.rs:17` — uses `crate::infer::InferEngine`

The `crate::infer::InferEngine` style is preferred: it's absolute, unambiguous, and doesn't break when files move.

- [ ] In `registry_bridge/mod.rs`: replace `super::super::InferEngine` with `crate::infer::InferEngine`
- [ ] In `methods/mod.rs`: replace `super::super::InferEngine` with `crate::infer::InferEngine`

---

## 04.3 Fix import grouping

**File(s):** `compiler/ori_types/src/infer/expr/methods/mod.rs:18-24`

Import groups should follow the 3-group convention: external, crate, relative (separated by blank lines). Currently 4 groups are present.

- [ ] Merge relative imports into the correct group (max 3 groups: external, crate, relative).

---

## 04.4 Remove decorative banners

**File(s):** `compiler/ori_types/src/lib.rs:84-86`

Lines 84-86 contain `// =====...=====` decorative banners around the Salsa compatibility section. Per hygiene rules, decorative banners (`// ===`, `// ---`) should be removed when touching a file.

- [ ] Remove the `// ====...====` banner lines (lines 84 and 86).
- [ ] Replace with a plain `// Salsa compatibility assertions` section comment if needed.

---

## 04.5 Completion Checklist

- [ ] `return_tag_to_idx` has `#[must_use]`
- [ ] No `super::super::InferEngine` in registry_bridge or methods modules
- [ ] Import groups in `methods/mod.rs` follow 3-group convention
- [ ] No decorative banners (`// ===`) in `lib.rs`
- [ ] `cargo test -p ori_types` passes
- [ ] `./clippy-all.sh` green

**Exit Criteria:** `grep -rn 'super::super::InferEngine' compiler/ori_types/src/infer/expr/` returns 0. `grep -n '=====' compiler/ori_types/src/lib.rs` returns 0.
