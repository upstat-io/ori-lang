# Proposal: Require Parameter Differentiation for Multi-Clause Functions

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-03-30
**Affects:** Compiler (type checker, canonicalization), spec (§08 Declarations)

---

## Summary

Restrict multi-clause function definitions to require at least one parameter with a differentiating pattern. Guard-only clauses on identical parameter signatures (no pattern variation) shall be a compile error, preventing accidental behavioral shadowing.

---

## Problem Statement

Currently, Ori permits multiple clauses of a function with identical parameter lists differentiated solely by `if` guards:

```ori
@check () -> str if !is_signed_in() = "not signed in";
@check () -> str if !account_valid() = "account expired";
@check () -> str = "ok";
```

This compiles and runs correctly — the clauses desugar into a decision tree with guard nodes. However, it introduces a subtle hazard:

### Silent Behavioral Shadowing

In a large file, a developer (or AI tool) could add a guarded clause to an existing zero-parameter function without realizing they are creating a multi-clause function:

```ori
// line 10: original function
@process () -> str = expensive_computation();

// ... 400 lines later ...

// line 410: someone adds this, not realizing @process already exists
@process () -> str if debug_mode() = "debug shortcut";
```

Because clauses are matched top-to-bottom, the new clause at line 410 is unreachable (the unguarded clause at line 10 always matches first). Or worse — if the original had a guard too, the new clause could silently change behavior depending on ordering.

**With parameter patterns, this hazard does not exist.** Two clauses like `@fib (0: int)` and `@fib (n: int)` are visually and semantically distinct — it's clear they are part of the same multi-clause function. Two clauses with identical `()` or `(n: int)` parameters and only guard differences look like duplicate function definitions, not intentional clause grouping.

### The Alternatives Are Better

Every use case for guard-only clauses has a clearer single-function equivalent:

**`if-then-else` chain (one expression):**
```ori
@check () -> str =
    if !is_signed_in() then "not signed in"
    else if !account_valid() then "account expired"
    else "ok";
```

**`match` on a computed value:**
```ori
@check () -> str = match (is_signed_in(), account_valid()) {
    (false, _) -> "not signed in",
    (_, false) -> "account expired",
    _ -> "ok",
};
```

**`?` for error propagation:**
```ori
@check () -> Result<void, Error> = {
    if !is_signed_in() then Err(Error.NotSignedIn)?;
    if !account_valid() then Err(Error.AccountExpired)?;
    Ok(())
}
```

All three are contained in a single function body — no risk of accidental shadowing across distant file locations.

---

## Proposed Rule

**Multi-clause functions shall require at least one parameter position where clauses differ by pattern.** "Differ by pattern" means at least one of:

1. **Different literal patterns**: `@f (0: int)` vs `@f (n: int)`
2. **Different constructor patterns**: `@f (Some(x): Option<T>)` vs `@f (None)`
3. **Different structural patterns**: `@f ([]: [T])` vs `@f ([x, ..xs])`
4. **Different wildcard vs binding**: `@f (_: T)` vs `@f (x: T)` counts only when paired with other differentiating clauses

Guards alone do **not** satisfy the differentiation requirement.

### Valid (parameter differentiation):

```ori
// Literal patterns differ
@factorial (0: int) -> int = 1;
@factorial (n: int) -> int = n * factorial(n: n - 1);

// Constructor patterns differ
@unwrap (Some(x): Option<int>) -> int = x;
@unwrap (None) -> int = 0;

// Pattern + guard is fine (pattern provides differentiation)
@classify (0: int) -> str = "zero";
@classify (n: int) -> str if n < 0 = "negative";
@classify (n: int) -> str = "positive";
```

### Invalid (guard-only differentiation):

```ori
// Error: clauses have identical parameter patterns, differ only by guard
@check () -> str if !signed_in() = "not signed in";
@check () -> str if !valid() = "expired";
@check () -> str = "ok";

// Error: same parameter binding, only guards differ
@describe (n: int) -> str if n < 0 = "negative";
@describe (n: int) -> str if n == 0 = "zero";
@describe (n: int) -> str = "positive";
```

