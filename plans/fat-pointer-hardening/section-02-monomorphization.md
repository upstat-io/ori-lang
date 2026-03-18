---
section: "02"
title: "Monomorphization of Captured Types"
status: in-progress
goal: "Closures capturing any non-scalar type (str, [T], structs, closures) compile correctly in AOT with fully resolved types"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Root Cause Analysis"
    status: complete
  - id: "02.2"
    title: "Fix Type Propagation for Capture Environments"
    status: complete
  - id: "02.3"
    title: "Fix Method Resolution on Captured Values"
    status: complete
  - id: "02.4"
    title: "Generalize to All Non-Scalar Capture Types"
    status: in-progress
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: in-progress
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

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs`

**Actual root cause (2026-03-18):** The plan's original premise (unresolved type variables reaching codegen) was incorrect. Types ARE correctly resolved — lambda parameters use `ptr dereferenceable(24)` (correct for str), not `i64`. The actual bug is in ARC RC emission:

`declare_and_process_lambda()` did NOT apply AIMS param ownership annotations before running the ARC pipeline. `process_arc_function()` (for top-level functions) correctly applies `Owned`/`Borrowed` from AIMS contracts, but lambdas skipped this step. As a result, `collect_all_borrowed_defs()` saw all lambda params as `Owned` (the default), failed to recognize borrowed params and their Let aliases, and the edge cleanup (`collect_invoke_edge_decs` Category 2) emitted spurious `RcDec` for borrowed-param aliases — causing double-free on non-SSO strings and other heap-allocated captures.

**Fix:** 12 lines added to `declare_and_process_lambda()` to apply AIMS contracts to lambda params before the name change (line 337). The contract lookup must use the original lambda name (before unique renaming).

- [x] Traced J17 program — types correctly resolved, no unresolved type variables (2026-03-18)
- [x] Compared J5 (scalar capture) vs J17 (fat pointer capture) — scalar captures work because they don't have RC operations; the bug is RC-specific, not type-specific (2026-03-18)
- [x] Identified root cause: `declare_and_process_lambda()` missing AIMS param ownership application (2026-03-18)
- [x] Verified via `ORI_LOG=ori_arc=trace`: `all_borrowed_defs` was empty for lambda (param ownership stuck at `Owned`) (2026-03-18)
- [x] Verified edge cleanup Category 2 (`collect_invoke_edge_decs`) was the emission path for the spurious `RcDec` (2026-03-18)
- [x] Verified ARC IR dump was misleading: dump's `run_arc_pipeline_all` ran on 1-block `Apply` form, but production pipeline ran on 3-block `Invoke` form (2026-03-18)
- [x] Lambda parameter types confirmed correct: `ptr dereferenceable(24)` not `i64`, drop functions use 24 bytes not 8 (2026-03-18)
- [x] Fix applied: AIMS param ownership now applied to lambdas in `define_phase.rs:declare_and_process_lambda()` (2026-03-18)

---

## 02.2 Fix Type Propagation for Capture Environments

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs`

**Actual fix (2026-03-18):** Type propagation was already correct. The fix was applying AIMS param ownership to lambda params before the ARC pipeline. See 02.1 for root cause.

- [x] Fix applied: AIMS param ownership applied in `declare_and_process_lambda()` before `run_arc_pipeline()` — 12 lines added (2026-03-18)
- [x] Capture env types already correctly resolved — no type propagation fix needed (2026-03-18)
- [x] Recursive case (closure A captures closure B) blocked by pre-existing type checker limitation ("only functions can be called" on returned closures) — not a regression (2026-03-18)
- [x] Multiple captures verified: `str + [int]` multi-capture test passes (`test_closure_multi_capture`) (2026-03-18)

---

## 02.3 Fix Method Resolution on Captured Values

**File(s):** N/A — method resolution was already correct.

**Actual status (2026-03-18):** Method resolution works correctly for captured values. The plan's description of `i64` thunk params and 8-byte drop functions was based on stale J17 results. Current J17 IR shows correct types: `ptr dereferenceable(24)` params, 24-byte `ori_rc_free`, `ori_str_len` correctly dispatched.

