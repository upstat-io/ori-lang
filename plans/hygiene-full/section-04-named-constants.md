---
section: "04"
title: "Named Constants for Tag Values & Field Indices"
status: in-progress
reviewed: true
goal: "Replace magic numbers (Option/Result tags, collection field indices, struct sizes) with named constants in canonical locations"
inspired_by:
  - "Rust compiler discriminant constants -- named rather than inline literals"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Option/Result Tag Constants"
    status: not-started
  - id: "04.2"
    title: "Collection Field Index Constants"
    status: not-started
  - id: "04.3"
    title: "FNV Hash Constants Unification"
    status: superseded
  - id: "04.4"
    title: "FatPointer/Closure/Range Size Constants"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Named Constants for Tag Values & Field Indices

**Status:** Not Started
**Goal:** All magic numbers for Option/Result tag discriminants, collection struct field indices, and struct sizes are replaced by named constants defined in one canonical location. After this section, changing a tag value or field layout requires modifying exactly one constant definition.

**Context:** The Option tag convention (Some=0, None=1) appears as bare `0`/`1` literals in 11+ files across `ori_llvm`, `ori_arc`, `ori_eval`, and `ori_rt`. Collection field indices (len=0, cap=1, data=2) appear in 8+ files. These magic numbers are invisible invariants -- changing one without finding all others silently corrupts behavior. (FNV-1a hash constants are handled in Section 03.3 as a cross-backend DRY consolidation in `ori_ir::hash_constants`.)

**Depends on:** None.

**Test strategy:** Pure refactoring -- replacing inline literals with named constants. No behavioral changes. The test matrix is the existing test suite: `./test-all.sh` must pass unchanged. Additionally:
- Const assertions: each named constant has a `const _: () = assert!(OPTION_TAG_SOME == 0);` style assertion at the definition site
- Semantic pin: a Rust test that constructs Option::Some and verifies its tag matches `OPTION_TAG_SOME`

---

## 04.1 Option/Result Tag Constants

**File(s):** Define in `compiler/ori_ir/src/` or `compiler/ori_registry/src/tags/`; consumers across `ori_llvm`, `ori_arc`, `ori_eval`, `ori_rt`

Option/Result tag constants (Some=0/None=1, Ok=0/Err=1) appear as inline comments and bare literals in:
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs:47,53,63,95,101,121` -- "Some = variant 0", "None = variant 1", "tag == 0"
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/compound_type_impls/option.rs:63` -- "None(1) < Some(0)"
- `compiler/ori_llvm/src/codegen/derive_codegen/field_ops/wrapper_cmp.rs:93` -- "None(1) < Some(0)"
- `compiler/ori_llvm/src/codegen/arc_emitter/operators/strategy.rs:131` -- "if Some(0) return payload"
- `compiler/ori_arc/src/decision_tree/flatten.rs:255` -- "Convention: Some = 0, None = 1"
- `compiler/ori_arc/src/lower/expr/mod.rs:431` -- "branch on tag == 0 (Some/Ok)"
- `compiler/ori_arc/src/lower/control_flow/for_yield_option.rs:23` -- "branch(tag == 0, some, none)"

- [ ] **LEAK:inline-policy** -- Option tag discriminants (Some=0, None=1) appear as bare `0`/`1` in 11+ files with only comments documenting the convention
- [x] Define `pub const OPTION_TAG_SOME: i64 = 0;` and `pub const OPTION_TAG_NONE: i64 = 1;` in a canonical location (2026-04-01) — defined in `ori_ir::tag_constants`, re-exported from `ori_ir`
- [x] Define `pub const RESULT_TAG_OK: i64 = 0;` and `pub const RESULT_TAG_ERR: i64 = 1;` in the same location (2026-04-01)
- [ ] Replace all bare `0`/`1` discriminant literals in the listed files with the named constants
- [ ] Add a `debug_assert!` or const assertion that these values match the actual enum layout

---

## 04.2 Collection Field Index Constants

**File(s):** Define in `compiler/ori_ir/src/` or `compiler/ori_registry/src/tags/`; consumers across `ori_llvm`, `ori_rt`

Collection struct field indices (len=0, cap=1, data=2 for `{ len, cap, data }` layout) appear as bare integers in GEP operations across:
- `compiler/ori_llvm/src/codegen/arc_emitter/` -- multiple GEP calls using `0`, `1`, `2` for list/str field access
- `compiler/ori_rt/src/list/` -- field offset arithmetic using `0`, `1`, `2`
- `compiler/ori_rt/src/string/` -- SSO layout field access
- `compiler/ori_llvm/src/codegen/type_info/` -- struct field assumptions

- [ ] **LEAK:inline-policy** -- Collection field indices (len=0, cap=1, data=2) hardcoded as bare integers in 8+ files across `ori_llvm` and `ori_rt`
- [x] Define named constants (e.g., `pub const LIST_FIELD_LEN: u32 = 0;`, `LIST_FIELD_CAP: u32 = 1;`, `LIST_FIELD_DATA: u32 = 2;`) in a canonical location (2026-04-01) — defined as `FIELD_LEN`, `FIELD_CAP`, `FIELD_DATA` in `ori_ir::tag_constants`
- [ ] Replace all bare field index literals in GEP operations and field access code

---

## 04.3 FNV Hash Constants Unification

**Status: SUPERSEDED by Section 03.3** — Implemented as `ori_ir::hash_constants` in Section 03.3, which is the correct cross-backend DRY location for constants shared between `ori_eval`, `ori_llvm`, and `ori_rt`. All 4 consumer sites and the canonical `ori_ir::hash_constants` definition are handled in Section 03.3. Nothing to do here.

---

## 04.4 FatPointer/Closure/Range Size Constants

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/`, `compiler/ori_arc/src/lower/`, `compiler/ori_llvm/src/codegen/abi/`

FatPointer size (2 pointers), Closure layout (fn_ptr at index 0, env_ptr at index 1), and Range struct fields (start, end, inclusive, step) appear as inline integer literals in GEP and sizing operations.

- [ ] **LEAK:inline-policy** -- FatPointer/Closure/Range struct sizes and field indices hardcoded as inline literals in codegen and lowering files
- [ ] Audit `ori_llvm` and `ori_arc` for bare integer constants representing closure field indices (fn_ptr=0, env_ptr=1), range field indices, and fat pointer sizes
- [x] Define named constants in `ori_ir` (shared dependency) for closure layout, range layout, and fat pointer size (2026-04-01) — `CLOSURE_FIELD_FN`, `CLOSURE_FIELD_ENV` defined in `ori_ir::tag_constants`
- [ ] Replace bare integer literals with named constants

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] Option/Result tag constants defined once and imported everywhere
- [ ] Collection field index constants defined once and imported everywhere
- [ ] FNV hash constants defined once in `ori_ir` and imported by `ori_eval`, `ori_llvm`, `ori_rt`
- [ ] No bare `0`/`1` tag discriminants remain in Option/Result dispatch code
- [ ] No "must match" cross-crate comments remain (replaced by actual imports)
- [ ] `timeout 150 ./test-all.sh` passes with zero regressions
- [ ] `./clippy-all.sh` passes
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 04` returns 0 annotations
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** `grep -rn 'Some = 0\|None = 1\|tag == 0\|tag == 1' compiler/ --include="*.rs" | grep -v test | grep -v const` returns zero results. All FNV constants import from one location. `./test-all.sh` green.
