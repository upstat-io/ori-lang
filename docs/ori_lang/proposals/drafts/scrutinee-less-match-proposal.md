# Proposal: Scrutinee-Less Match (Conditional Match)

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-03-30
**Affects:** Compiler (parser, type checker, evaluator, LLVM codegen), spec (§09 Expressions), grammar

---

## Summary

Allow `match` without a scrutinee expression, where each arm is a boolean condition evaluated top-to-bottom. The first arm whose condition is `true` produces the block's value. This is equivalent to Lisp's `cond`, Go's `switch {}`, and Kotlin's `when {}`.

```ori
match {
    !is_signed_in() -> Err(Error.NotSignedIn),
    !account_valid() -> Err(Error.AccountExpired),
    !is_admin() -> Err(Error.NotAdmin),
    _ -> Ok(()),
}
```

---

## Motivation

### The Problem

Ori has no `return` keyword by design — control flow is expression-based. For guard-clause patterns (check conditions, bail early), Ori offers `?`, `if-then-else` chains, and `match` on tuples. Each has drawbacks for the common "check N conditions, each with a different result" pattern:

**`?` operator** — only works with `Result`/`Option`, requires wrapping each check:
```ori
@access_admin_panel () -> Result<void, Error> = {
    if !is_signed_in() then Err(Error.NotSignedIn)?;
    if !account_valid() then Err(Error.AccountExpired)?;
    if !is_admin() then Err(Error.NotAdmin)?;
    Ok(())
}
```

**`if-then-else` chain** — works but grows unwieldy with many branches:
```ori
@access_admin_panel () -> Result<void, Error> =
    if !is_signed_in() then Err(Error.NotSignedIn)
    else if !account_valid() then Err(Error.AccountExpired)
    else if !is_admin() then Err(Error.NotAdmin)
    else Ok(());
```

**`match` on tuple** — forces all conditions to be evaluated upfront:
```ori
@access_admin_panel () -> Result<void, Error> =
    match (is_signed_in(), account_valid(), is_admin()) {
        (false, _, _) -> Err(Error.NotSignedIn),
        (_, false, _) -> Err(Error.AccountExpired),
        (_, _, false) -> Err(Error.NotAdmin),
        _ -> Ok(()),
    };
```

The tuple approach evaluates all conditions eagerly and requires tracking positional bools — it doesn't short-circuit and becomes unreadable beyond 3-4 conditions.

### The Solution

Scrutinee-less `match` — a `cond` table:

```ori
@access_admin_panel () -> Result<void, Error> = match {
    !is_signed_in() -> Err(Error.NotSignedIn),
    !account_valid() -> Err(Error.AccountExpired),
    !is_admin() -> Err(Error.NotAdmin),
    _ -> Ok(()),
};
```

- Flat, readable, one expression
- Short-circuits (each condition evaluated only if prior conditions were false)
- Scales to any number of conditions
- Uses existing `match` arm syntax — no new keywords or constructs
- `_` serves as the default/else branch

---

## Prior Art

| Language | Syntax | Notes |
|----------|--------|-------|
| Go | `switch { case cond: ... }` | Switch with no expression |
| Kotlin | `when { cond -> expr }` | When with no argument |
| Lisp/Scheme | `(cond (test expr) ...)` | The original |
| Clojure | `(cond test expr ...)` | Same as Lisp |
| Ruby | `case; when cond then ... end` | Case with no expression |
| Elixir | `cond do test -> expr end` | Explicit `cond` keyword |
| Rust | Not supported | Must use `match ()` with `_ if` guards |
| Haskell | Not directly | Uses guards `| cond = expr` on function definitions |

This is a well-established pattern across many language families. Go and Kotlin are the most syntactically similar to what Ori would do.

---

## Design

### Syntax

```ebnf
match_expr = "match" ( expression "{" match_arms "}" | "{" cond_arms "}" ) .
cond_arms  = [ cond_arm { "," cond_arm } [ "," ] ] .
cond_arm   = ( expression | "_" ) "->" expression .
```

When the parser sees `match {`, it parses conditional arms. When it sees `match expr {`, it parses pattern-matching arms as today.

### Semantics

1. Arms are evaluated top-to-bottom
2. Each arm's left side is a `bool` expression (type-checked to `bool`)
3. The first arm whose condition evaluates to `true` produces the match value
4. `_` matches unconditionally (equivalent to `true ->`)
5. All arm bodies must unify to the same type
6. Short-circuit evaluation — subsequent conditions are not evaluated after a match

### Exhaustiveness

A scrutinee-less match **shall** require a `_` (default) arm. Without it, the compiler cannot statically prove that at least one condition will be true. This differs from pattern `match` where exhaustiveness can be checked structurally.

```ori
// Error: non-exhaustive conditional match (no default arm)
match {
    x > 0 -> "positive",
    x < 0 -> "negative",
}

// OK: default arm present
match {
    x > 0 -> "positive",
    x < 0 -> "negative",
    _ -> "zero",
}
```

### Type of Arms

Each arm's condition must be `bool`. Non-bool conditions are a type error:

```ori
// Error: arm condition must be bool, found int
match {
    x + 1 -> "one",
    _ -> "other",
}
```

---

## Examples

### Guard Clauses (the motivating example)

```ori
@access_admin_panel () -> Result<void, Error> = match {
    !is_signed_in() -> Err(Error.NotSignedIn),
    !account_valid() -> Err(Error.AccountExpired),
    !is_admin() -> Err(Error.NotAdmin),
    _ -> Ok(()),
};
```

### FizzBuzz

```ori
@fizzbuzz (n: int) -> str = match {
    n % 15 == 0 -> "fizzbuzz",
    n % 3 == 0 -> "fizz",
    n % 5 == 0 -> "buzz",
    _ -> str(n),
};
```

### Classification

```ori
@classify (n: int) -> str = match {
    n < 0 -> "negative",
    n == 0 -> "zero",
    _ -> "positive",
};
```

### HTTP Status

```ori
@status_message (code: int) -> str = match {
    code < 200 -> "informational",
    code < 300 -> "success",
    code < 400 -> "redirect",
    code < 500 -> "client error",
    _ -> "server error",
};
```

### With Side Effects

```ori
@process (input: str) -> Result<Output, Error> = match {
    input.is_empty() -> Err(Error.Empty),
    !validate(input:) -> Err(Error.Invalid),
    cache.contains(key: input) -> Ok(cache.get(key: input)),
    _ -> Ok(compute(input:)),
};
```

---

## Desugaring

A scrutinee-less match desugars to a chain of `if-then-else` expressions:

```ori
// Source
match {
    cond1 -> expr1,
    cond2 -> expr2,
    _ -> expr3,
}

// Desugars to
if cond1 then expr1
else if cond2 then expr2
else expr3
```

This means:
- No new evaluation semantics needed — it's sugar over existing `if-then-else`
- Short-circuit behavior comes for free
- Type unification works the same way

### Implementation Strategy

The simplest implementation desugars in the parser or canonicalization phase:

1. **Parser**: Detect `match {` (no scrutinee). Parse arms as `expression -> expression`.
2. **Canonicalization**: Desugar to nested `if-then-else` before type checking.
3. **Type checker**: No changes needed — sees `if-then-else`.
4. **Evaluator**: No changes needed.
5. **LLVM codegen**: No changes needed.

Alternative: handle natively through all phases for better error messages and diagnostics.

---

## Interaction with Other Features

### With `try` blocks

```ori
@load_config () -> Result<Config, Error> = try {
    let $path = match {
        env_has("CONFIG_PATH") -> env_get("CONFIG_PATH"),
        file_exists("./config.ori") -> "./config.ori",
        _ -> "/etc/app/config.ori",
    };
    parse_config(path:)?
}
```

### With `for...yield`

```ori
for item in items yield match {
    item.is_premium() -> item.price * 0.9,
    item.is_bulk() -> item.price * 0.8,
    _ -> item.price,
}
```

### With pipe operator

```ori
data
    |> transform()
    |> (x -> match {
        x.len() > 100 -> x.take(count: 100),
        x.is_empty() -> default_data(),
        _ -> x,
    })
```

---

## Grammar Change

```diff
- match_expr = "match" expression "{" match_arms "}" .
+ match_expr = "match" expression "{" match_arms "}"
+            | "match" "{" cond_arms "}" .
+ cond_arms  = [ cond_arm { "," cond_arm } [ "," ] ] .
+ cond_arm   = ( expression | "_" ) "->" expression .
```

---

## Alternatives Considered

### A. New `cond` keyword

```ori
cond {
    !is_signed_in() -> Err(Error.NotSignedIn),
    _ -> Ok(()),
}
```

Rejected. Adds a new keyword for something `match` already conceptually covers. Ori's `match` is already the "pick one branch" construct — extending it to boolean conditions is natural.

### B. `match true { cond -> ... }`

```ori
match true {
    !is_signed_in() -> Err(Error.NotSignedIn),
    _ -> Ok(()),
}
```

Technically works today if arms are treated as patterns, but `true` is a dummy scrutinee and the arms would need to be boolean literals or guards, not expressions. Awkward and misleading.

### C. Do nothing — use `if-then-else`

The `if-then-else` chain works, but scrutinee-less `match` is visually superior for 3+ conditions. The arms align vertically, each condition and result are on the same line, and the `_` default is explicit. For 2 conditions, `if-then-else` remains preferable.

---

## Summary

| Aspect | Before | After |
|--------|--------|-------|
| Guard-clause pattern | `if-then-else` chain or `?` | `match { cond -> expr, _ -> expr }` |
| Boolean dispatch | `if-then-else` or `match` on tuple | `match { ... }` |
| New keywords | — | None |
| Grammar change | — | One new `match_expr` alternative |
| Parser change | — | Detect `match {` vs `match expr {` |
| Type checker change | — | None (desugars to `if-then-else`) |
| Evaluator change | — | None (desugars to `if-then-else`) |
| LLVM change | — | None (desugars to `if-then-else`) |