The second example (`describe`) requires using `if-then-else`, `match`, or introducing a literal pattern for the zero case:

```ori
// Option A: single function with if-then-else
@describe (n: int) -> str =
    if n < 0 then "negative"
    else if n == 0 then "zero"
    else "positive";

// Option B: match
@describe (n: int) -> str = match n {
    0 -> "zero",
    n if n < 0 -> "negative",
    _ -> "positive",
};

// Option C: use a literal pattern to differentiate
@describe (0: int) -> str = "zero";
@describe (n: int) -> str if n < 0 = "negative";
@describe (n: int) -> str = "positive";
```

Option C is valid because the `0` literal pattern provides parameter differentiation.

---

## Error Design

```
error[E????]: multi-clause function requires parameter pattern differentiation
  --> src/app.ori:5:1
   |
 5 | @check () -> str if !signed_in() = "not signed in";
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ this clause
   |
 8 | @check () -> str = "ok";
   | ^^^^^^^^^^^^^^^^^^^^^^^^ has identical parameters to this clause
   |
   = note: clauses differ only by guard, not by parameter pattern
   = help: use `if-then-else` or `match` inside a single function body
```

---

## Impact

### What Changes

- **Type checker / canonicalization**: Before desugaring multi-clause functions into a decision tree, verify that at least one parameter position has differing patterns across clauses. Emit error if all parameter positions are identical and differentiation is guard-only.
- **Spec (§08)**: Add normative text: "All clauses of a multi-clause function shall include at least one parameter position where clauses are differentiated by pattern. Clauses that differ only by guard clause are ill-formed."
- **Grammar**: No change (the grammar permits guards; this is a semantic restriction).

### What Does NOT Change

- Single-clause functions with guards remain valid: `@f (n: int) -> int if n > 0 = n;` followed by `@f (n: int) -> int = -n;` is still valid **if** there's pattern differentiation (there isn't in this example — this would now be an error too, which is the intended behavior).
- `match` with guards inside a single function body is unaffected.
- `for...if` guard syntax is unaffected.

### Migration

Any existing code using guard-only multi-clause functions must be rewritten as a single function with `if-then-else` or `match`. Since this feature is currently undocumented and the spec test file (`clause_params.ori`) is entirely commented out, the migration impact is expected to be zero for external users.

---

## Prior Art

| Language | Requires param differentiation? | Notes |
|----------|-------------------------------|-------|
| Erlang | No (guards alone OK) | But function clauses are visually grouped with `;` separator — harder to accidentally split |
| Elixir | No (guards alone OK) | Same as Erlang; `defp` grouping helps visibility |
| Haskell | No (guards alone OK) | Guards with `\|` are visually part of one equation |
| Scala | N/A (no multi-clause) | Uses `match` instead |
| Rust | N/A (no multi-clause) | Uses `match` instead |

Erlang/Elixir/Haskell permit guard-only differentiation, but their syntax makes clause grouping visually obvious (semicolons, `|` guards, indentation-based). Ori's clause syntax — separate `@name` declarations that can be anywhere in a file — makes accidental shadowing a realistic hazard that those languages don't share.

---

## Alternatives Considered

### A. Allow guard-only clauses but warn

Rejected. A warning that should always be heeded is just a soft error. If the pattern is always wrong, make it an error.

### B. Require clauses to be lexically adjacent

This would prevent the "400 lines apart" shadowing problem but still allows confusing code. It also requires defining "adjacent" (what about intervening comments? tests? constants?). The parameter-differentiation rule is simpler and more principled.

### C. Do nothing

The current behavior works correctly. However, "works" and "is a good idea" are different. The guard-only pattern has no advantage over `if-then-else` or `match` and introduces a footgun.

---

## Summary

| Aspect | Current | Proposed |
|--------|---------|----------|
| Guard-only multi-clause | Allowed | Error |
| Pattern-differentiated multi-clause | Allowed | Allowed (unchanged) |
| Pattern + guard multi-clause | Allowed | Allowed (unchanged) |
| Single function with guards in `match` | Allowed | Allowed (unchanged) |
