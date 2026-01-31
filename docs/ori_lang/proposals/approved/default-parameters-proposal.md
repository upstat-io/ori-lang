# Proposal: Default Parameter Values

**Status:** Approved
**Author:** Eric
**Created:** 2026-01-25
**Approved:** 2026-01-28

---

## Summary

Allow function parameters to specify default values, enabling callers to omit arguments.

```ori
@greet (name: str = "World") -> str = `Hello, {name}!`

greet()               // "Hello, World!"
greet(name: "Alice")  // "Hello, Alice!"
```

---

## Motivation

### The Problem

Currently, every parameter must be provided at every call site:

```ori
@fetch (url: str, timeout: Duration, retries: int, verbose: bool) -> Result<Response, Error>

// Every call must specify everything
fetch(url: "/api", timeout: 30s, retries: 3, verbose: false)
fetch(url: "/other", timeout: 30s, retries: 3, verbose: false)
fetch(url: "/another", timeout: 30s, retries: 3, verbose: false)
```

Common workarounds are verbose:

**Wrapper functions:**
```ori
@fetch_default (url: str) -> Result<Response, Error> =
    fetch(url: url, timeout: 30s, retries: 3, verbose: false)
```

**Option parameters:**
```ori
@fetch (url: str, timeout: Option<Duration>, retries: Option<int>) -> Result<Response, Error> = run(
    let t = timeout.unwrap_or(default: 30s),
    let r = retries.unwrap_or(default: 3),
    // ...
)
```

Both add boilerplate and obscure the API.

### Prior Art

| Language | Syntax | Notes |
|----------|--------|-------|
| Python | `def f(x=1):` | Positional defaults |
| JavaScript | `function f(x = 1)` | ES6 defaults |
| Kotlin | `fun f(x: Int = 1)` | Named + defaults |
| Swift | `func f(x: Int = 1)` | Named + defaults |
| C++ | `void f(int x = 1)` | Trailing defaults only |
| Rust | None | No default parameters |
| Go | None | No default parameters |

### The Ori Way

Ori already uses named arguments, making defaults natural:

```ori
@fetch (url: str, timeout: Duration = 30s, retries: int = 3) -> Result<Response, Error>

fetch(url: "/api")                        // uses all defaults
fetch(url: "/api", retries: 5)            // override one
fetch(url: "/api", timeout: 60s, retries: 5)  // override both
```

Named arguments mean any defaulted parameter can be omitted, not just trailing ones.

---

## Design

### Syntax

The `param` production in `grammar.ebnf` is extended:

```ebnf
param         = identifier ":" type [ "=" expression ] .
```

Parameters may have a default value expression after their type. The default expression follows the same precedence rules as any other expression.

### Basic Usage

```ori
@greet (name: str = "World") -> str = `Hello, {name}!`

greet()               // "Hello, World!"
greet(name: "Alice")  // "Hello, Alice!"
```

### Multiple Defaults

```ori
@connect (
    host: str,
    port: int = 8080,
    timeout: Duration = 30s,
    retries: int = 3,
) -> Connection

connect(host: "localhost")
connect(host: "localhost", port: 3000)
connect(host: "localhost", timeout: 60s)  // skip port, override timeout
connect(host: "localhost", retries: 5, port: 9000)  // any order
```

### Non-Trailing Defaults

Unlike C++, defaults can appear on any parameter (thanks to named arguments):

```ori
@format (
    template: str = "{}",
    value: int,
    width: int = 0,
) -> str

format(value: 42)                          // uses default template and width
format(template: "Value: {}", value: 42)   // override template
format(value: 42, width: 10)               // override width
```

### Default Expressions

Defaults can be any expression that:
1. Is valid at the function definition site
2. Has the correct type
3. Contains no references to other parameters

```ori
// Literals
@f (x: int = 42) -> int

// Constants
let $default_timeout = 30s
@fetch (timeout: Duration = $default_timeout) -> Response

// Function calls (evaluated at call time)
@log (timestamp: Time = now()) -> void

// Expressions
@paginate (page_size: int = $default_page_size * 2) -> [Item]
```

### Evaluation Timing

Default expressions are evaluated **at call time**, not definition time:

```ori
@log (timestamp: Time = now()) -> void =
    print(msg: `[{timestamp}] Log entry`)

log()  // Uses current time
// ... wait ...
log()  // Uses new current time
```

