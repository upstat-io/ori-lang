---
section: 1
title: Type System Foundation
status: in-progress
tier: 1
goal: Fix type checking to properly use type annotations
spec:
  - spec/06-types.md
  - spec/07-properties-of-types.md
  - spec/08-declarations.md
sections:
  - id: "1.1"
    title: Primitive Types
    status: complete
  - id: "1.1A"
    title: Duration and Size Types
    status: complete
  - id: "1.1B"
    title: Never Type Semantics
    status: in-progress
  - id: "1.2"
    title: Parameter Type Annotations
    status: complete
  - id: "1.3"
    title: Lambda Type Annotations
    status: complete
  - id: "1.4"
    title: Let Binding Types
    status: complete
  - id: "1.5"
    title: Section Completion Checklist
    status: in-progress
  - id: "1.6"
    title: Low-Level Future-Proofing (Reserved Slots)
    status: in-progress
  - id: "1.7"
    title: Section Completion Checklist (Updated)
    status: in-progress
---

# Section 1: Type System Foundation

**Goal**: Fix type checking to properly use type annotations

> **SPEC**: `spec/06-types.md`, `spec/07-properties-of-types.md`, `spec/08-declarations.md`

**Status**: Core (1.1-1.4) verified complete 2026-02-10. 1.1A mostly complete (LLVM AOT tests missing). 1.1B core Never semantics complete (advanced features pending). 1.6 partially started (keywords reserved, type system slots not yet added).

**Known Bug**: `let` bindings directly in `@main` body crash (`type_interner.rs` index out of bounds). Workaround: wrap in `run()`. Does NOT affect spec tests or AOT tests which all use `run()`.

---

## 1.1 Primitive Types

