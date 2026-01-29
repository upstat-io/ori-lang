# Phase 2: Declarations

**Goal**: Implement formatting for all declaration constructs: functions, types, traits, impls, imports, and constants.

> **DESIGN**: `docs/tooling/formatter/design/02-constructs/declarations.md`

## Phase Status: ⏳ Not Started

## 2.1 Function Declarations

- [ ] **Implement**: Simple function formatting
  - [ ] **Golden Tests**: `tests/fmt/declarations/functions/simple.ori`
  ```ori
  @add (a: int, b: int) -> int = a + b
  ```
- [ ] **Implement**: Multi-line function parameters (when >100 chars)
  - [ ] **Golden Tests**: `tests/fmt/declarations/functions/multiline_params.ori`
  ```ori
  @send_notification (
      user_id: int,
      message: str,
      priority: Priority,
  ) -> Result<void, Error> = ...
  ```
- [ ] **Implement**: Generic parameters formatting
  - [ ] **Golden Tests**: `tests/fmt/declarations/functions/generics.ori`
  ```ori
  @identity<T> (value: T) -> T = value
  ```
- [ ] **Implement**: Generic parameters broken (when >100 chars)
  - [ ] **Golden Tests**: Multi-line generics
- [ ] **Implement**: Where clause formatting (inline)
  - [ ] **Golden Tests**: `tests/fmt/declarations/functions/where.ori`
- [ ] **Implement**: Where clause broken (multiple constraints)
  - [ ] **Golden Tests**: Multi-line where clause
- [ ] **Implement**: Capabilities clause (`uses`)
  - [ ] **Golden Tests**: `tests/fmt/declarations/functions/capabilities.ori`
- [ ] **Implement**: Return type on new line (when signature too long)
  - [ ] **Golden Tests**: Return type breaking
- [ ] **Implement**: Function clauses (pattern matching)
  - [ ] **Golden Tests**: `tests/fmt/declarations/functions/clauses.ori`
- [ ] **Implement**: Default parameter values
  - [ ] **Golden Tests**: `tests/fmt/declarations/functions/defaults.ori`
- [ ] **Implement**: `pub` visibility modifier
  - [ ] **Golden Tests**: Public function formatting

## 2.2 Type Definitions

### Struct Types

- [ ] **Implement**: Inline struct (fits in 100 chars)
  - [ ] **Golden Tests**: `tests/fmt/declarations/types/struct_inline.ori`
  ```ori
  type Point = { x: int, y: int }
  ```
- [ ] **Implement**: Multi-line struct (>100 chars)
  - [ ] **Golden Tests**: `tests/fmt/declarations/types/struct_multiline.ori`
  ```ori
  type User = {
      id: int,
      name: str,
      email: str,
      created_at: DateTime,
  }
  ```
- [ ] **Implement**: Generic struct
  - [ ] **Golden Tests**: `tests/fmt/declarations/types/struct_generic.ori`

### Sum Types

- [ ] **Implement**: Inline sum type (fits in 100 chars)
  - [ ] **Golden Tests**: `tests/fmt/declarations/types/sum_inline.ori`
  ```ori
  type Color = Red | Green | Blue
  ```
- [ ] **Implement**: Multi-line sum type (>100 chars)
  - [ ] **Golden Tests**: `tests/fmt/declarations/types/sum_multiline.ori`
  ```ori
  type Result<T, E> =
      | Ok(value: T)
      | Err(error: E)
  ```
- [ ] **Implement**: Sum type with data fields
  - [ ] **Golden Tests**: Variants with fields
- [ ] **Implement**: Generic sum type
  - [ ] **Golden Tests**: `tests/fmt/declarations/types/sum_generic.ori`

### Type Aliases

- [ ] **Implement**: Simple type alias
  - [ ] **Golden Tests**: `tests/fmt/declarations/types/alias.ori`
  ```ori
  type UserId = int
  ```

## 2.3 Trait Definitions

- [ ] **Implement**: Simple trait (single method)
  - [ ] **Golden Tests**: `tests/fmt/declarations/traits/simple.ori`
  ```ori
  trait Printable { @to_str (self) -> str }
  ```
- [ ] **Implement**: Multi-method trait
  - [ ] **Golden Tests**: `tests/fmt/declarations/traits/multi_method.ori`
