# Zig Features Discussion for Sigil

**Date**: 2026-01-22
**Status**: Complete

This document catalogs Zig language features that were evaluated for potential adoption in Sigil. Each feature was discussed and a decision was made.

---

## Summary Table

| # | Feature | Decision | Rationale |
|---|---------|----------|-----------|
| 1 | Comptime | **Rejected** | Conflicts with minimalism; existing generics suffice |
| 2 | Explicit Allocators | **Rejected** | Incompatible with ARC memory model |
| 3 | Optional `?` Syntax | **Rejected** | Already have `Option<T>` with consistent syntax |
| 4 | Error Union Types | **Rejected** | Already have `Result<T, E>`; explicit errors preferred |
| 5 | defer/errdefer | **Rejected** | ARC + `with` pattern is the design bet |
| 6 | Sentinel-Terminated Types | **Rejected** | Low-level; Sigil uses length-prefixed strings |
| 7 | Packed Structs | **Rejected** | Low-level systems feature |
| 8 | Arbitrary-Width Integers | **Rejected** | Sigil is not low-level; `int` + `byte` suffice |
| 9 | Build Modes | **Rejected** | No build modes; consistent behavior everywhere |
| 10 | No Hidden Control Flow | **Already Adopted** | Core Sigil principle |
| 11 | Optional Stdlib | **Rejected** | Not targeting bare-metal; prelude is integral |
| 12 | C Interop | **Rejected** | Conflicts with high-level design; use HTTP/IPC |
| 13 | Integrated Build System | **Rejected** | Simple CLI approach sufficient |
| 14 | Colorblind Async | **Rejected** | Explicit `uses Async` chosen for testability |
| 15 | anytype Generics | **Rejected** | Explicit trait bounds preferred |
| 16 | inline for | **Rejected** | Coupled with comptime; let compiler optimize |
| 17 | Multiple Pointer Types | **Rejected** | Incompatible with ARC; pointers abstracted away |
| 18 | Error Return Traces | **Proposal** | Valuable for debugging; proposal created |
| 19 | Runtime Safety (Opt-Out) | **Already Adopted** | Safety always on; no opt-out |
| 20 | Order-Independent Decls | **Adopted** | Reduces cognitive load; AI-friendly |

---

## Key Insights

### Sigil is Not a Systems Language

The majority of rejections (12 of 16) stem from Sigil's position as a high-level, AI-first language rather than a systems programming language like Zig:

- No manual memory management (ARC instead)
- No bit-level control (no packed structs, arbitrary integers)
- No C interop priority
- No bare-metal targets

### Already Aligned Philosophies

Two features were already adopted in Sigil's design:

1. **No Hidden Control Flow** - Sigil's "no magic" principle
2. **Runtime Safety Checks** - Always on, no opt-out via build modes

### Adopted Features

1. **Order-Independent Declarations** - Enables natural code organization, mutual recursion, and reduces cognitive load for AI code generation

### Proposals Generated

1. **Error Return Traces** - Automatic stack trace collection for `Result` error paths. See `error-return-traces-proposal.md`

---

## Detailed Decisions

### 1. Comptime (Compile-Time Execution)

**Decision**: Rejected

**Zig Feature**: `comptime` forces evaluation at compile-time. Variables become compile-time constants, functions execute during compilation, and types can be computed/returned.

**Rationale**: Sigil's existing generic system with explicit trait bounds suffices. Comptime adds complexity that conflicts with Sigil's minimalism principle, and the AI-first design benefits from predictable, non-magical behavior over compile-time metaprogramming power.

---

### 2. Explicit Allocators

**Decision**: Rejected

**Zig Feature**: No implicit heap allocation. Every function that needs to allocate takes an explicit `Allocator` parameter.

**Rationale**: Fundamentally incompatible with Sigil's ARC-based memory model. Sigil explicitly chose ARC to avoid the complexity of manual memory management, prioritizing AI-friendly simplicity over fine-grained control.

---

### 3. Optional Types with `?` Syntax

**Decision**: Rejected

**Zig Feature**: `?T` represents an optional type.

**Rationale**: Sigil already has `Option<T>` with full functionality. The generic syntax is consistent with `Result<T, E>` and other types. Special syntax would add inconsistency for minimal benefit.

---

### 4. Error Union Types

**Decision**: Rejected

**Zig Feature**: `!T` or `ErrorType!T` represents a value that can be an error or success.

**Rationale**: Sigil already has `Result<T, E>` with equivalent functionality. Explicit error types in signatures align better with Sigil's explicitness principle than Zig's inferred error sets.

---

### 5. defer and errdefer

**Decision**: Rejected

**Zig Feature**: `defer` schedules code to run when scope exits. `errdefer` only runs if scope exits via error.

**Rationale**: Sigil is banking on ARC + `with` pattern being sufficient. ARC handles automatic cleanup via refcount, `Drop` trait handles destructors, and `with` provides structured resource management. This is a core design bet.

---

### 6. Sentinel-Terminated Types

**Decision**: Rejected

**Zig Feature**: Types can specify a sentinel value that terminates a sequence (e.g., null-terminated strings).

**Rationale**: Low-level systems feature primarily for C interop. Sigil's length-prefixed strings are safer and more modern. Doesn't align with Sigil's abstraction level or AI-first focus.

---

### 7. Packed Structs

**Decision**: Rejected

**Zig Feature**: Structs with no padding, backed by an integer type. Fields can be arbitrary bit widths.

