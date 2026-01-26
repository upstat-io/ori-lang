# Phase 1: Type System Foundation

**Goal**: Fix type checking to properly use type annotations

> **SPEC**: `spec/06-types.md`, `spec/07-properties-of-types.md`, `spec/08-declarations.md`

---

## 1.1 Primitive Types

- [x] **Implement**: `int` type — spec/06-types.md § int
  - [x] **Rust Tests**: `sigilc/src/typeck/` — type representation and checking
  - [x] **Sigil Tests**: `tests/spec/types/primitives.si`

- [x] **Implement**: `float` type — spec/06-types.md § float
  - [x] **Rust Tests**: `sigilc/src/typeck/` — type representation and checking
  - [x] **Sigil Tests**: `tests/spec/types/primitives.si`

- [x] **Implement**: `bool` type — spec/06-types.md § bool
  - [x] **Rust Tests**: `sigilc/src/typeck/` — type representation and checking
  - [x] **Sigil Tests**: `tests/spec/types/primitives.si`

- [x] **Implement**: `str` type — spec/06-types.md § str
  - [x] **Rust Tests**: `sigilc/src/typeck/` — type representation and checking
  - [x] **Sigil Tests**: `tests/spec/types/primitives.si`

- [x] **Implement**: `char` type — spec/06-types.md § char
  - [x] **Rust Tests**: `sigilc/src/typeck/` — type representation and checking
  - [x] **Sigil Tests**: `tests/spec/types/primitives.si`

- [x] **Implement**: `byte` type — spec/06-types.md § byte
  - [x] **Rust Tests**: `sigilc/src/typeck/` — type representation and checking
  - [x] **Sigil Tests**: `tests/spec/types/primitives.si`

- [x] **Implement**: `void` type — spec/06-types.md § void
  - [x] **Rust Tests**: `sigilc/src/typeck/` — type representation and checking
  - [x] **Sigil Tests**: `tests/spec/types/primitives.si`

- [x] **Implement**: `Never` type — spec/06-types.md § Never
  - [x] **Rust Tests**: `sigilc/src/typeck/` — type representation and checking
  - [x] **Sigil Tests**: `tests/spec/types/primitives.si`

**Note**: Also fixed parser bug where type keywords (`int`, `float`, etc.) couldn't be used as builtin conversion function calls. See `parser/mod.rs:1007-1042`.

> **Update Plan Status, and Pause**

---

## 1.2 Parameter Type Annotations

- [x] **Implement**: Add `type_id_to_type()` helper function — spec/08-declarations.md § Function Declarations
  - [x] **Rust Tests**: `sigilc/src/typeck/infer/` — type conversion tests
  - [x] **Sigil Tests**: `tests/spec/declarations/functions.si`

- [x] **Implement**: Use `Param.ty` when present in `infer_function_signature()` — spec/08-declarations.md § Function Declarations
  - [x] **Rust Tests**: `sigilc/src/typeck/infer/` — signature inference tests
  - [x] **Sigil Tests**: `tests/spec/declarations/functions.si`

- [x] **Implement**: Use declared return type when present — spec/08-declarations.md § Function Declarations
  - [x] **Rust Tests**: `sigilc/src/typeck/infer/` — return type handling tests
  - [x] **Sigil Tests**: `tests/spec/declarations/functions.si`

- [x] **Implement**: Handle `TypeId::INFER` for unannotated parameters — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `sigilc/src/typeck/infer/` — inference tests
  - [x] **Sigil Tests**: `tests/spec/declarations/functions.si`

> **Update Plan Status, and Pause**

---

## 1.3 Lambda Type Annotations

- [x] **Implement**: Typed lambda parameters `(x: int) -> x + 1` — spec/09-expressions.md § Lambda Expressions
  - [x] **Rust Tests**: `sigilc/src/typeck/infer/` — lambda type inference tests
  - [x] **Sigil Tests**: `tests/spec/expressions/lambdas.si`

- [x] **Implement**: Explicit return type `(x: int) -> int = x + 1` — spec/09-expressions.md § Lambda Expressions
  - [x] **Rust Tests**: `sigilc/src/typeck/infer/` — lambda return type tests
  - [x] **Sigil Tests**: `tests/spec/expressions/lambdas.si`

---

## 1.4 Let Binding Types

- [x] **Implement**: Type annotation in `let x: int = ...` — spec/09-expressions.md § Let Bindings
  - [x] **Rust Tests**: `sigilc/src/typeck/infer/` — let binding type tests
  - [x] **Sigil Tests**: `tests/spec/expressions/bindings.si`

> **Update Plan Status, and Pause**

---

## 1.5 Phase Completion Checklist

- [x] All items above have all three checkboxes marked `[x]`
- [x] 80+% test coverage (241 unit tests passing, exceeds 152 target)
- [x] Run full test suite: `cargo test && sigil test tests/spec/` — **241 unit tests + 64 spec tests pass**

**Phase 1 Status**: ✅ Complete
