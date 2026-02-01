# Proposal: Variadic Functions

**Status:** Approved
**Author:** Eric
**Created:** 2026-01-31
**Approved:** 2026-01-31
**Affects:** Parser, type checker, evaluator, codegen, FFI

---

## Summary

Add variadic function parameters allowing functions to accept a variable number of arguments of the same type.

```ori
@sum (numbers: ...int) -> int =
    numbers.fold(initial: 0, op: (acc, n) -> acc + n)

sum(1, 2, 3)     // 6
sum()            // 0

let nums = [1, 2, 3]
sum(...nums)     // 6 (spread into variadic)
```

---

## Motivation

### The Problem

Functions that logically accept "zero or more" arguments of the same type currently require list parameters:

```ori
// Current: must pass a list
@sum (numbers: [int]) -> int =
    numbers.fold(initial: 0, op: (acc, n) -> acc + n)

sum(numbers: [1, 2, 3])  // Verbose: explicit list + named argument
sum(numbers: [])         // Empty case requires empty list

// Current: format requires awkward list of trait objects
@format (template: str, args: [Printable]) -> str = ...

format(template: "{} + {} = {}", args: [1, 2, 3])  // Clunky
```

This is verbose and unergonomic for common patterns like:
- Aggregation functions (`sum`, `min`, `max`, `concat`)
- Logging and formatting (`print`, `format`, `log`)
- Builder patterns (`path.join`, `query.where`)

### Prior Art

| Language | Syntax | Type Safety | Heterogeneous |
|----------|--------|-------------|---------------|
| Go | `...T` | Homogeneous only | No |
| Python | `*args` | Untyped (runtime) | Yes |
| JavaScript | `...args` | Untyped (runtime) | Yes |
| TypeScript | `...args: T[]` | Homogeneous typed | No |
| Kotlin | `vararg items: T` | Homogeneous only | No |
| Rust | N/A (macros) | N/A | N/A |
| C | `...` | Untyped (va_list) | Yes (unsafe) |

### The Ori Way

Ori prioritizes type safety while providing ergonomic APIs:

1. **Homogeneous variadics** — All arguments must be the same type (`...int`)
2. **Trait object variadics** — Accept any type implementing a trait (`...Printable`)
3. **Spread expansion** — Pass lists as variadic arguments (`fn(...list)`)
4. **C interop** — Separate syntax for calling C variadics (unsafe)

---

## Design

### Basic Syntax

A variadic parameter uses `...` before the type:

```ori
@sum (numbers: ...int) -> int =
    numbers.fold(initial: 0, op: (acc, n) -> acc + n)

// Usage — no brackets, no named argument
sum(1, 2, 3)        // 6
sum(1)              // 1
sum()               // 0 (empty variadic is valid)
```

Inside the function, the variadic parameter is received as a list:

```ori
@debug_all (values: ...Debug) -> void = run(
    for value in values do    // values: [Debug]
        print(msg: value.debug())
)
```

### Grammar

```ebnf
// Function parameters
param          = identifier ":" type | variadic_param .
variadic_param = identifier ":" "..." type .

// Extern block parameters (includes C variadics)
extern_param   = identifier ":" type | c_variadic .
c_variadic     = "..." .  /* C-style, no type - only valid in extern "c" blocks */

// Call arguments (includes spread for variadic calls)
call_arg       = named_arg | positional_arg | spread_arg .
named_arg      = identifier ":" expression .
positional_arg = expression .
spread_arg     = "..." expression .
```

### Constraints

1. **One variadic parameter per function** — At most one variadic parameter allowed
2. **Must be last** — Variadic parameter must appear after all required parameters
3. **Cannot have default** — Variadic parameters cannot have default values (the default is empty list)
4. **Positional only at call site** — Variadic arguments are always positional; the parameter name cannot be used at call sites
5. **Named args before variadic** — All named arguments must precede the variadic position

```ori
// Valid
@log (level: str, messages: ...str) -> void

// Invalid: variadic not last
@bad (items: ...int, suffix: str) -> void  // Error

// Invalid: multiple variadics
@bad (a: ...int, b: ...str) -> void  // Error

// Invalid: variadic with default
@bad (items: ...int = [1, 2]) -> void  // Error
```