**Rationale**: Low-level systems feature for bit-level memory control. Sigil operates at a higher abstraction level. Binary protocol handling can be addressed through library functions rather than language-level constructs.

---

### 8. Arbitrary-Width Integers

**Decision**: Rejected

**Zig Feature**: Integer types can have any bit width: `u5`, `i27`, `u128`.

**Rationale**: Sigil is not a low-level language. The current `int` (64-bit signed) + `byte` (8-bit unsigned) is sufficient. Simplicity over precision - AI doesn't need to choose between integer sizes.

---

### 9. Build Modes

**Decision**: Rejected

**Zig Feature**: Four build modes (Debug, ReleaseSafe, ReleaseFast, ReleaseSmall) with different safety/performance tradeoffs.

**Rationale**: No build modes means consistent behavior everywhere. Safety checks always on, no behavioral differences between debug and release. This aligns with Sigil's predictability goals and prevents bugs where code works in debug but fails in release.

---

### 10. No Hidden Control Flow

**Decision**: Already Adopted

**Zig Feature**: If code doesn't look like a function call, it isn't one.

**Rationale**: Sigil already embraces this as a core design principle ("no magic"). Documented in design docs. Minor exceptions (ARC, operator-to-trait desugaring) are predictable and documented.

---

### 11. Optional Standard Library

**Decision**: Rejected

**Zig Feature**: Standard library is completely optional for bare-metal targets.

**Rationale**: Sigil isn't targeting bare-metal or embedded systems. The prelude types (`Option`, `Result`, etc.) are integral to the language design. No practical benefit for Sigil's application-level use cases.

---

### 12. First-Class C Interoperability

**Decision**: Rejected

**Zig Feature**: Direct C header imports, C ABI exports, drop-in compiler replacement.

**Rationale**: C interop introduces complexity (memory layouts, calling conventions, manual memory) that conflicts with Sigil's abstractions. FFI code is error-prone and not AI-friendly. Higher-level integration patterns (HTTP, IPC, subprocess) can serve integration needs.

---

### 13. Integrated Build System

**Decision**: Rejected

**Zig Feature**: Build system is part of the language with cross-compilation and package management.

**Rationale**: Sigil's simple CLI approach (`sigil run`, `sigil test`, `sigil fmt`) is sufficient. Complex build systems add configuration overhead that conflicts with AI-first simplicity. Package management can be addressed separately if needed.

---

### 14. Colorblind Async/Await

**Decision**: Rejected

**Zig Feature**: Functions aren't "colored" - same function works sync or async depending on context.

**Rationale**: Sigil deliberately chose explicit `uses Async` capability over colorblind async. The tradeoff of function coloring is accepted in exchange for clarity, explicitness, and easy testing via mock capability implementations.

---

### 15. anytype (Duck-Typed Generics)

**Decision**: Rejected

**Zig Feature**: `anytype` parameters accept any type with duck typing at compile time.

**Rationale**: Conflicts with Sigil's explicitness principle. Explicit trait bounds (`T: Trait`) make requirements visible in signatures, produce better error messages, and are more AI-friendly than duck typing that hides requirements in implementation.

---

### 16. inline for

**Decision**: Rejected

**Zig Feature**: Loop unrolling at compile time, can iterate over heterogeneous tuples.

**Rationale**: Coupled with comptime (already rejected). As a performance optimization, Sigil prefers letting the compiler handle unrolling automatically rather than exposing manual control. Doesn't fit the abstraction level.

---

### 17. Multiple Pointer Types

**Decision**: Rejected

**Zig Feature**: Different pointer types (`*T`, `[*]T`, `[]T`, `*[N]T`) for different use cases.

**Rationale**: Fundamentally incompatible with Sigil's ARC-based model. Sigil abstracts pointers away entirely, preventing entire classes of bugs (pointer arithmetic, dangling pointers, null dereferences). Simpler mental model for AI.

---

### 18. Error Return Traces

**Decision**: Proposal

**Zig Feature**: Stack traces for errors even in release builds, without unwinding overhead.

**Rationale**: Valuable for debugging without conflicting with Sigil's philosophy. Stack traces help diagnose where errors originate, benefiting both humans and AI in understanding failures. Manual error chaining via `source` is available but automatic traces would improve developer experience.

**Action**: Proposal created at `error-return-traces-proposal.md`

---

### 19. Runtime Safety Checks (Opt-Out)

**Decision**: Already Adopted (in principle)

**Zig Feature**: Bounds checking, overflow detection, null checks ON by default with explicit opt-out.

**Rationale**: Sigil already has safety by default (`Option` not null, bounds checking). No build modes means no opt-out mechanism - safety checks always on. More strict than Zig's opt-out model. Integer overflow behavior specified in separate proposal.

---

### 20. Order-Independent Top-Level Declarations

**Decision**: Adopted

**Zig Feature**: Top-level declarations can reference each other regardless of order.

**Rationale**: Reduces cognitive load for developers and AI. Enables natural code organization and mutual recursion without worrying about declaration order. Aligns with Sigil's AI-first goals.

---

## Sources

- [Zig Language Overview](https://ziglang.org/learn/overview/)
- [Why Zig over C++/D/Rust](https://ziglang.org/learn/why_zig_rust_d_cpp/)
- [Zig Documentation](https://ziglang.org/documentation/master/)
- [Colorblind Async/Await](https://kristoff.it/blog/zig-colorblind-async-await/)
- Sigil design documents in `docs/sigil_lang/0.1-alpha/design/`