- [x] **Implement**: `int` type — spec/06-types.md § int ✅ (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 0; type checker handles int
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — 162 tests (all pass)
  - [x] **LLVM Support**: `TypeInfo::Int` → i64 via `storage_type()` + `lower_int()` in `lower_literals.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 12 AOT tests using int

- [x] **Implement**: `float` type — spec/06-types.md § float ✅ (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 1; type checker handles float
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — float literal, negative, scientific, annotated, arithmetic, comparison tests
  - [x] **LLVM Support**: `TypeInfo::Float` → f64 via `storage_type()` + `lower_float()` in `lower_literals.rs`
  - [ ] **LLVM Rust Tests**: No AOT end-to-end tests for float (implementation exists)

- [x] **Implement**: `bool` type — spec/06-types.md § bool ✅ (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 2; type checker handles bool
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — bool literal, logic, short-circuit tests
  - [x] **LLVM Support**: `TypeInfo::Bool` → i1 via `storage_type()` + `lower_bool()` in `lower_literals.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 7 AOT tests using bool

- [x] **Implement**: `str` type — spec/06-types.md § str ✅ (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 3; type checker handles str
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — str literal, equality, concatenation tests
  - [x] **LLVM Support**: `TypeInfo::Str` → {i64 len, ptr data} via `storage_type()` + `lower_string()` in `lower_literals.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 1 AOT test (print_string)

- [x] **Implement**: `char` type — spec/06-types.md § char ✅ (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 4; type checker handles char
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — char literal, equality tests
  - [x] **LLVM Support**: `TypeInfo::Char` → i32 via `storage_type()` + `lower_char()` in `lower_literals.rs`
  - [ ] **LLVM Rust Tests**: No AOT end-to-end tests for char (implementation exists)

- [x] **Implement**: `byte` type — spec/06-types.md § byte ✅ (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 5; type checker handles byte
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — byte conversion, equality tests
  - [x] **LLVM Support**: `TypeInfo::Byte` → i8 via `storage_type()`
  - [ ] **LLVM Rust Tests**: No AOT end-to-end tests for byte (implementation exists)

- [x] **Implement**: `void` type — spec/06-types.md § void ✅ (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 6 (Unit); type checker handles void
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — void function return tests
  - [x] **LLVM Support**: `TypeInfo::Unit` → i64 via `storage_type()` + `lower_unit()` (LLVM void cannot be stored)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 5 AOT tests using void return

- [x] **Implement**: `Never` type — spec/06-types.md § Never ✅ (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 7; type checker handles Never
  - [x] **Ori Tests**: `tests/spec/types/never.ori` — 21 tests (all pass)
  - [x] **LLVM Support**: `TypeInfo::Never` → i64 via `storage_type()`
  - [ ] **LLVM Rust Tests**: No AOT end-to-end tests for Never (implementation exists)

**Note**: Also fixed parser bug where type keywords (`int`, `float`, etc.) couldn't be used as builtin conversion function calls.

---

## 1.1A Duration and Size Types

**Proposal**: `proposals/approved/duration-size-types-proposal.md`

Formalize Duration and Size primitive types with literal syntax, arithmetic, and conversion methods.

### Lexer

- [x] **Implement**: Duration literal tokenization with all units (ns, us, ms, s, m, h) ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — 10+ duration tests (units, decimal, many digits)
  - [x] **Ori Tests**: `tests/spec/lexical/duration_literals.ori` — 70+ tests
  - [x] **LLVM Support**: `lower_duration()` in `lower_literals.rs` — Duration → i64 (nanosecond precision)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 4 AOT tests (literals, negative, arithmetic, comparison) ✅ (2026-02-13)

- [x] **Implement**: Size literal tokenization with all units (b, kb, mb, gb, tb) ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — 5+ size tests
  - [x] **Ori Tests**: `tests/spec/lexical/size_literals.ori` — 70+ tests
  - [x] **LLVM Support**: `lower_size()` in `lower_literals.rs` — Size → i64 (bytes)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 3 AOT tests (literals, arithmetic, comparison) ✅ (2026-02-13)

- [x] **Implement**: Error for floating-point prefix on duration/size literals ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — float_duration/size error token tests
  - **Note**: Parse errors (E0911) cannot use `#[compile_fail]` which is for type errors only. Rust-level tests provide complete coverage.

### Type System

- [x] **Implement**: Duration type representation — spec/06-types.md § Duration ✅ (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 9; `TypeInfo::Duration`
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration type tests

- [x] **Implement**: Size type representation — spec/06-types.md § Size ✅ (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 10; `TypeInfo::Size`
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Size type tests

### Arithmetic Operations

- [x] **Implement**: Duration arithmetic (+, -, *, /, %, unary -) ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration arithmetic tests
  - [x] **Verified**: `1s + 500ms == 1500ms`, `2s * 3 == 6s`, `-(1s) == -1s` (via `ori parse`/`cargo st`)
  - [x] **LLVM Support**: Duration codegen exists (i64 arithmetic on nanosecond values)
  - [x] **LLVM Rust Tests**: Covered by `test_aot_duration_arithmetic` in `spec.rs` ✅ (2026-02-13)

- [x] **Implement**: Size arithmetic (+, -, *, /, %) ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Size arithmetic tests
  - [x] **Verified**: `1kb + 500b == 1500b`, `2kb * 3 == 6kb` (via `cargo st`)
  - [x] **LLVM Support**: Size codegen exists (i64 arithmetic on byte values)
  - [x] **LLVM Rust Tests**: Covered by `test_aot_size_arithmetic` in `spec.rs` ✅ (2026-02-13)

- [x] **Implement**: Compile error for unary negation on Size ✅ (2026-02-10)
  - [x] **Verified**: `-(1kb)` → E2001 "cannot negate `Size`: Size values must be non-negative"

- [x] **Implement**: Runtime panic for Duration overflow ✅ (2026-02-13)
  - [x] **Ori Tests**: `tests/spec/types/duration_overflow.ori` — 15 tests (8 #fail overflow/panic, 7 boundary/identity)
  - [x] **Verified**: Checked arithmetic in evaluator panics on add/sub/mul/div/mod/neg overflow, div-by-zero, mod-by-zero

- [x] **Implement**: Runtime panic for negative Size result ✅ (2026-02-13)
  - [x] **Ori Tests**: `tests/spec/types/size_overflow.ori` — 15 tests (9 #fail overflow/panic, 6 boundary/identity)
  - [x] **Verified**: Checked arithmetic panics on sub→negative, add overflow, mul overflow, mul/div by negative, div/mod by zero

### Conversion Methods

- [x] **Implement**: Duration extraction methods (.nanoseconds(), .microseconds(), etc.) ✅ (2026-02-10)
  - [x] **Verified**: `1s.nanoseconds() == 1000000000`, `1s.microseconds() == 1000000`, `1s.milliseconds() == 1000`, `1s.seconds() == 1`

- [x] **Implement**: Duration factory methods (Duration.from_seconds(), etc.) ✅ (2026-02-10)
  - [x] **Verified**: `Duration.from_seconds(5) == 5s`
  - **Note**: Associated function syntax implemented in Section 5.9

- [x] **Implement**: Size extraction methods (.bytes(), .kilobytes(), etc.) ✅ (2026-02-10)
  - [x] **Verified**: `1kb.bytes() == 1000`, `1mb.kilobytes() == 1000`

- [x] **Implement**: Size factory methods (Size.from_bytes(), etc.) ✅ (2026-02-10)
  - [x] **Verified**: `Size.from_bytes(1024) == 1024b`
  - **Note**: Associated function syntax implemented in Section 5.9

### Trait Implementations

- [x] **Implement**: Eq, Comparable for Duration ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_comparable.ori` — 16 tests (all pass)
  - [x] **Verified**: `1s > 500ms == true`

- [x] **Implement**: Eq, Comparable for Size ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_comparable.ori` — 16 tests (all pass)

- [x] **Implement**: Clone, Printable for Duration ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_clone_printable.ori` — 26 tests (all pass)

- [x] **Implement**: Clone, Printable for Size ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_clone_printable.ori` — 26 tests (all pass)

- [x] **Implement**: Hashable for Duration and Size ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_hashable.ori` — 13 tests (all pass)

- [x] **Implement**: Default for Duration and Size (0ns and 0b) ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_default.ori` — 10 tests (all pass)

- [x] **Implement**: Sendable for Duration and Size ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_sendable.ori` — 8 tests (all pass)

---

## 1.1B Never Type Semantics

**Proposal**: `proposals/approved/never-type-proposal.md`

Formalize the Never type as the bottom type with coercion rules, type inference behavior, and pattern matching exhaustiveness.

**Status**: Core complete (coercion and basic Never-producing expressions); advanced features pending

### Coercion

- [x] **Implement**: Never coerces to any type T in assignment contexts ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/never.ori` — 21 tests (all pass)
  - [ ] **LLVM Support**: LLVM codegen for Never coercion — not verified in AOT
  - [ ] **LLVM Rust Tests**: No AOT end-to-end tests for Never coercion

- [x] **Implement**: Never coerces in conditional branches ✅ (2026-02-10)
  - [x] **Verified**: `if true then 42 else panic(msg: "unreachable")` returns int correctly

- [x] **Implement**: Never coerces in match arms ✅ (2026-02-10)
  - [x] **Verified**: `match(Red, Red -> 42, Green -> panic(msg: "nope"), Blue -> panic(msg: "nope"))` returns int

### Expressions Producing Never

- [x] **Implement**: panic(msg:) returns Never ✅ (2026-02-10)
  - [x] **Verified**: `if true then 42 else panic(msg: "x")` type-checks as int (panic coerces to int)

- [x] **Implement**: todo() and todo(reason:) return Never ✅ (2026-02-10)
  - [x] **Verified**: `if true then 42 else todo()` type-checks as int

- [x] **Implement**: unreachable() and unreachable(reason:) return Never ✅ (2026-02-10)
  - [x] **Verified**: `if true then 42 else unreachable()` type-checks as int

### Pending (Future Work)

- [x] **Implement**: break/continue have type Never inside loops ✅ (2026-02-10)
  - [x] **Verified**: `loop(break 42)` returns int (break value used as loop result)
  - [ ] **LLVM Support**: Not verified in AOT

- [ ] **Implement**: Early-return path of ? operator has type Never
  - [ ] **Ori Tests**: `tests/spec/control_flow/never_propagation.ori` — not verified
  - [ ] **LLVM Support**: Not verified

- [ ] **Implement**: Infinite loop (no break) has type Never
  - [ ] **Ori Tests**: Not verified (hard to test without timeout)

- [ ] **Implement**: Never variants can be omitted from match exhaustiveness
  - [ ] Not verified

- [ ] **Implement**: Error E0920 for Never as struct field type
  - [ ] Not verified

- [ ] **Implement**: Allow Never in sum type variant payloads
  - [ ] Not verified

---

## 1.2 Parameter Type Annotations

- [x] **Implement**: Add `type_id_to_type()` helper function ✅ (2026-02-10)
  - [x] **Verified**: Type annotations on parameters work (e.g., `@add (a: int, b: int) -> int`)

- [x] **Implement**: Use `Param.ty` when present in `infer_function_signature()` ✅ (2026-02-10)
  - [x] **Verified**: `@greet (name: str) -> str = name` correctly infers str → str

- [x] **Implement**: Use declared return type when present ✅ (2026-02-10)
  - [x] **Verified**: `@typed_return () -> int = 42` correctly uses declared return type

- [x] **Implement**: Handle `TypeId::INFER` for unannotated parameters ✅ (2026-02-10)
  - [x] **Verified**: `@infer_param (x) -> int = x` correctly infers x: int from context

---

## 1.3 Lambda Type Annotations

- [x] **Implement**: Typed lambda parameters `(x: int) -> x + 1` ✅ (2026-02-10)
  - [x] **Verified**: `let f = (x: int) -> x + 1, f(41)` returns 42

- [x] **Implement**: Explicit return type `(x: int) -> int = x + 1` ✅ (2026-02-10)
  - [x] **Verified**: `let f = (x: int) -> int = x * 2, f(21)` returns 42

---

## 1.4 Let Binding Types

- [x] **Implement**: Type annotation in `let x: int = ...` ✅ (2026-02-10)
  - [x] **Verified**: `let x: int = 42`, `let x: float = 3.14` work correctly (inside `run()`)
  - [ ] **Bug**: `let x: int = 42` directly in `@main` body crashes (type_interner index OOB) — must use `run()` wrapper

---

## 1.6 Low-Level Future-Proofing (Reserved Slots)

**Proposal**: `proposals/approved/low-level-future-proofing-proposal.md`

Reserve architectural space in the type system for future low-level features (inline types, borrowed views). No user-visible changes — only internal structure.

### Type System Slots

- [ ] **Implement**: Add `LifetimeId` type to `ori_types`
  - [ ] `LifetimeId(u32)` newtype with `STATIC` constant only — **NOT IMPLEMENTED**

- [ ] **Implement**: Add `ValueCategory` enum to `ori_types`
  - [ ] `Boxed` (default), `Inline` (reserved), `View` (reserved) — **NOT IMPLEMENTED**

- [ ] **Implement**: Add `#[doc(hidden)]` `Borrowed` variant to `Type` enum
  - [ ] `Borrowed { inner: Box<Type>, lifetime: LifetimeId }` — **NOT IMPLEMENTED**

- [ ] **Implement**: Add `category` field to `TypeData::Struct` variant (if exists)
  - [ ] Default to `ValueCategory::Boxed` — **NOT IMPLEMENTED**

### Syntax Reservation

- [x] **Implement**: Add `inline` as reserved keyword in lexer ✅ (2026-02-10)
  - [x] Recognized in `ori_lexer/src/keywords.rs` (reserved-future list)
  - [ ] Does NOT produce helpful error — currently usable as identifier name

- [x] **Implement**: Add `view` as reserved keyword in lexer ✅ (2026-02-10)
  - [x] Recognized in `ori_lexer/src/keywords.rs` (reserved-future list)
  - [ ] Does NOT produce helpful error — currently usable as identifier name

- [ ] **Implement**: Reserve `&` in type position
  - [ ] Parser rejects `&T` with generic error ("expected ,, found &"), not helpful message
  - [ ] **Partially done**: `&` is rejected in type position but error message not user-friendly

- [ ] **Implement**: Parser rejects reserved keywords with helpful errors
  - [ ] Keywords recognized but NO helpful error messages produced
  - [ ] `inline` and `view` are usable as variable names (no rejection)

---

## 1.7 Section Completion Checklist

- [x] 1.1 Primitive types complete — all 8 types verified in type checker + evaluator + LLVM codegen ✅ (2026-02-10)
- [x] 1.1A Duration/Size complete — lexer, type system, arithmetic, conversions, all 7 traits ✅ (2026-02-10)
- [x] 1.1B Never core complete — coercion in conditionals, match arms; panic/todo/unreachable return Never ✅ (2026-02-10)
- [x] 1.2 Parameter type annotations complete ✅ (2026-02-10)
- [x] 1.3 Lambda type annotations complete ✅ (2026-02-10)
- [x] 1.4 Let binding types complete ✅ (2026-02-10)
- [ ] 1.6 Low-level future-proofing — keywords reserved; type system slots NOT implemented
- [ ] LLVM AOT tests incomplete — float, char, byte, Never lack end-to-end AOT tests (Duration and Size covered ✅ 2026-02-13)
- [ ] `@main` let binding bug — `let` directly in `@main` crashes (workaround: use `run()`)

**Remaining gaps:**
- 1.1A: Duration overflow panic, negative Size panic — not verified
- 1.1B: ? operator Never, infinite loop Never, exhaustiveness, E0920, sum variant — not verified
- 1.6: LifetimeId, ValueCategory, Borrowed variant, helpful keyword rejection errors — not implemented
- LLVM: 5 types lack AOT end-to-end tests (float, char, byte, Duration, Size)
