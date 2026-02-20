# Proposal: Pre/Post Checks for the `run` Pattern

**Status:** Approved (syntax superseded)
**Author:** Eric
**Created:** 2026-01-21
**Approved:** 2026-01-28

> **Errata (2026-02-19):** The syntax in this proposal (`pre_check:`/`post_check:` inside `run()`) has been superseded by `block-expression-syntax.md`. Contracts now use function-level `pre()`/`post()` declarations. The semantic decisions (evaluation order, scope, types, messages, desugaring) remain valid.

---

## Summary

Extend the `run` pattern with optional `pre_check:` and `post_check:` properties to support contract-style defensive programming without introducing new syntax or keywords.

```ori
@divide (a: int, b: int) -> int = {
    pre_check: b != 0
    a div b
    post_check: r -> r * b <= a
}
```

---

## Motivation

### The Problem

Defensive programming requires checking preconditions and postconditions:

- **Preconditions**: What must be true before a function executes
- **Postconditions**: What must be true after a function completes

Currently in Ori, these checks are ad-hoc:

```ori
// Current approach: manual, verbose, inconsistent
@divide (a: int, b: int) -> int = {
    if b == 0 then panic(msg: "divisor must be non-zero")
    let result = a div b
    if !(result * b <= a) then panic(msg: "postcondition failed")
    result
}
```

Problems with this approach:

1. Boilerplate obscures the actual logic
2. No standard pattern for contracts
3. Easy to forget or skip checks
4. Postconditions require manual result binding

### Prior Art: C++26 Contracts

C++26 introduces dedicated contract syntax:

```cpp
int divide(int a, int b)
    pre(b != 0)
    post(r: r * b <= a)
{ return a / b; }
```

This required:

- New contextual keywords (`pre`, `post`, `contract_assert`)
- New grammar productions
- Four evaluation semantics (ignore, observe, enforce, quick-enforce)
- Violation handler mechanism
- Years of committee standardization

### The Ori Way

Instead of new syntax, extend what already exists. The `run` pattern is:

- Already ubiquitous in Ori code
- Already accepts named properties
- Already sequences operations

Adding `pre_check:` and `post_check:` as optional properties requires zero new concepts.

---

## Design

### Syntax

```ori
{
    pre_check: condition,                    // Optional: checked before body
    pre_check: condition | "message",        // Optional: with custom message
    pre_check: another_condition,            // Optional: multiple checks allowed

    // ... body statements and expressions ...

    final_expression

    post_check: result -> condition,         // Optional: checked after body
    post_check: result -> condition | "msg", // Optional: with custom message
}
```

### Grammar

```ebnf
run_expr        = "run" "(" [ run_prechecks ] { binding "," } expression [ run_postchecks ] ")" .
run_prechecks   = { "pre_check" ":" check_expr "," } .
run_postchecks  = { "," "post_check" ":" postcheck_expr } .
check_expr      = expression [ "|" string_literal ] .
postcheck_expr  = lambda_params "->" check_expr .
```

### Positional Constraints (Parser-Enforced)

**`pre_check:` must appear first. `post_check:` must appear last.**

This is enforced by the parser, not convention. The following is a syntax error:

```ori
// ERROR: pre_check: must appear before body statements
{
    let x = compute()
    pre_check: input > 0,  // Syntax error!
    x + 1
}

// ERROR: post_check: must appear after all body statements
{
    post_check: r -> r > 0,  // Syntax error!
    let x = compute()
    x + 1
}
```

Valid ordering:

```ori
{
    pre_check: a > 0,           // First: all pre_checks
    pre_check: b > 0,           // (can have multiple)
    let x = a + b,              // Then: body
    let y = x * 2
    y,                          // Last expression before post_check
    post_check: r -> r > 0,     // Last: all post_checks
    post_check: r -> r < 1000
}
```

This constraint ensures the semantics are unambiguous: pre_checks run before any body code, post_checks run after all body code.

### Semantics

#### Evaluation Order

