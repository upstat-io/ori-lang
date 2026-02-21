# Proposal: Pipe Operator

**Status:** Rejected
**Author:** Eric
**Created:** 2026-01-25
**Rejected:** 2026-01-28

---

## Rejection Rationale

The pipe operator solves a problem that Ori doesn't have. Ori already provides multiple mechanisms for readable data transformation chains:

1. **Method chaining**: Collections have `.map()`, `.filter()`, `.fold()`, etc. as methods
2. **Extension methods**: The `extend` keyword allows adding methods to any type
3. **`run` blocks**: Sequential bindings with explicit intermediate values

Languages like Elixir and F# need pipe operators because they lack methods (Elixir) or rely heavily on curried free functions (F#/OCaml). Ori's object-oriented method syntax combined with extension methods covers these use cases more idiomatically.

**Example — what pipe would enable:**
```ori
data |> filter(predicate: x -> x > 0) |> map(transform: x -> x * 2)
```

**Already works in Ori:**
```ori
data.filter(predicate: x -> x > 0).map(transform: x -> x * 2)
```

Adding the pipe operator would introduce redundant syntax without meaningful benefit.

---

## Summary

Add a pipe operator `|>` for left-to-right function composition, enabling readable data transformation chains.

```ori
// Current (nested calls)
sum(filter(map(data, x -> x * 2), x -> x > 0))

// With pipe
data |> map(x -> x * 2) |> filter(x -> x > 0) |> sum()
```

---

## Motivation

### The Problem

Data transformations often chain multiple operations. Currently this requires either:

**Nested calls (inside-out reading):**
```ori
let result = join(
    separator: ", ",
    items: map(
        transform: u -> u.name,
        over: filter(
            predicate: u -> u.active,
            over: users,
        ),
    ),
)
```

**Method chaining (when available):**
```ori
let result = users
    .filter(predicate: u -> u.active)
    .map(transform: u -> u.name)
    .join(separator: ", ")
```

Problems:
1. Nested calls read inside-out, opposite to data flow
2. Method chaining only works for methods on the type
3. Free functions can't be chained
4. Deep nesting is hard to read and edit

### Prior Art

| Language | Syntax | Notes |
|----------|--------|-------|
| Elixir | `data \|> func()` | First arg piped |
| F# | `data \|> func` | Last arg piped |
| OCaml | `data \|> func` | Last arg piped |
| Hack | `data \|> func($$)` | Explicit placeholder |
| R | `data %>% func()` | magrittr package |
| JavaScript | `data \|> func(%)` | Stage 2 proposal |

### The Ori Way

Ori already uses named arguments, making the pipe target explicit:

```ori
users |> filter(predicate: u -> u.active, over: _)
```

The `_` placeholder shows exactly where the piped value goes. No ambiguity about first vs last argument.

---

## Design

### Syntax

```
pipe_expr = expression "|>" expression .
```

The pipe operator `|>` passes the left-hand value to the right-hand expression.

### Placeholder `_`

The right-hand side must contain exactly one `_` placeholder indicating where the piped value is inserted:

```ori
data |> process(input: _, options: defaults)
//               ^^^^^ piped value goes here
```

### Basic Usage

```ori
// Single pipe
5 |> double(x: _)  // double(x: 5)

// Chain
5 |> double(x: _) |> square(n: _) |> str(_)
// Evaluates: str(square(n: double(x: 5)))

// With collections
[1, 2, 3, 4, 5]
    |> filter(predicate: x -> x > 2, over: _)
    |> map(transform: x -> x * 2, over: _)
    |> sum(items: _)
// Result: 24
```

### Method Shorthand

When piping to a method call on the value itself, omit the placeholder:

```ori
// These are equivalent:
items |> _.len()
items |> len()

// Method chaining via pipe
"hello"
    |> _.to_upper()
    |> _.trim()
    |> _.split(separator: " ")
```

### Precedence

`|>` has lower precedence than most operators but higher than assignment:

| Prec | Operators |
|------|-----------|
| ... | ... |
| 14 | `??` |
| **15** | **`\|>`** |
| 16 | `=` (assignment) |

```ori
// Parsed as: (a + b) |> process(_)
a + b |> process(input: _)

// Parentheses for clarity when needed
a |> (b |> process(x: _, y: _))  // Error: two placeholders
```

### Associativity

Left-to-right associative:

```ori
a |> f(_) |> g(_) |> h(_)
// Equivalent to:
((a |> f(_)) |> g(_)) |> h(_)
// Equivalent to:
h(g(f(a)))
```

---

## Examples

### Data Processing Pipeline

