---
section: 1
title: "Overflow Check Elision"
status: in-progress
reviewed: true
goal: "Reduce unnecessary overflow checks from 19 to ≤8 for the compute benchmark, matching Rust with -C overflow-checks=yes"
depends_on: []
inspired_by:
  - "Rust MIR→LLVM lowering: uses `add nsw` for post-guard arithmetic and loop counters"
  - "LLVM `range()` metadata on function parameters"
third_party_review: { status: findings, updated: 2026-03-23 }
---

# §01 Overflow Check Elision

## Goal

Emit `add nsw` / `sub nsw` instead of `llvm.sadd.with.overflow` for arithmetic operations that are provably safe, without changing Ori's overflow-panic semantics. Target: reduce bench_compute overflow intrinsics from 19 to ≤8.

## Analysis: Why Ori Has 19 Checks Where Rust Has 8

Ori's `emit_int_binary_op()` (`operators/strategy.rs:103-105`) unconditionally routes `Add`→`checked_add`, `Sub`→`checked_sub`, `Mul`→`checked_mul`. No information about operand bounds is considered.

Rust with `-C overflow-checks=yes` still uses `add nsw` for 3 categories that Ori checks:

### Category A: Constant-Subtraction After Guard (5 instances)

```
if n < 2 then n else fib(n: n - 1) + fib(n: n - 2)
```

In the `else` branch, `n >= 2`. Therefore:
- `n - 1` is in `[1, i64::MAX-1]` — cannot underflow
- `n - 2` is in `[0, i64::MAX-2]` — cannot underflow

Rust emits `add nsw i64 %n, -1`. Ori emits `llvm.ssub.with.overflow`.

**Pattern**: After `icmp sge/sgt/slt/sle` that guards a subtraction of a small constant.

### Category B: Loop Counter Increments (already correct)

Ori already emits `add nuw nsw` for `for i in 0..N` loop counters. No changes needed here.

### Category C: Accumulator Additions (2-4 instances)

```
total = total + gcd(a: i, b: j)
```

The GCD of two positive i64 values fits in i64. The accumulation of 1000×1000 GCD values (each ≤ 3000) also fits. But proving this requires interprocedural analysis — **deferred to repr-opt §03**.

## Implementation

### Step 1: Add `add_nsw` / `sub_nsw` to IrBuilder

**File**: `compiler/ori_llvm/src/codegen/ir_builder/arithmetic.rs`

Add two new methods alongside existing `add()` / `sub()`:

```rust
/// Integer addition with no-signed-wrap guarantee.
///
/// The caller MUST prove the operation cannot overflow.
/// Emits `add nsw` — LLVM may assume the result is in-range.
pub fn add_nsw(&self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
    // Same as add() but with nsw flag
}

/// Integer subtraction with no-signed-wrap guarantee.
pub fn sub_nsw(&self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
    // Same as sub() but with nsw flag
}
```

Use inkwell's `build_int_nsw_add` / `build_int_nsw_sub` (verify these exist in inkwell API; if not, use `build_int_add` + set metadata).

### Step 2: Add Constant-Operand Elision in `emit_checked_binop`

**File**: `compiler/ori_llvm/src/codegen/ir_builder/checked_ops.rs`

Before the intrinsic call at line 125, add constant folding:

```rust
fn emit_checked_binop(&mut self, intrinsic_name: &'static str, lhs: ValueId, rhs: ValueId, name: &str, panic_msg: &str) -> ValueId {
    // CSE cache lookup (existing)
    // ...

    // NEW: Compile-time constant folding
    // If both operands are constants, compute the result at compile time.
    // If the operation overflows, still emit the intrinsic (to get the panic).
    let l = self.arena.get_value(lhs);
    let r = self.arena.get_value(rhs);
    if let (BasicValueEnum::IntValue(lv), BasicValueEnum::IntValue(rv)) = (l, r) {
        if let (Some(lc), Some(rc)) = (lv.get_sign_extended_constant(), rv.get_sign_extended_constant()) {
            let result = match intrinsic_name {
                "llvm.sadd.with.overflow" => lc.checked_add(rc),
                "llvm.ssub.with.overflow" => lc.checked_sub(rc),
                "llvm.smul.with.overflow" => lc.checked_mul(rc),
                _ => None,
            };
            if let Some(val) = result {
                // Provably safe — emit plain constant, skip intrinsic + panic block.
                let result_id = self.arena.push_value(self.scx.llcx.i64_type().const_int(val as u64, true).into());
                self.cse_cache.insert(cache_key, result_id);
                return result_id;
            }
            // Overflow at compile time — fall through to emit the panic.
        }
    }

    // Existing intrinsic emission...
}
```

### Step 3: Post-Guard Elision via Operand Analysis

**File**: `compiler/ori_llvm/src/codegen/ir_builder/checked_ops.rs`

