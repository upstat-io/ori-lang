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

```sigil
pattern.match(condition) -> result
```

The guard appears after the pattern using fluent `.match()` syntax:

```sigil
@classify (n: int) -> str = match(n,
    0 -> "zero",
    x.match(x < 0) -> "negative",
    x.match(x > 100) -> "large",
    x -> "small positive",
)
```

### Guards with Destructuring

Combine structural matching with conditions:

```sigil
type Result<T, E> = Ok(T) | Err(E)

@describe (r: Result<int, str>) -> str = match(r,
    Ok(n).match(n > 0) -> "positive result: " + str(n),
    Ok(n).match(n < 0) -> "negative result: " + str(n),
    Ok(_) -> "zero result",
    Err(e) -> "error: " + e,
)
```

### Guard Evaluation

Guards are evaluated in order:

```sigil
@grade (score: int) -> str = match(score,
    s.match(s >= 90) -> "A",   // checked first
    s.match(s >= 80) -> "B",   // checked if first fails
    s.match(s >= 70) -> "C",
    s.match(s >= 60) -> "D",
    _ -> "F",
)
```

**Important:** Guards don't affect exhaustiveness checking. The compiler still requires that all structural patterns are covered.

### Complex Guard Conditions

Guards can use any boolean expression:

```sigil
@validate (user: User) -> str = match(user,
    { age, .. }.match(age < 0) -> "invalid: negative age",
    { age, .. }.match(age > 150) -> "invalid: age too high",
    { name, age, .. }.match(len(name) == 0) -> "invalid: empty name",
    { name, age, .. } -> "valid: " + name,
)
```

### Guards with Multiple Bindings

```sigil
type Point = { x: int, y: int }

@quadrant (p: Point) -> str = match(p,
    { x, y }.match(x > 0 && y > 0) -> "Q1",
    { x, y }.match(x < 0 && y > 0) -> "Q2",
    { x, y }.match(x < 0 && y < 0) -> "Q3",
    { x, y }.match(x > 0 && y < 0) -> "Q4",
    _ -> "on axis",
)
```

### Guards with Function Calls

```sigil
@process (items: [int]) -> str = match(items,
    xs.match(len(xs) > 100) -> "too many items",
    xs.match(is_sorted(xs)) -> "already sorted",
    xs -> "processing " + str(len(xs)) + " items",
)

@validate_user (u: User) -> Result<User, str> = match(u,
    user.match(is_admin(user)) -> Ok(user),
    user.match(user.age >= 18) -> Ok(user),
    user -> Err("must be admin or adult"),
)
```

---

## Or Patterns

Or patterns match multiple alternatives with the same result. Use `|` to combine patterns.

### Basic Syntax

```sigil
pattern1 | pattern2 -> result
```

Both patterns lead to the same arm:

```sigil
type Day = Mon | Tue | Wed | Thu | Fri | Sat | Sun

@is_weekend (d: Day) -> bool = match(d,
    Sat | Sun -> true,
    _ -> false,
)
```

### Multiple Alternatives

```sigil
@day_type (d: Day) -> str = match(d,
    Sat | Sun -> "weekend",
    Mon | Fri -> "edge of week",
    Tue | Wed | Thu -> "midweek",
)
```

### Or Patterns with Variants

```sigil
type Status =
    | Pending
    | Queued
    | Running
    | Completed
    | Failed(str)
    | Cancelled

@is_terminal (s: Status) -> bool = match(s,
    Completed | Cancelled -> true,
    Failed(_) -> true,
    _ -> false,
)
```

### Binding Consistency

All alternatives in an or pattern must bind the same variables with the same types:

```sigil
// Valid: both alternatives bind 'n' as int
@extract (r: Result<int, int>) -> int = match(r,
    Ok(n) | Err(n) -> n,
)

// ERROR: different bindings
@bad (r: Result<int, str>) -> int = match(r,
    Ok(n) | Err(s) -> n  // 's' not bound in Ok branch
)
```

### Or Patterns with Literals

```sigil
@is_vowel (c: str) -> bool = match(c,
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

```sigil
@classify (n: int) -> str = match(n,
    x.match(x < -100 | x > 100) -> "extreme",  // ERROR: wrong syntax

    // Correct: guard after or pattern
    x.match(x < -100) -> "extreme negative",
    x.match(x > 100) -> "extreme positive",
    _ -> "moderate"
)

