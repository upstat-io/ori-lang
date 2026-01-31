# Phase 1: Type System Foundation

**Goal**: Fix type checking to properly use type annotations

> **SPEC**: `spec/06-types.md`, `spec/07-properties-of-types.md`, `spec/08-declarations.md`

**Status**: 🔶 Partial — Core complete (1.1-1.5), approved proposals pending (1.1A Duration/Size traits, 1.1B Never semantics)

---

## 1.1 Primitive Types

- [x] **Implement**: `int` type — spec/06-types.md § int
  - [x] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori`

- [x] **Implement**: `float` type — spec/06-types.md § float
  - [x] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori`

- [x] **Implement**: `bool` type — spec/06-types.md § bool
  - [x] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori`

- [x] **Implement**: `str` type — spec/06-types.md § str
  - [x] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori`

- [x] **Implement**: `char` type — spec/06-types.md § char
  - [x] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori`

- [x] **Implement**: `byte` type — spec/06-types.md § byte
  - [x] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori`

- [x] **Implement**: `void` type — spec/06-types.md § void
  - [x] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori`

- [x] **Implement**: `Never` type — spec/06-types.md § Never
  - [x] **Rust Tests**: `oric/src/typeck/` — type representation and checking
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori`

**Note**: Also fixed parser bug where type keywords (`int`, `float`, etc.) couldn't be used as builtin conversion function calls. See `parser/mod.rs:1007-1042`.

> **Update Plan Status, and Pause**

---

## 1.1A Duration and Size Types

**Proposal**: `proposals/approved/duration-size-types-proposal.md`

Formalize Duration and Size primitive types with literal syntax, arithmetic, and conversion methods.

### Lexer

- [x] **Implement**: Duration literal tokenization with all units (ns, us, ms, s, m, h)
  - [x] **Rust Tests**: `ori_ir/src/token.rs` — DurationUnit enum with Nanoseconds, Microseconds
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration literal tests
  - [x] **LLVM Support**: LLVM codegen for Duration literals (nanosecond precision)
  - [x] **LLVM Rust Tests**: `ori_llvm/src/tests/arithmetic_tests.rs`

- [x] **Implement**: Size literal tokenization with all units (b, kb, mb, gb, tb)
  - [x] **Rust Tests**: `ori_ir/src/token.rs` — SizeUnit enum with Terabytes
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Size literal tests
  - [x] **LLVM Support**: LLVM codegen for Size literals
  - [x] **LLVM Rust Tests**: `ori_llvm/src/tests/arithmetic_tests.rs`

- [ ] **Implement**: Error for floating-point prefix on duration/size literals
  - [ ] **Ori Tests**: `tests/compile-fail/duration_float_prefix.ori`

### Type System

- [x] **Implement**: Duration type representation — spec/06-types.md § Duration
  - [x] **Rust Tests**: `ori_types/src/core.rs` — Type::Duration
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration type tests

- [x] **Implement**: Size type representation — spec/06-types.md § Size
  - [x] **Rust Tests**: `ori_types/src/core.rs` — Type::Size
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Size type tests

### Arithmetic Operations

- [x] **Implement**: Duration arithmetic (+, -, *, /, %, unary -)
  - [x] **Rust Tests**: `ori_eval/src/operators.rs` — Duration binary ops
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration arithmetic tests
  - [ ] **LLVM Support**: LLVM codegen for Duration arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/duration_tests.rs`

- [x] **Implement**: Size arithmetic (+, -, *, /, %)
  - [x] **Rust Tests**: `ori_eval/src/operators.rs` — Size binary ops
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Size arithmetic tests
  - [ ] **LLVM Support**: LLVM codegen for Size arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/size_tests.rs`

- [x] **Implement**: Compile error for unary negation on Size
  - [x] **Rust Tests**: `ori_typeck/src/infer/expressions/operators.rs` — Size negation check
  - [ ] **Ori Tests**: `tests/compile-fail/size_unary_negation.ori`

- [x] **Implement**: Runtime panic for Duration overflow
  - [x] **Ori Tests**: Built into checked arithmetic (panics on overflow)

- [x] **Implement**: Runtime panic for negative Size result
  - [x] **Ori Tests**: Built into Size subtraction (panics on negative)

### Conversion Methods

- [x] **Implement**: Duration extraction methods (.nanoseconds(), .microseconds(), etc.)
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Duration extraction method tests

- [ ] **Implement**: Duration factory methods (Duration.from_seconds(), etc.)
  - [ ] **Ori Tests**: `tests/spec/types/duration_factory.ori`
  - **Note**: Requires associated function syntax support (Type.method())

- [x] **Implement**: Size extraction methods (.bytes(), .kilobytes(), etc.)
  - [x] **Ori Tests**: `tests/spec/types/primitives.ori` — Size extraction method tests

- [ ] **Implement**: Size factory methods (Size.from_bytes(), etc.)
  - [ ] **Ori Tests**: `tests/spec/types/size_factory.ori`
  - **Note**: Requires associated function syntax support (Type.method())

### Trait Implementations

- [ ] **Implement**: Eq, Comparable, Hashable for Duration
  - [ ] **Ori Tests**: `tests/spec/types/duration_traits.ori`

- [ ] **Implement**: Eq, Comparable, Hashable for Size
  - [ ] **Ori Tests**: `tests/spec/types/size_traits.ori`