1. Evaluate all `pre_check:` conditions in order
2. If any `pre_check:` fails, panic with message
3. Execute body statements and final expression
4. Bind result to each `post_check:` lambda parameter
5. Evaluate all `post_check:` conditions in order
6. If any `post_check:` fails, panic with message
7. Return result

#### Scope Constraints

- `pre_check:` expressions may only reference bindings visible in the enclosing scope (function parameters, module-level constants, and bindings from outer `run` blocks). Bindings created within the same `run` body are not visible to `pre_check:`.
- `post_check:` lambdas may reference the result (via the lambda parameter) plus all bindings visible to `pre_check:` plus bindings created in the `run` body.

#### Type Constraints

- `pre_check:` condition must have type `bool`
- `post_check:` must be a lambda from the result type to `bool`
- It is a compile-time error to use `post_check:` when the `run` body evaluates to `void`
- Message expressions (after `|`) must have type `str`

#### Desugaring

```ori
{
    pre_check: P
    body
    post_check: r -> Q
}

// Desugars to:
{
    if !P then panic(msg: "pre_check failed: P"),  // "P" is source text
    let __result = body
    if !Q(__result) then panic(msg: "post_check failed: Q"),  // "Q" is source text
    __result
}
```

The compiler embeds the condition's source text as a string literal for default messages.

#### With Custom Messages

```ori
{
    pre_check: x > 0 | "x must be positive"
    body
}

// Desugars to:
{
    if !(x > 0) then panic(msg: "x must be positive")
    body
}
```

#### Multiple Checks

```ori
{
    pre_check: A | "first check"
    pre_check: B
    body
    post_check: r -> C
    post_check: r -> D | "fourth check"
}

// Desugars to:
{
    if !A then panic(msg: "first check")
    if !B then panic(msg: "pre_check failed: B")
    let __result = body
    if !C(__result) then panic(msg: "post_check failed: C")
    if !D(__result) then panic(msg: "fourth check")
    __result
}
```

---

## Examples

### Basic Usage

```ori
@abs (x: int) -> int = {
    if x < 0 then -x else x
    post_check: r -> r >= 0
}

@sqrt (x: float) -> float = {
    pre_check: x >= 0.0
    newton_raphson(x: x)
    post_check: r -> r >= 0.0
}

@get<T> (items: [T], index: int) -> T = {
    pre_check: index >= 0 && index < len(collection: items)
    items[index]
}
```

### Multiple Conditions

```ori
@binary_search<T: Comparable> (items: [T], target: T) -> Option<int> = {
    pre_check: len(collection: items) > 0 | "items must not be empty"
    pre_check: is_sorted(items: items) | "items must be sorted"
    binary_search_impl(items: items, target: target, lo: 0, hi: len(collection: items))
    post_check: r -> match r {
        Some(i) -> items[i] == target
        None -> !items.contains(value: target)
    }
}
```

### Financial Example

```ori
@transfer (from: Account, to: Account, amount: int) -> (Account, Account) = {
    pre_check: amount > 0 | "transfer amount must be positive"
    pre_check: from.balance >= amount | "insufficient funds"
    pre_check: from.id != to.id | "cannot transfer to same account"
    let new_from = Account { id: from.id, balance: from.balance - amount }
    let new_to = Account { id: to.id, balance: to.balance + amount }
    (new_from, new_to)
    post_check: (f, t) -> f.balance == from.balance - amount
    post_check: (f, t) -> t.balance == to.balance + amount
    post_check: (f, t) -> f.balance + t.balance == from.balance + to.balance
}
```

### Composing with Other Patterns

```ori
// check + try
@safe_divide (a: int, b: int) -> Result<int, MathError> = {
    try {
        if b == 0 then Err(MathError.DivideByZero)
        Ok(a div b)
    }
    post_check: r -> is_ok(result: r) || b == 0
}

// check + validate
@create_user (input: UserInput) -> Result<User, [str]> = {
    pre_check: input.source == "trusted"
    validate(
        rules: [
            len(collection: input.name) > 0 | "name required"
            input.age >= 0 | "invalid age"
        ]
        then: User { name: input.name, age: input.age }
    )
}
```

