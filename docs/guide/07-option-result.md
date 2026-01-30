---
title: "Option and Result"
description: "Handle missing values with Option and failures with Result."
order: 7
part: "Error Handling & Safety"
---

# Option and Result

Ori has no `null` and no exceptions. This might sound limiting, but it's actually liberating — the type system tells you exactly what can go wrong, and the compiler ensures you handle it.

## The Problem with Null

In many languages, any reference might be null:

```javascript
// JavaScript
function getUser(id) {
  return users.find(u => u.id === id);  // might return undefined
}

let user = getUser(42);
console.log(user.name);  // CRASH if user is undefined
```

The type system doesn't tell you that `getUser` might not find anything. You have to read the documentation, study the implementation, or learn the hard way when your code crashes.

## Option: Values That Might Not Exist

In Ori, when something might not exist, the type says so:

```ori
@get_user (id: int) -> Option<User> = ...
```

`Option<T>` is a sum type with two variants:

```ori
type Option<T> = Some(T) | None
```

- `Some(value)` — a value exists
- `None` — no value exists

### Creating Options

```ori
let found = Some(42)                    // Has a value
let not_found: Option<int> = None       // No value

// Functions that might not return a value
@find_user (id: int) -> Option<User> = ...
@parse_int (s: str) -> Option<int> = ...
@first<T> (items: [T]) -> Option<T> = ...
```

### Pattern Matching with Option

The most direct way to handle `Option`:

```ori
let maybe_user = find_user(id: 42)

let message = match(
    maybe_user,
    Some(user) -> `Found: {user.name}`,
    None -> "User not found",
)
```

The compiler ensures you handle both cases. This won't compile:

```ori
// ERROR: non-exhaustive match
let message = match(
    maybe_user,
    Some(user) -> `Found: {user.name}`,
    // Forgot to handle None!
)
```

### Option Methods

**`is_some()` and `is_none()`** — check which variant:

```ori
let opt = Some(42)
is_some(option: opt)    // true
is_none(option: opt)    // false

let empty: Option<int> = None
is_some(option: empty)  // false
is_none(option: empty)  // true
```

**`unwrap_or()`** — get value or use default:

```ori
let value = Some(42).unwrap_or(default: 0)     // 42
let value = None.unwrap_or(default: 0)         // 0

// Real example
let user_name = find_user(id: 42)
    .map(transform: u -> u.name)
    .unwrap_or(default: "Anonymous")
```

**`map()`** — transform the value if it exists:

```ori
let maybe_num = Some(5)
let doubled = maybe_num.map(transform: x -> x * 2)  // Some(10)

let nothing: Option<int> = None
let still_nothing = nothing.map(transform: x -> x * 2)  // None
```

`map` is powerful because it lets you work with the value without manually matching:

```ori
// Without map (verbose)
let display = match(
    find_user(id: 42),
    Some(user) -> Some(`Name: {user.name}`),
    None -> None,
)

// With map (concise)
let display = find_user(id: 42).map(transform: u -> `Name: {u.name}`)
```

**`and_then()`** — chain operations that return Options:

```ori
@find_user (id: int) -> Option<User> = ...
@get_address (user: User) -> Option<Address> = ...

// Chain lookups
let address = find_user(id: 42)
    .and_then(transform: u -> get_address(user: u))
// Option<Address>
```

Without `and_then`, you'd need nested matches:

```ori
let address = match(
    find_user(id: 42),
    None -> None,
    Some(user) -> match(
        get_address(user: user),
        None -> None,
        Some(addr) -> Some(addr),
    ),
)
// Much more verbose!
```

**`filter()`** — keep value only if it matches a condition:

```ori
let positive = Some(5).filter(predicate: x -> x > 0)   // Some(5)
let filtered = Some(-3).filter(predicate: x -> x > 0)  // None
let nothing = None.filter(predicate: x -> x > 0)       // None
```

### The Coalesce Operator `??`

Use `??` as shorthand for "value or default":

```ori
let name = maybe_name ?? "Anonymous"
let count = parse_int(s: input) ?? 0
let config = load_config() ?? default_config
```

This is equivalent to `unwrap_or`:

```ori
maybe_name ?? "Anonymous"
maybe_name.unwrap_or(default: "Anonymous")
// Same result
```

### When to Use Option

Use `Option<T>` when:

- A value legitimately might not exist
- Absence is a normal, expected case
- The caller should decide what to do when there's no value

Examples:
- Looking up a user by ID (might not exist)
- Getting the first element of a list (might be empty)
- Parsing a string to a number (might be invalid)
- Finding an element that matches a condition (might not find one)

## Result: Operations That Can Fail

While `Option` represents "might not exist," `Result` represents "might fail."

```ori
type Result<T, E> = Ok(T) | Err(E)
```

- `Ok(value)` — operation succeeded
- `Err(error)` — operation failed

### Creating Results

```ori
let success: Result<int, str> = Ok(42)
let failure: Result<int, str> = Err("something went wrong")

// Functions that can fail
@read_file (path: str) -> Result<str, Error> uses FileSystem = ...
@parse_json (data: str) -> Result<Config, ParseError> = ...
@connect (url: str) -> Result<Connection, NetworkError> uses Http = ...
```

### Pattern Matching with Result

```ori
let result = read_file(path: "config.json")

match(
    result,
    Ok(content) -> process(data: content),
    Err(e) -> print(msg: `Error: {e.message}`),
)
```

### Result Methods

**`is_ok()` and `is_err()`** — check which variant:

```ori
let result = Ok(42)
is_ok(result: result)   // true
is_err(result: result)  // false
```

**`unwrap_or()`** — get value or use default:

```ori
let value = Ok(42).unwrap_or(default: 0)      // 42
let value = Err("oops").unwrap_or(default: 0) // 0
```

**`map()`** — transform the success value:

```ori
let result = Ok(5)
let doubled = result.map(transform: x -> x * 2)  // Ok(10)

let error: Result<int, str> = Err("failed")
let still_error = error.map(transform: x -> x * 2)  // Err("failed")
```

**`map_err()`** — transform the error:

```ori
let result: Result<int, str> = Err("raw error")
let wrapped = result.map_err(transform: e -> Error { message: e, code: 500 })
// Err(Error { message: "raw error", code: 500 })
```

**`and_then()`** — chain fallible operations:

```ori
@read_file (path: str) -> Result<str, Error> = ...
@parse_config (data: str) -> Result<Config, Error> = ...

let config = read_file(path: "config.json")
    .and_then(transform: data -> parse_config(data: data))
// Result<Config, Error>
```

**`ok()`** — convert to `Option` (discards error):

```ori
let result: Result<int, str> = Ok(42)
result.ok()    // Some(42)

let error: Result<int, str> = Err("failed")
error.ok()     // None
```

**`err()`** — get the error as `Option`:

```ori
let error: Result<int, str> = Err("failed")
error.err()    // Some("failed")

let success: Result<int, str> = Ok(42)
success.err()  // None
```

### The `??` Operator with Result

You can use `??` with `Result` too:

```ori
let value = risky_operation() ?? default_value
```

This extracts the `Ok` value or uses the default if it's an `Err`.

## Converting Between Option and Result

**`Option` to `Result`** — provide an error for the `None` case:

```ori
let maybe_user = find_user(id: 42)
let result = maybe_user.ok_or(error: Error { message: "User not found" })
// Result<User, Error>
```

**`Result` to `Option`** — discard the error:

```ori
let result = risky_operation()
let maybe = result.ok()  // Option<T>, error is lost
```

## Assertions

Ori provides assertions for testing Option and Result:

```ori
// Option assertions
assert_some(option: find_user(id: 1))
assert_none(option: find_user(id: -1))

// Result assertions
assert_ok(result: parse_int(text: "42"))
assert_err(result: parse_int(text: "not a number"))
```

## Common Patterns

### Safe Indexing

```ori
@safe_get<T> (items: [T], index: int) -> Option<T> =
    if index < 0 || index >= len(collection: items) then
        None
    else
        Some(items[index])
```

### Default on Error

```ori
let config = load_config().unwrap_or(default: Config.default())
let user = fetch_user(id: id).unwrap_or(default: guest_user)
```

### First Success