### Spread into Variadic

The spread operator `...` can be used to expand a list into variadic arguments:

```ori
@sum (numbers: ...int) -> int =
    numbers.fold(initial: 0, op: (acc, n) -> acc + n)

let nums = [1, 2, 3]

sum(...nums)           // 6 — spread list into variadic
sum(0, ...nums, 10)    // 14 — mix literals and spread
sum(...nums, ...nums)  // 12 — multiple spreads
```

**Type checking:** The spread expression must be a list whose element type matches the variadic parameter type.

```ori
let strs = ["a", "b"]
sum(...strs)  // Error: expected [int], got [str]
```

**Note:** This extends the spread operator to function call contexts, but **only for variadic parameter positions**. Spread in non-variadic function calls remains an error:

```ori
@add (a: int, b: int) -> int = a + b

add(...[1, 2])  // Error: spread not allowed (non-variadic function)
```

### Calling Convention

When calling a variadic function, named arguments for required parameters come first, followed by positional variadic arguments:

```ori
@log (level: str, messages: ...str) -> void

// Named arguments for required params, then variadic args (positional)
log(level: "INFO", "Request received", "User: 123")

// Spread
let context = ["user=123", "action=login"]
log(level: "INFO", "Request", ...context)
```

The variadic parameter name (`messages`) cannot be used at call sites — variadic arguments are always positional after any named arguments.

### Minimum Argument Count

Use required parameters before the variadic to enforce minimums:

```ori
// Requires at least one argument
@max (first: int, rest: ...int) -> int =
    rest.fold(initial: first, op: (a, b) -> if a > b then a else b)

max(5)         // 5 (first=5, rest=[])
max(1, 2, 3)   // 3 (first=1, rest=[2, 3])
max()          // Error: missing required argument 'first'
```

### Generic Variadics

Variadic parameters work with generics:

```ori
@print_all<T: Printable> (items: ...T) -> void = run(
    for item in items do
        print(msg: item.to_str())
)

print_all(1, 2, 3)        // OK: T = int
print_all("a", "b")       // OK: T = str
print_all(1, "a")         // Error: cannot unify int and str
```

### Trait Object Variadics

For heterogeneous arguments, use a trait name directly as the variadic type:

```ori
@print_any (items: ...Printable) -> void = run(
    for item in items do
        print(msg: item.to_str())
)

print_any(1, "hello", true)  // OK: all implement Printable
```

The arguments are boxed as trait objects and collected into `[Printable]`.

### Type Inference

The variadic element type can be inferred from arguments:

```ori
@collect<T> (items: ...T) -> [T] = items

collect(1, 2, 3)       // infers T = int, returns [int]
collect("a", "b")      // infers T = str, returns [str]
collect()              // Error E0XXX: cannot infer type T (no variadic arguments provided)

// With explicit type annotation
collect<int>()         // OK: [int] (empty)
```

When a generic type parameter `T` is only constrained by a variadic parameter `...T`, calls with zero arguments cannot infer `T`. An explicit type annotation is required. This applies even when `T` has bounds:

```ori
@display<T: Printable> (items: ...T) -> void = ...

display()              // Error: cannot infer T
display<str>()         // OK: empty variadic with T = str
```

### Function Type Representation

A variadic function's type is represented as accepting a list. When stored as a function value, variadic functions lose their special calling syntax:

```ori
@sum (numbers: ...int) -> int = ...

// sum has type ([int]) -> int
let f: ([int]) -> int = sum

// Must call with list when using function value
f([1, 2, 3])  // 6

// Direct call retains variadic syntax
sum(1, 2, 3)  // 6
```

This means variadic functions can be passed to higher-order functions that expect `([T]) -> R`:

```ori
@apply_to_numbers (fn: ([int]) -> int, numbers: [int]) -> int =
    fn(numbers)

apply_to_numbers(fn: sum, numbers: [1, 2, 3])  // 6
```

---

## C Variadic Interop

C variadic functions use a different, untyped mechanism. Ori provides separate syntax for calling them:

