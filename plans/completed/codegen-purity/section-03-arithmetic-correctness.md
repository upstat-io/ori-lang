---
section: "03"
title: "Arithmetic Correctness"
status: complete
goal: "Unary negation of INT_MIN panics instead of silently wrapping"
inspired_by:
  - "Rust rustc_codegen_llvm/mir/rvalue.rs — uses llvm.ssub.with.overflow for checked negation"
  - "Swift lib/SILOptimizer/ — traps on signed overflow including negation"
depends_on: []
sections:
  - id: "03.1"
    title: "Checked Unary Negation"
    status: complete
---

# Section 03: Arithmetic Correctness

**Status:** Complete
**Goal:** Unary negation (`-x`) on integers uses checked arithmetic. Negating `INT_MIN` (-9223372036854775808) panics with an overflow message instead of silently wrapping back to `INT_MIN`.

**Context:** The Ori spec mandates that integer overflow panics. All binary arithmetic operations (`+`, `-`, `*`) already use `@llvm.sadd/ssub/smul.with.overflow.i64` intrinsics with panic-on-overflow. However, unary negation uses bare `sub i64 0, %x` without overflow checking. This is a semantic correctness bug: `-INT_MIN` overflows (the mathematical result `9223372036854775808` doesn't fit in i64) but the program continues with the wrong value.

**Journey affected:** J2 (discovered in `my_abs` analysis).

**Scope note:** Float negation via `fneg` is correctly unaddressed — IEEE 754 sign flip has no overflow case. Only integer negation needs the overflow check.

**Existing infrastructure:** `emit_checked_binop()` in `arithmetic.rs` already implements the `@llvm.ssub.with.overflow.i64` pattern for binary subtraction. The negation fix can reuse this infrastructure directly (negation is `0 - x`).

**Current codegen location:** Unary integer negation lowers via `self.builder.checked_neg(operand, "neg")` in ARC emitter operator handling (`operators.rs`), which delegates to `IrBuilder::checked_neg()` → `emit_checked_binop("llvm.ssub.with.overflow", 0, x)` in `arithmetic.rs`. The unchecked `build_int_neg()` path still exists in `arithmetic.rs` as `neg()` but is no longer used for user-facing negation.

**Eval parity:** The eval interpreter **already handles this correctly**:
- `compiler/oric/src/eval/tests/unary_operators_tests.rs` and evaluator numeric paths use `checked_neg()` semantics for ints.
- LLVM AOT path currently does not, creating a live eval/AOT mismatch.
This confirms a **live eval/AOT behavioral mismatch** — eval panics on `-INT_MIN` but AOT silently wraps.

**Reference implementations:**
- **Rust** `rustc_codegen_llvm/mir/rvalue.rs`: Uses `@llvm.ssub.with.overflow.i64(i64 0, i64 %x)` for checked negation.
- **Go** `cmd/compile/internal/ssagen/ssa.go`: Emits overflow check for unary negation.

---

## 03.1 Checked Unary Negation

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/operators.rs` or `compiler/ori_llvm/src/codegen/ir_builder/arithmetic.rs`

Replace `sub i64 0, %x` with `@llvm.ssub.with.overflow.i64(i64 0, i64 %x)` + overflow branch to panic.

```llvm
; CURRENT (wrong):
%result = sub i64 0, %x

; TARGET (correct):
%ov = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 0, i64 %x)
%result = extractvalue { i64, i1 } %ov, 0
%overflow = extractvalue { i64, i1 } %ov, 1
br i1 %overflow, label %panic, label %ok

panic:
  call void @ori_panic_cstr(ptr @"integer overflow on negation\00")
  unreachable

ok:
  ; continue with %result
```

- [x] Write spec test: `let $x: int = -9223372036854775807 - 1; assert_panics(expr: () -> { -x })` (negating INT_MIN should panic)
- [x] Write spec test: `-0` returns `0` (no overflow)
- [x] Write spec test: `-1` returns `-1` (no overflow)
- [x] Write spec test: `-9223372036854775807` returns `-9223372036854775807` (INT_MAX negation, no overflow)
- [x] Verify spec tests FAIL before fix (confirm the bug exists) — AOT test confirmed bug: exit code 0 instead of panic
- [x] Locate unary negation codegen: `operators.rs` `UnaryOp::Neg` → `IrBuilder::checked_neg()` in `arithmetic.rs` (was unchecked `build_int_neg`, now `emit_checked_binop`)
- [x] Replace `sub i64 0, %x` with `@llvm.ssub.with.overflow.i64` + overflow check — `checked_neg()` reuses `emit_checked_binop`
- [x] Use the same panic message pattern as other overflow checks: `"integer overflow on negation\00"`
- [x] Verify all spec tests pass (4153 passed)
- [x] Verify existing tests pass (no regressions) — 11,972 tests, 0 failures
- [x] Run `diagnostics/dual-exec-verify.sh tests/spec/` and confirm no new mismatches — N/A (no @main programs); AOT tests verify parity directly

### 03.1 Completion Checklist

- [x] Unary negation uses `@llvm.ssub.with.overflow.i64` (not bare `sub`)
- [x] `-INT_MIN` panics with "integer overflow on negation"
- [x] `-0`, `-1`, `-MAX` all work correctly without panic
- [x] Spec tests added in `tests/spec/types/integer_safety.ori` for negation overflow
- [x] AOT tests in `compiler/ori_llvm/tests/aot/operators.rs` (4 tests: overflow + 3 non-overflow)
- [x] `diagnostics/dual-exec-verify.sh` confirms eval/AOT parity on negation — verified via AOT tests directly
- [x] `./test-all.sh` green (11,972 passed, 0 failed)
- [x] `./clippy-all.sh` green
- [x] No regressions in `cargo test -p ori_llvm`

---

## Section 03 Exit Criteria

`ori run` and AOT binary both panic on `-INT_MIN` with a clear overflow message. Dual-execution verification confirms eval and LLVM paths agree.
