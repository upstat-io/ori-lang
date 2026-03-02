# Guards and Bindings

This document covers advanced pattern matching features: guards with `.match()`, or patterns with `|`, and `@` binding for naming matched values.

---

## Guards

Guards add conditions to patterns. A guard is evaluated only if the pattern matches, providing additional filtering beyond structural matching.

### Formal Definition

The `.match()` method is a built-in guard mechanism that applies to any pattern binding:

```
guard_pattern = pattern "." "match" "(" boolean_expression ")"
```

Where:
- `pattern` is any valid pattern (binding, destructuring, variant, etc.)
- `boolean_expression` is an expression that evaluates to `bool`
- Variables bound in `pattern` are in scope within `boolean_expression`

**Semantics:**
1. First, the structural pattern is matched
2. If structural match succeeds, bound variables are available
3. The guard expression is evaluated using those bindings
4. The arm executes only if both pattern matches AND guard is true

### Basic Syntax

```ori
pattern.match(condition) -> result
```

The guard appears after the pattern using fluent `.match()` syntax:

```ori
@classify (number: int) -> str = match(number,
    0 -> "zero",
    value.match(value < 0) -> "negative",
    value.match(value > 100) -> "large",
    value -> "small positive",
)
```

### Guards with Destructuring

Combine structural matching with conditions:

```ori
type Result<T, E> = Ok(T) | Err(E)

@describe (result: Result<int, str>) -> str = match(result,
    Ok(value).match(value > 0) -> "positive result: " + str(value),
    Ok(value).match(value < 0) -> "negative result: " + str(value),
    Ok(_) -> "zero result",
    Err(error) -> "error: " + error,
)
```

### Guard Evaluation

Guards are evaluated in order:

```ori
@grade (score: int) -> str = match(score,
    // checked first
    value.match(value >= 90) -> "A",
    // checked if first fails
    value.match(value >= 80) -> "B",
    value.match(value >= 70) -> "C",
    value.match(value >= 60) -> "D",
    _ -> "F",
)
```

**Important:** Guards don't affect exhaustiveness checking. The compiler still requires that all structural patterns are covered.

### Complex Guard Conditions

Guards can use any boolean expression:

```ori
@validate (user: User) -> str = match(user,
    { age, .. }.match(age < 0) -> "invalid: negative age",
    { age, .. }.match(age > 150) -> "invalid: age too high",
    { name, age, .. }.match(len(.collection: name) == 0) -> "invalid: empty name",
    { name, age, .. } -> "valid: " + name,
)
```

### Guards with Multiple Bindings

```ori
type Point = { x: int, y: int }

@quadrant (point: Point) -> str = match(point,
    { x, y }.match(x > 0 && y > 0) -> "Q1",
    { x, y }.match(x < 0 && y > 0) -> "Q2",
    { x, y }.match(x < 0 && y < 0) -> "Q3",
    { x, y }.match(x > 0 && y < 0) -> "Q4",
    _ -> "on axis",
)
```

### Guards with Function Calls

```ori
@process (items: [int]) -> str = match(items,
    list.match(len(.collection: list) > 100) -> "too many items",
    list.match(is_sorted(list)) -> "already sorted",
    list -> "processing " + str(len(.collection: list)) + " items",
)

@validate_user (input: User) -> Result<User, str> = match(input,
    user.match(is_admin(user)) -> Ok(user),
    user.match(user.age >= 18) -> Ok(user),
    user -> Err("must be admin or adult"),
)
```

---

## Or Patterns

Or patterns match multiple alternatives with the same result. Use `|` to combine patterns.

### Basic Syntax

```ori
pattern1 | pattern2 -> result
```

Both patterns lead to the same arm:

```ori
type Day = Mon | Tue | Wed | Thu | Fri | Sat | Sun

@is_weekend (day: Day) -> bool = match(day,
    Sat | Sun -> true,
    _ -> false,
)
```

### Multiple Alternatives