// Or patterns with shared guard
@process (n: int) -> str = match(n,
    (1 | 2 | 3).match(is_valid(n)) -> "small valid",
    1 | 2 | 3 -> "small invalid",
    _ -> "other",
)
```

---

## @ Binding

The `@` operator binds a name to a value while also matching a pattern. This lets you access both the whole value and its parts.

### Basic Syntax

```sigil
name @ pattern -> result
```

The name binds to the entire matched value:

```sigil
@process (r: Result<int, str>) -> str = match(r,
    ok @ Ok(n).match(n > 0) -> "positive: " + debug(ok),
    Ok(n) -> "non-positive: " + str(n),
    Err(e) -> e,
)
```

### Preserving Original While Destructuring

```sigil
type User = { name: str, email: str, age: int }

@validate (u: User) -> Result<User, str> = match(u,
    user @ { age, .. }.match(age < 0) -> Err("invalid age"),
    user @ { age, .. }.match(age > 150) -> Err("age too high"),
    user @ { name, .. }.match(len(name) == 0) -> Err("empty name"),
    user @ _ -> Ok(user),  // return original
)
```

### @ Binding in List Patterns

```sigil
@transform (items: [int]) -> [int] = match(items,
    all @ [_, _, ..] -> all,     // at least 2: return unchanged
    [x] -> [x, x],               // exactly 1: duplicate
    [] -> [],                     // empty: stay empty
)
```

### Nested @ Bindings

```sigil
type Tree<T> = Leaf(T) | Node(Tree<T>, Tree<T>)

@transform_tree (t: Tree<int>) -> Tree<int> = match(t,
    Leaf(n) -> Leaf(n * 2),
    whole @ Node(left @ Leaf(_), right @ Leaf(_)) ->
        // whole is the Node, left and right are the Leaves
        Node(left, right),
    Node(left, right) -> Node(transform_tree(left), transform_tree(right)),
)
```

### @ Binding with Or Patterns

```sigil
@process (r: Result<int, int>) -> str = match(r,
    result @ (Ok(n) | Err(n)) -> "value " + str(n) + " in " + type_of(result),
)
```

### @ Binding in Variants

```sigil
type Option<T> = Some(T) | None

@double_if_positive (opt: Option<int>) -> Option<int> = match(opt,
    original @ Some(n).match(n > 0) -> Some(n * 2),
    original @ Some(_) -> original,  // return unchanged
    None -> None,
)
```

---

## Combining Features

### Guards + Or Patterns

```sigil
type Day = Mon | Tue | Wed | Thu | Fri | Sat | Sun

@schedule (d: Day, hour: int) -> str = match((d, hour),
    (Sat | Sun, h).match(h < 12) -> "weekend morning",
    (Sat | Sun, _) -> "weekend afternoon/evening",
    (_, h).match(h < 9) -> "before work",
    (_, h).match(h >= 17) -> "after work",
    (_, _) -> "work hours",
)
```

### Guards + @ Binding

```sigil
@process (opt: Option<int>) -> str = match(opt,
    original @ Some(n).match(n > 100) ->
        "large value " + str(n) + " from " + debug(original),
    Some(n) -> "normal: " + str(n),
    None -> "nothing",
)
```

### Or Patterns + @ Binding

```sigil
type Status = Active | Pending | Suspended | Banned

@can_login (s: Status) -> (bool, str) = match(s,
    status @ (Active | Pending) -> (true, debug(status)),
    status @ (Suspended | Banned) -> (false, debug(status)),
)
```

### All Three Together

```sigil
type Response =
    | Success(data: Data)
    | Redirect(url: str)
    | ClientError(code: int)
    | ServerError(code: int)

