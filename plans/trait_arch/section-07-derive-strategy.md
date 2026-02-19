---
section: "07"
title: Shared Derive Strategy
status: complete
goal: Eliminate eval/LLVM derive implementation duplication via a shared derivation strategy pattern
sections:
  - id: "07.1"
    title: Duplication Analysis
    status: complete
  - id: "07.2"
    title: Strategy Pattern Design
    status: complete
  - id: "07.3"
    title: Eval Backend Adaptation
    status: complete
  - id: "07.4"
    title: LLVM Backend Adaptation
    status: complete
  - id: "07.5"
    title: Completion Checklist
    status: complete
---

# Section 07: Shared Derive Strategy

**Status:** Complete
**Goal:** Define the logical structure of each trait derivation once in `ori_ir`, then have eval and LLVM backends interpret that structure. Adding a new derived trait means defining its strategy once, not implementing the same logic twice in different representations.

**Depends on:** Section 01 (trait metadata), Section 04 (LLVM derive factory)

**Reference compilers:**
- **Swift** `lib/Sema/DerivedConformances.cpp` — Per-trait strategy classes (`DerivedConformance_Equatable.cpp`, `DerivedConformance_Hashable.cpp`) define derivation logic; the codegen layer interprets.
- **Rust** `compiler/rustc_builtin_macros/src/deriving/` — Per-trait derivation modules (`eq.rs`, `hash.rs`, `cmp.rs`) generate AST expansion; shared via `TraitDef` struct.
- **Lean 4** `src/Lean/Compiler/IR/RC.lean` — Derivation strategies expressed as IR transformations, interpreted by multiple backends.

**Current state:** Both backends now use strategy-driven dispatch via `DerivedTrait::strategy()`. The derivation algorithm is defined once in `ori_ir`, interpreted by eval and LLVM backends through 4 generic functions each.

---

## 07.1 Duplication Analysis

### Logical Structure Comparison

| Trait | Eval Logic | LLVM Logic | Shared Pattern |
|-------|-----------|-----------|----------------|
| **Eq** | For each field: `field1 == field2`, short-circuit on `false` | For each field: emit `icmp eq`, branch on `false` | ForEachField + AllEqual |
| **Clone** | For each field: clone value, reconstruct struct | For each field: copy/RC-inc, construct struct | ForEachField + CloneField |
| **Hashable** | For each field: `hash_combine(acc, hash(field))` | For each field: emit hash call, emit combine | ForEachField + HashCombine |
| **Printable** | Format as "TypeName(field1, field2, ...)" | Emit string concat calls | FormatFields("(", ", ", ")") |
| **Debug** | Format as "TypeName { f1: v1, f2: v2 }" | Emit string concat calls with field names | FormatFields("{ ", ", ", " }") |
| **Default** | For each field: produce default value | For each field: emit zero/default | ForEachField + DefaultValue |
| **Comparable** | For each field: compare, short-circuit on non-Equal | For each field: emit compare, branch on non-Equal | ForEachField + LexicographicCmp |

### What's Actually Duplicated

The **field iteration logic** and **composition strategy** are identical. What differs is the **primitive operations** (eval uses `Value::eq()`, LLVM emits `icmp eq`). The abstraction captures the iteration/composition, not the primitives.

---

## 07.2 Strategy Pattern Design

### Core Types

Defined in `compiler/ori_ir/src/derives/strategy.rs`:

- `DeriveStrategy` — top-level: `struct_body: StructBody`, `sum_body: SumBody`
- `StructBody` — `ForEachField { field_op, combine }`, `FormatFields { open, separator, suffix, include_names }`, `DefaultConstruct`, `CloneFields`
- `FieldOp` — `Equals`, `Compare`, `Hash`
- `CombineOp` — `AllTrue`, `Lexicographic`, `HashCombine`
- `FormatOpen` — `TypeNameParen`, `TypeNameBrace`
- `SumBody` — `MatchVariants`, `NotSupported`

### Design Decisions

- Dropped `InitialValue` enum from plan — each `CombineOp` has a fixed initial value (true, Equal, FNV offset basis), making it redundant
- Named `FieldOp::Equals` (not `Eq`) to avoid confusion with Rust's `#[derive(Eq)]`
- Named `FormatOpen` (not `FormatPrefix`) — it's the opening delimiter, not a prefix string
- Named `StructBody`/`SumBody` (not `StructStrategy`/`SumStrategy`) — cleaner naming, avoids "strategy" overload