```ori
extern "c" from "libc" {
    @printf (format: CPtr, ...) -> c_int as "printf"
}

// Must be in unsafe block
unsafe {
    printf("Number: %d\n".as_c_str(), 42)
}
```

### Distinction from Ori Variadics

| Feature | Ori `...T` | C `...` |
|---------|------------|---------|
| Type safety | Homogeneous, checked | Unchecked |
| Context | Safe code | `unsafe` block only |
| Implementation | Collected into list | va_list ABI |
| Type annotation | Required (`...int`) | None (just `...`) |

### C Variadic Rules

1. **`extern` only** — C-style `...` only valid in `extern "c"` declarations
2. **No type** — C variadics have no type after `...`
3. **Unsafe required** — Calling C variadic functions requires `unsafe` block
4. **Platform ABI** — Arguments passed per platform's va_list convention

```ori
extern "c" {
    // C-style: no type after ...
    @sprintf (buf: CPtr, fmt: CPtr, ...) -> c_int

    // NOT C-style: this is Ori homogeneous variadic
    @ori_sum (nums: ...c_int) -> c_int
}
```

---

## Examples

### Format Function

```ori
@format (template: str, args: ...Printable) -> str = run(
    let mut result = ""
    let mut arg_index = 0
    let mut i = 0

    loop(run(
        if i >= template.len() then break result

        if template[i] == "{" && i + 1 < template.len() && template[i + 1] == "}" then run(
            if arg_index >= args.len() then
                panic(msg: "Not enough arguments for format string")
            result = result + args[arg_index].to_str()
            arg_index = arg_index + 1
            i = i + 2
        )
        else run(
            result = result + template[i]
            i = i + 1
        )
    ))
)

let msg = format("{} + {} = {}", 1, 2, 3)  // "1 + 2 = 3"
```

### Path Joining

```ori
@join_path (segments: ...str) -> str =
    segments.fold(initial: "", op: (acc, seg) -> run(
        if acc.is_empty() then seg
        else if acc.ends_with(suffix: "/") then acc + seg
        else acc + "/" + seg
    ))

join_path("home", "user", "documents")  // "home/user/documents"
join_path()                              // ""
```

### SQL Query Builder

```ori
type Query = { table: str, conditions: [str] }

@where (query: Query, conditions: ...str) -> Query =
    Query { ...query, conditions: [...query.conditions, ...conditions] }

let q = Query { table: "users", conditions: [] }
    |> where("active = true", "role = 'admin'")
// Query { table: "users", conditions: ["active = true", "role = 'admin'"] }
```

### Assertion Helpers

```ori
@assert_all (conditions: ...bool) -> void = run(
    for (i, cond) in conditions.enumerate() do
        if !cond then panic(msg: format("Assertion {} failed", i))
)

assert_all(x > 0, y > 0, x + y < 100)
```

### Logging with Context

```ori
@log (level: str, message: str, context: ...str) -> void uses Print = run(
    let ctx = if context.is_empty() then ""
              else " [" + context.join(separator: ", ") + "]"
    print(msg: format("[{}] {}{}", level, message, ctx))
)

log("INFO", "User logged in", "user_id=123", "ip=192.168.1.1")
// [INFO] User logged in [user_id=123, ip=192.168.1.1]
```

---

## Design Rationale

### Why Homogeneous Only?

