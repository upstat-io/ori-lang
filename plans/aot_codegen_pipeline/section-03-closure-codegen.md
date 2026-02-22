---
section: "03"
title: "Closure Codegen Completion"
status: not-started
goal: "Complete PartialApply emission with proper heap allocation, wrapper functions, and environment drop"
sections:
  - id: "03.1"
    title: "Closure environment heap allocation"
    status: not-started
  - id: "03.2"
    title: "Wrapper function generation"
    status: not-started
  - id: "03.3"
    title: "Environment drop functions"
    status: not-started
  - id: "03.4"
    title: "Tests"
    status: not-started
---

# Section 03: Closure Codegen Completion

**Status:** Not Started
**Goal:** `PartialApply` in ARC IR produces correct, RC-tracked closures in LLVM IR with proper environment drop.

**Context:** Closures have an impedance mismatch: ARC IR's `PartialApply(func, captures)` is a logical closure, but LLVM needs a concrete closure struct `{fn_ptr, env_ptr}`, a wrapper function that unpacks the environment, and a drop function that decrements captured RC values. The current implementation has a TODO at line 1811 of `arc_emitter/mod.rs`: `// TODO: proper env packing with RC-tracked allocation`.

**Reference:** Every compiler struggles here — Swift heap-allocates contexts with metadata, Lean boxes closures, Rust monomorphizes most away. The key is clean separation of concerns.

---

## 03.1 Closure Environment Heap Allocation

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

- [ ] Implement `emit_closure_env_alloc()`:
  - Compute environment struct layout: `{ capture_0: T0, capture_1: T1, ... }`
  - Allocate via `ori_rc_alloc(env_size)` — returns RC-tracked pointer
  - Store each captured value into the environment struct via GEP+store
  - `RcInc` each captured value that is `RcPointer`/`FatValue` (the env now owns them)

- [ ] Produce `EmittedValue::Pair { first: wrapper_fn_ptr, second: env_ptr }`

- [ ] Handle zero-capture case: `env_ptr = null`, no allocation needed

---

## 03.2 Wrapper Function Generation

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

- [ ] Implement `emit_closure_wrapper()`:
  - Generate a wrapper function `_ori_closure$N(env_ptr, arg0, arg1, ...)`:
    1. Cast `env_ptr` to the environment struct type
    2. Load each captured value via GEP+load
    3. Call the original function with captures + args
    4. Return the result
  - The wrapper has C-compatible calling convention

- [ ] Cache wrapper functions by `(original_func, capture_types)` to avoid duplicates

- [ ] Handle ABI correctly:
  - If original function uses `sret`, wrapper must forward the sret pointer
  - If captures include aggregates, pass by pointer

---

## 03.3 Environment Drop Functions

**Files:** `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs`, `mod.rs`

- [ ] Register closure environment types with the drop function cache:
  - When `PartialApply` is emitted, register `(env_struct_type, capture_types)` in `drop_fn_cache`
  - The drop function iterates captured fields and calls `RcDec` on each RC-tracked capture

- [ ] Ensure `RcDec` on a closure triggers the environment drop:
  - The runtime's `ori_rc_dec` already handles reaching refcount 0
  - The drop function pointer must be stored in the RC header or in a type-info table
  - **Decision needed:** inline drop (Swift pattern) vs runtime dispatch (Lean pattern)
  - Recommendation: inline drop via `_ori_drop$N` function (consistent with existing struct drop pattern)

- [ ] Handle nested closures: a closure that captures another closure needs its drop to recurse

---

## 03.4 Tests

- [ ] Unit tests for closure emission:
  - Zero-capture closure (function pointer, null env)
  - Single-capture closure (one RC value)
  - Multi-capture closure (mixed scalar + RC values)
  - Nested closure (closure capturing closure)

- [ ] AOT integration tests in `compiler/ori_llvm/tests/aot/`:
  - Lambda passed to higher-order function
  - Lambda returned from function
  - Lambda with captures mutated before call
  - Nested lambda with outer+inner captures
  - Lambda passed to iterator methods (map, filter, etc.)

- [ ] Memory leak verification:
  - Create and drop 1000 closures, verify RC count returns to 0
  - Use `ori_rc_live_count()` tracking from runtime

---

## 03.5 Completion Checklist

- [ ] `emit_closure_env_alloc()` allocates RC-tracked environments
- [ ] `emit_closure_wrapper()` generates bridge functions
- [ ] Wrapper cache prevents duplicate generation
- [ ] Environment drop functions generated and registered
- [ ] Nested closures handled correctly
- [ ] Zero-capture optimization (null env, no allocation)
- [ ] ABI compatibility verified (sret, indirect params)
- [ ] All tests pass, no memory leaks
- [ ] TODO at line 1811 of arc_emitter/mod.rs resolved

**Exit Criteria:** `ori build` compiles programs with lambdas/closures that pass through higher-order functions, with zero memory leaks and correct captured value lifetimes.
