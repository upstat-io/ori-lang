# Phase 7D: Stdlib Modules

**Goal**: Standard library modules including validate, resilience, math, testing, time, json, fs

> **DESIGN**: `modules/` documentation
> **PROPOSAL**: `proposals/approved/overflow-behavior-proposal.md` — Integer overflow behavior

---

## 7D.1 std.validate Module

- [ ] **Implement**: `validate(rules, value)` — modules/std.validate/index.md § validate
  - [ ] **Rust Tests**: `library/std/validate.rs` — validate function tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/validate.ori`
  - [ ] **LLVM Support**: LLVM codegen for validate
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/validate_tests.rs` — validate codegen

> **Syntax**: `use std.validate { validate }`
>
> ```ori
> validate(rules: [(cond, "error"), ...], value: val)
> ```
>
> Returns `Result<T, [str]>` — all rules checked, errors accumulated.

---

## 7D.2 std.resilience Module

- [ ] **Implement**: `retry(operation, attempts, backoff)` — modules/std.resilience/index.md § retry
  - [ ] **Rust Tests**: `library/std/resilience.rs` — retry function tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/resilience.ori`
  - [ ] **LLVM Support**: LLVM codegen for retry
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/resilience_tests.rs` — retry codegen

- [ ] **Implement**: `exponential(base: Duration) -> BackoffStrategy` — modules/std.resilience/index.md § exponential
  - [ ] **Rust Tests**: `library/std/resilience.rs` — exponential backoff tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/resilience.ori`
  - [ ] **LLVM Support**: LLVM codegen for exponential backoff
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/resilience_tests.rs` — exponential backoff codegen

- [ ] **Implement**: `linear(delay: Duration) -> BackoffStrategy` — modules/std.resilience/index.md § linear
  - [ ] **Rust Tests**: `library/std/resilience.rs` — linear backoff tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/resilience.ori`
  - [ ] **LLVM Support**: LLVM codegen for linear backoff
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/resilience_tests.rs` — linear backoff codegen

---

## 7D.3 std.math Module — Overflow-Safe Arithmetic

> **PROPOSAL**: `proposals/approved/overflow-behavior-proposal.md`

Default integer arithmetic panics on overflow. These functions provide explicit alternatives.

### 7D.3.1 Saturating Arithmetic

Clamps result to type bounds on overflow:

- [ ] **Implement**: `saturating_add(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — saturating_add tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_saturating.ori`
  - [ ] **LLVM Support**: LLVM codegen for saturating_add
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — saturating_add codegen

- [ ] **Implement**: `saturating_sub(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — saturating_sub tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_saturating.ori`
  - [ ] **LLVM Support**: LLVM codegen for saturating_sub
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — saturating_sub codegen

- [ ] **Implement**: `saturating_mul(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — saturating_mul tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_saturating.ori`
  - [ ] **LLVM Support**: LLVM codegen for saturating_mul
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — saturating_mul codegen

