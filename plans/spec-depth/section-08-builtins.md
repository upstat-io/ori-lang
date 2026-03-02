---
section: "08"
title: "Built-in Functions (Annex C)"
status: not-started
goal: "Expand each built-in with panic conditions, type constraints, edge cases, and examples"
inspired_by:
  - "Go spec 'Built-in functions' — each function gets dedicated subsection with full rules"
depends_on: ["05", "06", "07"]
sections:
  - id: "08.1"
    title: "Assertion Functions"
    status: not-started
  - id: "08.2"
    title: "Diverging Functions"
    status: not-started
  - id: "08.3"
    title: "Collection Functions"
    status: not-started
  - id: "08.4"
    title: "Comparison Functions"
    status: not-started
  - id: "08.5"
    title: "Debug and IO Functions"
    status: not-started
  - id: "08.6"
    title: "Compile-Time Functions"
    status: not-started
  - id: "08.7"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Built-in Functions (Annex C)

**Status:** Not Started
**Goal:** Annex C lists built-in functions with signatures but many lack: when they panic, edge cases on empty/nil inputs, type constraints beyond the signature, and interaction with capabilities. Go's built-in function chapter gives each function (append, len, cap, etc.) its own subsection with complete rules. Target: expand each entry by ~5-15 lines.

**Context:** The current Annex C is a reference listing. It's correct but thin. For example, `len(collection:)` says it returns length, but doesn't state: O(1) for all built-in types? What about user types implementing `Len`? Does `len` on a `Range` return the count of values? What about infinite ranges? These edge cases belong in the spec.

**Reference implementations:**
- **Go** `ref/spec#Built-in_functions`: Each function gets its own subsection
- **Go** `ref/spec#Length_and_capacity`: `len` and `cap` with precise type-by-type rules

---

## 08.1 Assertion Functions

**File:** `docs/ori_lang/v2026/spec/annex-c-built-in-functions.md`

For each assertion function, document:

- [ ] `assert(condition:)`:
  - Panics when `condition` is `false`
  - Panic message: "assertion failed"
  - Type: `(bool) -> void`
  - Capability: none (pure)

- [ ] `assert_eq(actual:, expected:)`:
  - Panics when `actual != expected`
  - Panic message includes both values (requires `Debug` on operands)
  - Type: `<T: Eq + Debug>(T, T) -> void`
  - Edge: NaN — `assert_eq(actual: NaN, expected: NaN)` panics (NaN != NaN)

- [ ] `assert_ne(actual:, unexpected:)`:
  - Panics when `actual == unexpected`
  - Type: `<T: Eq + Debug>(T, T) -> void`

- [ ] `assert_some(option:)` / `assert_none(option:)`:
  - Type: `<T>(Option<T>) -> T` / `<T>(Option<T>) -> void`
  - assert_some returns the inner value on success

- [ ] `assert_ok(result:)` / `assert_err(result:)`:
  - Type: `<T, E>(Result<T, E>) -> T` / `<T, E>(Result<T, E>) -> E`
  - assert_ok returns the Ok value; assert_err returns the Err value

- [ ] `assert_panics(expr:)` / `assert_panics_with(expr:, message:)`:
  - Evaluates `expr` (a `() -> void` closure); expects it to panic
  - Panics if expr does NOT panic
  - `assert_panics_with`: additionally checks panic message contains substring

---

## 08.2 Diverging Functions

**File:** `docs/ori_lang/v2026/spec/annex-c-built-in-functions.md`

- [ ] `panic(msg:)`:
  - Type: `(str) -> Never`
  - Always panics — never returns
  - Triggers `@panic` handler if defined

- [ ] `todo()` / `todo(reason:)`:
  - Type: `() -> Never` / `(str) -> Never`
  - Panics with "not yet implemented" or user message
  - Intended for development placeholders

- [ ] `unreachable()` / `unreachable(reason:)`:
  - Type: `() -> Never` / `(str) -> Never`
  - Panics with "entered unreachable code"
  - Indicates a code path the programmer believes is impossible