```ori
let report = transactions
    |> filter(predicate: t -> t.date >= start_date, over: _)
    |> group_by(key: t -> t.category, over: _)
    |> map(transform: (cat, txns) -> run(
        let total = txns |> map(transform: t -> t.amount, over: _) |> sum(items: _),
        CategoryTotal { category: cat, total: total },
    ), over: _)
    |> sort_by(key: c -> c.total, descending: true, over: _)
```

### String Processing

```ori
let slug = title
    |> _.to_lower()
    |> _.trim()
    |> _.replace(pattern: " ", with: "-")
    |> _.replace(pattern: "[^a-z0-9-]", with: "")
```

### Combining with Patterns

```ori
// Pipe into try
input
    |> validate(data: _)
    |> try(
        let validated = _?,
        let processed = transform(validated),
        Ok(processed),
    )

// Pipe into match
status_code
    |> match(_,
        200 -> "OK",
        404 -> "Not Found",
        500 -> "Server Error",
        _ -> "Unknown",
    )
```

### HTTP Request Building

```ori
let response = Request.new()
    |> _.with_method(method: "POST")
    |> _.with_url(url: `{$api_base}/users`)
    |> _.with_header(name: "Content-Type", value: "application/json")
    |> _.with_body(body: user_json)
    |> _.send()
```

### Avoiding Deep Nesting

```ori
// Without pipe (deeply nested)
let result = serialize(
    format: "json",
    data: sort(
        by: "name",
        items: filter(
            predicate: is_active,
            items: fetch_users(),
        ),
    ),
)

// With pipe (flat, readable)
let result = fetch_users()
    |> filter(predicate: is_active, items: _)
    |> sort(by: "name", items: _)
    |> serialize(format: "json", data: _)
```

---

## Design Rationale

### Why Require Placeholder?

Alternatives considered:

| Approach | Example | Problem |
|----------|---------|---------|
| First argument | `x \|> f(y)` = `f(x, y)` | Conflicts with named args |
| Last argument | `x \|> f(y)` = `f(y, x)` | Arbitrary, surprising |
| **Explicit placeholder** | `x \|> f(arg: _)` | Clear, works with named args |

Ori uses named arguments. The placeholder makes it explicit where the value goes — no guessing.

### Why `_` for Placeholder?

| Symbol | Precedent | Problem |
|--------|-----------|---------|
| `$$` | Hack | Two characters |
| `%` | JS proposal | Used elsewhere |
| `it` | Kotlin | Keyword conflict |
| **`_`** | Pattern matching | Already means "placeholder" in Ori |

`_` is already used in pattern matching as a wildcard/placeholder. Consistent meaning.

### Why Not Auto-Insert?

Some languages automatically insert the piped value as the first or last argument. Ori requires explicit placement because:

1. Named arguments make position ambiguous
2. Explicit is better than implicit
3. No surprises about where the value goes
4. Works with any argument position

### Why Not Just Use Methods?

Methods require the type to define them. Pipes work with:
- Free functions
- Functions from other modules
- Lambdas
- Pattern expressions

```ori
// Can't add methods to int, but can pipe
42 |> clamp(min: 0, max: 100, value: _)
```

---

## Edge Cases

### No Placeholder

Error if right-hand side has no `_`:

```ori
5 |> add(a: 1, b: 2)  // Error: pipe requires _ placeholder
```

### Multiple Placeholders

Error if right-hand side has multiple `_`:

```ori
5 |> add(a: _, b: _)  // Error: pipe allows only one _ placeholder
```

Use explicit binding for multiple uses:

```ori
5 |> (x -> add(a: x, b: x))(_)
// Or
run(
    let x = 5,
    add(a: x, b: x),
)
```

### Placeholder in Nested Expression

Placeholder binds to innermost pipe:

```ori
a |> f(x: b |> g(y: _))
// Equivalent to:
f(x: g(y: b), ...)  // Wait, where does 'a' go?
// Error: outer pipe has no placeholder
```

Each pipe needs its own placeholder:

```ori
a |> f(x: b |> g(y: _), z: _)
// Equivalent to:
f(x: g(y: b), z: a)
```

---

## Implementation Notes

### Parser Changes

Add `|>` as a binary operator with appropriate precedence.

### Desugaring

```ori
expr |> func(arg: _, other: val)

// Desugars to:
run(
    let __pipe = expr,
    func(arg: __pipe, other: val),
)
```

### Type Checking

The placeholder `_` takes the type of the left-hand expression. Type check the right-hand side with `_` bound to that type.

---

## Summary

| Aspect | Design |
|--------|--------|
| Operator | `\|>` |
| Placeholder | `_` (required, exactly one) |
| Precedence | Lower than `??`, higher than `=` |
| Associativity | Left-to-right |

The pipe operator enables readable, left-to-right data transformation chains while maintaining Ori's explicit philosophy through required placeholders.