- [x] Define `DeriveStrategy`, `StructBody`, `SumBody`, `FieldOp`, `CombineOp`, `FormatOpen` types in `ori_ir/derives/strategy.rs`
- [x] Implement `DerivedTrait::strategy()` for all 7 traits
- [x] Unit tests: each trait's strategy matches expected structure (9 tests)
- [x] Verify all types are Salsa-compatible (`Clone, Debug, PartialEq, Eq, Hash`)

---

## 07.3 Eval Backend Adaptation

Rewrote `ori_eval/src/interpreter/derived_methods.rs` from 548 → ~390 lines:

- 7-arm per-trait dispatch → 4-arm `StructBody` match
- `eval_for_each_field()` routes to `for_each_struct`, `for_each_variant_binary`, `for_each_variant_unary`
- `eval_format_fields()` handles both Printable and Debug via `include_names` flag
- Clone is a one-liner: `Ok(receiver)` (owned value IS the clone)
- DefaultConstruct kept same logic

- [x] Implement `eval_for_each_field()` using strategy types
- [x] Implement `eval_format_fields()` using strategy types
- [x] Implement `eval_default_construct()` using strategy types
- [x] Implement `eval_clone_fields()` using strategy types
- [x] Equivalence tests: strategy-driven output == per-trait output for all 7 traits
- [x] Replace per-trait dispatch with strategy dispatch
- [x] Delete `eval_derived_eq()`, `eval_derived_hash()`, etc. (6 functions)
- [x] `./test-all.sh` passes (3810 spec tests, 0 failures)

---

## 07.4 LLVM Backend Adaptation

Refactored `ori_llvm/src/codegen/derive_codegen/`:

- **field_ops.rs**: Removed local `FieldOp` enum, now uses `ori_ir::FieldOp` (eliminated a drift source)
- **bodies.rs**: Replaced 6 per-trait functions with 4 strategy-driven functions:
  - `compile_for_each_field()` — dispatches to `emit_all_true_body`, `emit_lexicographic_body`, `emit_hash_combine_body`
  - `compile_format_fields()` — parameterized by `FormatOpen`, separator, suffix, include_names
  - `compile_clone_fields()` — identity return
  - `compile_default_construct()` — zero-initialized struct
- **mod.rs**: Replaced 7-arm `match trait_kind` with 4-arm `match strategy.struct_body`
- **string_helpers.rs**: `emit_field_to_string` now accepts `trait_kind` to select the right method on nested structs and quote strings for Debug
- **Bonus**: Debug now works in LLVM codegen (was previously deferred/skipped)

- [x] Implement `compile_for_each_field()` using strategy types
- [x] Implement `compile_format_fields()` using strategy types
- [x] Implement `compile_default_construct()` using strategy types
- [x] Implement `compile_clone_fields()` using strategy types
- [x] Refactor `compile_derives()` to use strategy dispatch
- [x] Delete per-trait `compile_derive_*()` functions (6 functions)
- [x] `./llvm-test.sh` passes (375 passed, 7 pre-existing failures unrelated to derives)
- [x] `./test-all.sh` passes (all derive tests pass, LLVM spec crash is pre-existing)

---

## 07.5 Completion Checklist

- [x] `DeriveStrategy` and related types defined in `ori_ir/derives/strategy.rs`
- [x] `DerivedTrait::strategy()` returns correct strategy for all 7 traits
- [x] Eval backend uses strategy-driven dispatch, not per-trait handlers
- [x] LLVM backend uses strategy-driven dispatch, not per-trait handlers
- [x] Both backends produce identical output to the previous per-trait implementations
- [x] Per-trait handler functions deleted from both backends
- [x] Adding a new derived trait with `ForEachField` strategy requires zero eval/LLVM code changes — just the strategy definition
- [x] Adding a new derived trait with a novel strategy (new `StructBody` variant) requires one eval handler + one LLVM handler + the strategy definition
- [x] Unit tests: strategy correctness for all 7 traits
- [x] Integration tests: derived trait behavior unchanged for all spec tests
- [x] `./test-all.sh` passes with zero regressions

**Exit Criteria:** The derivation algorithm is defined once (strategy), interpreted twice (eval, LLVM). Adding Eq-like traits (ForEachField with AllTrue) requires zero backend changes. The codebase has one place that describes "how Eq derives work" — not two places implementing it independently.
