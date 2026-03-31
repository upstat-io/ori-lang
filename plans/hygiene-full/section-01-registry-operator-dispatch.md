---
section: "01"
title: "Registry as Universal SSOT (Operator Dispatch)"
status: in-progress
reviewed: true
goal: "Type checker and evaluator query ori_registry OpDefs instead of maintaining independent operator dispatch tables"
inspired_by:
  - "ori_registry operator/mod.rs OpDefs pattern — per-type operator strategy with compile-time coverage"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Typeck Arithmetic Operator Validation via Registry"
    status: complete
  - id: "01.2"
    title: "Typeck Unary Operator Validation via Registry"
    status: complete
  - id: "01.3"
    title: "Typeck Bitwise Operator Restrictions via Registry"
    status: complete
  - id: "01.4"
    title: "Typeck Comparison Operator Validation via Registry"
    status: complete
  - id: "01.5"
    title: "Evaluator Operator Dispatch Alignment"
    status: complete
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 01: Registry as Universal SSOT (Operator Dispatch)

**Status:** Not Started
**Goal:** The type checker (`ori_types`) and evaluator (`ori_eval`) query `ori_registry` `OpDefs` for operator validity instead of maintaining three independent operator dispatch tables. After this section, changing which operators a type supports requires modifying only the registry.

**Context:** The type checker's `infer_binary()` and `infer_unary()` in `compiler/ori_types/src/infer/expr/operators.rs` hardcode operator behavior per primitive type using inline `match` arms and tag checks. The registry already defines `OpDefs` with `OpStrategy` per operator per type (`compiler/ori_registry/src/operator/mod.rs`), but the type checker does not query it. This creates three independent sources of truth for operator validity: the registry, the type checker, and the evaluator.

**Reference implementations:**
- **ori_registry** `compiler/ori_registry/src/operator/mod.rs`: `OpDefs` struct with 20 `OpStrategy` fields -- the canonical source of truth
- **ori_registry** `compiler/ori_registry/src/defs/int.rs`: Example `INT` type definition with full `OpDefs`

**Depends on:** None.

**Note:** `ori_types` already depends on `ori_registry` (Cargo.toml) and has an existing `registry_bridge::tag_to_type_tag()` function in `compiler/ori_types/src/infer/expr/methods/mod.rs` that converts `Tag` to `TypeTag`. The same bridge can be used for operator queries. The `_enforce_exhaustiveness` pattern at line 110 ensures compile-time coverage when new `TypeTag` variants are added.

**Test strategy:** This section is pure refactoring -- no behavioral changes. The test matrix is the existing test suite: `./test-all.sh` must pass unchanged. Additionally:
- Semantic pin: add a Rust unit test that queries the registry for each primitive type's operator support and asserts it matches the previous hardcoded behavior
- Negative pin: add a `#compile_fail` Ori spec test that verifies unsupported operators are still rejected (e.g., `"hello" % "world"` should error)

---

## 01.1 Typeck Arithmetic Operator Validation via Registry

**File(s):** `compiler/ori_types/src/infer/expr/operators.rs` (lines 33-118)

The `infer_binary()` function hardcodes arithmetic operator behavior for Duration/Size/Int/Float/Str/List combinations in a `match (left_tag, right_tag, op)` block (lines 48-72). These special-case rules should be derivable from the registry's `OpDefs` -- the registry already knows which types support which arithmetic operators.

- [x] **LEAK:scattered-knowledge** `operators.rs:48-72` -- Duration/Size mixed-type arithmetic rules extracted to `check_cross_type_arithmetic()`, same-type validated via registry `OpDefs`
- [x] **LEAK:scattered-knowledge** `operators.rs:58` -- String concatenation now validated via registry query (`str.operators.add: RuntimeCall`)
- [x] **LEAK:scattered-knowledge** `operators.rs:60-66` -- List concatenation now validated via registry query (added `list.operators.add: RuntimeCall` to registry)
- [x] Added `binary_op_strategy()` and `is_binary_op_supported()` in `registry_bridge/mod.rs` as the primary registry query path for `infer_binary()`

---

## 01.2 Typeck Unary Operator Validation via Registry

