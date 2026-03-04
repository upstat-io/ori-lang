---
section: "03"
title: "Arithmetic Correctness"
status: not-started
goal: "Unary negation of INT_MIN panics instead of silently wrapping"
inspired_by:
  - "Rust rustc_codegen_llvm/mir/rvalue.rs — uses llvm.ssub.with.overflow for checked negation"
  - "Swift lib/SILOptimizer/ — traps on signed overflow including negation"
depends_on: []
sections:
  - id: "03.1"
    title: "Checked Unary Negation"
    status: not-started
  - id: "03.2"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Arithmetic Correctness

**Status:** Not Started
**Goal:** Unary negation (`-x`) on integers uses checked arithmetic. Negating `INT_MIN` (-9223372036854775808) panics with an overflow message instead of silently wrapping back to `INT_MIN`.

**Context:** The Ori spec mandates that integer overflow panics. All binary arithmetic operations (`+`, `-`, `*`) already use `@llvm.sadd/ssub/smul.with.overflow.i64` intrinsics with panic-on-overflow. However, unary negation uses bare `sub i64 0, %x` without overflow checking. This is a semantic correctness bug: `-INT_MIN` overflows (the mathematical result `9223372036854775808` doesn't fit in i64) but the program continues with the wrong value.

**Journey affected:** J2 (discovered in `my_abs` analysis).

**Scope note:** Float negation via `fneg` is correctly unaddressed — IEEE 754 sign flip has no overflow case. Only integer negation needs the overflow check.

**Existing infrastructure:** `emit_checked_binop()` in `arithmetic.rs` already implements the `@llvm.ssub.with.overflow.i64` pattern for binary subtraction. The negation fix can reuse this infrastructure directly (negation is `0 - x`).

**Current codegen location:** Unary integer negation currently lowers via `self.builder.neg(...)` (unchecked `build_int_neg`) in ARC emitter operator handling.

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

- [ ] Write spec test: `let $x: int = -9223372036854775807 - 1; assert_panics(expr: () -> { -x })` (negating INT_MIN should panic)
- [ ] Write spec test: `-0` returns `0` (no overflow)
- [ ] Write spec test: `-1` returns `1` (no overflow)
- [ ] Write spec test: `-9223372036854775807` returns `9223372036854775807` (max positive, no overflow)
- [ ] Verify spec tests FAIL before fix (confirm the bug exists)
- [ ] Locate unary negation codegen (likely in `operators.rs` or `arithmetic.rs`)
- [ ] Replace `sub i64 0, %x` with `@llvm.ssub.with.overflow.i64` + overflow check
- [ ] Use the same panic message pattern as other overflow checks: `"integer overflow on negation\00"`
- [ ] Verify all spec tests pass
- [ ] Verify existing tests pass (no regressions)
- [ ] Run `diagnostics/dual-exec-verify.sh tests/spec/` and confirm no new mismatches

---

## 03.2 Completion Checklist

- [ ] Unary negation uses `@llvm.ssub.with.overflow.i64`
- [ ] `-INT_MIN` panics with "integer overflow on negation"
- [ ] `-0`, `-1`, `-MAX` all work correctly without panic
- [ ] Spec tests added under an existing arithmetic operator spec file (or a new dedicated overflow-negation spec fixture) in `tests/spec/`
- [ ] AOT test in `compiler/ori_llvm/tests/aot/operators.rs`
- [ ] `./test-all.sh` green
- [ ] Eval interpreter already uses `checked_neg()` — verify LLVM codegen matches eval behavior (parity confirmed: eval panics, AOT must too)

**Exit Criteria:** `ori run` and AOT binary both panic on `-INT_MIN` with a clear overflow message. Dual-execution verification confirms eval and LLVM paths agree.
