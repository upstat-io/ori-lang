# Phase 5: Collections

**Goal**: Implement formatting for collection types: lists, maps, tuples, struct literals, and ranges.

> **DESIGN**: `docs/tooling/formatter/design/02-constructs/collections.md`

## Phase Status: ⏳ Not Started

## 5.1 Lists

### Simple Items (wrap multiple per line when broken)

- [ ] **Implement**: Empty list
  - [ ] **Golden Tests**: `tests/fmt/collections/lists/empty.ori`
  ```ori
  []
  ```
- [ ] **Implement**: Short list (inline)
  - [ ] **Golden Tests**: `tests/fmt/collections/lists/short.ori`
  ```ori
  [1, 2, 3]
  ```
- [ ] **Implement**: Long list of simple items (wrap multiple per line)
  - [ ] **Golden Tests**: `tests/fmt/collections/lists/simple_wrap.ori`
  ```ori
  [
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
      11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
  ]
  ```

### Complex Items (one per line when broken)

- [ ] **Implement**: List of structs
  - [ ] **Golden Tests**: `tests/fmt/collections/lists/structs.ori`
  ```ori
  [
      Point { x: 0, y: 0 },
      Point { x: 1, y: 1 },
      Point { x: 2, y: 2 },
  ]
  ```
- [ ] **Implement**: List of calls
  - [ ] **Golden Tests**: `tests/fmt/collections/lists/calls.ori`
- [ ] **Implement**: List of nested lists
  - [ ] **Golden Tests**: `tests/fmt/collections/lists/nested.ori`
- [ ] **Implement**: Mixed complexity (uses complex rules)
  - [ ] **Golden Tests**: `tests/fmt/collections/lists/mixed.ori`

### Spread Operator

- [ ] **Implement**: List spread
  - [ ] **Golden Tests**: `tests/fmt/collections/lists/spread.ori`
  ```ori
  [...a, ...b, extra]
  ```

### Complexity Detection

- [ ] **Implement**: Simple item detection (literals, identifiers)
  - [ ] **Rust Tests**: `ori_fmt/src/complexity/tests.rs`
- [ ] **Implement**: Complex item detection (calls, structs, nested)
  - [ ] **Rust Tests**: Complexity classification

## 5.2 Maps

- [ ] **Implement**: Empty map
  - [ ] **Golden Tests**: `tests/fmt/collections/maps/empty.ori`
  ```ori
  {}
  ```
- [ ] **Implement**: Short map (inline)
  - [ ] **Golden Tests**: `tests/fmt/collections/maps/short.ori`
  ```ori
  {"key": value}
  ```
- [ ] **Implement**: Multi-entry map (one per line)
  - [ ] **Golden Tests**: `tests/fmt/collections/maps/multi.ori`
  ```ori
  {
      "name": "Alice",
      "age": 30,
      "active": true,
  }
  ```
- [ ] **Implement**: Map with complex values
  - [ ] **Golden Tests**: `tests/fmt/collections/maps/complex.ori`
- [ ] **Implement**: Map spread
  - [ ] **Golden Tests**: `tests/fmt/collections/maps/spread.ori`
  ```ori
  {...defaults, ...overrides}
  ```

## 5.3 Tuples

- [ ] **Implement**: Unit tuple
  - [ ] **Golden Tests**: `tests/fmt/collections/tuples/unit.ori`
  ```ori
  ()
  ```
- [ ] **Implement**: Short tuple (inline)
  - [ ] **Golden Tests**: `tests/fmt/collections/tuples/short.ori`
  ```ori
  (1, "hello", true)
  ```
- [ ] **Implement**: Long tuple (one per line)
  - [ ] **Golden Tests**: `tests/fmt/collections/tuples/long.ori`
  ```ori
  (
      first_long_value,
      second_long_value,
      third_long_value,
  )
  ```
- [ ] **Implement**: Nested tuple
  - [ ] **Golden Tests**: `tests/fmt/collections/tuples/nested.ori`

## 5.4 Struct Literals

- [ ] **Implement**: Empty struct literal
  - [ ] **Golden Tests**: `tests/fmt/collections/structs/empty.ori`
  ```ori
  Unit {}
  ```
- [ ] **Implement**: Short struct (inline)
  - [ ] **Golden Tests**: `tests/fmt/collections/structs/short.ori`
  ```ori
  Point { x: 0, y: 0 }
  ```
- [ ] **Implement**: Long struct (one field per line)
  - [ ] **Golden Tests**: `tests/fmt/collections/structs/long.ori`
  ```ori
  User {
      id: 1,
      name: "Alice",
      email: "alice@example.com",
      created_at: now(),
  }
  ```
- [ ] **Implement**: Shorthand field (name matches variable)
  - [ ] **Golden Tests**: `tests/fmt/collections/structs/shorthand.ori`
  ```ori
  Point { x, y }
  ```
- [ ] **Implement**: Struct spread
  - [ ] **Golden Tests**: `tests/fmt/collections/structs/spread.ori`
  ```ori
  Point { ...original, x: 10 }
  ```
- [ ] **Implement**: Mixed shorthand and explicit
  - [ ] **Golden Tests**: `tests/fmt/collections/structs/mixed.ori`
- [ ] **Implement**: Nested struct literals
  - [ ] **Golden Tests**: `tests/fmt/collections/structs/nested.ori`

## 5.5 Ranges

Ranges are always inline (never break).

- [ ] **Implement**: Exclusive range
  - [ ] **Golden Tests**: `tests/fmt/collections/ranges/exclusive.ori`
  ```ori
  0..10
  ```
- [ ] **Implement**: Inclusive range
  - [ ] **Golden Tests**: `tests/fmt/collections/ranges/inclusive.ori`
  ```ori
  0..=10
  ```
- [ ] **Implement**: Stepped range
  - [ ] **Golden Tests**: `tests/fmt/collections/ranges/stepped.ori`
  ```ori
  0..10 by 2
  ```
- [ ] **Implement**: Descending range
  - [ ] **Golden Tests**: `tests/fmt/collections/ranges/descending.ori`
  ```ori
  10..0 by -1
  ```
- [ ] **Implement**: Range with expressions
  - [ ] **Golden Tests**: `tests/fmt/collections/ranges/expressions.ori`
  ```ori
  start..end by step
  ```

## 5.6 Set Literals

- [ ] **Implement**: Set literal formatting
  - [ ] **Golden Tests**: `tests/fmt/collections/sets/simple.ori`
  - Note: Sets use `Set { ... }` constructor syntax

## Completion Checklist

- [ ] All list formatting tests pass
- [ ] All map formatting tests pass
- [ ] All tuple formatting tests pass
- [ ] All struct literal tests pass
- [ ] All range formatting tests pass
- [ ] Complexity detection works correctly
- [ ] Round-trip verification for all collection types
