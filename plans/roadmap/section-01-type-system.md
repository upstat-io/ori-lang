---
section: 1
title: Type System Foundation
status: not-started
tier: 1
goal: Fix type checking to properly use type annotations
spec:
  - spec/06-types.md
  - spec/07-properties-of-types.md
  - spec/08-declarations.md
sections:
  - id: "1.1"
    title: Primitive Types
    status: not-started
  - id: "1.1A"
    title: Duration and Size Types
    status: not-started
  - id: "1.1B"
    title: Never Type Semantics
    status: not-started
  - id: "1.2"
    title: Parameter Type Annotations
    status: not-started
  - id: "1.3"
    title: Lambda Type Annotations
    status: not-started
  - id: "1.4"
    title: Let Binding Types
    status: not-started
  - id: "1.5"
    title: Section Completion Checklist
    status: not-started
  - id: "1.6"
    title: Low-Level Future-Proofing (Reserved Slots)
    status: not-started
  - id: "1.7"
    title: Section Completion Checklist (Updated)
    status: not-started
---

# Section 1: Type System Foundation

**Goal**: Fix type checking to properly use type annotations

> **SPEC**: `spec/06-types.md`, `spec/07-properties-of-types.md`, `spec/08-declarations.md`

**Status**: Complete — Core (1.1-1.5) complete, 1.1A trait implementations complete with Ori tests, 1.1B core Never semantics complete (advanced features pending)

---

## 1.1 Primitive Types

- [ ] **Implement**: `int` type — spec/06-types.md § int
  - [ ] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for int type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/primitive_tests.rs` — int type codegen

- [ ] **Implement**: `float` type — spec/06-types.md § float
  - [ ] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for float type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/primitive_tests.rs` — float type codegen

- [ ] **Implement**: `bool` type — spec/06-types.md § bool
  - [ ] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for bool type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/primitive_tests.rs` — bool type codegen

- [ ] **Implement**: `str` type — spec/06-types.md § str
  - [ ] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for str type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/primitive_tests.rs` — str type codegen

- [ ] **Implement**: `char` type — spec/06-types.md § char
  - [ ] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for char type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/primitive_tests.rs` — char type codegen

- [ ] **Implement**: `byte` type — spec/06-types.md § byte
  - [ ] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for byte type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/primitive_tests.rs` — byte type codegen

- [ ] **Implement**: `void` type — spec/06-types.md § void
  - [ ] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for void type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/primitive_tests.rs` — void type codegen

- [ ] **Implement**: `Never` type — spec/06-types.md § Never
  - [ ] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for Never type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/primitive_tests.rs` — Never type codegen

**Note**: Also fixed parser bug where type keywords (`int`, `float`, etc.) couldn't be used as builtin conversion function calls. See `parser/mod.rs:1007-1042`.

> **Update Plan Status, and Pause**

---

## 1.1A Duration and Size Types

**Proposal**: `proposals/approved/duration-size-types-proposal.md`

Formalize Duration and Size primitive types with literal syntax, arithmetic, and conversion methods.

### Lexer

- [ ] **Implement**: Duration literal tokenization with all units (ns, us, ms, s, m, h)
  - [ ] **Rust Tests**: `ori_ir/src/token.rs` — DurationUnit enum with Nanoseconds, Microseconds
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration literal tests
  - [ ] **LLVM Support**: LLVM codegen for Duration literals (nanosecond precision)
  - [ ] **LLVM Rust Tests**: `ori_llvm/src/tests/arithmetic_tests.rs`

- [ ] **Implement**: Size literal tokenization with all units (b, kb, mb, gb, tb)
  - [ ] **Rust Tests**: `ori_ir/src/token.rs` — SizeUnit enum with Terabytes
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori` — Size literal tests
  - [ ] **LLVM Support**: LLVM codegen for Size literals
  - [ ] **LLVM Rust Tests**: `ori_llvm/src/tests/arithmetic_tests.rs`

- [ ] **Implement**: Error for floating-point prefix on duration/size literals
  - [ ] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — float_duration/size error token tests
  - **Note**: Parse errors (E0911) cannot use `#[compile_fail]` which is for type errors only. Rust-level tests provide complete coverage.

### Type System

- [ ] **Implement**: Duration type representation — spec/06-types.md § Duration
  - [ ] **Rust Tests**: `ori_types/src/core.rs` — Type::Duration
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration type tests

