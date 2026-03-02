---
section: "05"
title: "Float Narrowing Pipeline"
status: not-started
goal: "Lower float (semantic f64) to f32 when the compiler can prove zero precision loss for the specific values used"
inspired_by:
  - "LLVM fptrunc/fpext analysis (lib/Transforms/InstCombine/InstCombineCasts.cpp)"
  - "GCC -fexcess-precision semantics (gcc/convert.cc)"
depends_on: ["03"]
sections:
  - id: "05.1"
    title: "Precision Analysis"
    status: not-started
  - id: "05.2"
    title: "Float Narrowing Conditions"
    status: not-started
  - id: "05.3"
    title: "LLVM Integration"
    status: not-started
  - id: "05.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Float Narrowing Pipeline

**Status:** Not Started
**Goal:** When a `float` value is provably representable as `f32` without precision loss, use `f32` in generated code. This halves memory for float-heavy data structures and enables SIMD vectorization on hardware that processes 2× more f32 operations per cycle.

**Context:** Float narrowing is much more constrained than integer narrowing because floating-point precision is non-linear. The set of values exactly representable in f32 is a strict subset of f64. Narrowing is only safe when:
1. All literal values are exactly representable in f32
2. All operations produce results exactly representable in f32
3. The accumulated rounding error difference between f32 and f64 paths is provably zero

In practice, this means float narrowing is mostly useful for:
- Constants that happen to be f32-exact (0.0, 1.0, 0.5, integer-valued floats up to 2²⁴)
- Pure storage/retrieval without arithmetic (data transfer)
- Graphics/audio where f32 precision is sufficient by domain knowledge

**Reference implementations:**
- **LLVM** `InstCombineCasts.cpp`: `canEvaluateTruncated()` — checks if fptrunc is lossless
- **GCC** `convert.cc`: Excess precision handling for C11 semantics

**Depends on:** §03 (range analysis, extended to float intervals).

---

## 05.1 Precision Analysis

**File(s):** `compiler/ori_repr/src/narrowing/float.rs`

- [ ] Define `FloatRange`:
  ```rust
  pub enum FloatRange {
      /// No info (keep f64)
      Top,
      /// All values are exactly representable in f32
      F32Exact,
      /// Value is a known constant
      Constant(f64),
      /// All values are integers in [-2²⁴, 2²⁴] (f32-exact integer range)
      IntegerValued { lo: i64, hi: i64 },
  }
  ```

- [ ] Implement f32 exactness check:
  ```rust
  pub fn is_f32_exact(value: f64) -> bool {
      let as_f32 = value as f32;
      let roundtripped = as_f32 as f64;
      roundtripped == value && !value.is_nan()
  }
  ```

- [ ] Implement operation precision tracking:
  ```rust
  /// Can this operation produce f32-exact results from f32-exact inputs?
  pub fn preserves_f32_precision(op: ArithOp) -> bool {
      match op {
          // Addition/subtraction of f32-exact values may not be f32-exact
          // (due to rounding). Only safe if we can bound the result.
          ArithOp::Add | ArithOp::Sub => false, // conservative
          ArithOp::Mul => false, // product may exceed f32 precision
          ArithOp::Div => false, // quotient may not be f32-exact
          ArithOp::Neg => true,  // negation is exact
      }
  }
  ```

---

## 05.2 Float Narrowing Conditions

**File(s):** `compiler/ori_repr/src/narrowing/float.rs`

Float narrowing is only applied under very strict conditions to avoid precision bugs.

- [ ] Define narrowing eligibility:
  ```rust
  pub fn can_narrow_to_f32(var: VarId, analysis: &FloatAnalysis) -> bool {
      let range = analysis.float_range(var);
      match range {
          FloatRange::Constant(v) => is_f32_exact(v),
          FloatRange::IntegerValued { lo, hi } => {
              // f32 can exactly represent integers up to 2^24
              lo >= -(1 << 24) && hi <= (1 << 24)
          }
          FloatRange::F32Exact => true,
          FloatRange::Top => false,
      }
  }
  ```

- [ ] Storage-only narrowing (most practical use case):
  - Float is stored in a struct field or collection but never used in arithmetic
  - All stored values are f32-exact (e.g., from parsing f32 input data)
  - Narrowing saves memory without affecting computation

- [ ] Arithmetic narrowing (aggressive, opt-in via `#[repr(f32)]` attribute):
  - Future: allow the programmer to annotate that f32 precision is acceptable
  - This is a semantic change (different rounding) — requires explicit opt-in
  - Not part of the automatic optimization pipeline

---

## 05.3 LLVM Integration

**File(s):** `compiler/ori_llvm/src/codegen/type_info/info.rs`

- [ ] Modify `TypeInfo::storage_type()` for float:
  ```rust
  TypeInfo::Float => match repr_plan.float_width(idx) {
      FloatWidth::F32 => context.f32_type().into(),
      FloatWidth::F64 => context.f64_type().into(),
  },
  ```

- [ ] Insert `fpext`/`fptrunc` at boundaries:
  - Load from f32 field → `fpext float to double` for computation
  - Store to f32 field → `fptrunc double to float` after computation
  - Function boundaries → always use f64 (canonical)

---

## 05.4 Completion Checklist

- [ ] `is_f32_exact()` correctly identifies all f32-representable f64 values
- [ ] Constants like `0.0`, `1.0`, `0.5`, `100.0` → narrowed to f32 in storage
- [ ] Arithmetic on f64 values is NEVER narrowed (conservative by default)
- [ ] `struct Color { r: float, g: float, b: float }` with values `0.0..1.0` uses f32 fields
- [ ] `fpext`/`fptrunc` visible at load/store boundaries in LLVM IR
- [ ] `./diagnostics/dual-exec-verify.sh` passes (no precision differences)
- [ ] `./test-all.sh` green

**Exit Criteria:** A program storing constant `0.5` in a struct field uses `float` (f32) in LLVM IR instead of `double` (f64), verified by inspecting generated IR. All floating-point spec tests continue to pass with bit-identical results.
