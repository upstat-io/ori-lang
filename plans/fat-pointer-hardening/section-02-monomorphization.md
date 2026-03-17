---
section: "02"
title: "Monomorphization of Captured Types"
status: not-started
goal: "Closures capturing any non-scalar type (str, [T], structs, closures) compile correctly in AOT with fully resolved types"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Root Cause Analysis"
    status: not-started
  - id: "02.2"
    title: "Fix Type Propagation for Capture Environments"
    status: not-started
  - id: "02.3"
    title: "Fix Method Resolution on Captured Values"
    status: not-started
  - id: "02.4"
    title: "Generalize to All Non-Scalar Capture Types"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Monomorphization of Captured Types

**Status:** Not Started
**Goal:** When a closure captures a non-scalar value (str, [T], struct, another closure) and calls methods on it or passes it to other functions, LLVM codegen receives fully resolved types for ALL variables in the closure body. No unresolved type variables (`Idx(N)`) leak to codegen. This applies to ALL non-scalar capture types, not just `str`.

**Context:** J17 discovered that `let f = s -> prefix.length() + s.length()` where `prefix: str` crashes during AOT codegen. The root cause chain: (1) monomorphization fails to propagate the concrete `str` type for the closure's lambda parameter when the closure also captures a fat pointer, (2) the unresolved type variable `Idx(N)` leaks into LLVM codegen, (3) codegen generates `i64` instead of `{i64, i64, ptr}` for the parameter, (4) `.length()` dispatch fails, (5) `ori_rc_dec` gets wrong types. The eval path works because it resolves types dynamically.

**Reference implementations:**
- **Rust** `compiler/rustc_monomorphize/src/collector.rs`: Monomorphization collects ALL types reachable from a function, including closure capture environments
- **Gleam** `compiler-core/src/analyse/`: Closure types include their capture environment types in the mono key
- **Lean 4** `src/Lean/Compiler/LCNF/MonoTypes.lean`: Lambda lifting resolves all capture types before codegen

---

> **Warning: High complexity.** The type propagation path crosses 3 crates (`ori_types` monomorphization, `ori_arc` lambda lowering, `ori_llvm` monomorphize + codegen). The root cause may be in any of these. The 02.1 analysis must identify which crate is the origin before any code changes. Do not guess — use `ORI_LOG=ori_types=debug,ori_arc=debug,ori_llvm=debug` to trace the full path.

## 02.1 Root Cause Analysis

**File(s):** `compiler/ori_types/src/infer/expr/calls/monomorphization.rs`, `compiler/ori_llvm/src/monomorphize/mod.rs`, `compiler/ori_llvm/src/codegen/type_info/store.rs`

The error message from codegen is: `unresolved type variable at codegen — type inference bug idx=Idx(202)`. This means a type variable that should have been resolved during type checking/monomorphization survived into LLVM codegen.

- [ ] Add `ORI_LOG=ori_types=debug` tracing to the J17 program and identify which specific type variable remains unresolved
- [ ] Trace the monomorphization path for the closure `s -> prefix.length() + s.length()` — what types are assigned to `prefix`, `s`, and the closure environment?
- [ ] Compare with J5's working closure `x -> x + n` where `n: int` — what's different in the mono path for scalar vs fat pointer capture?
- [ ] Identify the specific function/query in `ori_types` where the type variable should be resolved but isn't
- [ ] Check whether the issue is in closure environment type construction or in lambda parameter type propagation
- [ ] Trace `ori_arc/src/lower/calls/lambda.rs` — how does it receive the type for the lambda parameter `s`? Does it use the resolved type from ori_types or does it have a stale copy?
- [ ] Trace `ori_llvm/src/monomorphize/mod.rs` — when collecting mono instances for the lambda body, does it visit the lambda parameter type and attempt to resolve it?
- [ ] Check the J17 IR output: the lambda `@_ori___lambda_0` has signature `(ptr %0, i64 %1)` — the `i64 %1` proves the type was already wrong when `ori_llvm/src/codegen/function_compiler/` declared the function. Check `FunctionAbi` computation for lambda parameters

---

## 02.2 Fix Type Propagation for Capture Environments

**File(s):** To be determined by root cause analysis (likely `ori_types/src/infer/expr/calls/monomorphization.rs`, `ori_llvm/src/monomorphize/mod.rs`, or `ori_llvm/src/codegen/arc_emitter/closures.rs`)

The fix must ensure that when a closure captures a variable of type `T`, the closure's monomorphized instance includes `T` in its type signature, and all downstream uses (method calls, RC operations, codegen) see the concrete type.

- [ ] Fix the type propagation path so that closure capture environment types are fully resolved before mono instances are created
- [ ] Ensure the lambda parameter type is resolved in the context of the closure environment (not just the call site)
- [ ] Verify that the fix handles recursive cases: closure A captures closure B which captures a str
- [ ] Verify that the fix handles multiple captures: `let f = () -> a.length() + b.length()` where `a: str, b: [int]`