- [ ] **Implement**: Size type representation — spec/06-types.md § Size
  - [ ] **Rust Tests**: `ori_types/src/core.rs` — Type::Size
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori` — Size type tests

### Arithmetic Operations

- [ ] **Implement**: Duration arithmetic (+, -, *, /, %, unary -)
  - [ ] **Rust Tests**: `ori_eval/src/operators.rs` — Duration binary ops
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration arithmetic tests
  - [ ] **LLVM Support**: LLVM codegen for Duration arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/duration_tests.rs`

- [ ] **Implement**: Size arithmetic (+, -, *, /, %)
  - [ ] **Rust Tests**: `ori_eval/src/operators.rs` — Size binary ops
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori` — Size arithmetic tests
  - [ ] **LLVM Support**: LLVM codegen for Size arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/size_tests.rs`

- [ ] **Implement**: Compile error for unary negation on Size
  - [ ] **Rust Tests**: `ori_typeck/src/infer/expressions/operators.rs` — Size negation check
  - [ ] **Ori Tests**: `tests/compile-fail/size_unary_negation.ori`

- [ ] **Implement**: Runtime panic for Duration overflow
  - [ ] **Ori Tests**: Built into checked arithmetic (panics on overflow)

- [ ] **Implement**: Runtime panic for negative Size result
  - [ ] **Ori Tests**: Built into Size subtraction (panics on negative)

### Conversion Methods

- [ ] **Implement**: Duration extraction methods (.nanoseconds(), .microseconds(), etc.)
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration extraction method tests

- [ ] **Implement**: Duration factory methods (Duration.from_seconds(), etc.)
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration factory method tests
  - **Note**: Associated function syntax implemented in Section 5.9

- [ ] **Implement**: Size extraction methods (.bytes(), .kilobytes(), etc.)
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori` — Size extraction method tests

- [ ] **Implement**: Size factory methods (Size.from_bytes(), etc.)
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori` — Size factory method tests
  - **Note**: Associated function syntax implemented in Section 5.9

### Trait Implementations

- [ ] **Implement**: Eq, Comparable for Duration
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration comparison operators
  - [ ] **Ori Tests**: `tests/spec/types/duration_size_comparable.ori` — Duration compare() method

- [ ] **Implement**: Eq, Comparable for Size
  - [ ] **Ori Tests**: `tests/spec/types/primitives.ori` — Size comparison operators
  - [ ] **Ori Tests**: `tests/spec/types/duration_size_comparable.ori` — Size compare() method

- [ ] **Implement**: Clone, Printable for Duration
  - [ ] **Ori Tests**: `tests/spec/types/duration_size_clone_printable.ori` — Duration clone/to_str tests

- [ ] **Implement**: Clone, Printable for Size
  - [ ] **Ori Tests**: `tests/spec/types/duration_size_clone_printable.ori` — Size clone/to_str tests

- [ ] **Implement**: Hashable for Duration and Size
  - [ ] **Rust Implementation**: `ori_eval/src/methods.rs` — hash method
  - [ ] **Bound Checking**: `ori_typeck/src/checker/bound_checking.rs` — Hashable trait
  - [ ] **Ori Tests**: `tests/spec/types/duration_size_hashable.ori`

- [ ] **Implement**: Default for Duration and Size (0ns and 0b)
  - [ ] **Rust Implementation**: `ori_eval/src/methods.rs` — Duration.default(), Size.default()
  - [ ] **Type Checking**: `ori_typeck/src/infer/builtin_methods/units.rs` — default associated function
  - [ ] **Bound Checking**: `ori_typeck/src/checker/bound_checking.rs` — Default trait
  - [ ] **Ori Tests**: `tests/spec/types/duration_size_default.ori`

- [ ] **Implement**: Sendable for Duration and Size
  - [ ] **Bound Checking**: `ori_typeck/src/checker/bound_checking.rs` — Sendable trait
  - [ ] **Ori Tests**: `tests/spec/types/duration_size_sendable.ori`

---

## 1.1B Never Type Semantics

**Proposal**: `proposals/approved/never-type-proposal.md`

Formalize the Never type as the bottom type with coercion rules, type inference behavior, and pattern matching exhaustiveness.

**Status**: Core complete (coercion and basic Never-producing expressions); advanced features pending

### Coercion

