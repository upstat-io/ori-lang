---
section: "04"
title: "Integer Narrowing Pipeline"
status: not-started
goal: "Lower int (semantic i64) to the smallest machine integer (i8/i16/i32) that preserves correctness, saving memory in struct fields, collections, and stack slots"
inspired_by:
  - "Zig comptime_int narrowing to runtime types (src/Sema.zig)"
  - "Roc NumericRange → concrete layout (crates/compiler/mono/src/layout.rs)"
  - "LLVM InstCombine integer truncation (lib/Transforms/InstCombine/)"
depends_on: ["03"]
sections:
  - id: "04.1"
    title: "Width Selection Algorithm"
    status: not-started
  - id: "04.2"
    title: "ABI Boundary Widening"
    status: not-started
  - id: "04.3"
    title: "Overflow Guard Insertion"
    status: not-started
  - id: "04.4"
    title: "LLVM Codegen Integration"
    status: not-started
  - id: "04.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Integer Narrowing Pipeline

**Status:** Not Started
**Goal:** When range analysis (§03) proves a variable's range fits in `i32` (or `i16`, `i8`), the variable uses the narrower type in generated code. This reduces memory for struct fields (4 bytes instead of 8), improves cache utilization, and enables SIMD vectorization of narrow-integer loops.

**Context:** Today, every `int` is `i64` in LLVM IR. A loop counter that goes `0..100` wastes 7 bytes per element in array storage. A struct with `{ x: int, y: int }` where both fields are always `0..255` uses 16 bytes instead of 2. The savings compound in collections: `[Point]` with 1M elements wastes 14MB.

**Reference implementations:**
- **Zig** `src/Sema.zig`: `coerceInMemoryAllowedPtrAbiType()` — coerces comptime_int to runtime width
- **Roc** `crates/compiler/mono/src/layout.rs`: `Layout::from_var()` — selects concrete layout from type variable constraints
- **LLVM** `lib/Transforms/InstCombine/InstCombineCasts.cpp`: Integer truncation elimination

**Depends on:** §03 (range analysis provides the intervals).

---

## 04.1 Width Selection Algorithm

**File(s):** `compiler/ori_repr/src/narrowing/int.rs`

Given a `ValueRange`, select the minimum integer width that preserves the semantic contract.

- [ ] Implement width selection:
  ```rust
  pub fn select_int_width(range: ValueRange) -> IntWidth {
      match range {
          ValueRange::Top | ValueRange::Bottom => IntWidth::I64,
          ValueRange::Bounded { lo, hi } => {
              // Check signed ranges (Ori int is signed)
              if lo >= -128 && hi <= 127 { IntWidth::I8 }
              else if lo >= -32_768 && hi <= 32_767 { IntWidth::I16 }
              else if lo >= -2_147_483_648 && hi <= 2_147_483_647 { IntWidth::I32 }
              else { IntWidth::I64 }
          }
      }
  }
  ```

- [ ] Apply conservatism rules:
  - **Local variables**: narrow aggressively (widening is free in registers)
  - **Struct fields**: narrow aggressively (saves memory per instance)
  - **Function parameters**: narrow only if ALL call sites agree on the range
  - **Function returns**: narrow only if ALL callers can handle the narrow type
  - **Collection elements**: narrow aggressively (savings multiply by element count)
  - **Public API types**: do NOT narrow (external callers may pass full-range values)

- [ ] Implement `NarrowingPolicy`:
  ```rust
  pub enum NarrowingPolicy {
      /// Narrow as aggressively as range allows
      Aggressive,
      /// Only narrow if savings exceed threshold (e.g., struct field)
      Conservative { min_savings_bytes: u32 },
      /// Never narrow (public API boundary)
      Disabled,
  }
  ```

---

## 04.2 ABI Boundary Widening

**File(s):** `compiler/ori_repr/src/narrowing/abi.rs`

At function boundaries and FFI, narrowed integers must be widened back to canonical width. This is critical for correctness.

- [ ] Define ABI boundary rules:
  ```rust
  pub enum AbiBoundary {
      /// Internal function call — can use narrow types if both sides agree
      InternalCall,
      /// Public function — must use canonical i64
      PublicApi,
      /// FFI call — must match C ABI (platform-specific)
      Ffi,
      /// Trait method — must use canonical (unknown callers)
      TraitMethod,
      /// Closure — parameter types fixed at creation
      ClosureCapture,
  }
  ```