```ori
@day_type (day: Day) -> str = match(day,
    Sat | Sun -> "weekend",
    Mon | Fri -> "edge of week",
    Tue | Wed | Thu -> "midweek",
)
```

### Or Patterns with Variants

```ori
type Status =
    | Pending
    | Queued
    | Running
    | Completed
    | Failed(str)
    | Cancelled

@is_terminal (status: Status) -> bool = match(status,
    Completed | Cancelled -> true,
    Failed(_) -> true,
    _ -> false,
)
```

### Binding Consistency

All alternatives in an or pattern must bind the same variables with the same types:

```ori
// Valid: both alternatives bind 'value' as int
@extract (result: Result<int, int>) -> int = match(result,
    Ok(value) | Err(value) -> value,
)

// ERROR: different bindings
@bad (result: Result<int, str>) -> int = match(result,
    // 'text' not bound in Ok branch
    Ok(value) | Err(text) -> value
)
```

### Or Patterns with Literals

```ori
@is_vowel (character: str) -> bool = match(character,
    "a" | "e" | "i" | "o" | "u" -> true,
    "A" | "E" | "I" | "O" | "U" -> true,
    _ -> false,
)

@http_success (code: int) -> bool = match(code,
    200 | 201 | 204 -> true,
    _ -> false,
)
```

### Or Patterns with Guards

Guards apply to the entire or pattern:

```ori
@classify (number: int) -> str = match(number,
    // ERROR: wrong syntax
    value.match(value < -100 | value > 100) -> "extreme",

    // Correct: guard after or pattern
    value.match(value < -100) -> "extreme negative",
    value.match(value > 100) -> "extreme positive",
    _ -> "moderate"
)

// Or patterns with shared guard
@process (number: int) -> str = match(number,
    (1 | 2 | 3).match(is_valid(number)) -> "small valid",
    1 | 2 | 3 -> "small invalid",
    _ -> "other",
)
```

---

## @ Binding

The `@` operator binds a name to a value while also matching a pattern. This lets you access both the whole value and its parts.

### Basic Syntax

```ori
name @ pattern -> result
```

The name binds to the entire matched value:

```ori
@process (result: Result<int, str>) -> str = match(result,
    ok @ Ok(value).match(value > 0) -> "positive: " + debug(ok),
    Ok(value) -> "non-positive: " + str(value),
    Err(error) -> error,
)
```

### Preserving Original While Destructuring

```ori
type User = { name: str, email: str, age: int }

@validate (input: User) -> Result<User, str> = match(input,
    user @ { age, .. }.match(age < 0) -> Err("invalid age"),
    user @ { age, .. }.match(age > 150) -> Err("age too high"),
    user @ { name, .. }.match(len(.collection: name) == 0) -> Err("empty name"),
    // return original
    user @ _ -> Ok(user),
)
```

### @ Binding in List Patterns

```ori
@transform (items: [int]) -> [int] = match(items,
    // at least 2: return unchanged
    all @ [_, _, ..] -> all,
    // exactly 1: duplicate
    [x] -> [x, x],
    // empty: stay empty
    [] -> [],
)
```

### Nested @ Bindings

```ori
type Tree<T> = Leaf(T) | Node(Tree<T>, Tree<T>)

@transform_tree (tree: Tree<int>) -> Tree<int> = match(tree,
    Leaf(value) -> Leaf(value * 2),
    whole @ Node(left @ Leaf(_), right @ Leaf(_)) ->
        // whole is the Node, left and right are the Leaves
        Node(left, right),
    Node(left, right) -> Node(transform_tree(left), transform_tree(right)),
)
```

### @ Binding with Or Patterns

```ori
@process (input: Result<int, int>) -> str = match(input,
    result @ (Ok(value) | Err(value)) -> "value " + str(value) + " in " + type_of(result),
)
```

### @ Binding in Variants

```ori
type Option<T> = Some(T) | None

@double_if_positive (opt: Option<int>) -> Option<int> = match(opt,
    original @ Some(value).match(value > 0) -> Some(value * 2),
    // return unchanged
    original @ Some(_) -> original,
    None -> None,
)
```

