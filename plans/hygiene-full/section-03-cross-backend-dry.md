---
section: "03"
title: "Cross-Backend Algorithmic DRY (eval / LLVM)"
status: not-started
reviewed: false
goal: "Extract shared dispatch metadata between eval and LLVM backends so algorithmic skeletons are defined once"
inspired_by:
  - "ori_registry MethodDef pattern -- shared metadata consumed by multiple backends"
  - "Lean 4 IR/RC.lean -- shared RC decision metadata, backend-specific emission"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Iterator Method List Sync"
    status: not-started
  - id: "03.2"
    title: "Option/Result Routing Metadata"
    status: not-started
  - id: "03.3"
    title: "Equals/Compare/Hash Dispatch"
    status: not-started
  - id: "03.4"
    title: "Derive Processing Skeleton"
    status: not-started
  - id: "03.5"
    title: "Operator Dispatch Skeleton"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Cross-Backend Algorithmic DRY (eval / LLVM)

**Status:** Not Started
**Goal:** The evaluator (`ori_eval`) and LLVM codegen (`ori_llvm`) share dispatch metadata for Option/Result routing, equals/compare/hash, derive processing, and operator dispatch. Each backend retains its own emission logic but the routing decisions are defined once.

**Context:** Both backends implement the same semantic operations (Option unwrap, Result match, equals dispatch, hash computation, derive method generation) with parallel but independent dispatch skeletons. When a new variant or method is added, both backends must be updated independently -- and they have already drifted (iterator method lists). Extracting the shared *metadata* (which methods exist, which tag values mean what, which derive strategy to use) into a shared location eliminates this drift risk.

**Reference implementations:**
- **ori_registry** `compiler/ori_registry/src/defs/iterator/mod.rs`: shared iterator method definitions consumed by all backends
- **ori_ir** `compiler/ori_ir/src/derives/strategy.rs`: `DeriveStrategy` enum shared between eval and LLVM

**Depends on:** Sections 01, 02 (registry SSOT established first -- the shared metadata can leverage registry queries).

**Test strategy:** Pure refactoring -- no behavioral changes. Each subsection must verify:
- `timeout 150 ./test-all.sh` passes unchanged after each extraction
- Enforcement test: for each shared metadata structure, a Rust test verifies both backends consume it (not independent copies)
- Regression: existing eval-vs-LLVM parity tests (`diagnostics/dual-exec-verify.sh`) must show zero new mismatches

---

## 03.1 Iterator Method List Sync

**File(s):** `compiler/ori_eval/src/methods/`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/`

Iterator methods are defined in the registry (`compiler/ori_registry/src/defs/iterator/mod.rs`) but the eval and LLVM backends maintain independent method dispatch tables that have already drifted. The registry's `MethodDef` list should be the single driver for both backends' dispatch.

- [ ] **DRIFT** -- Iterator method dispatch lists in eval and LLVM have drifted from the registry's canonical `MethodDef` array; new methods added to one backend may be missing from the other
- [ ] Ensure both backends' iterator method dispatch is driven by or validated against the registry's iterator `TypeDef.methods`

---

## 03.2 Option/Result Routing Metadata

**File(s):** `compiler/ori_eval/src/operators/mod.rs` (lines 277-345), `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs`

Both backends implement Option/Result method routing with parallel dispatch:
- Eval: `eval_option_binary()` (line 277), `eval_result_binary()` (line 311)
- LLVM: `emit_option_method()` (line 37), `emit_result_method()` (line 85)

The routing logic (which method names map to which operations, tag semantics) is identical; only the emission differs.

- [ ] **LEAK:algorithmic-duplication** -- Option method routing duplicated between `eval_option_binary` and `emit_option_method`
- [ ] **LEAK:algorithmic-duplication** -- Result method routing duplicated between `eval_result_binary` and `emit_result_method`
- [ ] Extract shared Option/Result method routing metadata (method names, tag semantics, operation types) that both backends consume

---

## 03.3 Equals/Compare/Hash Dispatch

**File(s):** `compiler/ori_eval/src/methods/compare.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs`

Both backends implement equals/compare/hash dispatch for builtin types:
- LLVM: `emit_equals()` (line 196), `emit_compare()` (line 218), `emit_hash()` (line 241) in `traits.rs`
- Eval: comparison logic in `compare.rs` with `fnv1a_hash()`, `hash_combine()`, `hash_value()`

The dispatch routing (which types get which comparison strategy) parallels the registry's `OpDefs.eq` strategy.

- [ ] **LEAK:algorithmic-duplication** -- Equals/compare/hash type dispatch logic duplicated between eval `compare.rs` and LLVM `traits.rs`

---

## 03.4 Derive Processing Skeleton

**File(s):** `compiler/ori_eval/src/interpreter/derived_methods.rs`, `compiler/ori_llvm/src/codegen/derive_codegen/`

Both backends process derived trait methods with parallel dispatch on `DerivedTrait` variants:
- Eval: `eval_derived_method()` at line 23
- LLVM: `derive_codegen/` directory with per-trait codegen

The `DeriveStrategy` enum in `ori_ir/src/derives/strategy.rs` already provides shared metadata. The backends' dispatch skeletons should be driven by this shared strategy.

- [ ] **LEAK:algorithmic-duplication** -- Derive method dispatch skeleton paralleled between eval `derived_methods.rs` and LLVM `derive_codegen/`, both independently matching on `DerivedTrait` variants

---

## 03.5 Operator Dispatch Skeleton

**File(s):** `compiler/ori_eval/src/operators/mod.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/operators/mod.rs`

Both backends dispatch binary and unary operators with parallel match arms on `BinaryOp`/`UnaryOp` variants:
- Eval: operator dispatch in `operators/mod.rs`
- LLVM: `emit_binary_op()` at line 26 in `operators/mod.rs`

The routing (which op maps to which implementation) parallels the registry's `OpDefs` strategy fields.

- [ ] **LEAK:algorithmic-duplication** -- Operator dispatch routing duplicated between eval and LLVM backends; both independently map `BinaryOp` variants to type-specific implementations
- [ ] **LEAK:algorithmic-duplication** -- Exhaustiveness guards (handling of `Never`/`Error` types in operator dispatch) duplicated between backends

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] Iterator method dispatch in both backends is validated against registry `MethodDef` list
- [ ] Option/Result routing metadata is shared (method names, operation types defined once)
- [ ] Derive dispatch is driven by shared `DeriveStrategy` metadata
- [ ] Adding a new operator or method to the registry does not require parallel independent updates in both backends' routing logic
- [ ] `timeout 150 ./test-all.sh` passes with zero regressions
- [ ] `./clippy-all.sh` passes
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 03` returns 0 annotations
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** Shared dispatch metadata exists for all 7 finding areas. Both backends consume the shared metadata. No routing decisions (which methods exist, which ops are valid) are independently maintained in parallel. `./test-all.sh` green.