**File(s):** `compiler/ori_types/src/infer/expr/operators.rs` (lines 355-500)

The `infer_unary()` function hardcodes which types support negation, logical NOT, and bitwise NOT:
- Negation (`UnaryOp::Neg`): hardcodes `Tag::Int | Tag::Float | Tag::Duration` at line 371
- Logical NOT (`UnaryOp::Not`): hardcodes `Tag::Bool` at line 407
- Bitwise NOT (`UnaryOp::BitNot`): hardcodes `Tag::Int` at line 443

The registry already has `OpDefs.neg`, `OpDefs.not`, and `OpDefs.bit_not` fields with per-type strategies.

- [x] **LEAK:scattered-knowledge** `operators.rs:371` -- Negation now validated via registry `OpDefs.neg` query
- [x] **LEAK:scattered-knowledge** `operators.rs:407` -- NOT now validated via registry `OpDefs.not` query
- [x] **LEAK:scattered-knowledge** `operators.rs:443` -- BitNot now validated via registry `OpDefs.bit_not` query

---

## 01.3 Typeck Bitwise Operator Restrictions via Registry

**File(s):** `compiler/ori_types/src/infer/expr/operators.rs` (lines 198-264)

Bitwise operators (`BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`) hardcode `Tag::Int` as the only valid primitive type (line 206). The registry has per-type `bit_and`, `bit_or`, `bit_xor`, `shl`, `shr` fields that could drive this validation.

- [x] **LEAK:scattered-knowledge** `operators.rs:206` -- Bitwise operators now validated via registry `OpDefs.bit_and/bit_or/bit_xor/shl/shr` queries

---

## 01.4 Typeck Comparison Operator Validation via Registry

**File(s):** `compiler/ori_types/src/infer/expr/operators.rs` (lines 120-196)

Comparison operators (`Eq`, `NotEq`, `Lt`, `LtEq`, `Gt`, `GtEq`) and boolean operators (`And`, `Or`) are handled with hardcoded type checks. Comparisons accept any type that unifies (lines 120-141), but the registry has per-type `eq`, `lt`, etc. fields that indicate actual support. Boolean operators hardcode `Tag::Bool` (lines 151-152).

- [x] **LEAK:scattered-knowledge** `operators.rs:144-196` -- Boolean And/Or are not tracked in `OpDefs` (language-level boolean operators, not overloadable); `Tag::Bool` restriction is inherent and correct. Documented in code comment.
- [x] **LEAK:scattered-knowledge** `operators.rs:120-141` -- Comparison operators now validated via registry `OpDefs.eq/lt/gt` for primitive types (e.g., rejects `Ordering < Ordering`). Compound/user types still use trait dispatch.

---

## 01.5 Evaluator Operator Dispatch Alignment

**File(s):** `compiler/ori_eval/src/operators/mod.rs`

The evaluator has its own operator dispatch (e.g., `eval_option_binary` at line 277, `eval_result_binary` at line 311) that parallels both the type checker's dispatch and the registry's `OpDefs`. While the evaluator necessarily has its own execution logic, the *routing* decisions (which operators are valid for which types) should be validated against the registry.

- [x] **LEAK:duplicated-dispatch** `operators/mod.rs` -- Added registry sync enforcement tests; fixed float `%` and bool ordering bugs found by tests

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [x] Type checker queries `ori_registry` `OpDefs` for operator validity on primitive types
- [x] Adding a new operator to a type's `OpDefs` in the registry is sufficient to make the type checker accept it (no parallel edits needed in `operators.rs`)
- [x] Evaluator operator routing decisions are consistent with registry `OpDefs`
- [x] No hardcoded `Tag::Int`, `Tag::Float`, `Tag::Bool` checks for operator validity remain in `operators.rs` (replaced by registry queries)
- [x] `timeout 150 ./test-all.sh` passes with zero regressions
- [x] `./clippy-all.sh` passes
- [x] Plan annotation cleanup: no hygiene-full annotations in source code (verified via grep)
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** `operators.rs` no longer contains hardcoded per-type operator validity checks. All operator validity decisions flow through `ori_registry::OpDefs`. The registry is the single source of truth for which operators each type supports. `./test-all.sh` green.
