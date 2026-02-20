# Proposal: Standard Library Validate API

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-31
**Affects:** Standard library, `std.validate` module

---

## Summary

This proposal defines the `std.validate` module, providing declarative validation with error accumulation. Unlike fail-fast validation that stops at the first error, this module collects all validation failures, enabling comprehensive error reporting for forms, API inputs, and configuration files.

---

## Motivation

### The Problem

Validation is ubiquitous but often poorly implemented:

```ori
// Anti-pattern: fail-fast validation loses information
@validate_user (input: UserInput) -> Result<User, str> =
    if input.name.is_empty() then Err("name is required")
    else if input.age < 0 then Err("age must be non-negative")
    else if input.email.is_empty() then Err("email is required")
    else Ok(User { name: input.name, age: input.age, email: input.email })
```

Problems:
- **Fail-fast**: User only sees first error, must fix and resubmit repeatedly
- **No accumulation**: Cannot show all errors at once
- **Boilerplate**: Nested if-else chains are verbose and error-prone
- **Hard to compose**: Adding new validations requires restructuring

### Real-World Need

Forms and APIs need to show ALL errors:

```
Validation failed:
  - name: required
  - age: must be non-negative
  - email: invalid format
  - password: must be at least 8 characters
```

### The Solution

A declarative validation API that accumulates all errors:

```ori
use std.validate { validate }

@validate_user (input: UserInput) -> Result<User, [str]> =
    validate(
        rules: [
            (input.name.is_empty(), "name is required"),
            (input.age < 0, "age must be non-negative"),
            (input.email.is_empty(), "email is required"),
        ],
        value: User { name: input.name, age: input.age, email: input.email },
    )
```

---

## Design Principles

Following [stdlib-philosophy-proposal.md](../approved/stdlib-philosophy-proposal.md):

1. **Pure Ori implementation**: No FFI needed
2. **Declarative**: Rules as data, not control flow
3. **Accumulating**: All errors collected, not fail-fast
4. **Composable**: Validations can be combined
5. **No capabilities required**: Pure function

---

## API Design

### validate Function

The core validation function:

```ori
@validate<T> (
    rules: [(bool, str)],
    value: T,
) -> Result<T, [str]>
```

#### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `rules` | `[(bool, str)]` | List of (failure_condition, error_message) pairs |
| `value` | `T` | Value to return if all validations pass |

#### Semantics

1. Evaluate all rule conditions (not short-circuiting)
2. Collect error messages for all rules where condition is `true`
3. If no failures: return `Ok(value)`
4. If any failures: return `Err(errors)` with all error messages

#### Rule Interpretation

Each rule is `(condition, message)` where:
- `condition: bool` — `true` means validation **failed**
- `message: str` — error message to collect if failed

This "failure condition" style matches common validation patterns:

```ori
// Natural reading: "if empty, then error"
(input.name.is_empty(), "name is required")

// Natural reading: "if negative, then error"
(input.age < 0, "age must be non-negative")
```

#### All Rules Evaluated

Unlike short-circuit `&&`, all rules are always evaluated:

```ori
validate(
    rules: [
        (a.is_invalid(), "a is invalid"),
        (b.is_invalid(), "b is invalid"),  // Always checked
        (c.is_invalid(), "c is invalid"),  // Always checked
    ],
    value: result,
)
// Returns Err(["a is invalid", "b is invalid"]) if both a and b fail
```

### validate_with Function

Extended validation with field paths for structured errors:

```ori
@validate_with<T> (
    rules: [(bool, str, str)],
    value: T,
) -> Result<T, [ValidationError]>
```

#### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `rules` | `[(bool, str, str)]` | List of (failure_condition, field_path, error_message) |
| `value` | `T` | Value to return if all validations pass |

#### ValidationError Type

```ori
type ValidationError = {
    field: str,
    message: str,
}

#derive(Eq, Clone, Debug, Printable)
```

#### Example

```ori
use std.validate { validate_with, ValidationError }

@validate_user (input: UserInput) -> Result<User, [ValidationError]> =
    validate_with(
        rules: [
            (input.name.is_empty(), "name", "is required"),
            (input.age < 0, "age", "must be non-negative"),
            (input.email.is_empty(), "email", "is required"),
            (!is_valid_email(input.email), "email", "invalid format"),
        ],
        value: User { name: input.name, age: input.age, email: input.email },
    )
```

### validate_all Function

Compose multiple validation results:

```ori
@validate_all<T> (
    validations: [Result<T, [str]>],
) -> Result<[T], [str]>
```

Collects all errors from all validations:

```ori
let results = validate_all(validations: [
    validate_name(input.name),
    validate_age(input.age),
    validate_email(input.email),
])
// If any fail, returns Err with ALL error messages combined
// If all pass, returns Ok with list of values
```

### Predicate Helpers

Common validation predicates:

