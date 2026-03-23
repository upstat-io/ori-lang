# Proposal: Unsafe Operation Gating

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-03-22
**Affects:** Compiler (type checker, evaluator, LLVM), FFI spec
**Related:** `unsafe-semantics-proposal.md` (approved), `platform-ffi-proposal.md` (approved), `deep-ffi-proposal.md` (approved)
**Research:** `plans/deep-safety/research.md` (Part 4 scorecard — unsafe gating is prerequisite for 5 categories)

---

## Summary

Implement the operation-level gating defined in the approved unsafe-semantics proposal. Currently, `unsafe { }` is a syntactically valid but semantically transparent expression — it parses and evaluates but does not enforce that the operations inside it actually require unsafe context, nor does the type checker reject unsafe operations outside an unsafe context.

This proposal defines the specific operations that require unsafe context and the enforcement mechanism.

---

## Problem Statement

The unsafe-semantics proposal (approved 2026-02-20) defines five categories of operations that require `unsafe`:

1. Dereferencing raw pointers (`CPtr` → value)
2. Pointer arithmetic (`CPtr` + offset)
3. Accessing mutable statics
4. Transmuting types (reinterpreting bits)
5. Calling C variadic functions

The proposal was approved with the note that these operations would be gated "when FFI operations are implemented" (Phase 5 of the proposal's implementation plan). The current state is:

- `unsafe { expr }` parses and creates `ExprKind::Unsafe(inner)`
- The evaluator treats it as `eval_expr(inner)` (transparent)
- The type checker does **not** check whether `inner` contains operations requiring unsafe
- The type checker does **not** reject unsafe operations appearing outside an `unsafe` block
- E1250 ("operation requires unsafe context") is defined but never emitted

This means `unsafe` currently has no semantic effect — it is documentation, not enforcement. Any code can perform any operation regardless of whether it is inside an `unsafe` block.

---

## Design

### Which Operations Require Unsafe Context

As the FFI and low-level operation implementations mature, each operation below must require unsafe context. The gating should be implemented incrementally — each operation is gated when its implementation lands, not all at once.

#### Tier 1: Gate with FFI implementation

| Operation | Trigger | When |
|-----------|---------|------|
| Calling C variadic functions | `extern "c" { @f (fmt: CPtr, ...) }` — the `...` parameter | When variadic extern calls are implemented |
| Raw pointer dereference | Intrinsic: `__ptr_read(ptr: CPtr, offset: int) -> T` | When CPtr dereference is implemented |
| Raw pointer write | Intrinsic: `__ptr_write(ptr: CPtr, offset: int, value: T)` | When CPtr write is implemented |
| Pointer arithmetic | Intrinsic: `__ptr_offset(ptr: CPtr, offset: int) -> CPtr` | When CPtr arithmetic is implemented |

#### Tier 2: Gate with type system features

| Operation | Trigger | When |
|-----------|---------|------|
| Transmute | Intrinsic: `__transmute<S, T>(value: S) -> T` | When transmute is implemented |
| Mutable static access | Reading or writing a non-`$` module-level binding | When mutable statics are implemented |

#### Tier 3: Future (not in this proposal)

| Operation | Notes |
|-----------|-------|
| Inline assembly (`asm { }`) | Requires `InlineAsm` capability, not just `Unsafe` |
| Union field access | If unions are ever added to Ori |

### Enforcement Mechanism

#### Type Checker: Unsafe Context Tracking

The type checker shall maintain an `unsafe_context: bool` flag during expression type checking:

```rust
struct TypeCheckContext {
    // ... existing fields ...
    unsafe_context: bool,  // true inside unsafe { } blocks
}
```

When type-checking `ExprKind::Unsafe(inner)`:
1. Set `unsafe_context = true`
2. Type-check `inner`
3. Restore `unsafe_context` to previous value
4. Result type = `inner`'s type

When type-checking an operation that requires unsafe:
1. Check `unsafe_context == true`
2. If false, emit E1250

#### E1250 Error

```
error[E1250]: operation requires `unsafe` context
  --> src/driver.ori:42:5
   |
42 |     __ptr_read(ptr: device_ptr, offset: 0)
   |     ^^^^^^^^^^ this operation may violate memory safety
   |
   = help: wrap in `unsafe { ... }` or add `uses Unsafe` to function signature
   = note: raw pointer dereference bypasses Ori's memory safety guarantees
```

#### W0400 Warning (Future Lint)

When an `unsafe { }` block contains no operations that require unsafe context:

```
warning[W0400]: unnecessary `unsafe` block
  --> src/lib.ori:10:5
   |
10 |     unsafe { 1 + 2 }
   |     ^^^^^^ no unsafe operations in this block
   |
   = help: remove the `unsafe` wrapper
```

This lint is deferred until the gated operation set stabilizes. It shall not be implemented until all Tier 1 operations are gated.

### Incremental Gating Strategy

Each gated operation is behind a feature flag during development:

```rust
// In the type checker
fn check_requires_unsafe(&self, op: &UnsafeOp) -> bool {
    match op {
        UnsafeOp::PtrRead => true,      // Gate immediately when implemented
        UnsafeOp::PtrWrite => true,
        UnsafeOp::PtrOffset => true,
        UnsafeOp::Variadic => true,
        UnsafeOp::Transmute => true,
        UnsafeOp::MutableStatic => true,
    }
}
```

When a new operation is implemented in the evaluator/LLVM, its gating is activated simultaneously. Tests for the operation include both positive (inside `unsafe {}`) and negative (outside `unsafe {}` → E1250) cases.

---

## Interaction with `uses Unsafe`

A function that declares `uses Unsafe` implicitly makes its entire body an unsafe context. This is already defined in the unsafe-semantics proposal:

```ori
// Entire body is unsafe context — no unsafe { } blocks needed
@raw_read (ptr: CPtr, offset: int) -> byte uses Unsafe =
    __ptr_read(ptr: ptr, offset: offset);
```

The type checker sets `unsafe_context = true` for the body of any function declaring `uses Unsafe`.

### Interaction with `without Unsafe`

If the negative-effect `without` clause is implemented (see `negative-effect-without-proposal.md`), then `without Unsafe` forbids both:
- `unsafe { }` blocks within the function body
- `uses Unsafe` on any callee

This enables "provably safe" modules — code that is statically guaranteed to contain no unsafe operations.

---

## Implementation Phases

### Phase 1: Type Checker Infrastructure (3-5 days)

1. Add `unsafe_context: bool` to type checking context
2. Set it to `true` when entering `ExprKind::Unsafe`
3. Set it to `true` for bodies of functions declaring `uses Unsafe`
4. Add `check_requires_unsafe()` dispatcher (empty initially — no operations gated yet)
5. Tests: verify the context flag threads correctly through nested expressions

### Phase 2: Gate Operations As They Land (ongoing)

As each unsafe operation is implemented in the compiler:
1. Add it to the `UnsafeOp` enum
2. Add the type checker check at the operation's inference site
3. Add positive test (works inside `unsafe {}`)
4. Add negative test (E1250 outside `unsafe {}`)

This is not a big-bang change. Each operation is gated atomically with its implementation.

### Phase 3: W0400 Lint (deferred)

After all Tier 1 operations are gated and the set is stable:
1. Track whether any unsafe operation was encountered inside an `unsafe {}` block
2. If the block exits without encountering any, emit W0400
3. Initially as a warning, potentially upgradeable to error via lint configuration

---

## Spec Changes Required

1. **§26.5 Unsafe Expressions**: Update to list the specific gated operations (currently lists them in prose; should be a normative table)
2. **Error codes**: E1250 is already defined; add W0400

---

## Success Criteria

For each gated operation:

```ori
// Must compile
@safe_wrapper () -> byte uses FFI = unsafe { __ptr_read(ptr: p, offset: 0) };

// Must produce E1250
@broken () -> byte uses FFI = __ptr_read(ptr: p, offset: 0);
```

The `uses Unsafe` propagation must also work:

```ori
// Must compile — entire body is unsafe context
@low_level () -> byte uses Unsafe, FFI = __ptr_read(ptr: p, offset: 0);

// Must produce E1200 — caller needs Unsafe or unsafe { }
@caller () -> byte uses FFI = low_level();
```