- [ ] **Implement**: Never coerces to any type T in assignment contexts
  - [ ] **Rust Tests**: `ori_types/src/context.rs` — Never unification tests
  - [ ] **Ori Tests**: `tests/spec/types/never.ori`
  - [ ] **LLVM Support**: LLVM codegen for Never coercion in assignment contexts
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/never_tests.rs` — Never assignment coercion codegen

- [ ] **Implement**: Never coerces in conditional branches
  - [ ] **Ori Tests**: `tests/spec/types/never.ori`
  - [ ] **LLVM Support**: LLVM codegen for Never coercion in conditional branches
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/never_tests.rs` — Never conditional coercion codegen

- [ ] **Implement**: Never coerces in match arms
  - [ ] **Ori Tests**: `tests/spec/types/never.ori`
  - [ ] **LLVM Support**: LLVM codegen for Never coercion in match arms
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/never_tests.rs` — Never match arm coercion codegen

### Expressions Producing Never

- [ ] **Implement**: panic(msg:) returns Never
  - [ ] **Ori Tests**: `tests/spec/types/never.ori`
  - [ ] **LLVM Support**: LLVM codegen for panic(msg:) returning Never
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/never_tests.rs` — panic Never codegen

- [ ] **Implement**: todo() and todo(reason:) return Never
  - [ ] **Rust Tests**: `ori_patterns/src/builtins/todo.rs`
  - [ ] **Ori Tests**: `tests/spec/types/never.ori`
  - [ ] **LLVM Support**: LLVM codegen for todo() returning Never
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/never_tests.rs` — todo Never codegen

- [ ] **Implement**: unreachable() and unreachable(reason:) return Never
  - [ ] **Rust Tests**: `ori_patterns/src/builtins/unreachable.rs`
  - [ ] **Ori Tests**: `tests/spec/types/never.ori`
  - [ ] **LLVM Support**: LLVM codegen for unreachable() returning Never
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/never_tests.rs` — unreachable Never codegen

### Pending (Future Work)

- [ ] **Implement**: break/continue have type Never inside loops
  - [ ] **Ori Tests**: `tests/spec/control_flow/never_break_continue.ori`
  - [ ] **LLVM Support**: LLVM codegen for break/continue as Never type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/never_tests.rs` — break/continue Never codegen

- [ ] **Implement**: Early-return path of ? operator has type Never
  - [ ] **Ori Tests**: `tests/spec/control_flow/never_propagation.ori`
  - [ ] **LLVM Support**: LLVM codegen for ? operator early-return as Never
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/never_tests.rs` — ? operator Never codegen

- [ ] **Implement**: Infinite loop (no break) has type Never
  - [ ] **Ori Tests**: `tests/spec/control_flow/never_infinite_loop.ori`
  - [ ] **LLVM Support**: LLVM codegen for infinite loop as Never type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/never_tests.rs` — infinite loop Never codegen

- [ ] **Implement**: Never variants can be omitted from match exhaustiveness
  - [ ] **Rust Tests**: `oric/src/typeck/` — exhaustiveness with Never tests
  - [ ] **Ori Tests**: `tests/spec/patterns/never_exhaustiveness.ori`

- [ ] **Implement**: Error E0920 for Never as struct field type
  - [ ] **Rust Tests**: `oric/src/typeck/` — struct field restriction tests
  - [ ] **Ori Tests**: `tests/compile-fail/never_struct_field.ori`

- [ ] **Implement**: Allow Never in sum type variant payloads
  - [ ] **Ori Tests**: `tests/spec/types/never_sum_variant.ori`
  - [ ] **LLVM Support**: LLVM codegen for Never in sum type variant payloads
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/never_tests.rs` — Never sum variant codegen

---

## 1.2 Parameter Type Annotations

- [ ] **Implement**: Add `type_id_to_type()` helper function — spec/08-declarations.md § Function Declarations
  - [ ] **Rust Tests**: `oric/src/typeck/infer/` — type conversion tests
  - [ ] **Ori Tests**: `tests/spec/declarations/functions.ori`

- [ ] **Implement**: Use `Param.ty` when present in `infer_function_signature()` — spec/08-declarations.md § Function Declarations
  - [ ] **Rust Tests**: `oric/src/typeck/infer/` — signature inference tests
  - [ ] **Ori Tests**: `tests/spec/declarations/functions.ori`