@handle (r: Response) -> str = match(r,
    resp @ (ClientError(c) | ServerError(c)).match(c >= 500) ->
        "critical error " + str(c) + ": " + debug(resp),
    resp @ (ClientError(c) | ServerError(c)) ->
        "error " + str(c) + ": " + debug(resp),
    Success(d) -> "ok: " + d.to_string(),
    Redirect(url) -> "redirect to " + url,
)
```

---

## Range Patterns

Match numeric ranges with `..` syntax:

### Basic Ranges

```sigil
@grade (score: int) -> str = match(score,
    90..100 -> "A",    // 90 inclusive to 100 exclusive
    80..90 -> "B",
    70..80 -> "C",
    60..70 -> "D",
    0..60 -> "F",
    _ -> "invalid",
)
```

### Inclusive Ranges

Use `..=` for inclusive end:

```sigil
@category (n: int) -> str = match(n,
    1..=10 -> "small",      // 1 to 10 inclusive
    11..=100 -> "medium",
    101..=1000 -> "large",
    _ -> "other",
)
```

### Open-Ended Ranges

```sigil
@sign (n: int) -> str = match(n,
    ..0 -> "negative",    // up to 0 (exclusive)
    0 -> "zero",
    1.. -> "positive",    // 1 and above
)
```

### Range Patterns with Guards

```sigil
@http_status (code: int) -> str = match(code,
    200..300 -> "success",
    300..400 -> "redirect",
    400..500 -> "client error",
    500..600 -> "server error",
    c.match(c < 100) -> "invalid: too low",
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

```sigil
// This pattern:
result @ (Ok(n) | Err(n)).match(n > 0) -> ...

// Is evaluated as:
// 1. Try Ok(n), binding n
// 2. If that fails, try Err(n), binding n
// 3. Bind entire value to 'result'
// 4. Check guard n > 0 via .match()
// 5. If all pass, execute arm
```

### Parentheses for Clarity

```sigil
// Clear grouping
match(value,
    (A | B | C) -> "group 1",
    (D | E) -> "group 2",
)

// @ binding applies to whole pattern
result @ (Ok(n) | Err(n)) -> ...

// vs individual
(ok @ Ok(n)) | (err @ Err(n)) -> ...  // different bindings
```

---

## Common Idioms

### Optional Field Handling

```sigil
type Config = { timeout: Option<int>, retries: Option<int> }

@get_timeout (c: Config) -> int = match(c,
    { timeout: Some(t), .. }.match(t > 0) -> t,
    _ -> 30,  // default
)
```

### Validation with Details

```sigil
@validate (input: str) -> Result<str, str> = match(input,
    s.match(len(s) == 0) -> Err("cannot be empty"),
    s.match(len(s) > 100) -> Err("too long (max 100)"),
    s.match(!is_alphanumeric(s)) -> Err("must be alphanumeric"),
    s -> Ok(s),
)
```

### State Machine Transitions

```sigil
type State = Idle | Running(int) | Paused(int) | Done

@transition (s: State, cmd: Command) -> State = match((s, cmd),
    (Idle, Start) -> Running(0),
    (state @ Running(p), Pause) -> Paused(p),
    (Paused(p), Resume) -> Running(p),
    (Running(_) | Paused(_), Stop) -> Done,
    (state @ _, _) -> state,  // no change
)
```

### Recursive with Accumulator

```sigil
@sum_positive (items: [int]) -> int = match(items,
    [] -> 0,
    [x, ..rest].match(x > 0) -> x + sum_positive(rest),
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
5 | Ok(x) | Err(y) -> x
  | ^^^^^ ---^^^^
  | |         |
  | |         'y' not bound in all alternatives
  | 'x' not bound in all alternatives
  |
help: ensure all alternatives bind the same names:
  | Ok(n) | Err(n) -> n
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
error[E0100]: unknown identifier 'n'
  |
5 | _.match(n > 0) -> ...
  |         ^ 'n' is not bound in pattern '_'
  |
help: bind a variable in the pattern:
  | n.match(n > 0) -> ...
```

---

## Best Practices

### Use Guards Sparingly

Prefer structural patterns when possible:

```sigil
// Preferred: structural match
@is_some (opt: Option<int>) -> bool = match(opt,
    Some(_) -> true,
    None -> false,
)

// Avoid: guard for structural check
@is_some_bad (opt: Option<int>) -> bool = match(opt,
    x.match(is_variant(x, Some)) -> true,
    _ -> false,
)
```

### Group Related Or Patterns

```sigil
// Clear grouping
@day_category (d: Day) -> str = match(d,
    Mon | Tue | Wed | Thu | Fri -> "weekday",
    Sat | Sun -> "weekend",
)

// Confusing: mixed grouping
@day_category_bad (d: Day) -> str = match(d,
    Mon -> "weekday",
    Tue | Wed -> "weekday",
    Thu | Fri -> "weekday",
    Sat | Sun -> "weekend",
)
```

### Name @ Bindings Meaningfully

```sigil
// Good: meaningful name
original @ Some(n).match(n > 0) -> process(original)

// Avoid: generic names
x @ Some(n).match(n > 0) -> process(x)
```

### Document Complex Guards

```sigil
@validate (user: User) -> Result<User, str> = match(user,
    // Age must be in valid range
    u @ { age, .. }.match(age < 0 || age > 150) ->
        Err("age must be between 0 and 150"),

    // Name cannot be empty
    u @ { name, .. }.match(len(name) == 0) ->
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