- [ ] **Implement**: Clone, Debug, Printable, Default for Duration
  - [ ] **Ori Tests**: `tests/spec/types/duration_traits.ori`

- [ ] **Implement**: Clone, Debug, Printable, Default for Size
  - [ ] **Ori Tests**: `tests/spec/types/size_traits.ori`

---

## 1.1B Never Type Semantics

**Proposal**: `proposals/approved/never-type-proposal.md`

Formalize the Never type as the bottom type with coercion rules, type inference behavior, and pattern matching exhaustiveness.

**Status**: ✅ Core complete (coercion and basic Never-producing expressions); advanced features pending

### Coercion

- [x] **Implement**: Never coerces to any type T in assignment contexts
  - [x] **Rust Tests**: `ori_types/src/context.rs` — Never unification tests
  - [x] **Ori Tests**: `tests/spec/types/never.ori`

- [x] **Implement**: Never coerces in conditional branches
  - [x] **Ori Tests**: `tests/spec/types/never.ori`

- [x] **Implement**: Never coerces in match arms
  - [x] **Ori Tests**: `tests/spec/types/never.ori`

### Expressions Producing Never

- [x] **Implement**: panic(msg:) returns Never
  - [x] **Ori Tests**: `tests/spec/types/never.ori`

- [x] **Implement**: todo() and todo(reason:) return Never
  - [x] **Rust Tests**: `ori_patterns/src/builtins/todo.rs`
  - [x] **Ori Tests**: `tests/spec/types/never.ori`

- [x] **Implement**: unreachable() and unreachable(reason:) return Never
  - [x] **Rust Tests**: `ori_patterns/src/builtins/unreachable.rs`
  - [x] **Ori Tests**: `tests/spec/types/never.ori`

### Pending (Future Work)

- [ ] **Implement**: break/continue have type Never inside loops
  - [ ] **Ori Tests**: `tests/spec/control_flow/never_break_continue.ori`

- [ ] **Implement**: Early-return path of ? operator has type Never
  - [ ] **Ori Tests**: `tests/spec/control_flow/never_propagation.ori`

- [ ] **Implement**: Infinite loop (no break) has type Never
  - [ ] **Ori Tests**: `tests/spec/control_flow/never_infinite_loop.ori`

- [ ] **Implement**: Never variants can be omitted from match exhaustiveness
  - [ ] **Rust Tests**: `oric/src/typeck/` — exhaustiveness with Never tests
  - [ ] **Ori Tests**: `tests/spec/patterns/never_exhaustiveness.ori`

- [ ] **Implement**: Error E0920 for Never as struct field type
  - [ ] **Rust Tests**: `oric/src/typeck/` — struct field restriction tests
  - [ ] **Ori Tests**: `tests/compile-fail/never_struct_field.ori`

- [ ] **Implement**: Allow Never in sum type variant payloads
  - [ ] **Ori Tests**: `tests/spec/types/never_sum_variant.ori`

---

## 1.2 Parameter Type Annotations

- [x] **Implement**: Add `type_id_to_type()` helper function — spec/08-declarations.md § Function Declarations
  - [x] **Rust Tests**: `oric/src/typeck/infer/` — type conversion tests
  - [x] **Ori Tests**: `tests/spec/declarations/functions.ori`

- [x] **Implement**: Use `Param.ty` when present in `infer_function_signature()` — spec/08-declarations.md § Function Declarations
  - [x] **Rust Tests**: `oric/src/typeck/infer/` — signature inference tests
  - [x] **Ori Tests**: `tests/spec/declarations/functions.ori`

- [x] **Implement**: Use declared return type when present — spec/08-declarations.md § Function Declarations
  - [x] **Rust Tests**: `oric/src/typeck/infer/` — return type handling tests
  - [x] **Ori Tests**: `tests/spec/declarations/functions.ori`

- [x] **Implement**: Handle `TypeId::INFER` for unannotated parameters — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `oric/src/typeck/infer/` — inference tests
  - [x] **Ori Tests**: `tests/spec/declarations/functions.ori`

> **Update Plan Status, and Pause**

---

## 1.3 Lambda Type Annotations

- [x] **Implement**: Typed lambda parameters `(x: int) -> x + 1` — spec/09-expressions.md § Lambda Expressions
  - [x] **Rust Tests**: `oric/src/typeck/infer/` — lambda type inference tests
  - [x] **Ori Tests**: `tests/spec/expressions/lambdas.ori`

- [x] **Implement**: Explicit return type `(x: int) -> int = x + 1` — spec/09-expressions.md § Lambda Expressions
  - [x] **Rust Tests**: `oric/src/typeck/infer/` — lambda return type tests
  - [x] **Ori Tests**: `tests/spec/expressions/lambdas.ori`

---

## 1.4 Let Binding Types

- [x] **Implement**: Type annotation in `let x: int = ...` — spec/09-expressions.md § Let Bindings
  - [x] **Rust Tests**: `oric/src/typeck/infer/` — let binding type tests
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori`

> **Update Plan Status, and Pause**

---

## 1.5 Phase Completion Checklist

- [x] All items above have all three checkboxes marked `[x]`
- [x] 80+% test coverage (241 unit tests passing, exceeds 152 target)
- [x] Run full test suite: `./test-all` — **241 unit tests + 64 spec tests pass**

**Phase 1 Status**: ✅ Complete
