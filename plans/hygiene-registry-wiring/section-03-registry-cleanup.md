---
section: "03"
title: "Registry Cleanup (ori_registry)"
status: done
goal: "Fix file size violation, reduce duplication, add missing annotations and tests"
depends_on: []
sections:
  - id: "03.1"
    title: "Extract ReturnTag from tags/mod.rs"
    status: done
  - id: "03.2"
    title: "Deduplicate SELF_PARAM across def files"
    status: done
  - id: "03.3"
    title: "Add missing #[must_use] to query functions"
    status: done
  - id: "03.4"
    title: "Add MethodDef::associated_backend() constructor"
    status: done
  - id: "03.5"
    title: "Add bool.rs test file"
    status: done
  - id: "03.6"
    title: "Completion Checklist"
    status: done
---

# Section 03: Registry Cleanup (ori_registry)

**Status:** Not Started
**Goal:** Bring `ori_registry` up to hygiene standards: fix the 500-line limit violation in `tags/mod.rs`, eliminate the `SELF_PARAM` duplication across 15 def files, add missing `#[must_use]` annotations, provide a proper constructor for associated backend methods, and add missing test coverage for `bool.rs`.

**Context:** The `ori_registry` crate was created in the last 4 commits. While functionally correct, several hygiene issues accumulated during rapid implementation.

---

## 03.1 Extract ReturnTag from tags/mod.rs

**File(s):** `compiler/ori_registry/src/tags/mod.rs` (563 lines — exceeds 500-line limit)

The file contains 7 enums. `ReturnTag` and `TypeProjection` together account for ~100 lines and form a logical unit (return type classification) separate from the core type identity tags.

- [ ] Create `compiler/ori_registry/src/tags/return_tag.rs`
- [ ] Move `ReturnTag` enum + its impl block to the new file
- [ ] Move `TypeProjection` enum + its impl block to the new file
- [ ] Update `tags/mod.rs`: add `mod return_tag; pub use return_tag::{ReturnTag, TypeProjection};`
- [ ] Verify `tags/mod.rs` is now under 500 lines
- [ ] Verify all external imports of `ReturnTag` and `TypeProjection` still compile

---

## 03.2 Deduplicate SELF_PARAM across def files

**File(s):** `compiler/ori_registry/src/defs/*.rs` (15 files) and `compiler/ori_registry/src/method/mod.rs`

Each of the 15 def files defines its own `SELF_PARAM` (or `SELF_PARAM_BORROW`) constant. There are two distinct patterns:

1. **Primitives** (bool, byte, char, float, int): `[ParamDef::SELF_TYPE]` — uses existing `SELF_TYPE` constant (`name: "other"`, `ownership: Copy`)
2. **Collections/compound** (list, map, set, option, result, tuple, ordering, duration, size, str): raw `ParamDef { name: "other", ty: ReturnTag::SelfType, ownership: Ownership::Borrow }` — same structure but with `Borrow` instead of `Copy`

The primitives already use the shared `SELF_TYPE` constant but wrap it in a per-file `static SELF_PARAM: [ParamDef; 1]` array. The collections define the full struct inline.

- [ ] Add `ParamDef::SELF_BORROW` associated constant in `compiler/ori_registry/src/method/mod.rs`:
  ```rust
  pub const SELF_BORROW: Self = Self {
      name: "other",
      ty: ReturnTag::SelfType,
      ownership: Ownership::Borrow,
  };
  ```
- [ ] Add shared `static` arrays in a common location (e.g., `method/mod.rs` or a new `method/params.rs`):
  ```rust
  pub static ONE_SELF_COPY: [ParamDef; 1] = [ParamDef::SELF_TYPE];
  pub static TWO_SELF_COPY: [ParamDef; 2] = [ParamDef::SELF_TYPE, ParamDef::SELF_TYPE];
  pub static ONE_SELF_BORROW: [ParamDef; 1] = [ParamDef::SELF_BORROW];
  pub static TWO_SELF_BORROW: [ParamDef; 2] = [ParamDef::SELF_BORROW, ParamDef::SELF_BORROW];
  ```