- [ ] **Implement**: Trait with default implementations
  - [ ] **Golden Tests**: `tests/fmt/declarations/traits/defaults.ori`
- [ ] **Implement**: Trait with associated types
  - [ ] **Golden Tests**: `tests/fmt/declarations/traits/associated.ori`
- [ ] **Implement**: Trait inheritance
  - [ ] **Golden Tests**: `tests/fmt/declarations/traits/inheritance.ori`
- [ ] **Implement**: Blank lines between methods (except single-method)
  - [ ] **Golden Tests**: Method spacing

## 2.4 Impl Blocks

- [ ] **Implement**: Inherent impl
  - [ ] **Golden Tests**: `tests/fmt/declarations/impls/inherent.ori`
- [ ] **Implement**: Trait impl
  - [ ] **Golden Tests**: `tests/fmt/declarations/impls/trait.ori`
- [ ] **Implement**: Generic impl
  - [ ] **Golden Tests**: `tests/fmt/declarations/impls/generic.ori`
- [ ] **Implement**: Impl with where clause
  - [ ] **Golden Tests**: `tests/fmt/declarations/impls/where.ori`
- [ ] **Implement**: Blank lines between methods
  - [ ] **Golden Tests**: Method spacing in impl

## 2.5 Test Declarations

- [ ] **Implement**: Targeted test formatting
  - [ ] **Golden Tests**: `tests/fmt/declarations/tests/targeted.ori`
  ```ori
  @test_add tests @add () -> void = run(...)
  ```
- [ ] **Implement**: Free-floating test formatting
  - [ ] **Golden Tests**: `tests/fmt/declarations/tests/free.ori`
- [ ] **Implement**: Multiple targets
  - [ ] **Golden Tests**: `tests/fmt/declarations/tests/multi_target.ori`
- [ ] **Implement**: Test attributes (`#skip`, `#compile_fail`, `#fail`)
  - [ ] **Golden Tests**: `tests/fmt/declarations/tests/attributes.ori`

## 2.6 Import Statements

- [ ] **Implement**: Simple import
  - [ ] **Golden Tests**: `tests/fmt/declarations/imports/simple.ori`
  ```ori
  use std.math { sqrt, abs }
  ```
- [ ] **Implement**: Relative import
  - [ ] **Golden Tests**: `tests/fmt/declarations/imports/relative.ori`
- [ ] **Implement**: Import with alias
  - [ ] **Golden Tests**: `tests/fmt/declarations/imports/alias.ori`
- [ ] **Implement**: Private import (`::` prefix)
  - [ ] **Golden Tests**: `tests/fmt/declarations/imports/private.ori`
- [ ] **Implement**: Multi-line import (>100 chars)
  - [ ] **Golden Tests**: `tests/fmt/declarations/imports/multiline.ori`
- [ ] **Implement**: Import sorting (stdlib first, then relative)
  - [ ] **Golden Tests**: `tests/fmt/declarations/imports/sorting.ori`
- [ ] **Implement**: Alphabetical sorting within groups
  - [ ] **Golden Tests**: Alphabetic order
- [ ] **Implement**: Re-export (`pub use`)
  - [ ] **Golden Tests**: `tests/fmt/declarations/imports/reexport.ori`
- [ ] **Implement**: Extension imports
  - [ ] **Golden Tests**: `tests/fmt/declarations/imports/extension.ori`

## 2.7 Constants

- [ ] **Implement**: Simple constant
  - [ ] **Golden Tests**: `tests/fmt/declarations/constants/simple.ori`
  ```ori
  let $MAX_SIZE = 1024
  ```
- [ ] **Implement**: Typed constant
  - [ ] **Golden Tests**: `tests/fmt/declarations/constants/typed.ori`
- [ ] **Implement**: Public constant
  - [ ] **Golden Tests**: `tests/fmt/declarations/constants/public.ori`
- [ ] **Implement**: Const function
  - [ ] **Golden Tests**: `tests/fmt/declarations/constants/const_fn.ori`

## Completion Checklist

- [ ] All function formatting tests pass
- [ ] All type definition tests pass
- [ ] All trait/impl tests pass
- [ ] All test declaration tests pass
- [ ] All import tests pass
- [ ] All constant tests pass
- [ ] Round-trip verification for all declaration types