- [x] Method resolution verified: `str.length()` on captured str works in all 5 AOT tests (2026-03-18)
- [x] Thunk signature verified correct: `@_ori_partial_1(ptr %0, ptr %1)` — both `ptr` (2026-03-18)
- [x] Drop function verified correct: `_ori_drop$202` calls `ori_rc_free(ptr, 24, 8)` — correct str size (2026-03-18)
- [x] Method dispatch works for captured `str` and `[int]` in AOT tests (2026-03-18)
- [x] Chained method calls on captures: blocked by pre-existing limitation (`.trim()` not yet AOT-emitted) — tracked separately (2026-03-18)

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

- [x] Write AOT test: closure capturing `str` and calling `.length()` — `test_closure_capture_heap_str` (2026-03-18)
- [x] Write AOT test: closure capturing `[int]` and calling `.length()` — `test_closure_capture_list` (2026-03-18)
- [ ] Write AOT test: closure capturing a struct with str field and accessing the field <!-- blocked-by:5 -->
- [ ] Write AOT test: closure capturing another closure and calling it <!-- blocked-by:10 -->
- [ ] Write AOT test: closure capturing `Option<str>` and pattern matching on it <!-- blocked-by:9 -->
- [x] Write AOT test: closure with multiple non-scalar captures (`str` + `[int]`) — `test_closure_multi_capture` (2026-03-18)
- [ ] Write AOT test: nested closure — outer captures str, inner captures outer's captured str <!-- blocked-by:10 -->
- [ ] Write AOT test: closure capturing `(str, int)` tuple and accessing `.0` <!-- blocked-by:5 -->
- [x] Write AOT test: closure passed as higher-order argument — `test_closure_passed_with_str_capture` (2026-03-18)
- [ ] Write AOT test: closure returned from function — blocked by type checker limitation ("only functions can be called" on returned closures) <!-- blocked-by:10 -->
- [x] Write AOT test: closure with str param — `test_closure_capture_str_with_param` (J17 pattern) (2026-03-18)
- [x] All implemented tests pass in both eval and AOT with identical results — dual-exec-verify clean (2026-03-18)
- [x] Valgrind clean on all closure-capture tests (2026-03-18)

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [x] Closure capturing `str` compiles and runs correctly in AOT — `test_closure_capture_heap_str` (2026-03-18)
- [x] Closure capturing `[T]` compiles and runs correctly in AOT — `test_closure_capture_list` (2026-03-18)
- [ ] Closure capturing struct with fat fields compiles and runs correctly <!-- blocked-by:5 -->
- [ ] Closure capturing another closure compiles and runs correctly <!-- blocked-by:10 -->
- [ ] Nested closures with fat captures compile and run correctly <!-- blocked-by:10 -->
- [x] Multi-capture (str + [int]) compiles and runs correctly — `test_closure_multi_capture` (2026-03-18)
- [ ] Closure capturing tuple `(str, int)` compiles and runs correctly <!-- blocked-by:5 -->
- [ ] Closure returned from function with fat capture compiles and runs correctly <!-- blocked-by:10 -->
- [x] No unresolved type variables (`Idx(N)`) reach LLVM codegen — verified: types are correct (`ptr dereferenceable(24)`) (2026-03-18)
- [x] No `_ori_drop$N` functions with wrong size — verified: `_ori_drop$202` uses 24 bytes (correct for str) (2026-03-18)
- [x] All `_ori_partial_N` thunks have correct parameter types — verified: `@_ori_partial_1(ptr, ptr)` (2026-03-18)
- [x] `./test-all.sh` green — 12,972 pass, 0 fail (2026-03-18)
- [x] `./clippy-all.sh` green (2026-03-18)
- [x] Valgrind clean on all closure-capturing-fat-pointer tests (2026-03-18)
- [x] J17 re-run: AOT produces exit code 10 (matching eval), leak check clean, Valgrind clean (2026-03-18)

**Exit Criteria:** `ORI_LOG=error ori build` on all test programs above produces zero "unresolved type variable" errors, AND `diagnostics/dual-exec-verify.sh` reports 0 mismatches for all test programs, AND `diagnostics/valgrind-aot.sh` reports 0 errors.
