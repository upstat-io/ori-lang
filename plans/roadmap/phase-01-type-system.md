# Phase 1: Type System Foundation

**Goal**: Fix type checking to properly use type annotations

> **SPEC**: `spec/06-types.md`, `spec/07-properties-of-types.md`, `spec/08-declarations.md`

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

- [ ] **Implement**: Duration literal tokenization with all units (ns, us, ms, s, m, h)
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — duration literal tests
  - [ ] **Ori Tests**: `tests/spec/types/duration_literals.ori`
  - [ ] **LLVM Support**: LLVM codegen for Duration literals
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/duration_tests.rs`

- [ ] **Implement**: Size literal tokenization with all units (b, kb, mb, gb, tb)
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — size literal tests
  - [ ] **Ori Tests**: `tests/spec/types/size_literals.ori`
  - [ ] **LLVM Support**: LLVM codegen for Size literals
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/size_tests.rs`

- [ ] **Implement**: Error for floating-point prefix on duration/size literals
  - [ ] **Ori Tests**: `tests/compile-fail/duration_float_prefix.ori`

### Type System

- [ ] **Implement**: Duration type representation — spec/06-types.md § Duration
  - [ ] **Rust Tests**: `oric/src/typeck/` — Duration type tests
  - [ ] **Ori Tests**: `tests/spec/types/duration.ori`

- [ ] **Implement**: Size type representation — spec/06-types.md § Size
  - [ ] **Rust Tests**: `oric/src/typeck/` — Size type tests
  - [ ] **Ori Tests**: `tests/spec/types/size.ori`

### Arithmetic Operations

- [ ] **Implement**: Duration arithmetic (+, -, *, /, %, unary -)
  - [ ] **Rust Tests**: `oric/src/eval/` — Duration arithmetic tests
  - [ ] **Ori Tests**: `tests/spec/types/duration_arithmetic.ori`
  - [ ] **LLVM Support**: LLVM codegen for Duration arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/duration_tests.rs`

- [ ] **Implement**: Size arithmetic (+, -, *, /, %)
  - [ ] **Rust Tests**: `oric/src/eval/` — Size arithmetic tests
  - [ ] **Ori Tests**: `tests/spec/types/size_arithmetic.ori`
  - [ ] **LLVM Support**: LLVM codegen for Size arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/size_tests.rs`

- [ ] **Implement**: Compile error for unary negation on Size
  - [ ] **Rust Tests**: `oric/src/typeck/` — Size negation error
  - [ ] **Ori Tests**: `tests/compile-fail/size_unary_negation.ori`

- [ ] **Implement**: Runtime panic for Duration overflow
  - [ ] **Ori Tests**: `tests/spec/types/duration_overflow.ori`

- [ ] **Implement**: Runtime panic for negative Size result
  - [ ] **Ori Tests**: `tests/spec/types/size_negative_panic.ori`

### Conversion Methods

- [ ] **Implement**: Duration extraction methods (.nanoseconds(), .microseconds(), etc.)
  - [ ] **Ori Tests**: `tests/spec/types/duration_methods.ori`

- [ ] **Implement**: Duration factory methods (Duration.from_seconds(), etc.)
  - [ ] **Ori Tests**: `tests/spec/types/duration_factory.ori`

- [ ] **Implement**: Size extraction methods (.bytes(), .kilobytes(), etc.)
  - [ ] **Ori Tests**: `tests/spec/types/size_methods.ori`

- [ ] **Implement**: Size factory methods (Size.from_bytes(), etc.)
  - [ ] **Ori Tests**: `tests/spec/types/size_factory.ori`

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