---

## Combining Features

### Guards + Or Patterns

```ori
type Day = Mon | Tue | Wed | Thu | Fri | Sat | Sun

@schedule (day: Day, hour: int) -> str = match((day, hour),
    (Sat | Sun, time).match(time < 12) -> "weekend morning",
    (Sat | Sun, _) -> "weekend afternoon/evening",
    (_, time).match(time < 9) -> "before work",
    (_, time).match(time >= 17) -> "after work",
    (_, _) -> "work hours",
)
```

### Guards + @ Binding

```ori
@process (opt: Option<int>) -> str = match(opt,
    original @ Some(value).match(value > 100) ->
        "large value " + str(value) + " from " + debug(original),
    Some(value) -> "normal: " + str(value),
    None -> "nothing",
)
```

### Or Patterns + @ Binding

```ori
type Status = Active | Pending | Suspended | Banned

@can_login (input: Status) -> (bool, str) = match(input,
    status @ (Active | Pending) -> (true, debug(status)),
    status @ (Suspended | Banned) -> (false, debug(status)),
)
```

### All Three Together

```ori
type Response =
    | Success(data: Data)
    | Redirect(url: str)
    | ClientError(code: int)
    | ServerError(code: int)

@handle (response: Response) -> str = match(response,
    resp @ (ClientError(code) | ServerError(code)).match(code >= 500) ->
        "critical error " + str(code) + ": " + debug(resp),
    resp @ (ClientError(code) | ServerError(code)) ->
        "error " + str(code) + ": " + debug(resp),
    Success(data) -> "ok: " + data.to_string(),
    Redirect(url) -> "redirect to " + url,
)
```

---

## Range Patterns

Match numeric ranges with `..` syntax:

### Basic Ranges

```ori
@grade (score: int) -> str = match(score,
    // 90 inclusive to 100 exclusive
    90..100 -> "A",
    80..90 -> "B",
    70..80 -> "C",
    60..70 -> "D",
    0..60 -> "F",
    _ -> "invalid",
)
```

### Inclusive Ranges

Use `..=` for inclusive end:

```ori
@category (number: int) -> str = match(number,
    // 1 to 10 inclusive
    1..=10 -> "small",
    11..=100 -> "medium",
    101..=1000 -> "large",
    _ -> "other",
)
```

### Open-Ended Ranges

```ori
@sign (number: int) -> str = match(number,
    // up to 0 (exclusive)
    ..0 -> "negative",
    0 -> "zero",
    // 1 and above
    1.. -> "positive",
)
```

### Range Patterns with Guards

```ori
@http_status (code: int) -> str = match(code,
    200..300 -> "success",
    300..400 -> "redirect",
    400..500 -> "client error",
    500..600 -> "server error",
    value.match(value < 100) -> "invalid: too low",
    _ -> "unknown",
)
```

---

## Pattern Precedence

Understanding how patterns combine:

### Order of Operations

1. Literal/constructor matching
2. Destructuring
3. @ binding
4. Or alternatives
5. Guard evaluation (`.match()`)

```ori
// This pattern:
result @ (Ok(value) | Err(value)).match(value > 0) -> ...

// Is evaluated as:
// 1. Try Ok(value), binding value
// 2. If that fails, try Err(value), binding value
// 3. Bind entire value to 'result'
// 4. Check guard value > 0 via .match()
// 5. If all pass, execute arm
```

### Parentheses for Clarity

```ori
// Clear grouping
match(value,
    (A | B | C) -> "group 1",
    (D | E) -> "group 2",
)

// @ binding applies to whole pattern
result @ (Ok(value) | Err(value)) -> ...

// vs individual
// different bindings
(ok @ Ok(value)) | (err @ Err(value)) -> ...
```

---

## Common Idioms

### Optional Field Handling

```ori
type Config = { timeout: Option<int>, retries: Option<int> }

@get_timeout (config: Config) -> int = match(config,
    { timeout: Some(value), .. }.match(value > 0) -> value,
    // default
    _ -> 30,
)
```

