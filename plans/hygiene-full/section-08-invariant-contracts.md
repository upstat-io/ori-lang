---
section: "08"
title: "Cross-Phase Invariant Contracts"
status: not-started
reviewed: false
goal: "Add debug_assert validation for all cross-phase invariant contracts listed in impl-hygiene.md"
inspired_by:
  - "Zig Compilation.zig -- explicit validation passes between compiler phases"
  - "Swift SILVerifier -- invariant verification at phase boundaries"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "08.1"
    title: "Type Variable Resolution Before Codegen"
    status: not-started
  - id: "08.2"
    title: "RC Balance After ARC Pass"
    status: not-started
  - id: "08.3"
    title: "Error Node Filtering Before Codegen"
    status: not-started
  - id: "08.4"
    title: "TypeId/Idx Boundary Sync"
    status: not-started
  - id: "08.5"
    title: "ABI FIXME Resolution"
    status: not-started
  - id: "08.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "08.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Cross-Phase Invariant Contracts

**Status:** Not Started
**Goal:** Every cross-phase invariant contract listed in `impl-hygiene.md` has a corresponding `debug_assert!` or validation pass at the consumer's entry point. No implicit invariants remain.

**Context:** The `impl-hygiene.md` rules document explicit cross-phase contracts (type checker -> codegen: all type variables resolved, ARC pass -> codegen: RC ops balanced, canon -> all: no sugar variants). Currently, many of these contracts are implicit -- violated silently in release builds. Adding explicit validation catches corruption at the phase boundary rather than at the point of wrong code emission.

**Depends on:** None.

**Test strategy:** These are `debug_assert!` additions -- they add verification, not behavioral changes. Testing:
- `timeout 150 cargo t` in debug mode -- debug_asserts fire and must not trip on any existing test
- `timeout 150 cargo t --release` -- must also pass (asserts stripped, no behavioral change)
- Intentional violation test: if possible, add a Rust test that constructs IR with an unresolved type variable and passes it to the assertion, verifying it panics in debug mode

---

## 08.1 Type Variable Resolution Before Codegen

**File(s):** `compiler/ori_llvm/src/codegen/` entry point, `compiler/ori_llvm/src/evaluator/compile.rs`

Contract: "All type variables resolved -- No `Idx` with `Tag::Var` in typed IR" (from `impl-hygiene.md` cross-phase contracts table).

Currently, no `debug_assert!` validates this contract at codegen entry. An unresolved type variable reaching codegen would produce wrong LLVM IR silently.

- [ ] **GAP** -- No `debug_assert!` at codegen entry verifying all type variables are resolved (no `Tag::Var` remaining in the type pool for function types)
- [ ] Add a validation pass or `debug_assert!` at the codegen entry point that scans function signatures and body types for unresolved `Tag::Var`

---

## 08.2 RC Balance After ARC Pass

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline.rs` (after step 5/step 11)

Contract: "RC ops balanced per function -- Every `rc_inc` has a matching `rc_dec` on all paths" (from `impl-hygiene.md`).

The ARC pipeline has `verify()` steps (steps 6 and 11) but these may not check RC balance specifically. A `debug_assert!` after the AIMS pipeline verifying that every `RcInc` has a matching `RcDec` on all control flow paths would catch balance violations before they reach codegen.

- [ ] **GAP** -- No explicit `debug_assert!` for RC balance (matched inc/dec pairs) after the ARC pipeline; `verify()` may check IR well-formedness but not semantic RC balance
- [ ] Add or verify that the existing `verify()` steps include RC balance checking; if not, add a dedicated balance check

---

## 08.3 Error Node Filtering Before Codegen

**File(s):** `compiler/ori_llvm/src/codegen/` entry point

Contract: "Error nodes marked -- Error recovery nodes carry error marker" + "codegen requires error-free input" (from `impl-hygiene.md`).

If error nodes reach codegen without being filtered, codegen may emit incorrect LLVM IR or crash. A `debug_assert!` at codegen entry verifying no error nodes remain would catch filtering failures.

- [ ] **GAP** -- No `debug_assert!` at codegen entry verifying that no error nodes (`TypeId::ERROR`, `Idx::ERROR`) remain in the IR being compiled
- [ ] Add a validation check that the IR passed to codegen contains no error-typed expressions or error nodes

---

## 08.4 TypeId/Idx Boundary Sync

**File(s):** `compiler/ori_ir/src/type_id/mod.rs`, `compiler/ori_types/src/idx.rs`

`TypeId::FIRST_COMPOUND = 64` (in `ori_ir`) and `Idx::FIRST_DYNAMIC` (in `ori_types`) represent the boundary between pre-interned primitive types and user-defined types. These must stay in sync. Currently there is no compile-time or test-time assertion relating them.

- [ ] **GAP** -- `TypeId::FIRST_COMPOUND` (64) and `Idx::FIRST_DYNAMIC` are semantically related boundary constants with no sync assertion
- [ ] Add a const assertion or test that verifies the relationship between `TypeId::FIRST_COMPOUND` and `Idx::FIRST_DYNAMIC` (they should be equal or have a documented mapping)

---

## 08.5 ABI FIXME Resolution

**File(s):** `compiler/ori_llvm/src/codegen/abi/`

Review and resolve any ABI-related FIXME comments that represent deferred invariant decisions. ABI mismatches between caller and callee conventions are one of the most dangerous silent corruption vectors.

- [ ] **GAP** -- ABI FIXME comments representing deferred design decisions that could cause calling convention mismatches
- [ ] Audit all FIXME/TODO comments in `abi/` and either resolve them or document them as accepted limitations with test coverage

---

## 08.R Third Party Review Findings

- None.

---

## 08.N Completion Checklist

- [ ] `debug_assert!` at codegen entry: no unresolved type variables
- [ ] `debug_assert!` or verification pass: RC ops balanced after ARC pipeline
- [ ] `debug_assert!` at codegen entry: no error nodes in IR
- [ ] Const assertion or test: `TypeId::FIRST_COMPOUND` and `Idx::FIRST_DYNAMIC` sync
- [ ] ABI FIXME comments audited and resolved or documented
- [ ] `timeout 150 ./test-all.sh` passes in both debug and release
- [ ] `./clippy-all.sh` passes
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 08` returns 0 annotations
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** Every cross-phase contract in the `impl-hygiene.md` table has a corresponding validation mechanism. `./test-all.sh` green in both debug (assertions active) and release (assertions stripped).
