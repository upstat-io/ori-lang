---
section: "01"
title: "Emission Layer Typing"
status: not-started
goal: "Eliminate type-pool lookups during LLVM emission by pre-computing value representations"
inspired_by:
  - "Rust OperandValue (rustc_codegen_llvm/mir/operand.rs)"
  - "Lean 4 type-indexed phases (Compiler/IR/)"
sections:
  - id: "01.1"
    title: "Add ValueRepr to ARC IR"
    status: not-started
  - id: "01.2"
    title: "Add EmittedValue to ArcIrEmitter"
    status: not-started
  - id: "01.3"
    title: "Propagate ValueRepr through ARC passes"
    status: not-started
  - id: "01.4"
    title: "Tests & validation"
    status: not-started
---

# Section 01: Emission Layer Typing

**Status:** Not Started
**Goal:** Every ARC IR value carries its memory representation; the LLVM emitter never queries TypeInfo to decide how to load/store a value.

**Why this matters:** The DPR identified this as the #1 source of pain. Currently, `ArcIrEmitter::emit_instr` produces raw `ValueId` values and must re-derive "is this a pointer? a scalar? an aggregate?" by querying `TypeInfo` at every use site. This causes load/store confusion bugs and makes the 2,223-line `arc_emitter/mod.rs` harder to reason about than it needs to be. Rust's `OperandValue` enum eliminates this entire class of bugs by making representation explicit at the type level.

---

## 01.1 Add `ValueRepr` to ARC IR

**File:** `compiler/ori_arc/src/ir/mod.rs`

- [ ] Define `ValueRepr` enum in `ir/mod.rs`:
  ```rust
  /// How a value is represented in memory.
  /// Computed during lowering from ArcClassifier + Pool tag.
  /// Backend-independent — describes memory layout, not LLVM types.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  pub enum ValueRepr {
      /// Fits in a register: i64, f64, i1, i8, i32. No RC management.
      Scalar,
      /// Heap-allocated, reference-counted. data_ptr - 8 is the refcount.
      RcPointer,
      /// Stack aggregate: struct/tuple/enum passed by value.
      /// May contain RC'd fields requiring drop.
      Aggregate,
      /// Two-word fat value: {metadata, rc_pointer}.
      /// str = {len, data_ptr}, closure = {fn_ptr, env_ptr}.
      FatValue,
  }
  ```

- [ ] Add `repr: ValueRepr` field to all value-producing `ArcInstr` variants:
  - `Let { dst, value, repr }` — repr from the value's type
  - `Apply { dst, func, args, repr }` — repr from return type
  - `ApplyIndirect { dst, callee, args, repr }` — repr from return type
  - `PartialApply { dst, func, captures, repr }` — always `FatValue`
  - `Project { dst, base, field, repr }` — repr from field type
  - `Construct { dst, ctor, args, repr }` — repr from constructed type
  - `IsShared { dst, var }` — always `Scalar` (i1)
  - `Reuse { dst, token, ctor, args, repr }` — repr from constructed type

- [ ] Add `ValueRepr::from_classification()` bridge:
  ```rust
  impl ValueRepr {
      pub fn from_arc_class(class: ArcClass, pool: &Pool, idx: Idx) -> Self {
          match class {
              ArcClass::Scalar => ValueRepr::Scalar,
              ArcClass::DefiniteRef | ArcClass::PossibleRef => {
                  // Check if this is a fat value (str, closure)
                  if pool.is_str(idx) || pool.is_closure(idx) {
                      ValueRepr::FatValue
                  } else if pool.is_aggregate(idx) {
                      ValueRepr::Aggregate
                  } else {
                      ValueRepr::RcPointer
                  }
              }
          }
      }
  }
  ```

- [ ] Store per-variable repr in `ArcFunction`:
  ```rust
  pub struct ArcFunction {
      pub entry: ArcBlockId,
      pub var_types: Vec<Idx>,       // existing
      pub var_reprs: Vec<ValueRepr>, // NEW — parallel to var_types
      pub blocks: Vec<ArcBlock>,
      // ...
  }
  ```

---

## 01.2 Add `EmittedValue` to ArcIrEmitter

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

- [ ] Define `EmittedValue` enum:
  ```rust
  /// Tagged LLVM value with representation info.
  /// Prevents "did I load this already?" class of bugs.
  #[derive(Clone, Copy, Debug)]
  enum EmittedValue {
      /// Register scalar: i64, f64, i1, i8, i32.
      Immediate(ValueId),
      /// Pointer to heap-allocated RC'd memory.
      RcPointer(ValueId),
      /// Stack aggregate (struct, tuple, enum by value).
      Aggregate(ValueId),
      /// Two-word: {first, second} — str={len,ptr}, closure={fn,env}.
      Pair { first: ValueId, second: ValueId },
      /// No runtime representation (unit, never).
      ZeroSized,
  }
  ```

