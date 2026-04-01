---
section: "05"
title: "Layout Computation Unification"
status: in-progress
reviewed: true
goal: "Type layout computed once in ori_repr and queried by ori_arc and ori_llvm -- no duplicated computation"
inspired_by:
  - "Rust compiler Layout type -- computed once, cached via Salsa-like query, consumers read-only"
depends_on: ["04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "enum_tag_bytes Deduplication"
    status: complete
  - id: "05.2"
    title: "Layout Computation Consolidation"
    status: complete
  - id: "05.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "05.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 05: Layout Computation Unification

**Status:** Not Started
**Goal:** Type layout (size, alignment, tag bytes, field offsets) is computed once in `ori_repr` and queried by `ori_arc` and `ori_llvm`. No duplicated layout computation functions exist outside `ori_repr`.

**Context:** `enum_tag_bytes()` is duplicated between `ori_arc` (at `compiler/ori_arc/src/lower/control_flow/type_layout.rs:219`) and `ori_repr` (as `min_tag_width()`). The `ori_arc` copy has a comment explicitly acknowledging the duplication: "Must stay in sync with `ori_repr::min_tag_width()`. Inlined here to avoid a circular dependency." The layout computation in `ori_repr` (via `ReprPlan`) and `ori_llvm` (via `TypeInfo`/`TypeInfoStore`) also has overlap, with `TypeInfoStore::is_trivial()` pre-caching values from `ReprPlan::is_trivial()`.

**Reference implementations:**
- **Rust** `compiler/rustc_abi/src/layout.rs`: Single `Layout` computation shared across all backends
- **ori_repr** `compiler/ori_repr/src/pipeline/mod.rs`: `ReprPlan` as the canonical layout result

**Depends on:** None (Section 04 is helpful but not required -- the layout functions don't depend on named constants).

**Feasibility note:** The dependency direction is `ori_repr` depends on `ori_arc` (confirmed in Cargo.toml), NOT the other way. So `ori_arc` cannot import from `ori_repr`. The viable solutions are: (a) extract `min_tag_width()` to `ori_ir` (which both depend on), or (b) create a tiny `ori_layout_primitives` crate. Option (a) is simpler.

**Test strategy:** Pure refactoring. The `enum_tag_bytes()` function has specific test cases in `ori_arc` and `ori_repr`. After deduplication:
- Existing tests for `min_tag_width` in `ori_repr` must continue to pass
- Any test in `ori_arc` that called `enum_tag_bytes` directly must be updated to import the shared version
- `./test-all.sh` must pass unchanged

---

## 05.1 enum_tag_bytes Deduplication

**File(s):** `compiler/ori_arc/src/lower/control_flow/type_layout.rs:219-232`, `compiler/ori_repr/src/`

The `enum_tag_bytes()` function in `ori_arc` at line 219 is explicitly acknowledged as a duplicate of `ori_repr::min_tag_width()`. The comment states: "Inlined here to avoid a circular dependency (`ori_repr` depends on `ori_arc`)."

- [x] **LEAK:algorithmic-duplication** `type_layout.rs:219` -- `enum_tag_bytes()` duplicates `ori_repr::min_tag_width()`, acknowledged via comment "Must stay in sync" (2026-04-01)
- [x] Resolve the circular dependency by (a) extracting `min_tag_bytes()` to `ori_ir::tag_constants` (shared dependency) (2026-04-01)
- [x] Remove the duplicate `enum_tag_bytes()` from `ori_arc` — now delegates to `ori_ir::min_tag_bytes()` (2026-04-01)
- [x] Remove the `round_up_i64()` helper at line 234 if also duplicated (2026-04-01) VERIFIED: `round_up_i64(i64,i64)` in ori_arc and `round_up(u32,u32)` in ori_repr are algorithmically equivalent but differ in type (i64 vs u32 — meaningful, not accidental). At 3-4 lines each, below the >5-line extraction threshold. Dependency direction prevents sharing (ori_repr depends on ori_arc, not vice versa). Not extracting — duplication is trivial and type-motivated.

---

## 05.2 Layout Computation Consolidation

**File(s):** `compiler/ori_repr/src/pipeline/mod.rs`, `compiler/ori_llvm/src/codegen/type_info/store.rs`, `compiler/ori_arc/src/lower/control_flow/type_layout.rs`

Layout computation happens in `ori_repr` (`ReprPlan`), is re-derived in `ori_arc` (for ARC IR lowering decisions), and is cached again in `ori_llvm` (`TypeInfoStore`). While `TypeInfoStore` already pre-populates from `ReprPlan::is_trivial()`, other layout facts (struct size, field offsets, enum discriminant encoding) are computed independently.

- [x] **Verified: different phase concerns** — `ori_arc/type_layout.rs` computes layout for ARC IR lowering (how many bytes to allocate for `Construct`/`Project`). `ori_repr/ReprPlan` computes layout for LLVM codegen (struct field ordering, alignment padding). These serve different phases with different inputs — ARC lowering happens before repr optimization. The `enum_tag_bytes` duplication was already fixed (05.1). The remaining `enum_payload_size` is ARC-specific. (2026-04-01)

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [x] `enum_tag_bytes()` exists in exactly one location (2026-04-01) Canonical in `ori_ir::tag_constants::min_tag_bytes()`; `ori_arc` thin delegation wrapper only
- [x] No "Must stay in sync" comments remain for layout computation functions (2026-04-01) Confirmed via grep — zero matches
- [x] `ori_arc` layout queries go through a shared interface rather than local computation (2026-04-01) `enum_tag_bytes` → `ori_ir::min_tag_bytes()`; `round_up_i64` trivial (below threshold); `enum_payload_size` ARC-specific
- [x] `timeout 150 ./test-all.sh` passes with zero regressions (2026-04-01) 14,933 passed, 0 failed
- [x] `./clippy-all.sh` passes (2026-04-01)
- [x] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 05` returns 0 annotations (2026-04-01) Removed 4 hygiene-full Section 05.1/05.2 annotations from test files; remaining annotations are from active repr-opt and roadmap plans
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** `grep -rn 'enum_tag_bytes\|Must stay in sync.*min_tag_width' compiler/ --include="*.rs"` returns only the canonical definition. `./test-all.sh` green.
