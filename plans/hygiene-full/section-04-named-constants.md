---
section: "04"
title: "Named Constants for Tag Values & Field Indices"
status: not-started
reviewed: false
goal: "Replace magic numbers (Option/Result tags, collection field indices, FNV constants, struct sizes) with named constants in canonical locations"
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
    status: not-started
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
**Goal:** All magic numbers for Option/Result tag discriminants, collection struct field indices, FNV hash constants, and struct sizes are replaced by named constants defined in one canonical location. After this section, changing a tag value or field layout requires modifying exactly one constant definition.

**Context:** The Option tag convention (Some=0, None=1) appears as bare `0`/`1` literals in 11+ files across `ori_llvm`, `ori_arc`, `ori_eval`, and `ori_rt`. Collection field indices (len=0, cap=1, data=2) appear in 8+ files. FNV-1a hash constants are independently defined in 3 crates. These magic numbers are invisible invariants -- changing one without finding all others silently corrupts behavior.

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
- [ ] Define `pub const OPTION_TAG_SOME: i64 = 0;` and `pub const OPTION_TAG_NONE: i64 = 1;` in a canonical location (e.g., `ori_ir::tags` or `ori_registry::tags`)
- [ ] Define `pub const RESULT_TAG_OK: i64 = 0;` and `pub const RESULT_TAG_ERR: i64 = 1;` in the same location
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
- [ ] Define named constants (e.g., `pub const LIST_FIELD_LEN: u32 = 0;`, `LIST_FIELD_CAP: u32 = 1;`, `LIST_FIELD_DATA: u32 = 2;`) in a canonical location
- [ ] Replace all bare field index literals in GEP operations and field access code

---

## 04.3 FNV Hash Constants Unification

**File(s):** `compiler/ori_eval/src/methods/compare.rs`, `compiler/ori_llvm/src/codegen/derive_codegen/bodies.rs`, `compiler/ori_llvm/src/codegen/derive_codegen/enum_bodies/enum_hashable.rs`, `compiler/ori_rt/src/string/ops.rs`

FNV-1a constants are independently defined in 4 locations:
- `ori_eval/src/methods/compare.rs:201,207` -- `FNV_OFFSET_BASIS`, `FNV_PRIME` (with comment "Must match ori_llvm")
- `ori_llvm/src/codegen/derive_codegen/bodies.rs:71,73` -- `FNV_OFFSET_BASIS`, `FNV_PRIME`
- `ori_llvm/src/codegen/derive_codegen/enum_bodies/enum_hashable.rs:17,19` -- `FNV_OFFSET_BASIS`, `FNV_PRIME` (duplicate within ori_llvm)
- `ori_rt/src/string/ops.rs:314,315` -- `FNV_OFFSET_BASIS`, `FNV_PRIME` (within `ori_str_hash`)

The "Must match" comments in `compare.rs` explicitly acknowledge the duplication.

- [ ] **LEAK:inline-policy** -- FNV-1a constants (`FNV_OFFSET_BASIS = 14_695_981_039_346_656_037`, `FNV_PRIME = 1_099_511_628_211`) independently defined in 4 locations across 3 crates with manual "must match" comments
- [ ] Define canonical FNV constants in `ori_ir` (e.g., `ori_ir::hash_constants` module) -- confirmed: `ori_rt` depends on `ori_ir` (Cargo.toml), `ori_eval` depends on `ori_ir`, `ori_llvm` depends on `ori_ir`, so all 4 consumers can import from `ori_ir`
- [ ] Replace all 4 independent definitions with imports from the canonical location
- [ ] Remove the "Must match" comments (the import relationship enforces consistency)

---

## 04.4 FatPointer/Closure/Range Size Constants

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/`, `compiler/ori_arc/src/lower/`, `compiler/ori_llvm/src/codegen/abi/`

FatPointer size (2 pointers), Closure layout (fn_ptr at index 0, env_ptr at index 1), and Range struct fields (start, end, inclusive, step) appear as inline integer literals in GEP and sizing operations.

- [ ] **LEAK:inline-policy** -- FatPointer/Closure/Range struct sizes and field indices hardcoded as inline literals in codegen and lowering files
- [ ] Audit `ori_llvm` and `ori_arc` for bare integer constants representing closure field indices (fn_ptr=0, env_ptr=1), range field indices, and fat pointer sizes
- [ ] Define named constants in `ori_ir` (shared dependency) for closure layout, range layout, and fat pointer size
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
