# Proposal: Unsafe Semantics

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-02-20
**Approved:** 2026-02-20
**Affects:** Compiler (parser, IR, type checker, evaluator, LLVM), capabilities spec, FFI spec, grammar

---

## Summary

Define `Unsafe` as a **marker capability** (like `Suspend`) that gates operations the compiler cannot verify. The `unsafe { }` block discharges the `Unsafe` capability locally — a programmer assertion that the contained code is correct despite bypassing safety guarantees.

| Concept | Mechanism |
|---------|-----------|
| Marking unsafe operations | `uses Unsafe` on function signature |
| Containing unsafety | `unsafe { expr }` block discharges `Unsafe` locally |
| Propagation | Like any capability — callers must declare or discharge |
| Binding | Cannot be bound via `with...in` (marker, like `Suspend`) |
| Relationship to FFI | Orthogonal — most FFI calls are safe; only specific operations need `Unsafe` |

```ori
// Internal: exposes raw pointer operation
@ptr_read (ptr: CPtr, offset: int) -> byte uses Unsafe =
    __intrinsic_ptr_read(ptr, offset);

// Safe wrapper: discharges Unsafe locally
@safe_read (ptr: CPtr, offset: int) -> byte uses FFI =
    unsafe { ptr_read(ptr, offset) };

// Caller: no knowledge of unsafe internals
@process (ptr: CPtr) -> str uses FFI =
    str(safe_read(ptr, 0));
```

---

## Table of Contents