- [ ] Implement widening insertion:
  - Before public function return: `sext i32 %narrow to i64`
  - Before FFI call arguments: widen to C-ABI width
  - At module import boundaries: widen to canonical
  - When storing to generic collection: widen if collection is exported

- [ ] Cross-module narrowing via Merkle hashes:
  - If both modules agree on the range (via function signature annotations), use narrow type
  - If modules disagree, widen at the boundary
  - Merkle hash includes the MachineRepr, so different representations get different hashes

---

## 04.3 Overflow Guard Insertion

**File(s):** `compiler/ori_repr/src/narrowing/overflow.rs`

When a value is narrowed, arithmetic operations might overflow the narrow type even though they wouldn't overflow the canonical i64. The compiler must insert overflow checks.

- [ ] Implement overflow analysis:
  ```rust
  /// Given operand ranges and operation, will the result fit in the target width?
  pub fn can_overflow(
      op: ArithOp,
      lhs: ValueRange,
      rhs: ValueRange,
      target: IntWidth,
  ) -> bool {
      let result_range = match op {
          ArithOp::Add => range_add(lhs, rhs),
          ArithOp::Sub => range_sub(lhs, rhs),
          ArithOp::Mul => range_mul(lhs, rhs),
          // ...
      };
      !result_range.fits_in(target)
  }
  ```

- [ ] When overflow is possible, choose strategy:
  - **(a) Widen before operation**: Promote operands to wider type, compute, narrow result
    ```llvm
    %wide_a = sext i16 %a to i32
    %wide_b = sext i16 %b to i32
    %result = add i32 %wide_a, %wide_b
    ; range check: result fits in i16?
    %narrow = trunc i32 %result to i16
    ```
  - **(b) Compute at canonical width**: If overflow is common, just use i64 for this expression
  - **(c) Proven safe**: If range analysis proves no overflow, narrow directly

- [ ] Decision: prefer (c) when provable, (a) for rare overflow, (b) when overflow is common

---

## 04.4 LLVM Codegen Integration

**File(s):** `compiler/ori_llvm/src/codegen/type_info/info.rs`, `compiler/ori_llvm/src/codegen/expr_compiler.rs`

The LLVM backend must emit narrowed types and insert sign-extension/truncation at boundaries.

- [ ] Modify `TypeInfo::storage_type()` to consult `ReprPlan`:
  ```rust
  // Before:
  TypeInfo::Int => context.i64_type().into(),

  // After:
  TypeInfo::Int => match repr_plan.int_width(idx) {
      IntWidth::I8  => context.i8_type().into(),
      IntWidth::I16 => context.custom_width_int_type(16).into(),
      IntWidth::I32 => context.i32_type().into(),
      IntWidth::I64 => context.i64_type().into(),
  },
  ```

- [ ] Insert `sext`/`trunc` at narrowing boundaries:
  - Function entry: parameters arrive at canonical width → `trunc` to narrow
  - Function exit: narrow result → `sext` to canonical width
  - Struct field store: canonical value → `trunc` to field width
  - Struct field load: narrow field → `sext` to canonical for computation

- [ ] Handle comparison operations correctly:
  - Signed comparison (`icmp slt`) on narrow types is correct for signed narrowing
  - Unsigned narrowing (future, for byte → int) needs `zext` not `sext`

---

## 04.5 Completion Checklist

- [ ] `select_int_width()` returns correct width for all test ranges
- [ ] Loop counters in `for i in 0..100` use `i8` in generated LLVM IR
- [ ] Struct field `x: int` in `struct Pair { x: int, y: int }` uses narrowed type when constructor values are bounded
- [ ] ABI boundaries correctly widen: `sext` visible in LLVM IR at function boundaries
- [ ] Overflow guards inserted where narrowed arithmetic might overflow
- [ ] No semantic change: `./scripts/dual-exec-verify.sh` passes (eval and AOT produce identical results)
- [ ] `./test-all.sh` green
- [ ] `./scripts/valgrind-aot.sh` clean
- [ ] Performance: struct sizes measurably smaller for bounded-range fields

**Exit Criteria:** Compiling a program with `struct Pixel { r: int, g: int, b: int, a: int }` where all fields are `0..255` produces a 4-byte struct (4 × i8) instead of 32-byte struct (4 × i64), verified by checking LLVM IR struct definitions.