Add a method to detect "subtract small constant from value known >= that constant":

```rust
/// Check if a subtraction is provably safe (no underflow possible).
///
/// Returns true when subtracting a small positive constant from a value
/// that is known to be >= that constant due to LLVM's range analysis.
/// In this case, `sub nsw` is safe.
fn can_elide_sub_overflow(&self, lhs: ValueId, rhs: ValueId) -> bool {
    let r = self.arena.get_value(rhs);
    if let BasicValueEnum::IntValue(rv) = r {
        if let Some(rc) = rv.get_sign_extended_constant() {
            // Subtracting a small non-negative constant.
            // If rc is 0, 1, or 2, and the lhs came from a guarded branch
            // path, LLVM's own range analysis via nsw will handle it.
            // We can safely emit `sub nsw` when the constant is small
            // and the lhs is known non-negative (e.g., from loop counter,
            // or from a branch condition).
            return rc >= 0 && rc <= 2;
        }
    }
    false
}
```

**Note**: This is conservative. `sub nsw` with small constants from values that passed a `>= constant` guard is always safe. LLVM's own optimization will verify and either keep the `nsw` or lower it. The key insight: even Rust with overflow checks enabled uses `add nsw i64 %n, -1` in post-guard contexts because Rust's MIR includes range info that proves safety. Ori can do the same for constant subtractions.

However, this step requires more careful analysis to avoid false elisions. The safest approach is:
1. Start with constant folding only (Step 2)
2. Measure the benchmark improvement
3. If still >8 checks, add post-guard analysis

### Step 4: Update `emit_int_binary_op` to Use Elision

**File**: `compiler/ori_llvm/src/codegen/arc_emitter/operators/strategy.rs`

No changes needed in this file — the elision happens inside `checked_add`/`checked_sub`/`checked_mul` in `checked_ops.rs`. The dispatch remains the same; the implementation becomes smarter.

## Test Strategy

### Matrix Dimensions

**Type dimension**: `int` (only type with checked arithmetic)
**Pattern dimension**:
- Constant + constant (both known at compile time) → should fold
- Variable + small constant (e.g., `n - 1`) → elision candidate
- Variable + variable (both unknown) → must check
- Overflow case (INT_MAX + 1) → must panic
- Negative constant subtraction (e.g., `n - (-1)` = `n + 1`) → must check

### Semantic Pin Tests

1. **Pin: constant fold produces correct result** — `@main () -> int = 3 + 4` must return 7 with zero overflow intrinsics in IR
2. **Pin: overflow still panics** — `@main () -> int = 9223372036854775807 + 1` must panic (not silently wrap)
3. **Pin: variable arithmetic still checked** — `@f (a: int, b: int) -> int = a + b` must use `sadd.with.overflow`

### TDD Ordering

- [ ] Write failing test: verify bench_compute IR has ≤8 overflow intrinsics (currently 19)
- [ ] Write panic test: verify INT_MAX + 1 still panics after elision
- [ ] Implement constant folding in `emit_checked_binop`
- [ ] Implement `add_nsw` / `sub_nsw` in `arithmetic.rs`
- [ ] Verify all tests pass in debug AND release

### Test Files

- `compiler/ori_llvm/tests/aot/operators.rs` — extend existing overflow matrix (lines 587-822)
- `compiler/ori_llvm/tests/aot/ir_quality_attributes.rs` — add IR intrinsic count check

## §01.R Third Party Review Findings

- [ ] `[TPR-01-001][high]` `plans/aot-perf/section-01-overflow-elision.md:125` — Step 3 treats `nsw` as a hint LLVM can validate later instead of a frontend correctness promise.
  Evidence: `can_elide_sub_overflow()` returns `true` for any subtraction by `0..=2` without proving the left operand's range, and the note at line 144 says LLVM will "verify and either keep the `nsw` or lower it." LLVM does not validate `nsw`; it optimizes under that assumption.
  Impact: Implementing the section as written can silently remove required overflow panics and introduce wrong-code in checked arithmetic paths, violating the plan's "Correctness Preserved" requirement.
  Required plan update: Restrict §01 to cases with an actual proof in hand (for example compile-time constant folding only), or spell out the exact guard-to-range facts that must be recovered before emitting `add/sub nsw`. Do not describe speculative `nsw` as something LLVM will sanitize.

## Completion Checklist

- [ ] `add_nsw` / `sub_nsw` methods added to `IrBuilder`
- [ ] Constant folding implemented in `emit_checked_binop`
- [ ] All existing overflow tests still pass (no regressions)
- [ ] New semantic pin tests added and passing
- [ ] bench_compute overflow intrinsic count ≤ 8
- [ ] `./test-all.sh` passes in debug and release
- [ ] Benchmark re-run shows measurable improvement