- [Part I: Motivation](#part-i-motivation)
  - [1. What Safety Guarantees Does Ori Make?](#1-what-safety-guarantees-does-ori-make)
  - [2. What Operations Bypass These Guarantees?](#2-what-operations-bypass-these-guarantees)
  - [3. Why Not Just Use FFI?](#3-why-not-just-use-ffi)
  - [4. Prior Art](#4-prior-art)
- [Part II: Design](#part-ii-design)
  - [5. Unsafe as a Marker Capability](#5-unsafe-as-a-marker-capability)
  - [6. The unsafe Block](#6-the-unsafe-block)
  - [7. Grammar Changes](#7-grammar-changes)
  - [8. Type Checking Rules](#8-type-checking-rules)
  - [9. Evaluation Semantics](#9-evaluation-semantics)
  - [10. Interaction with Other Features](#10-interaction-with-other-features)
  - [11. Error Messages](#11-error-messages)
- [Part III: Implementation](#part-iii-implementation)
  - [12. Implementation Phases](#12-implementation-phases)
  - [13. Roadmap Impact](#13-roadmap-impact)
- [Part IV: Open Questions](#part-iv-open-questions)
  - [14. Open Questions](#14-open-questions)
  - [15. Non-Goals](#15-non-goals)

---

# Part I: Motivation

## 1. What Safety Guarantees Does Ori Make?

Ori's safety contract — what the compiler guarantees about well-typed programs:

| Guarantee | Mechanism |
|-----------|-----------|
| **Type safety** | Hindley-Milner inference; no implicit coercions |
| **Memory safety** | ARC — no dangling pointers, no use-after-free, no double-free |
| **No data races** | Value semantics; no shared mutable references |
| **Effect tracking** | Capabilities — all side effects declared in function signatures |
| **Null safety** | No null; `Option<T>` for optional values |
| **Integer safety** | Overflow traps in debug, wrapping in release (explicit `std.math` for alternatives) |

These guarantees hold for all Ori code **unless** the programmer explicitly opts out via `unsafe`.

## 2. What Operations Bypass These Guarantees?

Operations the compiler cannot verify and that may violate the safety contract:

| Operation | Bypasses | Status |
|-----------|----------|--------|
| Raw pointer dereference (`CPtr` → value) | Memory safety | Not yet implemented |
| Pointer arithmetic (`CPtr` + offset) | Memory safety | Not yet implemented |
| Transmute (reinterpret bits as different type) | Type safety | Not yet implemented |
| Calling C variadic functions | Type safety (args unchecked) | Parsing exists, calling not implemented |
| Accessing mutable globals | Data race freedom | Not yet implemented |
| Inline assembly | Everything | Not yet implemented |

> **Note:** None of these operations are currently implemented. This proposal defines the safety boundary *ahead of* their implementation so that when FFI (Section 11) is built, the `Unsafe` capability is ready.

## 3. Why Not Just Use FFI?

The `FFI` capability tracks *which functions call foreign code* — it's about **provenance** (where does code come from?). `Unsafe` tracks *which operations bypass safety* — it's about **trust** (what can go wrong?).

Most FFI calls are safe:

```ori
extern "c" {
    @strlen (s: CPtr) -> c_int      // Safe: well-defined behavior
    @printf (fmt: CPtr, ...) -> c_int // Unsafe: variadic args unchecked
}

// strlen: safe FFI call — only needs FFI capability
@safe_strlen (s: CPtr) -> int uses FFI = strlen(s) as int;

// printf: unsafe FFI call — needs both FFI and Unsafe
@call_printf (fmt: CPtr) -> void uses FFI, Unsafe = printf(fmt);
```

The two capabilities serve different purposes:

| Capability | Question | Mockable | Example |
|------------|----------|----------|---------|
| `FFI` | Does this call foreign code? | Yes — swap C impl for Ori mock | `with FFI = MockFFI in test()` |
| `Unsafe` | Does this bypass safety guarantees? | No — it's a trust assertion | `unsafe { raw_op() }` |

## 4. Prior Art

| Language | Model | Propagation | Key Difference from Ori |
|----------|-------|-------------|-------------------------|
| **Rust** | `unsafe { }` blocks + `unsafe fn` | Contained by blocks; `unsafe fn` propagates | No capability system; `unsafe` is standalone |
| **Swift** | `@unsafe` attribute + `Unsafe*Pointer` types | Structural (declarations + types) | Attribute-based, not block-based |
| **Go** | `unsafe` package (library-based) | Implicit (go vet only) | Not compiler-enforced |
| **Zig** | Per-type safety (pointer qualifiers) | Via type system | No blocks; safety is a type property |
| **Koka** | Effect system | Always propagates; handlers discharge | Effects are general; no special "unsafe" |
| **Lean 4** | `unsafe def` marker | Type-level enforcement | Simple marker, no containment mechanism |

**Ori's approach combines Rust's block containment with Koka's capability tracking.** The `Unsafe` capability propagates like any effect, but `unsafe { }` blocks discharge it locally — giving callers a clean, safe interface.

---

# Part II: Design

## 5. Unsafe as a Marker Capability

### Marker Capabilities

A _marker capability_ is a capability with no methods, no bindable implementation, and a specific discharge mechanism. Marker capabilities gate operations or contexts — they track *what the code does*, not *what API it uses*.

Shared semantics for all marker capabilities:
- **No methods** — gates operations, not API surface
- **Cannot be bound** via `with...in` (E1203: "marker capability cannot be explicitly bound")
- **Propagates** through the call chain like any capability
- **Each marker has its own discharge mechanism** — how the capability is satisfied

| Marker | Purpose | Discharge | May Suspend |
|--------|---------|-----------|-------------|
| `Suspend` | May suspend execution | Runtime / concurrency patterns | Yes |
| `Unsafe` | Bypasses safety guarantees | `unsafe { }` block | No |

`Unsafe` is a compiler intrinsic marker capability — like `Suspend`, it is special-cased in the type checker with no trait definition.

### Capability Table Update

| Capability | Purpose | May Suspend | Bindable | Discharge |
|------------|---------|-------------|----------|-----------|
| `Http` | HTTP client | Yes | Yes (`with`) | `with Http = impl in ...` |
| `Suspend` | Suspension marker | Yes | No | Runtime / concurrency patterns |
| **`Unsafe`** | **Safety bypass marker** | **No** | **No** | **`unsafe { }` block** |
| `FFI` | Foreign function interface | No | Yes (`with`) | `with FFI = impl in ...` |

## 6. The unsafe Block

The `unsafe { }` block is an expression that:

1. **Provides** the `Unsafe` capability within its scope
2. **Contains** unsafety — the surrounding function does NOT propagate `Unsafe`
3. **Returns** the value of its body expression (like any block)

```ori
// unsafe block — single expression
let result = unsafe { ptr_read(ptr, 0) };

// unsafe block — multi-statement
let result = unsafe {
    let raw = ptr_read(ptr, 0);
    let validated = validate(raw);
    validated
};
```

### Containment Semantics

The `unsafe { }` block stops propagation. A function that uses `unsafe { }` internally does NOT need `uses Unsafe`:

```ori
// This function is SAFE to callers — unsafe is contained
@safe_read (ptr: CPtr, offset: int) -> byte uses FFI =
    unsafe { ptr_read(ptr, offset) };

// Caller has no idea about unsafe internals
@caller () -> str uses FFI = str(safe_read(ptr, 0));
```

A function that exposes unsafe operations (without containing them) MUST declare `uses Unsafe`:

```ori
// This function IS unsafe — caller must handle it
@raw_read (ptr: CPtr, offset: int) -> byte uses FFI, Unsafe =
    ptr_read(ptr, offset);  // No unsafe block — Unsafe propagates
```

### Nested unsafe Blocks

`unsafe` blocks may be nested. Inner blocks are redundant but not an error:

```ori
unsafe {
    let a = ptr_read(ptr, 0);
    unsafe { ptr_write(ptr, 0, a) };  // Redundant but allowed
}
```

> **Lint opportunity:** A future `unused_unsafe` lint could warn about redundant `unsafe` blocks (matching Rust's `UNUSED_UNSAFE` lint). Not part of this proposal.

## 7. Grammar Changes

Update `grammar.ebnf`:

```ebnf
// --- Unsafe Expression ---
// See: spec/24-ffi.md § Unsafe Blocks
unsafe_expr = "unsafe" block_expr .
```

The grammar currently includes a parenthesized form (`"unsafe" "(" expression ")"`). This proposal removes the parenthesized form — only the block form (`unsafe { expr }`) is supported. The block form is unambiguous, consistent with Rust, and `unsafe { single_expr }` is already concise.

### IR Changes

Add `ExprKind::Unsafe`:

```rust
pub enum ExprKind {
    // ... existing variants ...

    /// `unsafe { expr }` — discharges Unsafe capability
    Unsafe(ExprId),
}
```

The `ExprId` points to a `Block` expression.

## 8. Type Checking Rules

### Rule 1: Unsafe Operations Require Unsafe Context

An _unsafe context_ exists when either:
- The current function declares `uses Unsafe`, OR
- The expression is inside an `unsafe { }` block

Operations requiring unsafe context (checked during type inference):
- Calling a function declared `uses Unsafe` without an `unsafe { }` block
- (Future) Dereferencing `CPtr`
- (Future) Pointer arithmetic on `CPtr`
- (Future) Calling C variadic extern functions
- (Future) Transmute operations

### Rule 2: unsafe Blocks Discharge Unsafe

When type-checking `unsafe { body }`:
1. Type-check `body` with the `Unsafe` capability available in scope
2. The result type of the `unsafe` expression is the result type of `body`
3. The `Unsafe` capability does NOT propagate to the enclosing function

### Rule 3: Marker Capability Binding Prohibition

Attempting to bind `Unsafe` via `with...in` is a compile-time error (E1203), following the same rule as `Suspend`. E1203 is generalized to cover all marker capabilities:

```
error[E1203]: marker capability `Unsafe` cannot be explicitly bound
  --> src/lib.ori:5:5
  |
5 |     with Unsafe = something in expr
  |          ^^^^^^ `Unsafe` is a marker capability
  |
  = help: use `unsafe { ... }` to assert safety
  = note: marker capabilities have no implementation to provide
```

### Rule 4: Unused Unsafe Warning (Future)

If an `unsafe { }` block contains no operations that require `Unsafe`, emit a warning:

```
warning[W0400]: unnecessary `unsafe` block
  --> src/lib.ori:5:5
  |
5 |     unsafe { 1 + 2 }
  |     ^^^^^^^^^^^^^^^^ no unsafe operations in this block
  |
  = help: remove the `unsafe` wrapper
```

This is a **lint**, not an error. Deferred to the linting infrastructure.

## 9. Evaluation Semantics

At runtime, `unsafe { expr }` evaluates to `expr`. The `unsafe` wrapper is purely a compile-time construct — it has no runtime effect.

```rust
// In evaluator: ExprKind::Unsafe(inner) simply evaluates inner
ExprKind::Unsafe(inner) => self.eval_expr(inner),
```

## 10. Interaction with Other Features

### With Capabilities

`Unsafe` is orthogonal to other capabilities. A function may require both:

```ori
@dangerous_io (ptr: CPtr) -> str uses FFI, Unsafe =
    str(ptr_read(ptr, 0));
```

### With Capsets

`Unsafe` may appear in capsets:

```ori
capset LowLevel = FFI, Unsafe
```

### With Testing

Since `Unsafe` cannot be mocked (no `with Unsafe = ...`), unsafe code requires integration-style testing:

```ori
// Test the safe wrapper, not the unsafe internals
@test_safe_read tests @safe_read () -> void = {
    // safe_read internally uses unsafe, but we don't need to know
    let result = safe_read(test_ptr, 0);
    assert_eq(result, expected_byte)
}
```

### With LLVM Codegen

`unsafe { expr }` generates the same IR as `expr`. No special LLVM handling needed — the safety boundary is purely a source-level concept.

## 11. Error Messages

| Code | Description |
|------|-------------|
| E1203 | Marker capability cannot be explicitly bound (generalized — covers `Suspend`, `Unsafe`) |
| E1250 | Unsafe operation outside unsafe context |
| E1252 | (Future, lint) Unnecessary `unsafe` block |

```
error[E1250]: operation requires `unsafe` context
  --> src/lib.ori:3:5
  |
3 |     ptr_read(ptr, 0)
  |     ^^^^^^^^^^^^^^^^ this operation may violate memory safety
  |
  = help: wrap in `unsafe { ... }` or add `uses Unsafe` to function signature
  = note: pointer dereference cannot be verified by the compiler
```

---

# Part III: Implementation

## 12. Implementation Phases

### Phase 0: Prerequisite — None

This feature has no blocking dependencies. It can be implemented at any time. However, it is most useful when paired with FFI (Section 11) which provides the operations that `Unsafe` gates.

### Phase 1: IR + Parser (can implement now)

1. Add `ExprKind::Unsafe(ExprId)` to `ori_ir/src/ast/expr.rs`
2. Implement `parse_unsafe_expr()` in `ori_parse/src/grammar/expr/primary.rs`:
   - Consume `TokenKind::Unsafe`
   - Expect `{` → parse block expression
   - Wrap result in `ExprKind::Unsafe`
3. Update `grammar.ebnf` to remove the parenthesized form
4. Add visitor support in `ori_ir/src/visitor.rs`
5. Add arena support for the new expression kind

**Tests:**
- Rust: `ori_parse/src/tests/parser.rs` — parse `unsafe { expr }`
- Ori: `tests/spec/syntax/unsafe/basic.ori` — syntax acceptance

### Phase 2: Type Checker

1. Add `Unsafe` to the standard capabilities list
2. Add `UnsafeContext` tracking to the type inference engine
3. Type-check `ExprKind::Unsafe(inner)`:
   - Push `Unsafe` capability into scope
   - Type-check `inner`
   - Result type = inner's type
   - Do NOT propagate `Unsafe` to enclosing function
4. Add E1250 diagnostic: unsafe operation outside unsafe context
5. Generalize E1203 to cover all marker capabilities (including `Unsafe`)

**Tests:**
- Rust: `ori_types/src/infer/expr/tests.rs` — type inference for unsafe blocks
- Ori: `tests/spec/capabilities/unsafe/` — E1250, E1203 errors

### Phase 3: Evaluator

1. Add `ExprKind::Unsafe(inner)` match arm — evaluates to `eval_expr(inner)`
2. No runtime effect — purely compile-time construct

**Tests:**
- Ori: `tests/spec/capabilities/unsafe/eval.ori` — unsafe blocks evaluate correctly

### Phase 4: LLVM Codegen

1. Add `ExprKind::Unsafe(inner)` → generate code for `inner` (transparent)
2. No special LLVM IR — safety is source-level only

### Phase 5: Wire Up Gated Operations (deferred to Section 11: FFI)

When FFI operations are implemented, add `Unsafe` requirement checks:
- C variadic calls → require unsafe context
- `CPtr` dereference → require unsafe context
- Pointer arithmetic → require unsafe context
- Transmute → require unsafe context

## 13. Roadmap Impact

| Section | Impact |
|---------|--------|
| **0 (Parser)** | Phase 1: parse `unsafe { }` |
| **6 (Capabilities)** | Add `Unsafe` to capability table; formalize marker capabilities |
| **11 (FFI)** | Phase 5: wire up gated operations |
| **14 (Testing)** | Document that unsafe code requires safe wrapper testing |

---

# Part IV: Open Questions

## 14. Open Questions

### Q1: Should `Unsafe` be a real trait or a compiler-intrinsic marker?

**Resolved:** Compiler intrinsic, like `Suspend`. Both are gating mechanisms, not API surfaces. No trait definition needed.

### Q2: Should `unsafe` blocks nest or stack?

**Resolved:** Allow nesting, with future lint for redundant inner blocks. Matches Rust's approach.

### Q3: Should there be an `unsafe @fn` shorthand?

**Resolved:** No. `uses Unsafe` on the signature serves the same purpose and is more consistent with Ori's capability model. Adding `unsafe` as a function modifier introduces a second syntax for the same concept.

### Q4: Should the parenthesized form `unsafe(expr)` be kept?

**Resolved:** Block-only form (`unsafe { expr }`). The parenthesized form was removed to avoid function-call ambiguity (`unsafe(foo)` looks like a function call) and maintain consistency with Rust. `unsafe { single_expr }` is already concise.

### Q5: What about `Intrinsics` — should SIMD operations require `Unsafe`?

**Resolved:** Keep separate. `Intrinsics` operations should be memory-safe by design (bounds-checked SIMD lanes). Only raw pointer operations need `Unsafe`.

## 15. Non-Goals

- **Runtime safety checks** — `unsafe` is compile-time only; no runtime overhead
- **Unsafe field access** — Ori has no `unsafe` fields (unlike Swift's `@unsafe` on properties)
- **Unsafe trait implementations** — deferred; may need `unsafe impl Trait` in the future for traits with safety invariants
- **Formal verification** — `unsafe` is a trust boundary, not a proof obligation
- **Linting infrastructure** — unused unsafe detection deferred to linting system

---

## Related Proposals

- [capability-unification-generics-proposal](../approved/capability-unification-generics-proposal.md) — establishes `with`/`uses` vocabulary
- [stateful-mock-testing-proposal](../approved/stateful-mock-testing-proposal.md) — handler system for capability mocking

## Supersedes

- `spec/24-ffi.md` § Unsafe Expressions (thin description, no capability integration)

## Honest Gaps

1. **No gated operations exist yet** — this proposal defines the boundary before the operations. Implementation of actual unsafe operations is deferred to Section 11 (FFI).
2. **No `unsafe impl`** — Rust has `unsafe trait` + `unsafe impl` for traits with safety invariants (e.g., `Send`, `Sync`). Ori may need this for `Sendable` (Section 17: Concurrency). Deferred.
3. **No formal safety model** — Ori's safety guarantees are described informally. A formal model (like RustBelt for Rust) is a research-level effort, not part of this proposal.