This matches Python/JavaScript behavior and is usually what users expect.

### Evaluation Order

When a function is called:

1. Explicitly provided arguments are evaluated in **written order** (left-to-right as they appear at the call site)
2. Default expressions for omitted parameters are evaluated in **parameter declaration order**
3. The function body then executes

```ori
@f (a: int = default_a(), b: int, c: int = default_c()) -> int

// Call: f(c: expr_c(), b: expr_b())
// Evaluation order:
//   1. expr_c()      (first written argument)
//   2. expr_b()      (second written argument)
//   3. default_a()   (first parameter with default, was omitted)
// Note: default_c() is NOT evaluated because c was provided
```

### Required After Default

A parameter without a default following one with a default is allowed (unlike some languages):

```ori
@process (
    prefix: str = "",
    data: str,           // Required, no default
    suffix: str = "",
) -> str

process(data: "hello")                        // OK
process(prefix: "> ", data: "hello")          // OK
process(data: "hello", suffix: "!")           // OK
```

Named arguments make this unambiguous.

---

## Examples

### HTTP Client

```ori
@request (
    method: str,
    url: str,
    headers: {str: str} = {},
    body: Option<str> = None,
    timeout: Duration = 30s,
    follow_redirects: bool = true,
) -> Result<Response, Error> uses Http

// Simple GET
request(method: "GET", url: "/users")

// POST with body
request(method: "POST", url: "/users", body: Some(user_json))

// Custom timeout
request(method: "GET", url: "/slow", timeout: 120s)
```

### Builder-Style APIs

```ori
@create_user (
    name: str,
    email: str,
    role: str = "user",
    active: bool = true,
    verified: bool = false,
    created_at: Time = now(),
) -> User

// Minimal
create_user(name: "Alice", email: "alice@example.com")

// With overrides
create_user(
    name: "Bob",
    email: "bob@example.com",
    role: "admin",
    verified: true,
)
```

### Logging

```ori
@log (
    message: str,
    level: str = "INFO",
    timestamp: Time = now(),
    context: {str: str} = {},
) -> void uses Logger

log(message: "Server started")
log(message: "Request failed", level: "ERROR")
log(message: "User login", context: {"user_id": "123"})
```

### Pagination

```ori
@list_items (
    filter: Option<str> = None,
    page: int = 1,
    page_size: int = 20,
    sort_by: str = "created_at",
    sort_order: str = "desc",
) -> [Item]

list_items()
list_items(filter: Some("active"))
list_items(page: 2, page_size: 50)
list_items(sort_by: "name", sort_order: "asc")
```

### Testing Utilities

```ori
@create_test_user (
    id: int = 1,
    name: str = "Test User",
    email: str = "test@example.com",
    active: bool = true,
) -> User

// Tests can override just what they care about
@test_inactive_user () -> void = run(
    let user = create_test_user(active: false),
    assert(!user.active),
)

@test_specific_id () -> void = run(
    let user = create_test_user(id: 42),
    assert_eq(actual: user.id, expected: 42),
)
```

---

## Design Rationale

### Why Allow Non-Trailing Defaults?

C++ requires defaults only on trailing parameters:
```cpp
void f(int a = 1, int b);  // Error in C++
```

Ori allows this because named arguments make it unambiguous:
```ori
@f (a: int = 1, b: int) -> int
f(b: 5)  // Clear: a uses default, b is 5
```

### Why Evaluate at Call Time?