---

## 02.3 Fix Method Resolution on Captured Values

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs`

The second error was: `unresolved function 'length' in invoke — missing mono instance?`. Even after the type is resolved, the method dispatch must find the correct mono instance for the method called on the captured value.

- [ ] Verify that method resolution works for captured values of all types in the type category table (02.4)
- [ ] Fix any gaps in mono instance creation for methods called on captured values
- [ ] Test that chained method calls on captured values work: `captured_str.trim().length()`
- [ ] Fix the `_ori_partial_1` thunk signature: the J17 IR shows `@_ori_partial_1(ptr %0, i64 %1)` — the second parameter `i64` is the unresolved type. The thunk forwards to `@_ori___lambda_0`, so both must agree on parameter types. Verify the thunk generation in `closures.rs` uses resolved types
- [ ] Fix `_ori_drop$202` — the J17 IR shows `ori_rc_free(ptr %0, i64 8, i64 8)` which frees 8 bytes (scalar size). This should be `ori_rc_free(ptr %0, i64 24, i64 8)` for a `str` type. The drop function was generated for the unresolved `forall t13` instead of `str`. Verify `element_fn_gen.rs` / `drop_gen.rs` receive resolved types

---

## 02.4 Generalize to All Non-Scalar Capture Types

The fix must work for ALL non-scalar capture types, not just `str`:

| Capture Type | LLVM Repr | Method Risk | RC Risk |
|-------------|-----------|-------------|---------|
| `str` | `{i64, i64, ptr}` | `.length()`, `.trim()`, etc. | FatPointer SSO guard |
| `[T]` | `{i64, i64, ptr}` | `.length()`, `.push()`, etc. | HeapPointer |
| `{K: V}` | `{i64, i64, ptr}` | `.get()`, `.contains()`, etc. | HeapPointer |
| Struct with fields | `%ori.Name` | `.field` access, methods | AggregateFields |
| Another closure | `{ptr, ptr}` | Calling it | Closure env ptr |
| `Option<str>` | `{i64, {i64, i64, ptr}}` | `.is_some()`, match | InlineEnum |
| `(str, int)` tuple | `{{i64, i64, ptr}, i64}` | `.0`, `.1` | AggregateFields |

- [ ] Write AOT test: closure capturing `str` and calling `.length()` (the J17 bug)
- [ ] Write AOT test: closure capturing `[int]` and calling `.length()`
- [ ] Write AOT test: closure capturing a struct with str field and accessing the field
- [ ] Write AOT test: closure capturing another closure and calling it
- [ ] Write AOT test: closure capturing `Option<str>` and pattern matching on it
- [ ] Write AOT test: closure with multiple non-scalar captures (`str` + `[int]`)
- [ ] Write AOT test: nested closure — outer captures str, inner captures outer's captured str
- [ ] Write AOT test: closure capturing `(str, int)` tuple and accessing `.0`
- [ ] Write AOT test: closure passed as higher-order argument — `@apply (f: (str) -> int, s: str) -> int = f(s)` where `f` captures a str
- [ ] Write AOT test: closure returned from function — `@make_counter (prefix: str) -> (() -> int) = () -> prefix.length()`
- [ ] All tests pass in both eval and AOT with identical results
- [ ] All tests pass under `diagnostics/dual-exec-verify.sh` (behavioral equivalence)

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] Closure capturing `str` compiles and runs correctly in AOT
- [ ] Closure capturing `[T]` compiles and runs correctly in AOT
- [ ] Closure capturing struct with fat fields compiles and runs correctly
- [ ] Closure capturing another closure compiles and runs correctly
- [ ] Nested closures with fat captures compile and run correctly
- [ ] Multi-capture (str + [int]) compiles and runs correctly
- [ ] Closure capturing tuple `(str, int)` compiles and runs correctly
- [ ] Closure returned from function with fat capture compiles and runs correctly
- [ ] No unresolved type variables (`Idx(N)`) reach LLVM codegen (verify with `ORI_LOG=error`)
- [ ] No `_ori_drop$N` functions with wrong size (all drop functions use correct type size, not 8 bytes for fat pointer types)
- [ ] All `_ori_partial_N` thunks have correct parameter types matching their target lambda
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] Valgrind clean on all closure-capturing-fat-pointer tests
- [ ] J17 re-run: AOT produces exit code 10 (matching eval), score improves from 3.0

**Exit Criteria:** `ORI_LOG=error ori build` on all test programs above produces zero "unresolved type variable" errors, AND `diagnostics/dual-exec-verify.sh` reports 0 mismatches for all test programs, AND `diagnostics/valgrind-aot.sh` reports 0 errors.