### Validation with Details

```ori
@validate (input: str) -> Result<str, str> = match(input,
    text.match(len(.collection: text) == 0) -> Err("cannot be empty"),
    text.match(len(.collection: text) > 100) -> Err("too long (max 100)"),
    text.match(!is_alphanumeric(text)) -> Err("must be alphanumeric"),
    text -> Ok(text),
)
```

### State Machine Transitions

```ori
type State = Idle | Running(int) | Paused(int) | Done

@transition (current: State, command: Command) -> State = match((current, command),
    (Idle, Start) -> Running(0),
    (state @ Running(progress), Pause) -> Paused(progress),
    (Paused(progress), Resume) -> Running(progress),
    (Running(_) | Paused(_), Stop) -> Done,
    // no change
    (state @ _, _) -> state,
)
```

### Recursive with Accumulator

```ori
@sum_positive (items: [int]) -> int = match(items,
    [] -> 0,
    [head, ..rest].match(head > 0) -> head + sum_positive(rest),
    [_, ..rest] -> sum_positive(rest),
)
```

---

## Error Messages

### Invalid Guard

```
error[E0405]: guard must be a boolean expression
  |
5 | Ok(n).match(n) -> ...
  |             ^ expected bool, found int
```

### Inconsistent Or Pattern Bindings

```
error[E0406]: or-pattern bindings differ
  |
5 | Ok(value) | Err(text) -> value
  | ^^^^^^^^^ ---^^^^^^^^
  | |             |
  | |             'text' not bound in all alternatives
  | 'value' not bound in all alternatives
  |
help: ensure all alternatives bind the same names:
  | Ok(value) | Err(value) -> value
```

### Invalid @ Binding

```
error[E0407]: @ binding must precede a pattern
  |
5 | x @ -> result
  |    ^^ expected pattern after @
```

### Guard Referencing Unbound Variable

```
error[E0100]: unknown identifier 'value'
  |
5 | _.match(value > 0) -> ...
  |         ^ 'value' is not bound in pattern '_'
  |
help: bind a variable in the pattern:
  | value.match(value > 0) -> ...
```

---

## Best Practices

### Use Guards Sparingly

Prefer structural patterns when possible:

```ori
// Preferred: structural match
@is_some (opt: Option<int>) -> bool = match(opt,
    Some(_) -> true,
    None -> false,
)

// Avoid: guard for structural check
@is_some_bad (opt: Option<int>) -> bool = match(opt,
    value.match(is_variant(value, Some)) -> true,
    _ -> false,
)
```

### Group Related Or Patterns

```ori
// Clear grouping
@day_category (day: Day) -> str = match(day,
    Mon | Tue | Wed | Thu | Fri -> "weekday",
    Sat | Sun -> "weekend",
)

// Confusing: mixed grouping
@day_category_bad (day: Day) -> str = match(day,
    Mon -> "weekday",
    Tue | Wed -> "weekday",
    Thu | Fri -> "weekday",
    Sat | Sun -> "weekend",
)
```

### Name @ Bindings Meaningfully

```ori
// Good: meaningful name
original @ Some(value).match(value > 0) -> process(original)

// Avoid: generic names
x @ Some(value).match(value > 0) -> process(x)
```

### Document Complex Guards

```ori
@validate (input: User) -> Result<User, str> = match(input,
    // Age must be in valid range
    user @ { age, .. }.match(age < 0 || age > 150) ->
        Err("age must be between 0 and 150"),

    // Name cannot be empty
    user @ { name, .. }.match(len(.collection: name) == 0) ->
        Err("name is required"),

    // All checks passed
    user @ _ -> Ok(user),
)
```

---

## See Also

- [Match Pattern](01-match-pattern.md) — Basic match syntax
- [Destructuring](02-destructuring.md) — Struct and list destructuring
- [Exhaustiveness](04-exhaustiveness.md) — Complete pattern coverage
- [Type Narrowing](05-type-narrowing.md) — Flow-sensitive typing