### Testing Checks

```ori
@test_divide tests @divide () -> void = {
    // Normal cases
    assert_eq(actual: divide(a: 10, b: 2), expected: 5)
    assert_eq(actual: divide(a: 7, b: 3), expected: 2)

    // Pre-check violations
    assert_panics(f: () -> divide(a: 10, b: 0))
}

@test_sqrt tests @sqrt () -> void = {
    // Normal cases
    assert_eq(actual: sqrt(x: 4.0), expected: 2.0)
    assert_eq(actual: sqrt(x: 0.0), expected: 0.0)

    // Pre-check violations
    assert_panics(f: () -> sqrt(x: -1.0))
}
```

---

## Comparison

### vs. C++26 Contracts

| Aspect | C++26 | Ori |
|--------|-------|-----|
| New keywords | Yes (`pre`, `post`, `contract_assert`) | No |
| New grammar | Yes | Minimal extension to `run` |
| Learning curve | Moderate | Zero (if you know `run`) |
| Visible in signature | Yes | No (in body) |
| Incremental adoption | Requires refactoring | Add properties to existing `run` |

### vs. Manual Checks

```ori
// Before: manual, verbose
@sqrt (x: float) -> float = {
    if x < 0.0 then panic(msg: "x must be non-negative")
    let result = newton_raphson(x: x)
    if result < 0.0 then panic(msg: "result must be non-negative")
    result
}

// After: declarative, clear
@sqrt (x: float) -> float = {
    pre_check: x >= 0.0
    newton_raphson(x: x)
    post_check: r -> r >= 0.0
}
```

### vs. Separate `check` Pattern

A dedicated `check` pattern was considered:

```ori
@sqrt (x: float) -> float = check(
    pre: x >= 0.0,
    body: newton_raphson(x: x),
    post: r -> r >= 0.0,
)
```

Rejected because:

- Adds a new pattern to learn
- `run` is already ubiquitous
- `pre_check`/`post_check` on `run` is more incremental

---

## Design Rationale

### Why Named Properties?

Ori patterns use named properties (`over:`, `transform:`, `predicate:`). This is consistent:

```ori
for(over: items, match: Some(x) -> x, default: 0)
recurse(condition: n <= 1, base: n, step: self(n - 1))
{pre_check: x > 0, body, post_check: r -> r > x}
```

### Why `pre_check` and `post_check`?

Alternatives considered:

| Name | Problem |
|------|---------|
| `pre:` / `post:` | Ambiguous - pre/post what? |
| `requires:` / `ensures:` | Longer, less obvious |
| `precondition:` / `postcondition:` | Too verbose |
| `pre_check:` / `post_check:` | Clear, explicit, action-oriented |

### Why `|` for Messages?

The `|` operator is already used for:
- Bitwise or (`a | b`)
- Or-patterns in match (`A | B`)
- Sum type variants

However, in the context of `pre_check:` and `post_check:`, `|` unambiguously introduces a message string because:
- It appears after a boolean condition
- It's followed by a string literal

The parser can disambiguate based on context, similar to how `..` is both range syntax and rest pattern syntax.

### Why Not in the Signature?

C++26 puts contracts on declarations:

```cpp
int sqrt(float x) pre(x >= 0) post(r: r >= 0);
```

Ori puts them in the body:

```ori
@sqrt (x: float) -> float = {
    pre_check: x >= 0.0
    ...
}
```

Tradeoffs:

| Signature (C++26) | Body (Ori) |
|-------------------|------------|
| Visible without reading body | Requires looking at implementation |
| Clutters function signature | Keeps signature clean |
| Requires new syntax | Uses existing pattern syntax |
| All-or-nothing | Incremental adoption |

Ori's choice is consistent with its philosophy:

