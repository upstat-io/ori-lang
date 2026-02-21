# std.testing

Testing utilities and assertions.

```ori
use std.testing { assert, assert_eq, assert_ne, expect_panic }
```

**No capability required** (for pure assertions)

---

## Overview

The `std.testing` module provides:

- Enhanced assertions with detailed messages
- Test fixtures and setup/teardown
- Mocking utilities
- Property-based testing

> **Note:** Basic `assert` and `assert_eq` are in `std`, but `std.testing` provides enhanced versions with better error messages.

---

## Assertions

### @assert

```ori
@assert (condition: bool) -> void
@assert (condition: bool, message: str) -> void
```

Asserts condition is true.

```ori
use std.testing { assert }

assert(result.is_ok())
assert(value > 0, "value must be positive")
```

---

### @assert_eq

```ori
@assert_eq<T: Eq + Printable> (actual: T, expected: T) -> void
@assert_eq<T: Eq + Printable> (actual: T, expected: T, message: str) -> void
```

Asserts values are equal with detailed diff.

```ori
use std.testing { assert_eq }

assert_eq(
    .actual: calculate(5),
    .expected: 25,
)
// Failure:
// expected: 25
// actual:   24
```

---

### @assert_ne

```ori
@assert_ne<T: Eq + Printable> (actual: T, unexpected: T) -> void
```

Asserts values are not equal.

```ori
use std.testing { assert_ne }

assert_ne(
    .actual: user.id,
    .unexpected: 0,
)
```

---

### @assert_approx

```ori
@assert_approx (actual: float, expected: float, epsilon: float) -> void
```

Asserts floats are approximately equal.

```ori
use std.testing { assert_approx }

assert_approx(
    .actual: calculate_pi(),
    .expected: 3.14159,
    .epsilon: 0.00001,
)
```

---

### @assert_contains

```ori
@assert_contains<T: Eq> (collection: [T], element: T) -> void
@assert_contains (haystack: str, needle: str) -> void
```

Asserts collection contains element.

```ori
use std.testing { assert_contains }

assert_contains(
    .collection: result,
    .element: expected_item,
)
assert_contains(
    .haystack: output,
    .needle: "success",
)
```

---

### @assert_matches

```ori
@assert_matches (value: str, pattern: str) -> void
```

Asserts string matches regex pattern.

```ori
use std.testing { assert_matches }

assert_matches(
    .value: email,
    .pattern: r"^[\w.]+@[\w.]+\.\w+$",
)
```

---

## Error Testing

### @expect_panic

```ori
@expect_panic (f: () -> T) -> void
@expect_panic (f: () -> T, message: str) -> void
```

Asserts that function panics.

```ori
use std.testing { expect_panic }

expect_panic(() -> panic("error"))
expect_panic(
    .f: () -> divide(
        .a: 1,
        .b: 0,
    ),
    .message: "division by zero",
)
```

---

### @expect_err

```ori
@expect_err<T, E> (result: Result<T, E>) -> E
```

Asserts result is Err and returns the error.

```ori
use std.testing { expect_err }

let err = expect_err(parse_int("abc"))
assert_eq(
    .actual: err.message,
    .expected: "invalid integer",
)
```

---

### @expect_ok

```ori
@expect_ok<T, E> (result: Result<T, E>) -> T
```

Asserts result is Ok and returns the value.

```ori
use std.testing { expect_ok }

let value = expect_ok(parse_int("42"))
assert_eq(
    .actual: value,
    .expected: 42,
)
```

---

## Test Fixtures

### @before_each / @after_each

```ori
@before_each (setup: () -> T) -> T
@after_each (teardown: T -> void) -> void
```

Setup and teardown for tests.

```ori
use std.testing { before_each, after_each }

let db = before_each(() -> create_test_db())
after_each(db -> db.drop())

@test_user_creation tests @create_user () -> void = {
    let user = create_user(
        .db: db,
        .name: "Alice",
    )?
    assert_eq(
        .actual: user.name,
        .expected: "Alice",
    )
}
```

---

## Mocking

### Mock<T>

```ori
type Mock<T>
```

A mock that records calls and returns configured values.

```ori
use std.testing { Mock }

let http_mock = Mock<HttpClient>.new()
http_mock.when(c -> c.get("https://api.example.com"))
         .returns(Ok(Response { status: 200, body: "{}" }))

let service = Service.new(http_mock)
let result = service.fetch_data()

assert(http_mock.was_called())
assert_eq(
    .actual: http_mock.call_count(),
    .expected: 1,
)
```

**Methods:**
- `new() -> Mock<T>` — Create mock
- `when(matcher: T -> bool) -> MockConfig` — Configure behavior
- `returns(value: R)` — Set return value
- `was_called() -> bool` — Check if called
- `call_count() -> int` — Number of calls
- `calls() -> [CallRecord]` — All recorded calls

---

## Property Testing

### @property

```ori
@property (name: str, test: T -> bool) -> void
```

Property-based test that generates random inputs.

```ori
use std.testing { property }

@test_reverse_twice tests @reverse () -> void =
    property("reverse twice is identity", (s: str) ->
        reverse(reverse(s)) == s
    )

@test_sort_idempotent tests @sort () -> void =
    property("sort is idempotent", (arr: [int]) ->
        sort(sort(arr)) == sort(arr)
    )
```

---

## Examples

### Comprehensive test

```ori
use std.testing { assert_eq, assert_ne, expect_err, expect_ok }

@test_user_validation tests @validate_user () -> void = {
    // Valid user
    let valid = User { name: "Alice", age: 30 }
    let result = expect_ok(validate_user(valid))
    assert_eq(
        .actual: result.name,
        .expected: "Alice",
    )

    // Invalid: empty name
    let invalid = User { name: "", age: 30 }
    let err = expect_err(validate_user(invalid))
    assert_eq(
        .actual: err,
        .expected: ValidationError.EmptyName,
    )

    // Invalid: negative age
    let invalid2 = User { name: "Bob", age: -1 }
    let err2 = expect_err(validate_user(invalid2))
    assert_eq(
        .actual: err2,
        .expected: ValidationError.InvalidAge(-1),
    )
}
```

### Testing with mocks

```ori
use std.testing { Mock, assert_eq }

@test_email_service tests @send_welcome () -> void = {
    let mailer = Mock<Mailer>.new()
    mailer.when(m -> true).returns(Ok(()))

    let service = UserService.new(mailer)
    service.send_welcome("alice@example.com")?

    assert(mailer.was_called())
    let call = mailer.calls()[0]
    assert_eq(
        .actual: call.args.to,
        .expected: "alice@example.com",
    )
}
```

---

## See Also

- [Mandatory Testing](../../spec/13-testing.md) — Test requirements
- [Design: Testing](../../design/11-testing/) — Testing philosophy