```ori
@try_sources (id: int) -> Option<Data> = run(
    // Try cache first
    let cached = cache_lookup(id: id),
    if is_some(option: cached) then return cached,

    // Try database
    let from_db = db_lookup(id: id),
    if is_some(option: from_db) then return from_db,

    // Try remote
    remote_lookup(id: id).ok(),
)
```

### Chaining Multiple Optionals

```ori
@get_user_city (id: int) -> Option<str> =
    find_user(id: id)
        .and_then(transform: u -> u.address)
        .and_then(transform: a -> a.city)
```

## Complete Example

```ori
type User = { id: int, name: str, email: str }
type ValidationError = { field: str, message: str }

// Parse ID from string
@parse_id (s: str) -> Option<int> =
    s as? int

@test_parse_id tests @parse_id () -> void = run(
    assert_eq(actual: parse_id(s: "42"), expected: Some(42)),
    assert_eq(actual: parse_id(s: "abc"), expected: None),
)

// Validate email format
@validate_email (email: str) -> Option<str> =
    if email.contains(substring: "@") then Some(email) else None

@test_validate_email tests @validate_email () -> void = run(
    assert_some(option: validate_email(email: "test@example.com")),
    assert_none(option: validate_email(email: "invalid")),
)

// Validate user data
@validate_user (name: str, email: str) -> Result<void, [ValidationError]> = run(
    let errors: [ValidationError] = [],

    let errors = if is_empty(collection: name) then
        [...errors, ValidationError { field: "name", message: "Name required" }]
    else errors,

    let errors = if is_none(option: validate_email(email: email)) then
        [...errors, ValidationError { field: "email", message: "Invalid email" }]
    else errors,

    if is_empty(collection: errors) then Ok(()) else Err(errors),
)

@test_validate_user tests @validate_user () -> void = run(
    assert_ok(result: validate_user(name: "Alice", email: "a@b.com")),
    assert_err(result: validate_user(name: "", email: "invalid")),
)

// Process user request
@process_request (id_str: str) -> str = run(
    let id = parse_id(s: id_str),

    match(
        id,
        None -> "Invalid ID format",
        Some(id) -> match(
            find_user(id: id),
            None -> `User {id} not found`,
            Some(user) -> `Found: {user.name}`,
        ),
    ),
)

// Simulated user lookup
@find_user (id: int) -> Option<User> = match(
    id,
    1 -> Some(User { id: 1, name: "Alice", email: "alice@example.com" }),
    2 -> Some(User { id: 2, name: "Bob", email: "bob@example.com" }),
    _ -> None,
)

@test_find_user tests @find_user () -> void = run(
    assert_some(option: find_user(id: 1)),
    assert_none(option: find_user(id: 999)),
)

@test_process_request tests @process_request () -> void = run(
    assert_eq(actual: process_request(id_str: "abc"), expected: "Invalid ID format"),
    assert_eq(actual: process_request(id_str: "1"), expected: "Found: Alice"),
    assert_eq(actual: process_request(id_str: "999"), expected: "User 999 not found"),
)
```

## Quick Reference

### Option

```ori
type Option<T> = Some(T) | None

// Create
let some = Some(42)
let none: Option<int> = None

// Check
is_some(option: opt), is_none(option: opt)

// Extract
opt.unwrap_or(default: val)
opt ?? default

// Transform
opt.map(transform: fn)
opt.and_then(transform: fn)
opt.filter(predicate: fn)

// Convert
opt.ok_or(error: err)     // -> Result
```

### Result

```ori
type Result<T, E> = Ok(T) | Err(E)

// Create
let ok = Ok(42)
let err = Err("failed")

// Check
is_ok(result: res), is_err(result: res)

// Extract
res.unwrap_or(default: val)
res ?? default

// Transform
res.map(transform: fn)
res.map_err(transform: fn)
res.and_then(transform: fn)

// Convert
res.ok()                  // -> Option<T>
res.err()                 // -> Option<E>
```

## What's Next

Now that you understand Option and Result:

- **[Error Propagation](/guide/08-error-propagation)** — The `?` operator and error traces
- **[Panic and Recovery](/guide/09-panic-recovery)** — Unrecoverable errors and contracts