- [ ] **Implement**: Use declared return type when present — spec/08-declarations.md § Function Declarations
  - [ ] **Rust Tests**: `oric/src/typeck/infer/` — return type handling tests
  - [ ] **Ori Tests**: `tests/spec/declarations/functions.ori`

- [ ] **Implement**: Handle `TypeId::INFER` for unannotated parameters — spec/06-types.md § Type Inference
  - [ ] **Rust Tests**: `oric/src/typeck/infer/` — inference tests
  - [ ] **Ori Tests**: `tests/spec/declarations/functions.ori`

> **Update Plan Status, and Pause**

---

## 1.3 Lambda Type Annotations

- [ ] **Implement**: Typed lambda parameters `(x: int) -> x + 1` — spec/09-expressions.md § Lambda Expressions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/` — lambda type inference tests
  - [ ] **Ori Tests**: `tests/spec/expressions/lambdas.ori`

- [ ] **Implement**: Explicit return type `(x: int) -> int = x + 1` — spec/09-expressions.md § Lambda Expressions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/` — lambda return type tests
  - [ ] **Ori Tests**: `tests/spec/expressions/lambdas.ori`

---

## 1.4 Let Binding Types

- [ ] **Implement**: Type annotation in `let x: int = ...` — spec/09-expressions.md § Let Bindings
  - [ ] **Rust Tests**: `oric/src/typeck/infer/` — let binding type tests
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`

> **Update Plan Status, and Pause**

---

## 1.6 Low-Level Future-Proofing (Reserved Slots)

**Proposal**: `proposals/approved/low-level-future-proofing-proposal.md`

Reserve architectural space in the type system for future low-level features (inline types, borrowed views). No user-visible changes — only internal structure.

### Type System Slots

- [ ] **Implement**: Add `LifetimeId` type to `ori_types`
  - [ ] `LifetimeId(u32)` newtype with `STATIC` constant only
  - [ ] **Rust Tests**: `ori_types/src/` — LifetimeId basic tests

- [ ] **Implement**: Add `ValueCategory` enum to `ori_types`
  - [ ] `Boxed` (default), `Inline` (reserved), `View` (reserved)
  - [ ] Document `Inline` as "NO ARC header" (distinct from small-struct optimization)
  - [ ] **Rust Tests**: `ori_types/src/` — ValueCategory tests

- [ ] **Implement**: Add `#[doc(hidden)]` `Borrowed` variant to `Type` enum
  - [ ] `Borrowed { inner: Box<Type>, lifetime: LifetimeId }`
  - [ ] Never constructed — exists only to reserve the concept
  - [ ] **Rust Tests**: Verify enum size assertion still passes

- [ ] **Implement**: Add `category` field to `TypeData::Struct` variant (if exists)
  - [ ] Default to `ValueCategory::Boxed`
  - [ ] **Rust Tests**: Verify backward compatibility

### Syntax Reservation

- [ ] **Implement**: Add `inline` as reserved keyword in lexer
  - [ ] Recognizes but does not assign special semantics
  - [ ] **Rust Tests**: `ori_lexer/src/` — keyword recognition

- [ ] **Implement**: Add `view` as reserved keyword in lexer
  - [ ] Recognizes but does not assign special semantics
  - [ ] **Rust Tests**: `ori_lexer/src/` — keyword recognition

- [ ] **Implement**: Reserve `&` in type position
  - [ ] Parser rejects `&T` with helpful error message
  - [ ] `&` remains available in expression position (future bitwise-and)
  - [ ] **Rust Tests**: `ori_parse/src/` — reserved syntax rejection

- [ ] **Implement**: Parser rejects reserved keywords with helpful errors
  - [ ] `inline type` → "inline types are reserved for a future version of Ori"
  - [ ] `view T` → "view types are reserved for a future version of Ori"
  - [ ] `&T` → "borrowed references (&T) are reserved for a future version"
  - [ ] **Ori Tests**: Error message verification (if compile-fail tests support parser errors)

---

## 1.7 Section Completion Checklist

- [ ] All items above have all three checkboxes marked `[ ]` (1.1-1.5)
- [ ] 80+% test coverage (241 unit tests passing, exceeds 152 target)
- [ ] Run full test suite: `./test-all.sh` — **241 unit tests + 64 spec tests pass**
- [ ] Low-level future-proofing slots reserved (1.6)

**Section 1 Status**: Complete (core), In Progress (1.6 pending)
