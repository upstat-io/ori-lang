# Proposal: Remove Dot Prefix from Named Arguments

**Status:** Draft
**Author:** Claude (with Eric)
**Created:** 2026-01-24

---

## Summary

Remove the `.` prefix from named arguments. Use `name: value` syntax instead of `.name: value`.

```ori
// Before
fetch_user(.id: 1)
send_email(
    .to: recipient,
    .subject: "Hello",
    .body: content,
)

// After
fetch_user(id: 1)
send_email(
    to: recipient,
    subject: "Hello",
    body: content,
)
```

---

## Motivation

### The Dot Adds Ceremony Without Clarity

```ori
print(.msg: "Hello")      // Verbose
len(.of: items)           // Ceremonial
fetch_user(.id: user_id)  // Noisy
```

The parameter name already provides clarity. The dot is visual noise that doesn't aid comprehension.

### The Colon Is Sufficient

In `name: value` syntax, the colon unambiguously separates:
- **Before colon:** parameter name
- **After colon:** value expression

```ori
fetch_user(id: user_id)
//         ^^  ^^^^^^^
//         |   └── value (expression)
//         └────── parameter name
```

No ori needed. Position relative to colon determines role.

### Familiar Syntax

`name: value` for named arguments is established in:
- **Swift:** `greet(name: "Alice", age: 30)`
- **Kotlin:** `greet(name = "Alice", age = 30)` (different separator, same idea)
- **Python kwargs:** `greet(name="Alice", age=30)`

Developers recognize this pattern instantly.

### Cleaner Function Names

With mandatory named parameters, function names can be simpler:

```ori
// Named params carry the semantics
fetch_user(id: 1)
fetch_user(email: "alice@example.com")
send_email(to: user, subject: "Hi")

// Without named params, function names must encode params
fetch_user_by_id(1)
fetch_user_by_email("alice@example.com")
send_email_to_user_with_subject(user, "Hi")
```

The call site `fetch_user(id: 1)` reads naturally: "fetch user, id 1."

---

## Design

### Grammar

```
named_arg := IDENTIFIER ':' expression
```

Simple. No dot prefix.

### Call Syntax by Function Type

| Function Type | Syntax | Example |
|---------------|--------|---------|
| Built-in conversions | Positional | `int(x)`, `str(value)` |
| Built-in functions | Positional | `print("Hello")`, `len(items)` |
| User-defined functions | Named required | `fetch_user(id: 1)` |
| Patterns (function_exp) | Named required | `map(over: items, transform: fn)` |

### Built-in Functions (Positional)

Common built-ins allow positional arguments — they're universal and obvious:

```ori
// Type conversions
int(x)
str(value)
float(n)
byte(c)

// Core functions
print("Hello, world!")
len(items)
is_empty(collection)
assert(x > 0)
assert_eq(result, expected)
```

### User-Defined Functions (Named Required)

User-defined functions require named arguments for self-documentation:

```ori
@fetch_user (id: UserId) -> Result<User, Error> = ...
@send_email (to: str, subject: str, body: str) -> Result<void, Error> = ...

// Calls
fetch_user(id: 42)
fetch_user(id: user_id)

send_email(
    to: recipient,
    subject: "Hello",
    body: content,
)
```

### Patterns (Named Required)

Patterns use named arguments for clarity:

```ori
map(over: items, transform: x -> x * 2)

filter(over: users, predicate: u -> u.active)

fold(
    over: numbers,
    init: 0,
    op: (acc, n) -> acc + n,
)

retry(
    op: fetch(url: "/api"),
    attempts: 3,
    backoff: exponential(base: 100ms, max: 5s),
)
```

### Formatting

**One rule: if it fits within line width, inline. Otherwise, stack.**

```ori
// Fits = inline
fetch_user(id: 1)
map(over: items, transform: x -> x * 2)
send_email(to: a, subject: b, body: c)
Point(x: 0, y: 0, z: 0)

// Doesn't fit = stack
send_email(
    to: recipient_email_address,
    subject: email_subject_line,
    body: generated_email_content,
)

retry(
    op: fetch(url: "/api/users", timeout: 30s),
    attempts: 3,
    backoff: exponential(base: 100ms, max: 5s),
)
```

No count-based rules. No special cases. The formatter measures and decides.

### No Shorthand