Heterogeneous variadics (like Python's `*args`) sacrifice type safety:

```python
def process(*args):
    # args could be anything — no static guarantees
    pass
```

Ori maintains type safety by requiring all variadic arguments to be the same type (or implement the same trait). For truly heterogeneous needs, use `...Trait` (trait object variadic) or explicit tuple/struct parameters.

### Why `...T` Syntax?

| Option | Example | Notes |
|--------|---------|-------|
| `...T` | `items: ...int` | Matches spread, Go, TypeScript |
| `*T` | `items: *int` | Python-like, conflicts with pointer |
| `vararg T` | `vararg items: int` | Kotlin-like, new keyword |
| `[T...]` | `items: [int...]` | Novel, potentially confusing |

`...T` is chosen because:
1. Matches the spread operator (`...expr`)
2. Familiar from Go, TypeScript
3. Visually indicates "more of this type"

### Why Separate C Variadic Syntax?

C variadics have fundamentally different semantics:
- No type checking (printf-style format strings)
- Platform-specific ABI (va_list)
- Inherently unsafe

Mixing them with Ori's type-safe variadics would be confusing and dangerous. The `...` without type clearly indicates "C-style, unsafe."

### Why Allow Empty Variadic Calls?

Functions like `sum()` returning `0` for empty input are natural. If a minimum is needed, use required parameters:

```ori
// sum() is valid, returns 0
@sum (numbers: ...int) -> int

// max() requires at least one
@max (first: int, rest: ...int) -> int
```

### Why Extend Spread to Calls?

The approved spread proposal prohibits spread in function calls. This proposal extends spread **only** for variadic parameters, because:

1. It's the expected behavior — `fn(...list)` is natural
2. Type safety is maintained — element type must match
3. It enables powerful composition patterns

Non-variadic functions still reject spread:

```ori
@add (a: int, b: int) -> int = a + b
add(...[1, 2])  // Still an error — not variadic
```

---

## The Four Uses of `...`

This proposal introduces additional uses of `...` in Ori:

| Context | Syntax | Meaning |
|---------|--------|---------|
| Spread expression | `[...list]` | Expand collection in literal |
| Variadic parameter | `items: ...int` | Accept variable arguments |
| Spread in call | `fn(...list)` | Pass list to variadic |
| C variadic (extern only) | `@printf (...)` | Untyped C va_list |

Additionally, `..` (two dots) is used in rest patterns:

| Context | Syntax | Meaning |
|---------|--------|---------|
| Rest pattern | `[x, ..rest]` | Bind remaining elements |

The distinction:
- `...` (three dots) — spread/variadic (expressions, types, and C FFI)
- `..` (two dots) — rest pattern (pattern matching only)

---

## Implementation Notes

### Parser Changes

1. Add variadic parameter parsing in function signatures
2. Add spread expression parsing in call arguments
3. Validate variadic parameter constraints (last, single)

### Type Checker Changes

1. Convert `...T` parameter to `[T]` internally
2. Type check call arguments against variadic type
3. Handle spread expressions in calls — verify list element type
4. Infer generic type parameters from variadic arguments
5. Box trait objects for trait object variadics (`...Printable`)

### Evaluator Changes

1. Collect variadic arguments into a list
2. Expand spread expressions before collection
3. Handle mixed literal and spread arguments

### Codegen Changes

1. Allocate list for variadic arguments
2. Populate list from call site arguments
3. For C variadics: use platform va_list ABI

### FFI Changes

1. Parse C-style `...` in extern blocks
2. Generate va_list-based calling convention
3. Require unsafe context for C variadic calls

---

## Formatting Rules

```ori
// Variadic parameter: no space after ...
@sum (numbers: ...int) -> int

// Spread in call: no space after ...
sum(...nums)
sum(1, ...middle, 10)

// Multiple arguments on one line if short
sum(1, 2, 3)

// Break to multiple lines if long
format(
    "{} logged in from {}",
    username,
    ip_address,
)
```

---

## Summary

| Feature | Syntax | Notes |
|---------|--------|-------|
| Variadic param | `items: ...T` | Receives as `[T]` |
| Empty call | `sum()` | Valid, receives `[]` |
| Multiple args | `sum(1, 2, 3)` | Collected to `[1, 2, 3]` |
| Spread | `sum(...list)` | Expand list into variadic |
| Mixed | `sum(0, ...list, 10)` | Literals and spread |
| Generic | `...T` | Type inferred from args |
| Trait object | `...Printable` | Heterogeneous via boxing |
| Minimum args | `(first: T, rest: ...T)` | Use required params |
| C variadic | `extern ... { @fn (...) }` | Unsafe, no type |

Variadic functions provide ergonomic APIs for variable-argument patterns while maintaining Ori's commitment to type safety. The `...T` syntax is familiar, composable with spread, and clearly distinguishes safe Ori variadics from unsafe C interop.
