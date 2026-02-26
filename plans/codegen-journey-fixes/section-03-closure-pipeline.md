---
section: "03"
title: "Closure Pipeline"
status: complete
goal: "Non-capturing lambdas avoid trampoline overhead; trampolines inherit nounwind from their targets"
inspired_by:
  - "Rust rustc_codegen_llvm closure ABI (compiler/rustc_codegen_llvm/src/mir/block.rs — FnPtr vs Closure)"
  - "Swift SIL thin function type (no context pointer for non-capturing closures)"
depends_on: ["01"]
sections:
  - id: "03.1"
    title: "Non-Capturing Lambda Optimization"
    status: complete
  - id: "03.2"
    title: "Trampoline Nounwind Propagation"
    status: complete
  - id: "03.3"
    title: "Completion Checklist"
    status: complete
---

# Section 03: Closure Pipeline

**Status:** Complete
**Goal:** Non-capturing lambdas are represented as bare function pointers without closure allocation or trampoline indirection. Trampoline functions inherit the `nounwind` attribute when their target is provably nounwind.

**Context:** Journey 4 found two closure pipeline issues. First, a non-capturing lambda like `(x: int) -> int = x + 1` still requires a `{ ptr, ptr }` closure pair with null `env_ptr` and a trampoline function `_ori_partial_0` that just forwards the call. This is unnecessary overhead for the common case. Second, trampolines like `_ori_partial_0` lack `nounwind` even when they only call a nounwind lambda.

**Reference implementations:**
- **Rust** `compiler/rustc_codegen_llvm/src/mir/block.rs`: Rust distinguishes `FnPtr` (bare function pointer, no closure) from `Closure` (with captured environment). Non-capturing closures are coerced to `FnPtr` at the type level.
- **Swift** SIL: Uses `@convention(thin)` for non-capturing closures (just a function pointer) vs `@convention(thick)` for capturing closures (function pointer + context).

**Depends on:** Section 01 (nounwind must be sound before propagating to trampolines).

---

## 03.1 Non-Capturing Lambda Optimization

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` (lambda compilation), `compiler/ori_llvm/src/codegen/arc_emitter/builtins/trampolines.rs`

**Finding #6** (MEDIUM): Every lambda, even `(x: int) -> int = x + 1`, creates:
1. A `{ ptr, ptr }` closure struct with null `env_ptr`
2. A trampoline function `_ori_partial_N` that unpacks the closure and forwards the call
3. An indirect call through the closure's `fn_ptr`

For non-capturing lambdas, the closure struct and trampoline are pure overhead.

**Target behavior:** When a lambda captures no variables:
1. Pass the function pointer directly (no closure struct)
2. Skip trampoline generation (caller invokes the function directly)
3. The `env_ptr` is not allocated or passed

**Design consideration:** The caller (e.g., `map()`, `filter()`) currently always expects a `{ ptr, ptr }` closure pair. Optimizing non-capturing lambdas requires either:
- **(a)** Specializing the caller for bare function pointers (more optimal, more complex)
- **(b)** Keeping the `{ fn_ptr, null }` pair but skipping the trampoline — the `fn_ptr` points directly to the lambda function (simpler, still eliminates the trampoline indirection)

**Recommended path:** Option (b) first — eliminate the trampoline indirection while keeping the `{ fn_ptr, null }` closure ABI for compatibility. Option (a) can be a future optimization.

- [x] Detect non-capturing lambdas during `compile_lambda_arc()`
  - A lambda is non-capturing if its ArcFunction has zero captured environment variables
  - Check: `arc_function.num_captures == 0` (field added to `ArcFunction`)

- [x] For non-capturing lambdas, set `fn_ptr` directly to the lambda function (no trampoline)
  - Non-capturing lambdas declared with `ccc` + phantom `ptr %_env` prepended
  - `emit_partial_apply()` fast path: uses lambda function pointer directly, null env
  - Trampoline generation skipped entirely

- [x] Set `env_ptr` to null for non-capturing lambdas (already done, but make explicit)

- [x] Test: non-capturing lambda `(x: int) -> int = x + 1` — no `_ori_partial` trampoline in IR
- [x] Test: capturing lambda `let y = 5; (x: int) -> int = x + y` — trampoline still generated
- [x] Test: non-capturing lambda passed to HOF — correct results
- [x] Verify no regressions: `./llvm-test.sh` — all 1016 tests pass

---

## 03.2 Trampoline Nounwind Propagation

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/trampolines.rs`, `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

**Finding #9** (LOW): `_ori_partial_0` (the closure trampoline) lacks `nounwind` even when it only calls a nounwind lambda. Cosmetic for most cases but could affect optimization if the trampoline is called from another nounwind function.

**Note:** This depends on §01 being complete — if nounwind analysis is unsound, propagating nounwind to trampolines could introduce new UB.

**Fix approach:**

After generating a trampoline, check if the target lambda is in `nounwind_functions`. If so, mark the trampoline `nounwind` too.

- [x] After `generate_closure_wrapper()`, check target nounwind status
  - Added `target_is_nounwind: bool` parameter to `generate_closure_wrapper()`
  - Sets `add_nounwind_attribute(wrapper_func_id)` when target is nounwind
  - Call site in `emit_partial_apply()` checks `self.ctx.nounwind_functions.contains(&callee)`

- [x] Handle edge case: if §03.1 eliminates the trampoline for non-capturing lambdas, this only applies to capturing-lambda trampolines
  - Non-capturing fast path returns early before `generate_closure_wrapper` is called

- [x] Test: trampoline for nounwind lambda → trampoline has `nounwind` attribute
- [x] Test: trampoline for may-unwind lambda → trampoline does NOT have `nounwind`
- [x] Verify no regressions: `./llvm-test.sh` — all 1460 tests pass

---

## 03.3 Completion Checklist

- [x] Non-capturing lambdas produce no `_ori_partial` trampoline in IR
- [x] Capturing lambdas still produce correct trampolines
- [x] Trampoline for nounwind target has `nounwind` attribute
- [x] Trampoline for may-unwind target does NOT have `nounwind`
- [x] Journey 4 program produces correct results with optimized closure IR (exit code 16, correct)
- [x] `./test-all.sh` green (10184 passed, 0 failed)
- [x] `./llvm-test.sh` green (1460 passed, 0 failed)
- [x] `./llvm-clippy.sh` green (no warnings)

**Exit Criteria:** Journey 4 program compiles with one fewer trampoline function (non-capturing lambda optimized away). Remaining trampolines correctly inherit nounwind. Zero regressions.