---

## 08.3 Collection Functions

**File:** `docs/ori_lang/v2026/spec/annex-c-built-in-functions.md`

- [ ] `len(collection:)`:
  - Type: `<T: Len>(T) -> int`
  - O(1) for all built-in types
  - For user types: complexity is implementation-defined
  - Result is always ≥ 0
  - Infinite ranges: compile-time error (infinite ranges don't implement `Len`)
  - Relationship to `.len()` method: `len(collection: x)` desugars to `x.len()`

- [ ] `is_empty(collection:)`:
  - Type: `<T: IsEmpty>(T) -> bool`
  - Equivalent to `collection.is_empty()`
  - Equivalent to `len(collection: x) == 0` for types implementing both

- [ ] `is_some(option:)` / `is_none(option:)`:
  - Type: `<T>(Option<T>) -> bool`

- [ ] `is_ok(result:)` / `is_err(result:)`:
  - Type: `<T, E>(Result<T, E>) -> bool`

- [ ] `drop_early(value:)`:
  - Type: `<T>(T) -> void`
  - Drops the value before end of scope
  - The binding becomes inaccessible after this call
  - Useful for releasing resources early

---

## 08.4 Comparison Functions

- [ ] `compare(left:, right:)`:
  - Type: `<T: Comparable>(T, T) -> Ordering`
  - Desugars to `left.compare(other: right)`

- [ ] `min(left:, right:)` / `max(left:, right:)`:
  - Type: `<T: Comparable>(T, T) -> T`
  - On equal values: returns `left` (first argument)
  - NaN handling: follows `Comparable` trait (NaN > all)

- [ ] `hash_combine(seed:, value:)`:
  - Type: `(int, int) -> int`
  - Pure function (no side effects)
  - Boost hash_combine algorithm

---

## 08.5 Debug and IO Functions

- [ ] `print(msg:)`:
  - Type: `(str) -> void`
  - Capability: `Print` (default capability, always available)
  - Writes to stdout with trailing newline
  - In `@panic` handler: writes to stderr

- [ ] `dbg(value:)` / `dbg(value:, label:)`:
  - Type: `<T: Debug>(T) -> T` / `<T: Debug>(T, str) -> T`
  - Prints to stderr: `[file:line] label = value.debug()`
  - Returns the value unchanged (pass-through)
  - Useful for debugging pipelines: `x |> dbg |> process`

- [ ] `repeat(value:)`:
  - Type: `<T: Clone>(T) -> impl Iterator where Item == T`
  - Produces an infinite iterator of cloned values
  - Must be bounded with `.take(count:)` before `.collect()`

---

## 08.6 Compile-Time Functions

- [ ] `compile_error(msg:)`:
  - Type: `(str) -> Never`
  - Triggers a compile-time error with user message
  - Useful in conditional compilation to reject unsupported configurations

- [ ] `embed(path)`:
  - Type: context-driven — `str` for text files, `[byte]` for binary
  - Embeds file contents at compile time
  - Path relative to the source file
  - File not found → compile-time error

- [ ] `has_embed(path)`:
  - Type: `(str) -> bool`
  - Returns `true` if the file exists at compile time
  - No runtime file access

- [ ] `is_cancelled()`:
  - Type: `() -> bool`
  - Capability: requires `Suspend`
  - Checks if the current task has been requested to cancel

---

## 08.7 Completion Checklist

- [ ] Every built-in function has: signature, return type, panic conditions, edge cases
- [ ] Type constraints verified against compiler (Debug/Eq requirements)
- [ ] Capability requirements noted where applicable
- [ ] NaN/empty/None edge cases documented for each relevant function
- [ ] Cross-references to trait definitions where functions desugar to trait methods
- [ ] All additions use ISO normative style

**Exit Criteria:** A user can look up any built-in function in Annex C and know: its exact type, when it panics, what it does on edge case inputs, and what capabilities it requires.
