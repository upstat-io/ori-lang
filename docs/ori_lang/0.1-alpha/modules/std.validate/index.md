# std.validate

Input validation utilities.

```ori
use std.validate { validate }
```

---

## Functions

### @validate

```ori
@validate<T> (rules: [(bool, str)], value: T) -> Result<T, [str]>
```

Validates all rules and returns accumulated errors.

```ori
use std.validate { validate }

let user = validate(
    rules: [
        (age >= 0, "age must be non-negative"),
        (name != "", "name required"),
    ],
    value: User { name, age },
)
// Ok(User { name: "Alice", age: 25 }) or Err(["age must be non-negative"])
```

All rules are checked even if earlier ones fail, allowing collection of all validation errors in a single pass.

**Parameters:**
- `rules` — List of `(condition, error_message)` tuples
- `value` — Value to return on success

**Returns:** `Result<T, [str]>` — `Ok(value)` if all rules pass, `Err([messages])` with all failed rule messages otherwise.

---

## See Also

- [std](../std/) — Core utilities
