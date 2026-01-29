---
title: "Declarations"
description: "Ori Formatter Design — Declaration Formatting"
order: 2
---

# Declarations

Formatting rules for top-level declarations: functions, constants, types, traits, implementations, and tests.

## Functions

### Inline Format

Used when the complete signature fits in 100 characters:

```ori
@add (a: int, b: int) -> int = a + b

@greet (name: str) -> str = "Hello, " + name + "!"

@transform (user_id: int, transform: (User) -> User) -> Result<User, Error> = do_work()
```

### Broken Parameters

When the signature exceeds 100 characters, break parameters one per line:

```ori
@send_notification_to_user (
    user_id: int,
    notification: Notification,
    preferences: NotificationPreferences,
    retry_config: RetryConfig,
) -> Result<void, NotificationError> = do_notify()
```

### Broken Return Type

If the `) -> ReturnType =` line still exceeds 100 after breaking parameters, break the return type.

**Body placement**: Body on same line if it fits, otherwise indented on next line.

```ori
// Body fits on return type line
@long_function_name (
    first: int,
    second: str,
) -> Result<HashMap<UserId, Preferences>, ServiceError> = do_work()

// Body exceeds 100, indent to next line
@very_long_function_name (
    first_parameter: int,
    second_parameter: str,
) -> Result<HashMap<UserId, NotificationPreferences>, NotificationServiceError> =
    compute_something_complex(input: data)
```

### Generics

Inline if they fit:

```ori
@identity<T> (x: T) -> T = x

@transform<T, U, V> (a: T, b: U, c: V) -> Result<V, Error> = do_it()
```

Break if exceeding 100 characters:

```ori
@complex_transform<
    InputType,
    OutputType,
    ErrorType,
    ConfigurationType,
> (input: InputType) -> Result<OutputType, ErrorType> = do_work()
```

### Where Clauses

Inline if short:

```ori
@sort<T> (items: [T]) -> [T] where T: Comparable = do_sort()
```

Break with `where` on new line if multiple or long:

```ori
@process<T, U> (items: [T], f: (T) -> U) -> [U]
    where T: Clone + Debug,
          U: Default + Printable = do_it()
```

### Capabilities

Inline if they fit:

```ori
@fetch (url: str) -> Result<str, Error> uses Http = http_get(url)

@fetch_and_log (url: str) -> Result<str, Error> uses Http, Logger = do_work()
```

Break `uses` to new line if exceeding 100:

```ori
@complex_operation (input: Data) -> Result<Output, Error>
    uses Http, FileSystem, Logger, Cache = do_it()
```

### Function Clauses

Multiple clauses for pattern matching in parameters:

```ori
@factorial (0: int) -> int = 1
@factorial (n) -> int = n * factorial(n - 1)

@fib (0: int) -> int = 0
@fib (1: int) -> int = 1
@fib (n) -> int = fib(n - 1) + fib(n - 2)
```

Each clause follows the same breaking rules independently.

## Constants

Constants at module level use `let $name`:

```ori
let $timeout = 30
let $max_retries = 3
let $api_url = "https://api.example.com"
```

Group related constants with blank lines between groups:

```ori
let $api_base = "https://api.example.com"
let $api_version = "v1"

let $timeout = 30s
let $max_retries = 3

let $debug_mode = false
```

## Type Definitions

### Structs

Inline if fits:

```ori
type Point = { x: int, y: int }

type User = { id: int, name: str, email: str }
```

Break fields one per line if exceeding 100:

```ori
type UserProfile = {
    id: int,
    name: str,
    email: str,
    created_at: Timestamp,
    preferences: UserPreferences,
}
```

### Sum Types

Inline if fits:

```ori
type Color = Red | Green | Blue

type Status = Pending | Running | Complete | Failed

type Result<T, E> = Ok(value: T) | Err(error: E)
```

Break variants one per line with leading `|` if exceeding 100:

```ori
type Event =
    | Click(x: int, y: int)
    | KeyPress(key: char, modifiers: Modifiers)
    | Scroll(delta_x: float, delta_y: float, source: ScrollSource)
```

### Type Aliases

```ori
type UserId = int
type UserMap = {int: User}
```

### Attributes

Attributes appear on their own line above the type:

```ori
#derive(Eq, Clone, Debug)
type Point = { x: int, y: int }

#derive(Eq, Clone)
#deprecated("use NewType instead")
type OldType = { value: int }
```

## Traits

Opening brace on same line, methods indented:

```ori
trait Printable {
    @to_str (self) -> str
}

trait Default {
    @default () -> Self
}
```

Multiple methods separated by blank line (except single-method blocks):

```ori
// Single method - no blank line
trait Printable {
    @to_str (self) -> str
}

// Multiple methods - blank lines between
trait Iterator {
    type Item

    @next (self) -> (Option<Self.Item>, Self)

    @count (self) -> int = run(
        let (item, rest) = self.next(),
        match(item,
            None -> 0,
            Some(_) -> 1 + rest.count(),
        ),
    )
}
```

Trait inheritance:

```ori
trait DoubleEndedIterator: Iterator {
    @next_back (self) -> (Option<Self.Item>, Self)
}
```

## Implementations

### Trait Implementations

```ori
impl Printable for Point {
    @to_str (self) -> str = `({self.x}, {self.y})`
}
```

### Inherent Implementations

```ori
impl Point {
    @new (x: int, y: int) -> Point = Point { x, y }

    @distance (self, other: Point) -> float = run(
        let dx = self.x - other.x,
        let dy = self.y - other.y,
        sqrt(float(dx * dx + dy * dy)),
    )

    @translate (self, dx: int, dy: int) -> Point =
        Point { x: self.x + dx, y: self.y + dy }
}
```

### Generic Implementations

```ori
impl<T: Clone> Clone for Option<T> {
    @clone (self) -> Self = match(self,
        Some(value) -> Some(value.clone()),
        None -> None,
    )
}
```

## Tests

Tests follow function formatting rules. The `tests` clause stays with the signature:

```ori
@test_add tests @add () -> void = run(
    assert_eq(actual: add(a: 1, b: 2), expected: 3),
    assert_eq(actual: add(a: -1, b: 1), expected: 0),
)

@test_math tests @add tests @subtract () -> void = run(
    assert_eq(actual: add(a: 2, b: 2), expected: 4),
    assert_eq(actual: subtract(a: 5, b: 3), expected: 2),
)

@test_integration tests _ () -> void = run(
    let user = create_test_user(),
    let result = process_user(user),
    assert_ok(result: result),
)
```

### Test Attributes

```ori
#skip("not implemented yet")
@test_future tests @future_feature () -> void = run(
    assert(condition: false),
)

#compile_fail("expected type error")
@test_type_error tests _ () -> void = run(
    let x: int = "not an int",
)
```

## Imports

Stdlib imports first, then relative imports, separated by blank line:

```ori
use std.collections { HashMap, Set }
use std.math { abs, sqrt }
use std.time { Duration }

use "../utils" { format }
use "./helpers" { compute, validate }
use "./local" { helper }
```

Items sorted alphabetically:

```ori
use std.math { abs, cos, sin, sqrt, tan }
```

Break to multiple lines if exceeding 100:

```ori
use std.collections {
    BTreeMap,
    BTreeSet,
    HashMap,
    HashSet,
    LinkedList,
}
```

Module aliases:

```ori
use std.net.http as http
use "./internal" { LongTypeName as Short }
```
