---
section: "01"
title: LLVM Codegen Completeness
status: in-progress
goal: Make the existing ARC pipeline produce correct LLVM IR for all instruction variants
sections:
  - id: "1.1"
    title: Atomic Refcount Operations
    status: complete
  - id: "1.2"
    title: Drop Function Generation
    status: complete
  - id: "1.3"
    title: IsShared Inline Check
    status: complete
  - id: "1.4"
    title: Reuse Emission
    status: complete
  - id: "1.5"
    title: PartialApply Closure Environment
    status: complete
  - id: "1.6"
    title: Completion Checklist
    status: not-started
---

# Section 01: LLVM Codegen Completeness

**Status:** In Progress
**Goal:** Close all stubs in `arc_emitter.rs` and `ori_rt` so the ARC pipeline produces correct, production-ready LLVM IR.

**Design Reference:** `plans/dpr_arc-optimization_02212026.md` — Phase 1: Foundation

---

## 1.1 Atomic Refcount Operations

Replace non-atomic `*rc_ptr += 1` / `*rc_ptr -= 1` in `ori_rt` with atomic operations following the Swift `swift_retain`/`swift_release` memory ordering pattern.

- [x] Replace `ori_rc_inc` with `AtomicI64::fetch_add(1, Ordering::Relaxed)`
  - `Relaxed` is sufficient — increment only needs to be visible before next decrement
  - Matches Swift's `swift_retain` and Rust's `Arc::clone`
- [x] Replace `ori_rc_dec` with `AtomicI64::fetch_sub(1, Ordering::Release)` + `Acquire` fence before drop
  - `Release` on decrement ensures all writes to the object are visible
  - `Acquire` fence before drop ensures the deallocating thread sees all prior writes
  - This is the standard pattern from Rust's `Arc::drop`
- [x] Add `--single-threaded` compile flag for non-atomic fast path
  - Programs that don't use task parallelism can skip atomic overhead
  - Gate via `#[cfg]` or runtime flag
- [x] Add tests: concurrent increment/decrement from multiple threads (use `std::thread`)
- [x] Add tests: verify drop function is called exactly once when refcount reaches zero
- [x] *(hygiene review 2026-02-21)* Wrap `drop_fn` call in `catch_unwind` + `abort` — enforces the `nounwind` LLVM attribute on `ori_rc_dec`
- [x] *(hygiene review 2026-02-21)* Add `debug_assert!(prev > 0)` to catch use-after-free in debug builds
- [x] *(hygiene review 2026-02-21)* Replace `static mut ORI_PANIC_TRAMPOLINE` with `AtomicPtr<()>` — eliminates data-race UB
- [x] *(hygiene review 2026-02-21)* Fix `ori_str_from_bool` to heap-allocate (uniform ownership across `ori_str_from_*`)
- [x] *(hygiene review 2026-02-21)* Route `ori_assert*` through `ori_panic_cstr` on failure; use `extern "C-unwind"` for unwind-capable FFI functions
- [x] *(hygiene review 2026-02-21)* Fix list allocation alignment: `Layout::from_size_align(total, 8)` (was alignment 1)

---

## 1.2 Drop Function Generation

Wire `DropInfo`/`DropKind` from `ori_arc::drop` into `arc_emitter.rs` for `RcDec`. Currently `RcDec` passes `null` as `drop_fn`.

- [x] Generate per-type LLVM IR drop functions, cached by mangled name (`_ori_drop$TypeName`)
  - `DropKind::Trivial` -> direct `ori_rc_free` call (no field cleanup needed)
  - `DropKind::Fields` -> GEP per field + recursive `RcDec` for ref-typed fields + `ori_rc_free`
  - `DropKind::Enum` -> switch on tag + per-variant field Dec + `ori_rc_free`
  - `DropKind::Collection` -> iteration loop calling `RcDec` per element + `ori_rc_free`
  - `DropKind::Map` -> key/value iteration loop + `ori_rc_free`
  - `DropKind::ClosureEnv` -> same as Fields (GEP per captured variable + recursive Dec)
- [x] Wire generated drop function pointer into `RcDec` emission in `emit_instr`
- [x] Mark generated drop functions as `nounwind` in LLVM IR — `ori_rc_dec`'s `call_drop_fn` aborts if a drop function unwinds
- [x] Add drop function cache to `ArcIrEmitter` (`FxHashMap<MangledTypeName, FunctionId>`)
- [x] Add tests: struct with ref-typed fields verifies recursive Dec
- [x] Add tests: enum with mixed scalar/ref variants verifies tag dispatch
- [x] Add tests: nested containers (list of lists) verify iteration + recursive Dec