Shorthand syntax (`foo(x)` meaning `foo(x: x)`) is **not** supported:

```ori
let id = 42
fetch_user(id: id)    // Valid - explicit
fetch_user(id)        // ERROR - named argument required
```

**Rationale:** Shorthand appears self-documenting but isn't. The variable name `id` at the call site doesn't indicate which id or what kind. Explicit `id: value` always shows the mapping.

---

## Examples

### Simple Calls

```ori
// Built-ins - positional
print("Hello, world!")
len(items)
assert(x > 0)

// User functions - named
fetch_user(id: 1)
create_point(x: 0, y: 0)
```

### Complex Calls

```ori
@process_batch (ids: [UserId]) -> [Result<User, Error>] uses Http, Async =
    parallel(
        tasks: map(
            over: ids,
            transform: id -> retry(
                op: fetch_user(id: id),
                attempts: 3,
                backoff: exponential(base: 100ms, max: 5s),
            ),
        ),
        max_concurrent: 10,
        timeout: 60s,
    )
```

### Method Calls

```ori
user.update(name: "New Name", email: "new@example.com")

items.sort(by: x -> x.priority, descending: true)

result.map(transform: x -> x * 2)
```

### Tests

```ori
@test_fetch_user tests @fetch_user () -> void =
    with Http = MockHttp(responses: { "/users/1": json_user }) in
    run(
        let result = fetch_user(id: 1),
        assert_ok(result),
        assert_eq(result.unwrap().name, "Alice"),
    )
```

---

## Migration

### Automated Migration

The transformation is mechanical:

1. Find all `.identifier:` in call expressions
2. Remove the leading `.`

```bash
ori migrate remove-dot-prefix
```

### Example Diff

```diff
-fetch_user(.id: 1)
+fetch_user(id: 1)

-send_email(
-    .to: recipient,
-    .subject: "Hello",
-    .body: content,
-)
+send_email(
+    to: recipient,
+    subject: "Hello",
+    body: content,
+)

-map(
-    .over: items,
-    .transform: x -> x * 2,
-)
+map(
+    over: items,
+    transform: x -> x * 2,
+)
```

---

## Tradeoffs

| Cost | Mitigation |
|------|------------|
| `id: id` looks repetitive | Reads fine; explicit mapping is valuable |
| Less "ori-y" | `@` and `$` remain distinctive |
| No visual marker for param names | Position (before colon) is unambiguous |

### The `id: id` Case

```ori
fetch_user(id: id)
```

This looks repetitive but is actually clear:
- First `id` = parameter name (before colon)
- Second `id` = variable value (after colon)

The colon separates them. No ambiguity.

When names differ, it's even clearer:
```ori
fetch_user(id: user_id)
fetch_user(id: row.primary_key)
fetch_user(id: parse_id(input))
```

---

## Benefits

| Benefit | Description |
|---------|-------------|
| **Less noise** | No dot on every parameter |
| **Familiar** | Swift, Kotlin, Python developers know this |
| **Flexible formatting** | Inline or stacked based on length |
| **Clean function names** | Params carry semantics, not function names |
| **Simple grammar** | `name: value`, no prefix |
| **Consistent** | Same syntax as struct literals |

---

## Implementation

### Compiler Changes

1. **Parser:** Update `parse_named_arg` to expect `IDENTIFIER ':'` instead of `'.' IDENTIFIER ':'`
2. **Error messages:** Update to show new syntax in suggestions

### Formatter Changes

1. Remove dot emission
2. Apply width-only rule: inline if fits, stack if not
3. Remove "always stack" and count-based rules

### Files to Update

- `compiler/oric/src/parser/grammar/expr.rs` — Call argument parsing
- `compiler/oric/src/formatter/` — Formatting rules
- `docs/ori_lang/0.1-alpha/spec/` — Grammar and examples
- `CLAUDE.md` — Quick reference

### Migration Tool

```bash
ori migrate remove-dot-prefix [--dry-run] [path]
```

---

## Summary

Remove the `.` prefix from named arguments:

- `name: value` instead of `.name: value`
- Colon separates parameter name from value — no ambiguity
- Familiar syntax from Swift, Kotlin, Python
- Built-ins allow positional; user functions require named
- Width-only formatting: inline if fits, stack if not
- No shorthand — always explicit `name: value`

The dot was ceremony without purpose. The colon does the job.
