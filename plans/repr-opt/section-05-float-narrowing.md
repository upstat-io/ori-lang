---
section: "05"
title: "Float Narrowing Pipeline"
status: not-started
reviewed: false
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

**Note:** `f64` does not implement `Eq` or `Hash`. If `FloatRange` is ever used as a map key or Salsa query input, the `Constant` variant must store bits as `u64` or use `OrderedFloat<f64>`.

- [ ] Define `FloatRange`:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq)]
  pub enum FloatRange {
      /// No info (keep f64)
      Top,
      /// All values are exactly representable in f32
      F32Exact,
      /// Value is a known constant (stored as bits for Hash/Eq if needed)
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

- [ ] Arithmetic narrowing (aggressive, opt-in via `#repr("f32")` attribute):
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

**Test matrix for §05 (write failing tests FIRST, verify they fail, then implement):**

| Input pattern | Expected narrowing | Semantic pin |
|---|---|---|
| Struct field `r: float` where all stored values are `0.0` | `float` (f32) in LLVM struct | Yes — no `double` in LLVM struct type |
| Struct field `scale: float` storing constant `0.5` | `float` (f32) storage | Yes — `fptrunc` at store, `fpext` at load |
| Struct field `scale: float` storing constant `1e300` (non-f32-exact) | `double` (f64) — no narrowing | Yes — `is_f32_exact(1e300) == false` |
| Float arithmetic `a + b` → result stored in struct | `double` computation, NOT narrowed | Yes — no arithmetic narrowing |
| `struct Color { r: float, g: float, b: float }` with all values from `0.0..=1.0` literal range | `float` fields (f32) | Yes — all literal values are f32-exact |
| `float` field receiving arithmetic result | `double` — conservative (no narrowing) | Yes — arithmetic result stays f64 |
| `is_f32_exact(0.1)` | `false` (0.1 is not f32-exact) | Yes — precision pin |
| `is_f32_exact(0.5)` | `true` (exact in f32) | Yes |

- [ ] Write failing test matrix BEFORE implementation (verify tests fail with current `double`-only codegen)
- [ ] `is_f32_exact()` correctly identifies all f32-representable f64 values
- [ ] `is_f32_exact(0.0)`, `is_f32_exact(1.0)`, `is_f32_exact(0.5)`, `is_f32_exact(100.0)` → `true`
- [ ] `is_f32_exact(1.0/3.0)`, `is_f32_exact(1e300)`, `is_f32_exact(f64::NAN)` → `false`
- [ ] Edge case: `is_f32_exact(-0.0)` → `true` (negative zero is f32-exact)
- [ ] Constants like `0.0`, `1.0`, `0.5`, `100.0` → narrowed to f32 in struct field storage
- [ ] Arithmetic on float values is NEVER narrowed (conservative by default)
- [ ] `struct Color { r: float, g: float, b: float }` with literal `0.0..=1.0` values uses f32 fields
- [ ] `fpext`/`fptrunc` visible at load/store boundaries in LLVM IR
- [ ] Add semantic pin test: a struct storing `0.5` uses `float` (f32) field in LLVM IR, with `fptrunc double to float` at store and `fpext float to double` at load. This test can ONLY pass with float narrowing enabled.
- [ ] Bit-identical results: `./diagnostics/dual-exec-verify.sh` passes (zero precision differences)
- [ ] `./test-all.sh` green in both debug (`cargo b`) and release (`cargo b --release`) builds
- [ ] `./clippy-all.sh` green

**Exit Criteria:** A program storing constant `0.5` in a struct field uses `float` (f32) in LLVM IR instead of `double` (f64), verified by inspecting generated IR. All floating-point spec tests continue to pass with bit-identical results.