```ori
// String validations
@is_empty (s: str) -> bool
@is_blank (s: str) -> bool  // empty or whitespace only
@min_length (s: str, min: int) -> bool  // true if too short
@max_length (s: str, max: int) -> bool  // true if too long
@matches (s: str, pattern: str) -> bool  // regex match (requires std.regex)

// Numeric validations
@is_negative (n: int) -> bool
@is_positive (n: int) -> bool
@in_range (n: int, min: int, max: int) -> bool  // true if IN range
@out_of_range (n: int, min: int, max: int) -> bool  // true if OUT of range

// Collection validations
@is_empty (c: [T]) -> bool
@min_count (c: [T], min: int) -> bool  // true if too few
@max_count (c: [T], max: int) -> bool  // true if too many

// Option validations
@is_missing (o: Option<T>) -> bool  // true if None
```

> **Note:** These helpers return `true` for the **failure** condition, matching the rule semantics.

---

## Composing Validations

### Nested Validation

Validate nested structures:

```ori
type Address = { street: str, city: str, zip: str }
type Person = { name: str, address: Address }

@validate_address (a: Address) -> Result<Address, [ValidationError]> =
    validate_with(
        rules: [
            (a.street.is_empty(), "street", "is required"),
            (a.city.is_empty(), "city", "is required"),
            (a.zip.is_empty(), "zip", "is required"),
        ],
        value: a,
    )

@validate_person (p: Person) -> Result<Person, [ValidationError]> = {
    let address_result = validate_address(p.address)
        .map_err(transform: errs ->
            for e in errs yield ValidationError {
                field: `address.{e.field}`,
                message: e.message
            }
        );

    let person_result = validate_with(
        rules: [(p.name.is_empty(), "name", "is required")],
        value: p,
    );

    // Combine errors from both
    match (person_result, address_result) {
        (Ok(_), Ok(_)) -> Ok(p)
        (Err(e1), Ok(_)) -> Err(e1)
        (Ok(_), Err(e2)) -> Err(e2)
        (Err(e1), Err(e2)) -> Err([...e1, ...e2])
    }
}
```

### Conditional Validation

Validate only when relevant:

```ori
@validate_optional_field<T> (
    value: Option<T>,
    validate_fn: (T) -> Result<T, [str]>,
) -> Result<Option<T>, [str]> =
    match value {
        None -> Ok(None)
        Some(v) -> validate_fn(v).map(transform: v -> Some(v))
    }
```

### Cross-Field Validation

Validate relationships between fields:

```ori
@validate_date_range (start: Date, end: Date) -> Result<(Date, Date), [str]> =
    validate(
        rules: [
            (end < start, "end date must be after start date"),
        ],
        value: (start, end),
    )
```

---

## Error Formatting

### Simple List

```ori
let result = validate_user(input);
match result {
    Ok(user) -> process(user)
    Err(errors) -> print(msg: errors.join(separator: "\n"))
}
```

Output:
```
name is required
age must be non-negative
email is required
```

### Structured Errors

```ori
let result = validate_user(input);
match result {
    Ok(user) -> process(user)
    Err(errors) -> {
        print(msg: "Validation failed:");
        for e in errors do
            print(msg: `  - {e.field}: {e.message}`)
    }
}
```

Output:
```
Validation failed:
  - name: is required
  - age: must be non-negative
  - email: is required
```

### JSON Error Response

```ori
use std.json { to_json_string }

@validation_response (errors: [ValidationError]) -> str =
    to_json_string(value: {
        "success": false,
        "errors": for e in errors yield {
            "field": e.field,
            "message": e.message,
        },
    })
```

Output:
```json
{
  "success": false,
  "errors": [
    { "field": "name", "message": "is required" },
    { "field": "age", "message": "must be non-negative" }
  ]
}
```

---

## Examples

### Form Validation

```ori
use std.validate { validate_with, ValidationError }

type SignupForm = {
    username: str,
    email: str,
    password: str,
    confirm_password: str,
}

@validate_signup (form: SignupForm) -> Result<SignupForm, [ValidationError]> =
    validate_with(
        rules: [
            (form.username.is_empty(), "username", "is required"),
            (min_length(s: form.username, min: 3), "username", "must be at least 3 characters"),
            (max_length(s: form.username, max: 20), "username", "must be at most 20 characters"),
            (form.email.is_empty(), "email", "is required"),
            (!is_valid_email(form.email), "email", "invalid format"),
            (form.password.is_empty(), "password", "is required"),
            (min_length(s: form.password, min: 8), "password", "must be at least 8 characters"),
            (form.password != form.confirm_password, "confirm_password", "passwords do not match"),
        ],
        value: form,
    )
```

### API Input Validation

```ori
use std.validate { validate }

type CreateOrderRequest = {
    customer_id: int,
    items: [OrderItem],
    shipping_address: Option<Address>,
}

@validate_order (req: CreateOrderRequest) -> Result<CreateOrderRequest, [str]> =
    validate(
        rules: [
            (req.customer_id <= 0, "invalid customer ID"),
            (req.items.is_empty(), "order must have at least one item"),
            (max_count(c: req.items, max: 100), "order cannot exceed 100 items"),
            (req.shipping_address.is_none(), "shipping address is required"),
        ],
        value: req,
    )
```

