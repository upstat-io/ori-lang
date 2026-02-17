---
section: 1
title: Type System Foundation
status: complete
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
    status: complete
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
    status: complete
  - id: "1.6"
    title: Low-Level Future-Proofing (Reserved Slots)
    status: complete
  - id: "1.7"
    title: Section Completion Checklist (Updated)
    status: complete
---

# Section 1: Type System Foundation

**Goal**: Fix type checking to properly use type annotations

> **SPEC**: `spec/06-types.md`, `spec/07-properties-of-types.md`, `spec/08-declarations.md`

**Status**: **COMPLETE** [done] (2026-02-13). Core (1.1-1.4) verified 2026-02-10. 1.1 all LLVM AOT tests 2026-02-13. 1.1A constant folding 2026-02-13. 1.1B `?` LLVM support 2026-02-13. 1.6 type system slots (LifetimeId, ValueCategory, Borrowed tag, StructDef category) + parser `&T` error + keyword rejection verified 2026-02-13.

**Known Bug (RESOLVED)**: `let` bindings directly in `@main` body previously crashed (`type_interner.rs` index out of bounds). Fixed as of 2026-02-13 — `type_interner.rs` was removed during hygiene refactors. 6 regression tests added.

---

## 1.1 Primitive Types

- [x] **Implement**: `int` type — spec/06-types.md § int [done] (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 0; type checker handles int
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — 162 tests (all pass)
  - [x] **LLVM Support**: `TypeInfo::Int` → i64 via `storage_type()` + `lower_int()` in `lower_literals.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 12 AOT tests using int

- [x] **Implement**: `float` type — spec/06-types.md § float [done] (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 1; type checker handles float
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — float literal, negative, scientific, annotated, arithmetic, comparison tests
  - [x] **LLVM Support**: `TypeInfo::Float` → f64 via `storage_type()` + `lower_float()` in `lower_literals.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 4 AOT tests (literals, arithmetic, comparison, negation) [done] (2026-02-13)

- [x] **Implement**: `bool` type — spec/06-types.md § bool [done] (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 2; type checker handles bool
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — bool literal, logic, short-circuit tests
  - [x] **LLVM Support**: `TypeInfo::Bool` → i1 via `storage_type()` + `lower_bool()` in `lower_literals.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 7 AOT tests using bool

- [x] **Implement**: `str` type — spec/06-types.md § str [done] (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 3; type checker handles str
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — str literal, equality, concatenation tests
  - [x] **LLVM Support**: `TypeInfo::Str` → {i64 len, ptr data} via `storage_type()` + `lower_string()` in `lower_literals.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 1 AOT test (print_string)

- [x] **Implement**: `char` type — spec/06-types.md § char [done] (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 4; type checker handles char
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — char literal, equality tests
  - [x] **LLVM Support**: `TypeInfo::Char` → i32 via `storage_type()` + `lower_char()` in `lower_literals.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 2 AOT tests (literals, comparison) [done] (2026-02-13)

- [x] **Implement**: `byte` type — spec/06-types.md § byte [done] (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 5; type checker handles byte
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — byte conversion, equality tests
  - [x] **LLVM Support**: `TypeInfo::Byte` → i8 via `storage_type()`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 1 AOT test (basics, equality, boundary values); fixed byte codegen bug (i64→i8 store mismatch) [done] (2026-02-13)

- [x] **Implement**: `void` type — spec/06-types.md § void [done] (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 6 (Unit); type checker handles void
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — void function return tests
  - [x] **LLVM Support**: `TypeInfo::Unit` → i64 via `storage_type()` + `lower_unit()` (LLVM void cannot be stored)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 5 AOT tests using void return

- [x] **Implement**: `Never` type — spec/06-types.md § Never [done] (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 7; type checker handles Never
  - [x] **Ori Tests**: `tests/spec/types/never.ori` — 21 tests (all pass)
  - [x] **LLVM Support**: `TypeInfo::Never` → i64 via `storage_type()`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 2 AOT tests (panic coercion, multi-type conditional branches) [done] (2026-02-13)

**Note**: Also fixed parser bug where type keywords (`int`, `float`, etc.) couldn't be used as builtin conversion function calls.

---

## 1.1A Duration and Size Types

**Proposal**: `proposals/approved/duration-size-types-proposal.md`

Formalize Duration and Size primitive types with literal syntax, arithmetic, and conversion methods.

### Lexer

- [x] **Implement**: Duration literal tokenization with all units (ns, us, ms, s, m, h) [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — 10+ duration tests (units, decimal, many digits)
  - [x] **Ori Tests**: `tests/spec/lexical/duration_literals.ori` — 70+ tests
  - [x] **LLVM Support**: `lower_duration()` in `lower_literals.rs` — Duration → i64 (nanosecond precision)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 4 AOT tests (literals, negative, arithmetic, comparison) [done] (2026-02-13)

- [x] **Implement**: Size literal tokenization with all units (b, kb, mb, gb, tb) [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — 5+ size tests
  - [x] **Ori Tests**: `tests/spec/lexical/size_literals.ori` — 70+ tests
  - [x] **LLVM Support**: `lower_size()` in `lower_literals.rs` — Size → i64 (bytes)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 3 AOT tests (literals, arithmetic, comparison) [done] (2026-02-13)

- [x] **Implement**: Error for floating-point prefix on duration/size literals [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — float_duration/size error token tests
  - **Note**: Parse errors (E0911) cannot use `#[compile_fail]` which is for type errors only. Rust-level tests provide complete coverage.

### Type System

- [x] **Implement**: Duration type representation — spec/06-types.md § Duration [done] (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 9; `TypeInfo::Duration`
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration type tests

- [x] **Implement**: Size type representation — spec/06-types.md § Size [done] (2026-02-10)
  - [x] **Rust Tests**: Type pool pre-interned at index 10; `TypeInfo::Size`
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Size type tests

### Arithmetic Operations

- [x] **Implement**: Duration arithmetic (+, -, *, /, %, unary -) [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration arithmetic tests
  - [x] **Verified**: `1s + 500ms == 1500ms`, `2s * 3 == 6s`, `-(1s) == -1s` (via `ori parse`/`cargo st`)
  - [x] **LLVM Support**: Duration codegen exists (i64 arithmetic on nanosecond values)
  - [x] **LLVM Rust Tests**: Covered by `test_aot_duration_arithmetic` in `spec.rs` [done] (2026-02-13)

- [x] **Implement**: Size arithmetic (+, -, *, /, %) [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Size arithmetic tests
  - [x] **Verified**: `1kb + 500b == 1500b`, `2kb * 3 == 6kb` (via `cargo st`)
  - [x] **LLVM Support**: Size codegen exists (i64 arithmetic on byte values)
  - [x] **LLVM Rust Tests**: Covered by `test_aot_size_arithmetic` in `spec.rs` [done] (2026-02-13)

- [x] **Implement**: Compile error for unary negation on Size [done] (2026-02-10)
  - [x] **Verified**: `-(1kb)` → E2001 "cannot negate `Size`: Size values must be non-negative"

- [x] **Implement**: Runtime panic for Duration overflow [done] (2026-02-13)
  - [x] **Ori Tests**: `tests/spec/types/duration_overflow.ori` — 15 tests (8 #fail overflow/panic, 7 boundary/identity)
  - [x] **Verified**: Checked arithmetic in evaluator panics on add/sub/mul/div/mod/neg overflow, div-by-zero, mod-by-zero

- [x] **Implement**: Runtime panic for negative Size result [done] (2026-02-13)
  - [x] **Ori Tests**: `tests/spec/types/size_overflow.ori` — 15 tests (9 #fail overflow/panic, 6 boundary/identity)
  - [x] **Verified**: Checked arithmetic panics on sub→negative, add overflow, mul overflow, mul/div by negative, div/mod by zero

### Conversion Methods

- [x] **Implement**: Duration extraction methods (.nanoseconds(), .microseconds(), etc.) [done] (2026-02-10)
  - [x] **Verified**: `1s.nanoseconds() == 1000000000`, `1s.microseconds() == 1000000`, `1s.milliseconds() == 1000`, `1s.seconds() == 1`

- [x] **Implement**: Duration factory methods (Duration.from_seconds(), etc.) [done] (2026-02-10)
  - [x] **Verified**: `Duration.from_seconds(5) == 5s`
  - **Note**: Associated function syntax implemented in Section 5.9

- [x] **Implement**: Size extraction methods (.bytes(), .kilobytes(), etc.) [done] (2026-02-10)
  - [x] **Verified**: `1kb.bytes() == 1000`, `1mb.kilobytes() == 1000`

- [x] **Implement**: Size factory methods (Size.from_bytes(), etc.) [done] (2026-02-10)
  - [x] **Verified**: `Size.from_bytes(1024) == 1024b`
  - **Note**: Associated function syntax implemented in Section 5.9

### Trait Implementations

- [x] **Implement**: Eq, Comparable for Duration [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_comparable.ori` — 16 tests (all pass)
  - [x] **Verified**: `1s > 500ms == true`

- [x] **Implement**: Eq, Comparable for Size [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_comparable.ori` — 16 tests (all pass)

- [x] **Implement**: Clone, Printable for Duration [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_clone_printable.ori` — 26 tests (all pass)

- [x] **Implement**: Clone, Printable for Size [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_clone_printable.ori` — 26 tests (all pass)

- [x] **Implement**: Hashable for Duration and Size [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_hashable.ori` — 13 tests (all pass)

- [x] **Implement**: Default for Duration and Size (0ns and 0b) [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_default.ori` — 10 tests (all pass)

- [x] **Implement**: Sendable for Duration and Size [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/duration_size_sendable.ori` — 8 tests (all pass)

### Constant Folding

- [x] **Implement**: Duration/Size constant folding in `ori_canon` [done] (2026-02-13)
  - Added `extract_const_value()` arms for `CanExpr::Duration`/`CanExpr::Size` → `ConstValue`
  - Added `fold_binary()` rules: Duration±Duration, Size±Size, Duration*int, Size*int, Duration/int, Size/int, mod, all comparisons
  - Added `fold_unary()` rule: Duration negation (Size negation correctly rejected)
  - [x] **Rust Tests**: 14 unit tests in `ori_canon/src/const_fold.rs` — addition, subtraction, comparison, cross-unit equality, negation, mul/div with int, overflow/negative rejection
  - [x] **Ori Tests**: `tests/spec/types/duration_size_const.ori` — 18 tests covering constant Duration/Size in let bindings, cross-unit arithmetic, comparisons, mixed int operations
  - [x] **LLVM Support**: Already handled — `lower_constant()` dispatches to `lower_duration()`/`lower_size()` with unit conversion

---

## 1.1B Never Type Semantics

**Proposal**: `proposals/approved/never-type-proposal.md`

Formalize the Never type as the bottom type with coercion rules, type inference behavior, and pattern matching exhaustiveness.

**Status**: All Never type features implemented and verified [done] (2026-02-13). Result type layout bug fixed — `TypeInfoStore` resolves type variables via `Pool::resolve_fully()`; `Result::unwrap()` coerces payload to ok type.

### Coercion

- [x] **Implement**: Never coerces to any type T in assignment contexts [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/never.ori` — 21 tests (all pass)
  - [x] **LLVM Support**: Never coercion works in AOT — conditional branches with panic() produce correct values [done] (2026-02-13)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/spec.rs` — 2 AOT tests (panic coercion, multi-type conditional branches) [done] (2026-02-13)

- [x] **Implement**: Never coerces in conditional branches [done] (2026-02-10)
  - [x] **Verified**: `if true then 42 else panic(msg: "unreachable")` returns int correctly

- [x] **Implement**: Never coerces in match arms [done] (2026-02-10)
  - [x] **Verified**: `match(Red, Red -> 42, Green -> panic(msg: "nope"), Blue -> panic(msg: "nope"))` returns int

### Expressions Producing Never

- [x] **Implement**: panic(msg:) returns Never [done] (2026-02-10)
  - [x] **Verified**: `if true then 42 else panic(msg: "x")` type-checks as int (panic coerces to int)

- [x] **Implement**: todo() and todo(reason:) return Never [done] (2026-02-10)
  - [x] **Verified**: `if true then 42 else todo()` type-checks as int

- [x] **Implement**: unreachable() and unreachable(reason:) return Never [done] (2026-02-10)
  - [x] **Verified**: `if true then 42 else unreachable()` type-checks as int

### Pending (Future Work)

- [x] **Implement**: break/continue have type Never inside loops [done] (2026-02-10)
  - [x] **Verified**: `loop(break 42)` returns int (break value used as loop result)
  - [x] **LLVM Support**: Verified in AOT [done] (2026-02-13) — 5 tests: basic break value, conditional break, break Never coercion, continue Never coercion, break+continue combined

- [x] **Implement**: Early-return path of ? operator has type Never [done] (2026-02-13)
  - [x] **Bug Fix**: `ControlAction::Propagate` was not caught at function call boundaries — `?` errors leaked through all call frames instead of becoming the function's return value. Fixed in `function_call.rs` with `catch_propagation()`.
  - [x] **Ori Tests**: `tests/spec/control_flow/never_propagation.ori` — 14 tests (Result and Option propagation, chaining, nested calls, conditional branches, multiple ? in same expression)
  - [x] **LLVM Support**: Fixed — `TypeInfoStore` now follows `Pool::resolve_fully()` for `Tag::Var` type variables; `Result::unwrap()` coerces payload to ok type [done] (2026-02-13)
  - [x] **LLVM Rust Tests**: 11 AOT tests in `ori_llvm/tests/aot/spec.rs` — Result/Option constructors, `?` operator (ok unwrap, err propagation, chained), method dispatch (is_ok/is_err/unwrap)

- [x] **Implement**: Infinite loop (no break) has type Never [done] (2026-02-13)
  - [x] **Fix**: `infer_loop()` returned `Idx::UNIT` for unresolved break type; now returns `Idx::NEVER`. A `break` (even without value) unifies with Unit, so unresolved truly means no break exists.
  - [x] **Rust Tests**: Updated `test_infer_infinite_loop` to assert `Idx::NEVER`
  - [x] **Verified**: `@diverge () -> int = loop(())` type-checks — Never coerces to int

- [x] **Implement**: Never variants can be omitted from match exhaustiveness [done] (2026-02-13)
  - [x] **Verified**: `type MaybeNever = Value(v: int) | Impossible(n: Never)` — match omitting Impossible passes
  - [x] Added `is_variant_uninhabited()` in `ori_canon/src/exhaustiveness.rs`
  - [x] 3 unit tests + 2 Ori spec tests in `tests/spec/patterns/exhaustiveness.ori`

- [x] **Implement**: Error E2019 for Never as struct field type [done] (2026-02-13)
  - [x] Added `UninhabitedStructField` variant to `TypeErrorKind`
  - [x] Check in `registration.rs` during struct type registration
  - [x] Compile-fail test: `tests/compile-fail/never_struct_field.ori`
  - [x] Integration tests: `never_struct_field_rejected`, `never_in_sum_variant_allowed`

- [x] **Verify**: Allow Never in sum type variant payloads [done] (2026-02-13)
  - [x] Already works — `type MaybeNever = Value(v: int) | Impossible(n: Never)` compiles
  - [x] Exhaustiveness checker correctly treats Never variants as uninhabited
  - [x] Integration test: `never_in_sum_variant_allowed`

---

## 1.2 Parameter Type Annotations

- [x] **Implement**: Add `type_id_to_type()` helper function [done] (2026-02-10)
  - [x] **Verified**: Type annotations on parameters work (e.g., `@add (a: int, b: int) -> int`)

- [x] **Implement**: Use `Param.ty` when present in `infer_function_signature()` [done] (2026-02-10)
  - [x] **Verified**: `@greet (name: str) -> str = name` correctly infers str → str

- [x] **Implement**: Use declared return type when present [done] (2026-02-10)
  - [x] **Verified**: `@typed_return () -> int = 42` correctly uses declared return type

- [x] **Implement**: Handle `TypeId::INFER` for unannotated parameters [done] (2026-02-10)
  - [x] **Verified**: `@infer_param (x) -> int = x` correctly infers x: int from context

---

## 1.3 Lambda Type Annotations

- [x] **Implement**: Typed lambda parameters `(x: int) -> x + 1` [done] (2026-02-10)
  - [x] **Verified**: `let f = (x: int) -> x + 1, f(41)` returns 42

- [x] **Implement**: Explicit return type `(x: int) -> int = x + 1` [done] (2026-02-10)
  - [x] **Verified**: `let f = (x: int) -> int = x * 2, f(21)` returns 42

---

## 1.4 Let Binding Types

- [x] **Implement**: Type annotation in `let x: int = ...` [done] (2026-02-10)
  - [x] **Verified**: `let x: int = 42`, `let x: float = 3.14` work correctly (inside `run()`)
  - [x] **Bug fixed**: `let x: int = 42` directly in `@main` body no longer crashes [done] (2026-02-13) — `type_interner.rs` removed during hygiene refactors; 6 regression tests added in `oric/tests/phases/common/typecheck.rs`

---

## 1.6 Low-Level Future-Proofing (Reserved Slots)

**Proposal**: `proposals/approved/low-level-future-proofing-proposal.md`

Reserve architectural space in the type system for future low-level features (inline types, borrowed views). No user-visible changes — only internal structure.

### Type System Slots

- [x] **Implement**: Add `LifetimeId` type to `ori_types` [done] (2026-02-13)
  - [x] `LifetimeId(u32)` newtype with `STATIC` and `SCOPED` constants in `ori_types/src/lifetime.rs`
  - [x] 7 unit tests (roundtrip, display, equality, hash, size assertion)
  - [x] Salsa compatibility assertion in `lib.rs`

- [x] **Implement**: Add `ValueCategory` enum to `ori_types` [done] (2026-02-13)
  - [x] `Boxed` (default), `Inline` (reserved), `View` (reserved) in `ori_types/src/value_category.rs`
  - [x] 5 unit tests (default, predicates, display, size, hash)
  - [x] Salsa compatibility assertion in `lib.rs`

- [x] **Implement**: Add `Borrowed` variant to `Tag` enum [done] (2026-02-13)
  - [x] `Tag::Borrowed = 34` in two-child containers range, extra layout: `[inner_idx, lifetime_id]`
  - [x] Updated all exhaustive matches: `tag.rs`, `pool/mod.rs`, `pool/format.rs`, `type_error/diff.rs`, `ori_arc/classify.rs`, `ori_llvm/type_info.rs`
  - [x] Note: `#[doc(hidden)]` removed per clippy — variant is internal to compiler, fully visible

- [x] **Implement**: Add `category` field to `StructDef` [done] (2026-02-13)
  - [x] `category: ValueCategory` field on `StructDef`, default to `ValueCategory::Boxed`
  - [x] Updated all 4 construction sites: `registry/types.rs`, `output/mod.rs`, `ori_llvm/type_registration.rs` (×2)

### Syntax Reservation

- [x] **Implement**: Add `inline` as reserved keyword in lexer [done] (2026-02-10)
  - [x] Recognized in `ori_lexer/src/keywords.rs` (reserved-future list)
  - [x] Produces E0015 error: "`inline` is reserved for future use" [done] (verified 2026-02-13)

- [x] **Implement**: Add `view` as reserved keyword in lexer [done] (2026-02-10)
  - [x] Recognized in `ori_lexer/src/keywords.rs` (reserved-future list)
  - [x] Produces E0015 error: "`view` is reserved for future use" [done] (verified 2026-02-13)

- [x] **Implement**: Reserve `&` in type position [done] (2026-02-13)
  - [x] Parser detects `&` in `parse_type()` and produces E1001: "borrowed references (`&T`) are reserved for a future version of Ori"
  - [x] Recovers by parsing inner type, enabling continued parsing
  - [x] 3 parser tests: `&int`, `&MyType`, `&` alone

- [x] **Implement**: Parser rejects reserved keywords with helpful errors [done] (verified 2026-02-13)
  - [x] Lexer cooker produces `LexError::ReservedFutureKeyword` with E0015 for all 5 reserved-future keywords (`asm`, `inline`, `static`, `union`, `view`)
  - [x] Token still interned as `Ident` for parse recovery; error reported to user

---

## 1.7 Section Completion Checklist

- [x] 1.1 Primitive types complete — all 8 types verified in type checker + evaluator + LLVM codegen [done] (2026-02-10)
- [x] 1.1A Duration/Size complete — lexer, type system, arithmetic, conversions, all 7 traits [done] (2026-02-10)
- [x] 1.1B Never type fully implemented [done] (2026-02-13) — infinite loop→Never, `?` propagation fix, exhaustiveness for Never variants, E2019, sum variant payloads, `?` LLVM support (fixed type variable leak + Result::unwrap coercion)
- [x] 1.2 Parameter type annotations complete [done] (2026-02-10)
- [x] 1.3 Lambda type annotations complete [done] (2026-02-10)
- [x] 1.4 Let binding types complete [done] (2026-02-10)
- [x] 1.6 Low-level future-proofing complete [done] (2026-02-13) — LifetimeId, ValueCategory, Borrowed tag, StructDef category field, `&T` parser error, keyword rejection verified
- [x] LLVM AOT tests complete — all 8 primitive types have AOT tests [done] (2026-02-13); fixed byte codegen bug (i64→i8 store mismatch causing segfault)
- [x] Loop/break/continue AOT tests — 5 tests verifying Never coercion in loops [done] (2026-02-13)
- [x] `@main` let binding bug fixed [done] (2026-02-13) — `type_interner.rs` removed during hygiene refactors; 6 regression tests added

**Section 1 complete.** All subsections (1.1–1.4, 1.6) implemented and verified.