Alternatives:
1. **Definition time** (like Python's mutable default gotcha)
2. **Call time** (like JavaScript/Kotlin)

Call-time evaluation:
- Avoids mutable default surprises
- Allows `now()`, `random()`, etc. as defaults
- Matches user expectations

```ori
// Safe: new list created each call
@append (items: [int] = []) -> [int]

// Useful: current time each call
@log (timestamp: Time = now()) -> void
```

### Why Not Function Overloading?

Some languages use overloading instead of defaults:
```java
void log(String msg) { log(msg, "INFO"); }
void log(String msg, String level) { ... }
```

Problems:
- Combinatorial explosion with multiple optional parameters
- Duplicated logic or delegation chains
- Harder to see all options at once

Defaults are more concise and self-documenting.

### Interaction with Named Arguments

Ori's named arguments make defaults more powerful than in positional languages:

```ori
@f (a: int = 1, b: int = 2, c: int = 3) -> int

// Can override any subset in any order
f()
f(b: 20)
f(c: 30, a: 10)
f(b: 20, a: 10, c: 30)
```

In positional languages, you'd need sentinel values or overloads.

---

## Edge Cases

### Default References Other Parameters

Not allowed — defaults are evaluated before binding:

```ori
@f (a: int, b: int = a * 2) -> int  // Error: cannot reference 'a' in default

// Use explicit logic instead:
@f (a: int, b: Option<int> = None) -> int = run(
    let actual_b = b.unwrap_or(default: a * 2),
    a + actual_b,
)
```

### Mutable Default Values

Works correctly because defaults are evaluated each call:

```ori
@append (item: int, list: [int] = []) -> [int] =
    [...list, item]

append(item: 1)  // [1]
append(item: 2)  // [2], not [1, 2]
```

### Capabilities in Defaults

Defaults can use capabilities if the function declares them:

```ori
@fetch (url: str, timestamp: Time = Clock.now()) -> Response uses Http, Clock

// Clock capability required because default uses it
```

**Important**: The function must declare all capabilities used by any default expression, even if the caller provides that argument explicitly. The capability requirement is determined statically from the function signature, not dynamically per call site.

```ori
@fetch (url: str, timestamp: Time = Clock.now()) -> Response uses Http, Clock

// Both calls require Clock capability to be available, even though
// the second call doesn't actually use the default:
fetch(url: "/api")                              // uses Clock.now()
fetch(url: "/api", timestamp: fixed_time)       // still requires Clock
```

This keeps capability checking simple and predictable.

### Async in Defaults

Default expressions may use `Async` operations if the function declares `uses Suspend`:

```ori
@process (config: Config = load_config()?) -> Result<Output, Error> uses Suspend, FileSystem

// load_config() may suspend; function must declare `uses Suspend`
```

The same static requirement rule applies: the function must declare `uses Suspend` if any default expression may suspend, regardless of whether that default is used at a particular call site.

### Generic Functions

Defaults work with generics:

```ori
@get_or<T> (opt: Option<T>, default: T = T.default()) -> T
    where T: Default =
    opt.unwrap_or(default: default)
```

The default expression `T.default()` calls the `default` method on the type parameter's `Default` trait implementation.

### Trait Method Defaults

Default parameter values are allowed in trait method signatures:

```ori
trait Configurable {
    @configure (self, options: Options = Options.default()) -> void
}
```

**Rules:**

1. Implementations may keep the same default, provide a different default, or remove the default (making the parameter required)
2. If an implementation removes the default, callers through that concrete type must provide the argument
3. Callers through trait objects (`dyn Trait`) use the trait's declared default

```ori
impl Configurable for Widget {
    // Override with different default
    @configure (self, options: Options = widget_defaults()) -> void = ...
}

impl Configurable for Button {
    // Remove default — callers must provide options
    @configure (self, options: Options) -> void = ...
}

let w: Widget = ...
w.configure()                    // uses widget_defaults()

let b: Button = ...
b.configure()                    // Error: missing argument 'options'
b.configure(options: opts)       // OK

let d: dyn Configurable = ...
d.configure()                    // uses Options.default() (trait default)
```

---

## Implementation Notes

### Parser Changes

Extend parameter parsing to accept `= expression` after type annotation.

### Type Checking

- Verify default expression has parameter's type
- Verify default doesn't reference other parameters
- Track which parameters have defaults for call validation

### Call Site Validation

At call sites:
- Required parameters (no default) must be provided
- Parameters with defaults are optional
- Duplicate arguments are still an error

### Code Generation

```ori
@f (a: int, b: int = 10) -> int = a + b

f(a: 5)

// Desugars to:
f(a: 5, b: 10)
```

Insert default expressions for omitted arguments before evaluation.

### Default Expression Capture

For closures/capabilities in defaults, capture at call time:

```ori
@log (time: Time = now()) -> void uses Clock
//            ^^^^^ Clock.now() evaluated when log() is called
```

---

## Summary

| Feature | Behavior |
|---------|----------|
| Syntax | `param: Type = default_expr` |
| Evaluation | At call time |
| Position | Any parameter (not just trailing) |
| Omission | Any defaulted param can be omitted |
| References | Cannot reference other parameters |

Default parameters reduce boilerplate, make APIs more ergonomic, and work naturally with Ori's named argument system.