- [ ] Add helper methods on `EmittedValue`:
  - `into_raw(self) -> ValueId` — extract single ValueId (panics on Pair/ZeroSized)
  - `rc_data_ptr(self) -> Option<ValueId>` — get RC-trackable pointer if any
  - `is_rc_managed(self) -> bool` — true for RcPointer and Pair (with rc second)
  - `from_repr(repr: ValueRepr, value: ValueId) -> Self` — bridge from ARC IR repr

- [ ] Replace `var_map: Vec<Option<ValueId>>` with `var_map: Vec<Option<EmittedValue>>`

- [ ] Update `emit_instr()` to produce `EmittedValue`:
  - `Let` with `Scalar` repr → `EmittedValue::Immediate(value)`
  - `Let` with `RcPointer` repr → `EmittedValue::RcPointer(value)`
  - `Construct` with `Aggregate` repr → `EmittedValue::Aggregate(alloca)`
  - `PartialApply` → `EmittedValue::Pair { first: fn_ptr, second: env_ptr }`
  - `IsShared` → `EmittedValue::Immediate(i1_result)`

- [ ] Update all consumers to destructure `EmittedValue`:
  - `emit_terminator` — `Return` extracts based on variant
  - `emit_apply` — arg passing based on variant (Immediate→direct, Aggregate→pointer, Pair→split)
  - `emit_invoke` — same as emit_apply
  - `emit_rc_inc`/`emit_rc_dec` — only operates on `RcPointer`/`Pair.second`
  - `emit_construct` — field values based on variant

---

## 01.3 Propagate ValueRepr Through ARC Passes

**Files:** All pass modules in `compiler/ori_arc/src/`

- [ ] Update `lower_function_can()` to populate `var_reprs` for every variable created during lowering
  - Each `fresh_var()` call must also compute and store `ValueRepr`
  - The `ArcClassifier` is already available in the lowering context

- [ ] Update RC insertion (`rc_insert/mod.rs`) to preserve `var_reprs` when creating new variables for RC ops
  - `RcInc`/`RcDec` don't produce values, so no repr needed
  - Any new `Let` instructions from expansion need repr

- [ ] Update reset/reuse detection and expansion to preserve `var_reprs`
  - `Reset`/`Reuse` instructions produce values — assign repr from the reused type

- [ ] Update RC elimination to use `var_reprs` for sanity checks
  - `debug_assert!` that `RcInc`/`RcDec` only target variables with `RcPointer`/`FatValue`/`Aggregate` repr

---

## 01.4 Tests & Validation

- [ ] Unit tests in `compiler/ori_arc/src/ir/tests.rs`:
  - `ValueRepr::from_arc_class` for all type categories
  - Round-trip: lower → check `var_reprs` matches expected for each type

- [ ] Unit tests in `compiler/ori_llvm/src/codegen/arc_emitter/tests.rs`:
  - `EmittedValue` helper methods (into_raw, rc_data_ptr, is_rc_managed)
  - Verify emit_instr produces correct EmittedValue variant for each ArcInstr

- [ ] AOT integration tests in `compiler/ori_llvm/tests/aot/`:
  - Existing tests must continue passing (no behavioral change)
  - Add test exercising all ValueRepr variants in one function

- [ ] Run `./test-all.sh` — zero regressions

---

## 01.5 Completion Checklist

- [ ] `ValueRepr` enum defined in `ir/mod.rs`
- [ ] `var_reprs: Vec<ValueRepr>` in `ArcFunction`
- [ ] All lowering paths populate `var_reprs`
- [ ] All ARC passes preserve `var_reprs`
- [ ] `EmittedValue` enum defined in `arc_emitter/mod.rs`
- [ ] `var_map` uses `EmittedValue` instead of `ValueId`
- [ ] All emit_* methods produce/consume `EmittedValue`
- [ ] No remaining `TypeInfo` queries in emit_instr for representation decisions
- [ ] All tests pass
- [ ] `./clippy-all.sh` clean

**Exit Criteria:** The LLVM emitter never calls TypeInfo to determine whether a value is a scalar, pointer, aggregate, or fat value. All representation decisions come from `ValueRepr` (set during lowering) and `EmittedValue` (set during emission).