- [ ] Replace per-file `static SELF_PARAM` and `static TWO_SELF_PARAMS` in primitive defs with references to shared arrays.
- [ ] Replace per-file raw `ParamDef { ... }` in collection/compound defs with references to shared arrays.
- [ ] Handle `str.rs` special case: it uses `SELF_PARAM_BORROW` (same pattern as collections).
- [ ] Verify: `grep -rn 'static SELF_PARAM' compiler/ori_registry/src/defs/` returns zero matches after replacement.

---

## 03.3 Add missing #[must_use] to query functions

**File(s):** `compiler/ori_registry/src/query/mod.rs`

Four public functions that return iterators are missing `#[must_use]`:
- `method_names_for` (line 137)
- `borrowing_methods` (line 166)
- `dei_only_methods` (line 206)
- `iterator_method_names` (line 217)

- [ ] Add `#[must_use]` to `method_names_for`
- [ ] Add `#[must_use]` to `borrowing_methods`
- [ ] Add `#[must_use]` to `dei_only_methods`
- [ ] Add `#[must_use]` to `iterator_method_names`

---

## 03.4 Add MethodDef::associated_backend() constructor

**File(s):** `compiler/ori_registry/src/defs/str.rs:185-208` and `compiler/ori_registry/src/method/mod.rs`

`str.from_utf8` and `str.from_utf8_unchecked` use raw struct literal syntax to construct `MethodDef` with `kind: MethodKind::Associated` and `backend_required: true`, bypassing the existing `MethodDef::associated()` constructor.

- [ ] Add `MethodDef::associated_backend()` (or extend `associated()` with a `backend_required` parameter) in `method/mod.rs`.
- [ ] Replace the raw struct literals in `str.rs:185-208` with the new constructor.
- [ ] Verify no other def files have similar raw struct bypasses.

---

## 03.5 Add bool.rs test file

**File(s):** `compiler/ori_registry/src/defs/bool.rs` (no sibling test file)

The `bool.rs` def file has partial test coverage via the shared `defs/tests.rs` (operator strategy tests like `bool_comparison_is_unsigned`, `bool_equality_is_bool_logic`, `bool_not_is_bool_logic`), but lacks a dedicated test file verifying its method definitions, unlike the directory-based types (list, map, option, etc.) which each have `tests.rs`.

- [ ] Check if `bool.rs` needs to be moved to `bool/mod.rs` + `bool/tests.rs` to follow the sibling test convention (like other types that have dedicated tests).
- [ ] Add test: verify bool has expected method count.
- [ ] Add test: verify bool method names include expected entries (`to_str`, `equals`, etc.).
- [ ] Add `#[cfg(test)] mod tests;` declaration if a dedicated test file is created.
- [ ] Note: simple primitive types (`byte.rs`, `char.rs`, `float.rs`, `int.rs`) are in the same situation. Consider adding a shared parametric test in `defs/tests.rs` that covers all primitive types' method lists if individual files are not warranted.

---

## 03.6 Completion Checklist

- [ ] `tags/mod.rs` is under 500 lines
- [ ] `ReturnTag` and `TypeProjection` live in `tags/return_tag.rs`
- [ ] Zero per-file `SELF_PARAM` constants in `defs/` — all use shared constant
- [ ] All 4 query functions have `#[must_use]`
- [ ] `str.from_utf8` uses a proper constructor, not raw struct literal
- [ ] `bool.rs` has test coverage
- [ ] `cargo test -p ori_registry` passes
- [ ] `./clippy-all.sh` green

**Exit Criteria:** `wc -l compiler/ori_registry/src/tags/mod.rs` < 500. `grep -rn 'SELF_PARAM' compiler/ori_registry/src/defs/` returns 0. All query functions annotated. All def files use constructors.