---

## 1.3 IsShared Inline Check

Replace the `const_bool(false)` stub with a real refcount check. This is the gate for reset/reuse correctness.

- [x] Emit inline 3-instruction sequence:
  ```
  %rc_ptr = getelementptr i8, ptr %data_ptr, i64 -8
  %rc_val = load i64, ptr %rc_ptr
  %is_shared = icmp sgt i64 %rc_val, 1
  ```
- [x] Verify this is inlined (not a function call) to avoid per-check overhead
- [x] Add tests: unique object returns `false`, shared object returns `true`
- [x] Add tests: object becomes unique after last sharer decrements

---

## 1.4 Reuse Emission

Complete the `Reuse` instruction emission. On fast path (after `IsShared` returns false), mutate in-place; on slow path (shared), fall back to `RcDec` + fresh `Construct`.

- [x] Fast path: emit `Set` instructions for field mutation via GEP + store
- [x] Fast path: emit `SetTag` for variant changes (enum reuse across variants)
- [x] Slow path: emit `RcDec` of old value + `Construct` of new value (current behavior)
- [x] Wire fast/slow paths via `br i1 %is_shared, label %slow, label %fast`
- [x] Add tests: reuse of same-type struct (fast path taken)
- [x] Add tests: reuse of enum across variants (tag change + field mutation)
- [x] Add tests: shared object falls through to slow path correctly

---

## 1.5 PartialApply Closure Environment

Generate proper closure environment allocation and wrapper functions. ~~Currently emits null pointers.~~

**Fixed across 4 files, 2 crates (ori_arc + ori_llvm):**

- [x] `ori_arc/lower/mod.rs`: Add `var_type()` accessor and `emit_partial_apply()` to `ArcIrBuilder`
- [x] `ori_arc/lower/calls/mod.rs`: Rewrite `lower_lambda` with real capture analysis (`collect_captures`) + `PartialApply` emission (was emitting empty captures + `Construct(Closure)`)
- [x] `ori_arc/lower/calls/mod.rs`: Get actual param types from `Pool::function_params(ty)` (was hardcoded `Idx::UNIT`)
- [x] `ori_llvm/function_compiler/mod.rs`: Stop discarding lambda `ArcFunction`s — add `compile_lambda_arc()` + `compute_arc_function_abi()`
- [x] Allocate environment struct via `ori_rc_alloc` with size = sum of captured variable sizes
- [x] Pack captured variables into environment via GEP + store (one per capture)
- [x] Generate drop function for env struct (RC-decs captured ref-typed vars + `ori_rc_free`)
- [x] Generate wrapper function that:
  - Receives `env_ptr` as first parameter
  - Unpacks each captured variable via GEP + load
  - Forwards to actual callee with unpacked captures + remaining arguments
  - Handles calling convention bridging (wrapper is `ccc`, callee is `fastcc`)
  - Handles sret return bridging (alloca+store+load when callee uses sret)
- [x] Wire `{ fn_ptr, env_ptr }` thick closure representation
- [x] Add tests: lambda capturing one variable (`test_arc_lambda_capture_int`)
- [x] Add tests: lambda capturing multiple variables (`test_arc_lambda_capture_multiple`)
- [x] Add tests: no-capture lambda (`test_arc_lambda_no_capture`)
- [x] Add tests: closure passed as function argument (`test_arc_lambda_passed_to_function`)

**Bonus fix discovered during testing:**
- [x] `ir_builder/comparisons.rs`: Add `is_null_ptr()` — `icmp_eq` only handled integers, causing segfault on pointer null checks in closure cleanup (Tier 1) and RcInc/RcDec closure handling (Tier 2)

**Note:** ARC codegen (Tier 2) is not yet globally enabled — tests exercise the Tier 1 closure path. See `.claude/rules/arc.md` "Enabling ARC Codegen" for how to activate Tier 2. The ARC infrastructure (lowering, lambda compilation, emitter) is fully wired and ready.

---

## 1.6 Completion Checklist

- [x] All `arc_emitter.rs` stubs replaced with real implementations
- [x] `ori_rt` refcount operations are atomic
- [ ] No FIXME/TODO/stub comments remain in ARC codegen path
- [x] All existing AOT tests pass (`./llvm-test.sh`) — 425 passed, 0 failed
- [x] New tests cover each subsection above
- [x] Run `./test-all.sh` — 10,452 passed, 0 failed, no regressions

**Exit Criteria:** Every ARC IR instruction produces correct LLVM IR. The `IsShared` / `Reuse` / `RcDec` + drop chain works end-to-end for a struct with ref-typed fields.