- [ ] **Implement**: Byte variants (`saturating_add(a: byte, b: byte) -> byte`, etc.)
  - [ ] **Rust Tests**: `library/std/math.rs` — byte saturating tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_saturating.ori`
  - [ ] **LLVM Support**: LLVM codegen for byte saturating arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — byte saturating codegen

### 7D.3.2 Wrapping Arithmetic

Wraps around on overflow (modular arithmetic):

- [ ] **Implement**: `wrapping_add(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — wrapping_add tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_wrapping.ori`
  - [ ] **LLVM Support**: LLVM codegen for wrapping_add
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — wrapping_add codegen

- [ ] **Implement**: `wrapping_sub(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — wrapping_sub tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_wrapping.ori`
  - [ ] **LLVM Support**: LLVM codegen for wrapping_sub
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — wrapping_sub codegen

- [ ] **Implement**: `wrapping_mul(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — wrapping_mul tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_wrapping.ori`
  - [ ] **LLVM Support**: LLVM codegen for wrapping_mul
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — wrapping_mul codegen

- [ ] **Implement**: Byte variants (`wrapping_add(a: byte, b: byte) -> byte`, etc.)
  - [ ] **Rust Tests**: `library/std/math.rs` — byte wrapping tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_wrapping.ori`
  - [ ] **LLVM Support**: LLVM codegen for byte wrapping arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — byte wrapping codegen

### 7D.3.3 Checked Arithmetic

Returns `Option<T>` — `None` on overflow:

- [ ] **Implement**: `checked_add(a: int, b: int) -> Option<int>`
  - [ ] **Rust Tests**: `library/std/math.rs` — checked_add tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_checked.ori`
  - [ ] **LLVM Support**: LLVM codegen for checked_add
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — checked_add codegen

- [ ] **Implement**: `checked_sub(a: int, b: int) -> Option<int>`
  - [ ] **Rust Tests**: `library/std/math.rs` — checked_sub tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_checked.ori`
  - [ ] **LLVM Support**: LLVM codegen for checked_sub
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — checked_sub codegen

- [ ] **Implement**: `checked_mul(a: int, b: int) -> Option<int>`
  - [ ] **Rust Tests**: `library/std/math.rs` — checked_mul tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_checked.ori`
  - [ ] **LLVM Support**: LLVM codegen for checked_mul
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — checked_mul codegen

- [ ] **Implement**: Byte variants (`checked_add(a: byte, b: byte) -> Option<byte>`, etc.)
  - [ ] **Rust Tests**: `library/std/math.rs` — byte checked tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_checked.ori`
  - [ ] **LLVM Support**: LLVM codegen for byte checked arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — byte checked codegen

### 7D.3.4 Type Bounds Constants

- [ ] **Implement**: `int.min`, `int.max` constants
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — type constants tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/type_bounds.ori`
  - [ ] **LLVM Support**: LLVM codegen for int.min/max constants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — int constants codegen

- [ ] **Implement**: `byte.min`, `byte.max` constants
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — byte constants tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/type_bounds.ori`
  - [ ] **LLVM Support**: LLVM codegen for byte.min/max constants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — byte constants codegen

### 7D.3.5 Default Overflow Behavior

- [ ] **Implement**: Arithmetic operators panic on overflow
  - [ ] Addition, subtraction, multiplication emit overflow checks
  - [ ] Division by zero and `int.min / -1` panic
  - [ ] Consistent behavior in debug and release builds
  - [ ] **Rust Tests**: `oric/src/eval/exec/binary.rs` — overflow panic tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/overflow_panic.ori`
  - [ ] **LLVM Support**: LLVM codegen for overflow panic behavior
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — overflow panic codegen

- [ ] **Implement**: Compile-time constant overflow is a compile error
  - [ ] `$big = int.max + 1` → ERROR: constant overflow
  - [ ] **Rust Tests**: `oric/src/typeck/checker/const_eval.rs` — constant overflow tests
  - [ ] **Ori Tests**: `tests/compile-fail/constant_overflow.ori`
  - [ ] **LLVM Support**: LLVM codegen for compile-time overflow errors
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — constant overflow codegen

---

## 7D.4 std.testing Module

> Move testing assertions from built-ins to std.testing.

- [ ] **Implement**: `assert_eq(actual, expected)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_eq tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_eq
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_eq codegen

- [ ] **Implement**: `assert_ne(actual, unexpected)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_ne tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_ne
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_ne codegen

- [ ] **Implement**: `assert_some(option)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_some tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_some
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_some codegen

- [ ] **Implement**: `assert_none(option)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_none tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_none
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_none codegen

- [ ] **Implement**: `assert_ok(result)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_ok tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_ok
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_ok codegen

- [ ] **Implement**: `assert_err(result)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_err
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_err codegen

- [ ] **Implement**: `assert_panics(expr)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_panics tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_panics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_panics codegen

- [ ] **Implement**: `assert_panics_with(expr, message)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_panics_with tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_panics_with
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_panics_with codegen

---

## 7D.5 Developer Functions

> **PROPOSAL**: `proposals/drafts/developer-functions-proposal.md`
>
> Convenience functions for development: placeholders and debugging.

- [ ] **Implement**: `todo()` and `todo(reason: str)` → `Never`
  - Panics with "not yet implemented" and location
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — todo tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/todo.ori`
  - [ ] **LLVM Support**: LLVM codegen for todo
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — todo codegen

- [ ] **Implement**: `unreachable()` and `unreachable(reason: str)` → `Never`
  - Panics with "unreachable code reached" and location
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — unreachable tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/unreachable.ori`
  - [ ] **LLVM Support**: LLVM codegen for unreachable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — unreachable codegen

- [ ] **Implement**: `dbg(value: T)` and `dbg(value: T, label: str)` → `T`
  - Requires `T: Debug`
  - Prints `[file:line] label = <debug>` to stderr
  - Returns value unchanged
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — dbg tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/dbg.ori`
  - [ ] **LLVM Support**: LLVM codegen for dbg
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — dbg codegen

- [ ] **Implement**: Location capture for `todo`, `unreachable`, `dbg`
  - Compiler passes call-site location implicitly
  - [ ] **Rust Tests**: `oric/src/eval/location.rs`
  - [ ] **LLVM Support**: LLVM codegen for location capture
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — location capture codegen

---

## 7D.6 std.time Module

**Proposal**: `proposals/approved/stdlib-time-api-proposal.md`

Date/time types, formatting, parsing, arithmetic, and timezone handling.

### 7D.6.1 Core Types

- [ ] **Implement**: `Instant` type — UTC timestamp (nanoseconds since Unix epoch)
  - `Instant.now()`, `from_unix_secs()`, `from_unix_millis()`, `to_unix_secs()`, `to_unix_millis()`
  - `add()`, `sub()`, `diff()` for Duration arithmetic
  - Implements `Comparable`
  - [ ] **Rust Tests**: `library/std/time/instant.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/instant.ori`
  - [ ] **LLVM Support**: LLVM codegen for Instant
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Instant codegen

- [ ] **Implement**: `DateTime` type — date and time in a specific timezone
  - `now()`, `now_utc()`, `from_instant()`, `from_parts()`
  - `to_instant()`, `to_timezone()`, `to_utc()`, `to_local()`
  - `date()`, `time()`, `weekday()` component accessors
  - `add()`, `add_days()`, `add_months()`, `add_years()` arithmetic
  - [ ] **Rust Tests**: `library/std/time/datetime.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/datetime.ori`
  - [ ] **LLVM Support**: LLVM codegen for DateTime
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — DateTime codegen

- [ ] **Implement**: `Date` type — date only (no time component)
  - `today()`, `new()`
  - `weekday()`, `day_of_year()`, `is_leap_year()`, `days_in_month()`
  - `add_days()`, `add_months()`, `add_years()`, `diff_days()`
  - [ ] **Rust Tests**: `library/std/time/date.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/date.ori`
  - [ ] **LLVM Support**: LLVM codegen for Date
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Date codegen

- [ ] **Implement**: `Time` type — time of day only (no date component)
  - `now()`, `new()`, `midnight()`, `noon()`
  - `to_seconds()`, `to_millis()`
  - [ ] **Rust Tests**: `library/std/time/time.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/time.ori`
  - [ ] **LLVM Support**: LLVM codegen for Time
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Time codegen

- [ ] **Implement**: `Timezone` type — timezone info (opaque)
  - `utc()`, `local()`, `from_name()`, `from_offset()`, `fixed()`
  - `name()`, `offset_at()`
  - [ ] **Rust Tests**: `library/std/time/timezone.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/timezone.ori`
  - [ ] **LLVM Support**: LLVM codegen for Timezone
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Timezone codegen

- [ ] **Implement**: `Weekday` sum type — `Monday | Tuesday | ... | Sunday`
  - `is_weekend()`, `next()`, `prev()`, `all()`
  - [ ] **Rust Tests**: `library/std/time/weekday.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/weekday.ori`
  - [ ] **LLVM Support**: LLVM codegen for Weekday
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Weekday codegen

### 7D.6.2 Duration Extension Methods

> **Note:** These are extension methods requiring `use std.time { Duration }`.

- [ ] **Implement**: Duration construction methods
  - `from_nanos()`, `from_micros()`, `from_millis()`, `from_secs()`, `from_mins()`, `from_hours()`, `from_days()`
  - [ ] **Rust Tests**: `library/std/time/duration.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/duration.ori`
  - [ ] **LLVM Support**: LLVM codegen for Duration construction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Duration construction codegen

- [ ] **Implement**: Duration extraction methods
  - `to_nanos()`, `to_micros()`, `to_millis()`, `to_secs()`, `to_mins()`, `to_hours()`
  - [ ] **Rust Tests**: `library/std/time/duration.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/duration.ori`
  - [ ] **LLVM Support**: LLVM codegen for Duration extraction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Duration extraction codegen

- [ ] **Implement**: Duration component methods
  - `hours_part()`, `minutes_part()`, `seconds_part()`
  - [ ] **Rust Tests**: `library/std/time/duration.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/duration.ori`
  - [ ] **LLVM Support**: LLVM codegen for Duration components
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Duration components codegen

- [ ] **Implement**: Duration arithmetic and checks
  - `add()`, `sub()`, `mul()`, `div()`
  - `is_zero()`, `is_negative()`
  - [ ] **Rust Tests**: `library/std/time/duration.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/duration.ori`
  - [ ] **LLVM Support**: LLVM codegen for Duration arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Duration arithmetic codegen

### 7D.6.3 Formatting

- [ ] **Implement**: `format(dt, pattern)` — DateTime formatting with pattern specifiers
  - Pattern specifiers: `YYYY`, `YY`, `MM`, `M`, `DD`, `D`, `HH`, `H`, `hh`, `h`, `mm`, `ss`, `SSS`, `a`, `E`, `EEEE`, `MMM`, `MMMM`, `Z`, `ZZ`, `z`
  - [ ] **Rust Tests**: `library/std/time/format.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/format.ori`
  - [ ] **LLVM Support**: LLVM codegen for format
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — format codegen

- [ ] **Implement**: `format_date(d, pattern)` — Date-only formatting
  - [ ] **Rust Tests**: `library/std/time/format.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/format.ori`
  - [ ] **LLVM Support**: LLVM codegen for format_date
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — format_date codegen

- [ ] **Implement**: `format_time(t, pattern)` — Time-only formatting
  - [ ] **Rust Tests**: `library/std/time/format.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/format.ori`
  - [ ] **LLVM Support**: LLVM codegen for format_time
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — format_time codegen

- [ ] **Implement**: ISO 8601 formatting
  - `to_iso8601(dt)`, `to_iso8601_date(d)`, `to_iso8601_time(t)`
  - [ ] **Rust Tests**: `library/std/time/format.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/iso8601.ori`
  - [ ] **LLVM Support**: LLVM codegen for ISO 8601 formatting
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — ISO 8601 formatting codegen

### 7D.6.4 Parsing

- [ ] **Implement**: `parse(source, pattern, tz)` — DateTime parsing with optional timezone
  - `tz` parameter defaults to UTC for patterns without timezone info
  - [ ] **Rust Tests**: `library/std/time/parse.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/parse.ori`
  - [ ] **LLVM Support**: LLVM codegen for parse
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — parse codegen

- [ ] **Implement**: `parse_date(source, pattern)` — Date-only parsing
  - [ ] **Rust Tests**: `library/std/time/parse.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/parse.ori`
  - [ ] **LLVM Support**: LLVM codegen for parse_date
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — parse_date codegen

- [ ] **Implement**: `parse_time(source, pattern)` — Time-only parsing
  - [ ] **Rust Tests**: `library/std/time/parse.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/parse.ori`
  - [ ] **LLVM Support**: LLVM codegen for parse_time
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — parse_time codegen

- [ ] **Implement**: ISO 8601 parsing
  - `from_iso8601(source)`, `from_iso8601_date(source)`
  - [ ] **Rust Tests**: `library/std/time/parse.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/iso8601.ori`
  - [ ] **LLVM Support**: LLVM codegen for ISO 8601 parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — ISO 8601 parsing codegen

### 7D.6.5 Error Type

- [ ] **Implement**: `TimeError` and `TimeErrorKind`
  - `InvalidDate`, `InvalidTime`, `InvalidTimezone`, `ParseError`, `Overflow`
  - [ ] **Rust Tests**: `library/std/time/error.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/error.ori`
  - [ ] **LLVM Support**: LLVM codegen for TimeError
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — TimeError codegen

### 7D.6.6 Clock Capability

- [ ] **Implement**: `Clock` trait update
  - `now() -> Instant`, `local_timezone() -> Timezone`
  - [ ] **Rust Tests**: `oric/src/capabilities/clock.rs`
  - [ ] **Ori Tests**: `tests/spec/capabilities/clock.ori`
  - [ ] **LLVM Support**: LLVM codegen for Clock capability
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` — Clock codegen

- [ ] **Implement**: `MockClock` for testing
  - `MockClock.new(now)` constructor
  - `advance(by)` with interior mutability
  - [ ] **Rust Tests**: `library/std/time/mock.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/mock_clock.ori`
  - [ ] **LLVM Support**: LLVM codegen for MockClock
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — MockClock codegen

---

## 7D.7 std.json Module

**Proposal**: `proposals/approved/stdlib-json-api-proposal.md`

JSON parsing, serialization, and manipulation.

### 7D.7.1 Core Types

- [ ] **Implement**: `JsonValue` sum type
  - `Null | Bool(bool) | Number(float) | String(str) | Array([JsonValue]) | Object({str: JsonValue})`
  - [ ] **Rust Tests**: `library/std/json/value.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/value.ori`
  - [ ] **LLVM Support**: LLVM codegen for JsonValue
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — JsonValue codegen

- [ ] **Implement**: `JsonError` and `JsonErrorKind` types
  - `ParseError | TypeError | MissingField | UnknownField | ValueError`
  - Fields: `kind`, `message`, `path`, `position`
  - [ ] **Rust Tests**: `library/std/json/error.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/error.ori`
  - [ ] **LLVM Support**: LLVM codegen for JsonError
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — JsonError codegen

- [ ] **Implement**: `Json` trait
  - `@to_json (self) -> JsonValue`
  - `@from_json (json: JsonValue) -> Result<Self, JsonError>`
  - [ ] **Rust Tests**: `library/std/json/trait.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/trait.ori`
  - [ ] **LLVM Support**: LLVM codegen for Json trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — Json trait codegen

### 7D.7.2 Parsing API

- [ ] **Implement**: `parse(source: str) -> Result<JsonValue, JsonError>`
  - [ ] **Rust Tests**: `library/std/json/parse.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/parse.ori`
  - [ ] **LLVM Support**: LLVM codegen for parse
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — parse codegen

- [ ] **Implement**: `parse_as<T: Json>(source: str) -> Result<T, JsonError>`
  - [ ] **Rust Tests**: `library/std/json/parse.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/parse.ori`
  - [ ] **LLVM Support**: LLVM codegen for parse_as
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — parse_as codegen

### 7D.7.3 Serialization API

- [ ] **Implement**: `stringify(value: JsonValue) -> str`
  - [ ] **Rust Tests**: `library/std/json/stringify.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/stringify.ori`
  - [ ] **LLVM Support**: LLVM codegen for stringify
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — stringify codegen

- [ ] **Implement**: `stringify_pretty(value: JsonValue, indent: int = 2) -> str`
  - [ ] **Rust Tests**: `library/std/json/stringify.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/stringify.ori`
  - [ ] **LLVM Support**: LLVM codegen for stringify_pretty
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — stringify_pretty codegen

- [ ] **Implement**: `to_json_string<T: Json>(value: T) -> str`
  - [ ] **Rust Tests**: `library/std/json/stringify.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/stringify.ori`
  - [ ] **LLVM Support**: LLVM codegen for to_json_string
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — to_json_string codegen

- [ ] **Implement**: `to_json_string_pretty<T: Json>(value: T, indent: int = 2) -> str`
  - [ ] **Rust Tests**: `library/std/json/stringify.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/stringify.ori`
  - [ ] **LLVM Support**: LLVM codegen for to_json_string_pretty
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — to_json_string_pretty codegen

### 7D.7.4 JsonValue Methods

- [ ] **Implement**: Type check methods
  - `is_null()`, `is_bool()`, `is_number()`, `is_string()`, `is_array()`, `is_object()`
  - [ ] **Rust Tests**: `library/std/json/value.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/value_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for type check methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — type check methods codegen

- [ ] **Implement**: Safe extraction methods
  - `as_bool()`, `as_number()`, `as_int()`, `as_string()`, `as_array()`, `as_object()`
  - `as_int()` returns `Some` only for exact integers within int range (no truncation)
  - [ ] **Rust Tests**: `library/std/json/value.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/value_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for extraction methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — extraction methods codegen

- [ ] **Implement**: Indexing methods
  - `get(key: str)` for objects, `get_index(index: int)` for arrays
  - [ ] **Rust Tests**: `library/std/json/value.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/value_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for indexing methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — indexing methods codegen

- [ ] **Implement**: Path access method
  - `at(path: str)` — dot notation with array index support (`"users[0].name"`)
  - [ ] **Rust Tests**: `library/std/json/value.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/path_access.ori`
  - [ ] **LLVM Support**: LLVM codegen for path access
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — path access codegen

### 7D.7.5 Derive Macro

- [ ] **Implement**: `#derive(Json)` for structs
  - Generate `to_json` and `from_json` implementations
  - [ ] **Rust Tests**: `oric/src/typeck/derives/json.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/derive_struct.ori`
  - [ ] **LLVM Support**: LLVM codegen for derive(Json) structs
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — derive(Json) struct codegen

- [ ] **Implement**: `#derive(Json)` for sum types
  - Simple variants serialize as strings, payload variants as objects
  - Support `#json(tag: "type", content: "data")` for tagged unions
  - [ ] **Rust Tests**: `oric/src/typeck/derives/json.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/derive_enum.ori`
  - [ ] **LLVM Support**: LLVM codegen for derive(Json) enums
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — derive(Json) enum codegen

- [ ] **Implement**: Field attributes for `#derive(Json)`
  - `#json(rename: "name")` — different JSON field name
  - `#json(skip)` — exclude from serialization
  - `#json(default: value)` — default if field missing
  - `#json(flatten)` — merge nested object into parent (compile error on conflicts)
  - [ ] **Rust Tests**: `oric/src/typeck/derives/json.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/derive_attrs.ori`
  - [ ] **LLVM Support**: LLVM codegen for field attributes
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — field attributes codegen

### 7D.7.6 Standard Type Implementations

- [ ] **Implement**: Primitive Json implementations
  - `bool`, `int`, `float`, `str`
  - [ ] **Rust Tests**: `library/std/json/impls.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/impls_primitive.ori`
  - [ ] **LLVM Support**: LLVM codegen for primitive Json impls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — primitive impls codegen

- [ ] **Implement**: Collection Json implementations
  - `[T]` (array), `{str: V}` (object), `Set<T>` (array), `Option<T>` (null or value), `(A, B)` (array)
  - Non-string map keys serialize as strings
  - [ ] **Rust Tests**: `library/std/json/impls.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/impls_collection.ori`
  - [ ] **LLVM Support**: LLVM codegen for collection Json impls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — collection impls codegen

- [ ] **Implement**: Built-in type Json implementations
  - `Duration` → ISO 8601 duration string (`"PT1H30M"`)
  - `Size` → integer bytes
  - [ ] **Rust Tests**: `library/std/json/impls.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/impls_builtin.ori`
  - [ ] **LLVM Support**: LLVM codegen for built-in Json impls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — built-in impls codegen

### 7D.7.7 Streaming API

- [ ] **Implement**: `JsonParser` type with Iterator trait
  - `new(source: str)` constructor
  - Implements `Iterator` and `Iterable` with `Item = JsonEvent`
  - [ ] **Rust Tests**: `library/std/json/stream.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/streaming.ori`
  - [ ] **LLVM Support**: LLVM codegen for JsonParser
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — JsonParser codegen

- [ ] **Implement**: `JsonEvent` sum type
  - `StartObject | EndObject | StartArray | EndArray | Key(str) | Value(JsonValue)`
  - [ ] **Rust Tests**: `library/std/json/stream.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/streaming.ori`
  - [ ] **LLVM Support**: LLVM codegen for JsonEvent
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — JsonEvent codegen

---

## 7D.8 std.fs Module

**Proposal**: `proposals/approved/stdlib-fs-api-proposal.md`
**FFI Implementation**: `proposals/approved/stdlib-fs-api-ffi-revision.md`

File system operations including reading, writing, directory manipulation, and file metadata.

**Depends on**: `std.time` (for `Instant` type in `FileInfo`), Fixed-capacity lists proposal (for FFI struct arrays)

### 7D.8.1 Core Types

- [ ] **Implement**: `Path` type — file system path abstraction
  - `from_str()`, `join()`, `join_str()`, `parent()`, `file_name()`, `extension()`
  - `with_extension()`, `is_absolute()`, `to_str()`, `relative_to()`
  - [ ] **Rust Tests**: `library/std/fs/path.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/path.ori`
  - [ ] **LLVM Support**: LLVM codegen for Path
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — Path codegen

- [ ] **Implement**: `FileInfo` type — file metadata
  - Fields: `path`, `size`, `is_file`, `is_dir`, `is_symlink`, `modified` (Instant), `created` (Option<Instant>), `readonly`
  - [ ] **Rust Tests**: `library/std/fs/types.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/info.ori`
  - [ ] **LLVM Support**: LLVM codegen for FileInfo
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — FileInfo codegen

- [ ] **Implement**: `FileError` and `FileErrorKind` types
  - `NotFound | PermissionDenied | AlreadyExists | NotAFile | NotADirectory | DirectoryNotEmpty | IoError | InvalidPath`
  - [ ] **Rust Tests**: `library/std/fs/error.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/error.ori`
  - [ ] **LLVM Support**: LLVM codegen for FileError
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — FileError codegen

- [ ] **Implement**: `WriteMode` sum type
  - `Create` (error if exists), `Append` (create or append), `Truncate` (create or overwrite)
  - [ ] **Rust Tests**: `library/std/fs/types.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/write_mode.ori`
  - [ ] **LLVM Support**: LLVM codegen for WriteMode
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — WriteMode codegen

- [ ] **Implement**: `Permissions` type
  - Fields: `readable`, `writable`, `executable`
  - [ ] **Rust Tests**: `library/std/fs/types.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/permissions.ori`
  - [ ] **LLVM Support**: LLVM codegen for Permissions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — Permissions codegen

### 7D.8.2 Reading Files

- [ ] **Implement**: `read(path: str) -> Result<str, FileError> uses FileSystem`
  - [ ] **Rust Tests**: `library/std/fs/read.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/read.ori`
  - [ ] **LLVM Support**: LLVM codegen for read
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — read codegen

- [ ] **Implement**: `read_bytes(path: str) -> Result<[byte], FileError> uses FileSystem`
  - [ ] **Rust Tests**: `library/std/fs/read.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/read.ori`
  - [ ] **LLVM Support**: LLVM codegen for read_bytes
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — read_bytes codegen

- [ ] **Implement**: `read_lines(path: str) -> Result<[str], FileError> uses FileSystem`
  - [ ] **Rust Tests**: `library/std/fs/read.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/read.ori`
  - [ ] **LLVM Support**: LLVM codegen for read_lines
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — read_lines codegen

- [ ] **Implement**: `FileReader` type for streaming reads
  - `open_read(path: str)`, `read_chunk()`, `read_line()`, `close()`
  - Implements `Iterable` for line-by-line iteration
  - [ ] **Rust Tests**: `library/std/fs/reader.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/reader.ori`
  - [ ] **LLVM Support**: LLVM codegen for FileReader
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — FileReader codegen

### 7D.8.3 Writing Files

- [ ] **Implement**: `write(path: str, content: str) -> Result<void, FileError> uses FileSystem`
  - [ ] **Rust Tests**: `library/std/fs/write.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/write.ori`
  - [ ] **LLVM Support**: LLVM codegen for write
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — write codegen

- [ ] **Implement**: `write_bytes(path: str, content: [byte]) -> Result<void, FileError> uses FileSystem`
  - [ ] **Rust Tests**: `library/std/fs/write.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/write.ori`
  - [ ] **LLVM Support**: LLVM codegen for write_bytes
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — write_bytes codegen

- [ ] **Implement**: `write_with(path, content, mode, create_dirs)` with options
  - Default `mode: Truncate`, `create_dirs: false`
  - [ ] **Rust Tests**: `library/std/fs/write.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/write.ori`
  - [ ] **LLVM Support**: LLVM codegen for write_with
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — write_with codegen

- [ ] **Implement**: `FileWriter` type for streaming writes
  - `open_write(path, mode)`, `write_chunk()`, `write_str()`, `write_line()`, `flush()`, `close()`
  - [ ] **Rust Tests**: `library/std/fs/writer.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/writer.ori`
  - [ ] **LLVM Support**: LLVM codegen for FileWriter
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — FileWriter codegen

### 7D.8.4 Directory Operations

- [ ] **Implement**: `list_dir(path: str) -> Result<[str], FileError> uses FileSystem`
  - [ ] **Rust Tests**: `library/std/fs/dir.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/dir.ori`
  - [ ] **LLVM Support**: LLVM codegen for list_dir
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — list_dir codegen

- [ ] **Implement**: `list_dir_info(path: str) -> Result<[FileInfo], FileError> uses FileSystem`
  - [ ] **Rust Tests**: `library/std/fs/dir.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/dir.ori`
  - [ ] **LLVM Support**: LLVM codegen for list_dir_info
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — list_dir_info codegen

- [ ] **Implement**: `walk_dir(path: str) -> Result<[FileInfo], FileError> uses FileSystem`
  - [ ] **Rust Tests**: `library/std/fs/dir.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/walk.ori`
  - [ ] **LLVM Support**: LLVM codegen for walk_dir
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — walk_dir codegen

- [ ] **Implement**: `walk_dir_with(path, max_depth, follow_symlinks)` with options
  - [ ] **Rust Tests**: `library/std/fs/dir.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/walk.ori`
  - [ ] **LLVM Support**: LLVM codegen for walk_dir_with
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — walk_dir_with codegen

- [ ] **Implement**: `create_dir(path: str)` and `create_dir_all(path: str)`
  - [ ] **Rust Tests**: `library/std/fs/dir.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/dir.ori`
  - [ ] **LLVM Support**: LLVM codegen for create_dir/create_dir_all
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — create_dir codegen

- [ ] **Implement**: `remove_dir(path: str)` and `remove_dir_all(path: str)`
  - [ ] **Rust Tests**: `library/std/fs/dir.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/dir.ori`
  - [ ] **LLVM Support**: LLVM codegen for remove_dir/remove_dir_all
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — remove_dir codegen

### 7D.8.5 File Operations

- [ ] **Implement**: `copy(from: str, to: str)` and `copy_with(from, to, overwrite)`
  - [ ] **Rust Tests**: `library/std/fs/ops.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/ops.ori`
  - [ ] **LLVM Support**: LLVM codegen for copy/copy_with
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — copy codegen

- [ ] **Implement**: `move(from: str, to: str)` and `rename(from: str, to: str)` (alias)
  - [ ] **Rust Tests**: `library/std/fs/ops.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/ops.ori`
  - [ ] **LLVM Support**: LLVM codegen for move/rename
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — move codegen

- [ ] **Implement**: `remove(path: str) -> Result<void, FileError> uses FileSystem`
  - [ ] **Rust Tests**: `library/std/fs/ops.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/ops.ori`
  - [ ] **LLVM Support**: LLVM codegen for remove
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — remove codegen

### 7D.8.6 File Info Functions

- [ ] **Implement**: `info(path: str) -> Result<FileInfo, FileError> uses FileSystem`
  - [ ] **Rust Tests**: `library/std/fs/info.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/info.ori`
  - [ ] **LLVM Support**: LLVM codegen for info
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — info codegen

- [ ] **Implement**: `exists(path: str) -> bool uses FileSystem`
  - Returns `false` on permission denied (simpler API)
  - [ ] **Rust Tests**: `library/std/fs/info.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/info.ori`
  - [ ] **LLVM Support**: LLVM codegen for exists
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — exists codegen

- [ ] **Implement**: `is_file(path: str) -> bool` and `is_dir(path: str) -> bool`
  - [ ] **Rust Tests**: `library/std/fs/info.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/info.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_file/is_dir
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — is_file/is_dir codegen

### 7D.8.7 Glob Patterns

- [ ] **Implement**: `glob(pattern: str) -> Result<[str], FileError> uses FileSystem`
  - Supports `*`, `**`, `?`, `[abc]`, `{a,b}` patterns
  - [ ] **Rust Tests**: `library/std/fs/glob.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/glob.ori`
  - [ ] **LLVM Support**: LLVM codegen for glob
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — glob codegen

### 7D.8.8 Temporary Files

- [ ] **Implement**: `temp_dir() -> Path uses FileSystem`
  - [ ] **Rust Tests**: `library/std/fs/temp.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/temp.ori`
  - [ ] **LLVM Support**: LLVM codegen for temp_dir
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — temp_dir codegen

- [ ] **Implement**: `create_temp_file(prefix: str)` and `create_temp_dir(prefix: str)`
  - [ ] **Rust Tests**: `library/std/fs/temp.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/temp.ori`
  - [ ] **LLVM Support**: LLVM codegen for create_temp_file/create_temp_dir
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — temp file codegen

- [ ] **Implement**: `with_temp_file<T>(prefix, action)` and `with_temp_dir<T>(prefix, action)`
  - Auto-cleanup scoped temp files
  - [ ] **Rust Tests**: `library/std/fs/temp.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/temp.ori`
  - [ ] **LLVM Support**: LLVM codegen for with_temp_file/with_temp_dir
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — scoped temp codegen

### 7D.8.9 Permissions

- [ ] **Implement**: `get_permissions(path: str) -> Result<Permissions, FileError> uses FileSystem`
  - [ ] **Rust Tests**: `library/std/fs/permissions.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/permissions.ori`
  - [ ] **LLVM Support**: LLVM codegen for get_permissions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — get_permissions codegen

- [ ] **Implement**: `set_permissions(path: str, permissions: Permissions)`
  - [ ] **Rust Tests**: `library/std/fs/permissions.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/permissions.ori`
  - [ ] **LLVM Support**: LLVM codegen for set_permissions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — set_permissions codegen

- [ ] **Implement**: `set_readonly(path: str, readonly: bool)`
  - [ ] **Rust Tests**: `library/std/fs/permissions.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/permissions.ori`
  - [ ] **LLVM Support**: LLVM codegen for set_readonly
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — set_readonly codegen

### 7D.8.10 Path Utilities

- [ ] **Implement**: `cwd()` and `set_cwd(path: str)`
  - [ ] **Rust Tests**: `library/std/fs/path_utils.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/path_utils.ori`
  - [ ] **LLVM Support**: LLVM codegen for cwd/set_cwd
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — cwd codegen

- [ ] **Implement**: `canonicalize(path: str)` and `resolve(path: str)`
  - [ ] **Rust Tests**: `library/std/fs/path_utils.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/path_utils.ori`
  - [ ] **LLVM Support**: LLVM codegen for canonicalize/resolve
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — canonicalize codegen

- [ ] **Implement**: `relative(from: str, to: str) -> Result<Path, FileError>`
  - [ ] **Rust Tests**: `library/std/fs/path_utils.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/fs/path_utils.ori`
  - [ ] **LLVM Support**: LLVM codegen for relative
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/fs_tests.rs` — relative codegen

---

## 7D.9 std.crypto Module

**Proposal**: `proposals/approved/stdlib-crypto-api-proposal.md`
**FFI Implementation**: `proposals/approved/stdlib-crypto-ffi-native-proposal.md`

Cryptographic primitives including hashing, encryption, signatures, key exchange, and secure random.

**Backend**: libsodium (modern algorithms) + OpenSSL (RSA only)

### 7D.9.1 Core Types

- [ ] **Implement**: `HashAlgorithm` sum type
  - `Sha256 | Sha512 | Blake2b`
  - [ ] **Rust Tests**: `library/std/crypto/hash.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/hash.ori`
  - [ ] **LLVM Support**: LLVM codegen for HashAlgorithm
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — HashAlgorithm codegen

- [ ] **Implement**: `SecretKey` type — symmetric key with auto-zeroization
  - Fields: `bytes: [byte]`
  - Implements Drop with memory zeroization
  - [ ] **Rust Tests**: `library/std/crypto/symmetric.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/symmetric.ori`
  - [ ] **LLVM Support**: LLVM codegen for SecretKey
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — SecretKey codegen

- [ ] **Implement**: `CryptoError` and `CryptoErrorKind` types
  - `DecryptionFailed | InvalidKey | InvalidSignature | KeyDerivationFailed | RandomGenerationFailed | KeyExchangeFailed`
  - [ ] **Rust Tests**: `library/std/crypto/error.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/error.ori`
  - [ ] **LLVM Support**: LLVM codegen for CryptoError
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — CryptoError codegen

### 7D.9.2 Signing Key Types

- [ ] **Implement**: `SigningAlgorithm` sum type
  - `Ed25519 | Rsa2048 | Rsa4096`
  - [ ] **Rust Tests**: `library/std/crypto/signing.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/signing.ori`
  - [ ] **LLVM Support**: LLVM codegen for SigningAlgorithm
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — SigningAlgorithm codegen

- [ ] **Implement**: `SigningKeyPair`, `SigningPublicKey`, `SigningPrivateKey` types
  - Private key with auto-zeroization on drop
  - [ ] **Rust Tests**: `library/std/crypto/signing.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/signing.ori`
  - [ ] **LLVM Support**: LLVM codegen for signing key types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — signing key types codegen

### 7D.9.3 Encryption Key Types

- [ ] **Implement**: `EncryptionAlgorithm` sum type
  - `Rsa2048 | Rsa4096`
  - [ ] **Rust Tests**: `library/std/crypto/encryption.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/encryption.ori`
  - [ ] **LLVM Support**: LLVM codegen for EncryptionAlgorithm
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — EncryptionAlgorithm codegen

- [ ] **Implement**: `EncryptionKeyPair`, `EncryptionPublicKey`, `EncryptionPrivateKey` types
  - Private key with auto-zeroization on drop
  - [ ] **Rust Tests**: `library/std/crypto/encryption.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/encryption.ori`
  - [ ] **LLVM Support**: LLVM codegen for encryption key types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — encryption key types codegen

### 7D.9.4 Key Exchange Types

- [ ] **Implement**: `KeyExchangeAlgorithm` sum type
  - `X25519`
  - [ ] **Rust Tests**: `library/std/crypto/key_exchange.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/key_exchange.ori`
  - [ ] **LLVM Support**: LLVM codegen for KeyExchangeAlgorithm
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — KeyExchangeAlgorithm codegen

- [ ] **Implement**: `KeyExchangeKeyPair`, `KeyExchangePublicKey`, `KeyExchangePrivateKey` types
  - Private key with auto-zeroization on drop
  - [ ] **Rust Tests**: `library/std/crypto/key_exchange.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/key_exchange.ori`
  - [ ] **LLVM Support**: LLVM codegen for key exchange types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — key exchange types codegen

### 7D.9.5 Hashing API

- [ ] **Implement**: `hash(data: [byte], algorithm: HashAlgorithm = Sha256) -> [byte] uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/hash.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/hash.ori`
  - [ ] **LLVM Support**: LLVM codegen for hash
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — hash codegen

- [ ] **Implement**: `hash_hex(data: str, algorithm: HashAlgorithm = Sha256) -> str uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/hash.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/hash.ori`
  - [ ] **LLVM Support**: LLVM codegen for hash_hex
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — hash_hex codegen

- [ ] **Implement**: `hash_password(password: str) -> str uses Crypto`
  - Uses Argon2id with secure defaults
  - [ ] **Rust Tests**: `library/std/crypto/password.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/password.ori`
  - [ ] **LLVM Support**: LLVM codegen for hash_password
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — hash_password codegen

- [ ] **Implement**: `verify_password(password: str, hash: str) -> bool uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/password.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/password.ori`
  - [ ] **LLVM Support**: LLVM codegen for verify_password
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — verify_password codegen

### 7D.9.6 HMAC API

- [ ] **Implement**: `hmac(key: [byte], data: [byte], algorithm: HashAlgorithm = Sha256) -> [byte] uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/hmac.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/hmac.ori`
  - [ ] **LLVM Support**: LLVM codegen for hmac
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — hmac codegen

- [ ] **Implement**: `verify_hmac(key: [byte], data: [byte], mac: [byte], algorithm: HashAlgorithm = Sha256) -> bool uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/hmac.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/hmac.ori`
  - [ ] **LLVM Support**: LLVM codegen for verify_hmac
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — verify_hmac codegen

### 7D.9.7 Symmetric Encryption API

- [ ] **Implement**: `generate_key() -> SecretKey uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/symmetric.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/symmetric.ori`
  - [ ] **LLVM Support**: LLVM codegen for generate_key
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — generate_key codegen

- [ ] **Implement**: `encrypt(key: SecretKey, plaintext: [byte]) -> [byte] uses Crypto`
  - Uses XSalsa20-Poly1305 with random nonce (prepended to ciphertext)
  - [ ] **Rust Tests**: `library/std/crypto/symmetric.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/symmetric.ori`
  - [ ] **LLVM Support**: LLVM codegen for encrypt
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — encrypt codegen

- [ ] **Implement**: `decrypt(key: SecretKey, ciphertext: [byte]) -> Result<[byte], CryptoError> uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/symmetric.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/symmetric.ori`
  - [ ] **LLVM Support**: LLVM codegen for decrypt
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — decrypt codegen

- [ ] **Implement**: `encrypt_with_nonce(key, nonce, plaintext, aad)` and `decrypt_with_nonce(key, nonce, ciphertext, aad)`
  - [ ] **Rust Tests**: `library/std/crypto/symmetric.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/symmetric_nonce.ori`
  - [ ] **LLVM Support**: LLVM codegen for encrypt/decrypt with nonce
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — nonce API codegen

### 7D.9.8 Asymmetric Encryption API

- [ ] **Implement**: `generate_encryption_keypair(algorithm: EncryptionAlgorithm = Rsa2048) -> EncryptionKeyPair uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/encryption.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/encryption.ori`
  - [ ] **LLVM Support**: LLVM codegen for generate_encryption_keypair
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — generate_encryption_keypair codegen

- [ ] **Implement**: `encrypt_for(recipient: EncryptionPublicKey, plaintext: [byte]) -> [byte] uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/encryption.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/encryption.ori`
  - [ ] **LLVM Support**: LLVM codegen for encrypt_for
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — encrypt_for codegen

- [ ] **Implement**: `decrypt_with(key: EncryptionPrivateKey, ciphertext: [byte]) -> Result<[byte], CryptoError> uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/encryption.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/encryption.ori`
  - [ ] **LLVM Support**: LLVM codegen for decrypt_with
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — decrypt_with codegen

### 7D.9.9 Digital Signatures API

- [ ] **Implement**: `generate_signing_keypair(algorithm: SigningAlgorithm = Ed25519) -> SigningKeyPair uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/signing.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/signing.ori`
  - [ ] **LLVM Support**: LLVM codegen for generate_signing_keypair
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — generate_signing_keypair codegen

- [ ] **Implement**: `sign(key: SigningPrivateKey, data: [byte]) -> [byte] uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/signing.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/signing.ori`
  - [ ] **LLVM Support**: LLVM codegen for sign
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — sign codegen

- [ ] **Implement**: `verify_signature(key: SigningPublicKey, data: [byte], signature: [byte]) -> bool uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/signing.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/signing.ori`
  - [ ] **LLVM Support**: LLVM codegen for verify_signature
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — verify_signature codegen

### 7D.9.10 Key Exchange API

- [ ] **Implement**: `generate_key_exchange_keypair(algorithm: KeyExchangeAlgorithm = X25519) -> KeyExchangeKeyPair uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/key_exchange.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/key_exchange.ori`
  - [ ] **LLVM Support**: LLVM codegen for generate_key_exchange_keypair
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — generate_key_exchange_keypair codegen

- [ ] **Implement**: `derive_shared_secret(my_private: KeyExchangePrivateKey, their_public: KeyExchangePublicKey) -> [byte] uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/key_exchange.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/key_exchange.ori`
  - [ ] **LLVM Support**: LLVM codegen for derive_shared_secret
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — derive_shared_secret codegen

### 7D.9.11 Secure Random API

- [ ] **Implement**: `random_bytes(count: int) -> [byte] uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/random.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/random.ori`
  - [ ] **LLVM Support**: LLVM codegen for random_bytes
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — random_bytes codegen

- [ ] **Implement**: `random_int(min: int, max: int) -> int uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/random.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/random.ori`
  - [ ] **LLVM Support**: LLVM codegen for random_int
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — random_int codegen

- [ ] **Implement**: `random_uuid() -> str uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/random.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/random.ori`
  - [ ] **LLVM Support**: LLVM codegen for random_uuid
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — random_uuid codegen

### 7D.9.12 Key Derivation API

- [ ] **Implement**: `derive_key(password: str, salt: [byte], key_length: int = 32) -> [byte] uses Crypto`
  - [ ] **Rust Tests**: `library/std/crypto/kdf.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/kdf.ori`
  - [ ] **LLVM Support**: LLVM codegen for derive_key
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — derive_key codegen

- [ ] **Implement**: `stretch_key(input_key: [byte], info: [byte] = [], length: int = 32) -> [byte] uses Crypto`
  - Uses HKDF for key derivation
  - [ ] **Rust Tests**: `library/std/crypto/kdf.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/kdf.ori`
  - [ ] **LLVM Support**: LLVM codegen for stretch_key
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — stretch_key codegen

### 7D.9.13 Key Serialization

- [ ] **Implement**: `SecretKey.to_bytes()` and `SecretKey.from_bytes()`
  - [ ] **Rust Tests**: `library/std/crypto/serialization.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/serialization.ori`
  - [ ] **LLVM Support**: LLVM codegen for SecretKey serialization
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — SecretKey serialization codegen

- [ ] **Implement**: Public/private key PEM serialization
  - `to_pem()`, `from_pem()`, `to_encrypted_pem()`, `from_encrypted_pem()`
  - [ ] **Rust Tests**: `library/std/crypto/serialization.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/serialization.ori`
  - [ ] **LLVM Support**: LLVM codegen for PEM serialization
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — PEM serialization codegen

- [ ] **Implement**: Public/private key byte serialization
  - `to_bytes()`, `from_bytes()` for all key types
  - [ ] **Rust Tests**: `library/std/crypto/serialization.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/serialization.ori`
  - [ ] **LLVM Support**: LLVM codegen for byte serialization
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — byte serialization codegen

### 7D.9.14 Utilities

- [ ] **Implement**: `constant_time_eq(a: [byte], b: [byte]) -> bool uses Crypto`
  - Timing-attack resistant comparison
  - [ ] **Rust Tests**: `library/std/crypto/util.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/util.ori`
  - [ ] **LLVM Support**: LLVM codegen for constant_time_eq
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — constant_time_eq codegen

### 7D.9.15 Crypto Capability

- [ ] **Implement**: `Crypto` capability trait
  - Non-suspending capability for cryptographic operations
  - [ ] **Rust Tests**: `oric/src/capabilities/crypto.rs`
  - [ ] **Ori Tests**: `tests/spec/capabilities/crypto.ori`
  - [ ] **LLVM Support**: LLVM codegen for Crypto capability
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` — Crypto codegen

- [ ] **Implement**: `MockCrypto` for testing
  - Deterministic random, predictable outputs for test verification
  - [ ] **Rust Tests**: `library/std/crypto/mock.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/mock.ori`
  - [ ] **LLVM Support**: LLVM codegen for MockCrypto
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/crypto_tests.rs` — MockCrypto codegen

### 7D.9.16 Algorithm Deprecation

- [ ] **Implement**: Compiler warning for deprecated algorithms
  - `#allow(deprecated_algorithm)` to suppress
  - [ ] **Rust Tests**: `oric/src/diagnostics/deprecated.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/crypto/deprecation.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

---

## 7D.10 Phase Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all`
- [ ] **LLVM Support**: All LLVM codegen tests pass
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/` — full stdlib LLVM test coverage

**Exit Criteria**: Basic programs can use stdlib modules