### Configuration Validation

```ori
use std.validate { validate_with }

type ServerConfig = {
    host: str,
    port: int,
    max_connections: int,
    timeout_seconds: int,
}

@validate_config (config: ServerConfig) -> Result<ServerConfig, [ValidationError]> =
    validate_with(
        rules: [
            (config.host.is_empty(), "host", "is required"),
            (config.port < 1 || config.port > 65535, "port", "must be between 1 and 65535"),
            (config.max_connections < 1, "max_connections", "must be at least 1"),
            (config.max_connections > 10000, "max_connections", "cannot exceed 10000"),
            (config.timeout_seconds < 1, "timeout_seconds", "must be at least 1 second"),
            (config.timeout_seconds > 300, "timeout_seconds", "cannot exceed 5 minutes"),
        ],
        value: config,
    )
```

---

## Comparison with Alternatives

### vs. Fail-Fast Validation

| Aspect | `validate` | Fail-fast `if-else` |
|--------|------------|---------------------|
| Error collection | All errors | First error only |
| User experience | Fix all at once | Fix one, resubmit, repeat |
| Performance | Evaluates all rules | Short-circuits |
| Use case | Forms, APIs | Quick checks |

### vs. Contract Checks

| Aspect | `validate` | `pre()` |
|--------|------------|---------|
| Return type | `Result<T, [str]>` | Panics on failure |
| Error handling | Caller decides | Unrecoverable |
| Use case | User input | Programming errors |

```ori
// pre() for invariants (programmer errors)
@divide (a: int, b: int) -> int
    pre(b != 0)
    = a / b

// validate: for user input (expected failures)
@parse_age (input: str) -> Result<int, [str]> =
    validate(
        rules: [
            (!input.is_numeric(), "age must be a number"),
            (input.to_int() < 0, "age must be non-negative"),
            (input.to_int() > 150, "age must be realistic"),
        ],
        value: input.to_int(),
    )
```

---

## Error Messages

### Empty Rules List

```
warning[W1200]: `validate` with empty rules always succeeds
  --> src/main.ori:5:5
   |
 5 |     validate(rules: [], value: x)
   |              ^^^^^^^^^ empty rules list
   |
   = note: this always returns Ok(value)
```

### Rule Type Mismatch

```
error[E1201]: validate rule must be (bool, str) tuple
  --> src/main.ori:5:15
   |
 5 |     validate(rules: [("not a bool", "error")], value: x)
   |                      ^^^^^^^^^^^^^^^^^^^^^^^^ expected (bool, str)
   |
   = note: first element must be a boolean failure condition
```

---

## Implementation Notes

### Pure Ori Implementation

```ori
@validate<T> (
    rules: [(bool, str)],
    value: T,
) -> Result<T, [str]> = {
    let errors = for (failed, message) in rules if failed yield message;
    if errors.is_empty() then Ok(value)
    else Err(errors)
}

@validate_with<T> (
    rules: [(bool, str, str)],
    value: T,
) -> Result<T, [ValidationError]> = {
    let errors = for (failed, field, message) in rules if failed yield
        ValidationError { field: field, message: message };
    if errors.is_empty() then Ok(value)
    else Err(errors)
}

@validate_all<T> (
    validations: [Result<T, [str]]>,
) -> Result<[T], [str]> = {
    let values: [T] = [];
    let errors: [str] = [];
    for v in validations do match v {
        Ok(value) -> values = [...values, value]
        Err(errs) -> errors = [...errors, ...errs]
    };
    if errors.is_empty() then Ok(values)
    else Err(errors)
}
```

### No Capabilities Required

The validate functions are pure—no `Suspend`, `Clock`, or other capabilities needed. This makes them usable anywhere.

---

## Spec Changes Required

### Update `11-built-in-functions.md`

Add reference to `std.validate` module.

### Create `modules/std.validate/index.md`

Document the full module API.

---

## Summary

| Aspect | Details |
|--------|---------|
| Module | `std.validate` |
| Core functions | `validate`, `validate_with`, `validate_all` |
| Rule format | `(failure_condition, message)` or `(failure_condition, field, message)` |
| Error type | `[str]` or `[ValidationError]` |
| Behavior | Accumulating (all rules evaluated) |
| Capabilities | None required (pure functions) |
| Implementation | Pure Ori |

---

## Future Considerations

### Schema Validation

A future extension could add schema-based validation:

```ori
// Not in this proposal
type UserSchema = Schema {
    name: Required<str>,
    age: Optional<int>.where(n -> n >= 0),
    email: Required<str>.format(Email),
}

validate_schema(schema: UserSchema, value: input)
```

### Async Validation

For validations requiring I/O (e.g., checking username availability):

```ori
// Not in this proposal
@validate_async<T> (
    rules: [(() -> Result<void, str>) uses Suspend],
    value: T,
) -> Result<T, [str]> uses Suspend
```

These are deferred to future proposals to keep this focused on core synchronous validation.