- Mandatory tests already require looking beyond signatures
- Doc comments can expose contracts
- Tooling (LSP) can surface checks on hover
- Function bodies are typically short

---

## Implementation Notes

### Parser Changes

Extend the `run` pattern parser to recognize `pre_check:` and `post_check:` properties. Enforce positional constraints:
- `pre_check:` must appear before any body bindings/expressions
- `post_check:` must appear after the final body expression

### Type Checking

- `pre_check:` expression must have type `bool`
- `post_check:` must be a lambda from result type to `bool`
- Compile error if `post_check:` used with void body
- Message expressions (after `|`) must have type `str`

### Code Generation

Desugar to explicit conditional checks and panics during AST lowering. The compiler embeds the source text of conditions for default error messages.

### Error Messages

```
PANIC at src/math.ori:12:5
  in function: @sqrt
  pre_check failed: x >= 0.0
    x = -4.0

  stack trace:
    @sqrt (src/math.ori:12)
    @main (src/main.ori:5)
```

---

## Future Extensions

### Check Modes

A future proposal may introduce configurable check behavior:

```ori
$check_mode = enforce  // Default

// Possible modes:
// - enforce: Check condition, panic on failure
// - observe: Check condition, log warning on failure, continue
// - ignore: Skip all checks (production performance)
```

This is deferred to allow the core pre_check/post_check mechanism to be established first.

### Invariant Checks

For types with invariants:

```ori
type BankAccount = { id: str, balance: int }
    invariant: self.balance >= 0

// All functions returning BankAccount automatically check invariant
```

### Static Analysis

Tooling could verify that callers satisfy callees' preconditions:

```ori
@caller () -> int = {
    let x = -5
    sqrt(x: float(x))  // Warning: sqrt pre_check (x >= 0.0) may fail
}
```

---

## Summary

The `pre_check:` and `post_check:` properties for `run` provide contract-style defensive programming that:

1. **Requires minimal new syntax** — Named properties on existing pattern
2. **Has zero learning curve** — If you know `run`, you know this
3. **Enables incremental adoption** — Add checks to existing code without refactoring
4. **Is purely Ori** — Patterns + named properties, nothing foreign
5. **Solves real problems** — Clear preconditions, postconditions, better error messages

The design prioritizes consistency with Ori's existing patterns over visibility in function signatures, a tradeoff appropriate for a language with mandatory testing and short function bodies.

---

## Errata (added 2026-02-19)

> **Superseded by `block-expression-syntax`**: The approved block expression syntax proposal removes `run()` from the language entirely, replacing it with `{ }` block expressions. Since contracts (`pre_check:`/`post_check:`) were housed inside `run()`, they move to **function-level declarations** using C++-style `pre()`/`post()` syntax:
>
> ```ori
> // Old (this proposal): contracts inside {}
> @divide (a: int, b: int) -> int = {
>     pre_check: b != 0
>     a div b
>     post_check: r -> r * b <= a
> }
>
> // New (block-expression-syntax): contracts on function declaration
> @divide (a: int, b: int) -> int
>     pre(b != 0)
>     post(r -> r * b <= a)
> = {
>     a div b
> }
> ```
>
> **What changed:**
> - **Placement**: Body-level (`run()`) -> function-level (between signature and `=`)
> - **Keywords**: `pre_check`/`post_check` -> `pre`/`post`
> - **Syntax**: Named properties (`pre_check: expr`) -> function-like (`pre(expr)`)
>
> **What is preserved:**
> - Evaluation order (pre before body, post after body)
> - Scope constraints (pre sees params only, post sees result)
> - Type constraints (pre: bool, post: T -> bool, no post on void)
> - Message syntax (`| "message"`)
> - Desugaring to conditional panic
> - Multiple checks allowed
>
> The "Why Not in the Signature?" rationale (Section: Design Rationale) was based on the premise "extend what already exists (`run`)." With `run()` removed, that premise no longer holds, and function-level placement becomes the natural choice.
