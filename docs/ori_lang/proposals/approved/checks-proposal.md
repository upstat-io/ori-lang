# Proposal: Pre/Post Checks for the `run` Pattern

**Status:** Draft
**Author:** Eric
**Created:** 2026-01-21

---

## Summary

Extend the `run` pattern with optional `pre_check:` and `post_check:` properties to support contract-style defensive programming without introducing new syntax or keywords.

```ori
@divide (a: int, b: int) -> int = run(
    pre_check: b != 0,
    a div b,
    post_check: r -> r * b <= a
)
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
@divide (a: int, b: int) -> int = run(
    if b == 0 then panic("divisor must be non-zero"),
    let result = a div b,
    if !(result * b <= a) then panic("postcondition failed"),
    result
)
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
run(
    pre_check: condition,                    // Optional: checked before body
    pre_check: condition | "message",        // Optional: with custom message
    pre_check: [cond1, cond2, ...],          // Optional: multiple conditions

    // ... body statements and expressions ...,

    final_expression,

    post_check: result -> condition,         // Optional: checked after body
    post_check: result -> condition | "msg", // Optional: with custom message
    post_check: result -> [cond1, cond2]     // Optional: multiple conditions
)
```

### Positional Constraints (Parser-Enforced)

**`pre_check:` must appear first. `post_check:` must appear last.**

This is enforced by the parser, not convention. The following is a syntax error:

```ori
// ERROR: pre_check: must appear before body statements
run(
    let x = compute(),
    pre_check: input > 0,  // Syntax error!
    x + 1
)

// ERROR: post_check: must appear after all body statements
run(
    post_check: r -> r > 0,  // Syntax error!
    let x = compute(),
    x + 1
)
```

Valid ordering:

```ori
run(
    pre_check: a > 0,           // First: all pre_checks
    pre_check: b > 0,           // (can have multiple)
    let x = a + b,               // Then: body
    let y = x * 2,
    y,                           // Last expression before post_check
    post_check: r -> r > 0      // Last: all post_checks
)
```

This constraint ensures the semantics are unambiguous: pre_checks run before any body code, post_checks run after all body code.

#### Why Enforce in Parser?

Alternatives considered:

| Approach | Problem |
|----------|---------|
| Convention only | Easy to misplace, ambiguous semantics |
| Compiler hoisting | Implicit behavior, violates "explicit over implicit" |
| Separate `body:` property | Adds nesting, changes `run` significantly |
| **Parser enforcement** | Clear, explicit, matches execution order |
```

### Semantics

#### Evaluation Order

1. Evaluate all `pre_check:` conditions in order
2. If any `pre_check:` fails, panic with message
3. Execute body statements and final expression
4. Bind result to `post_check:` parameter
5. Evaluate all `post_check:` conditions in order
6. If any `post_check:` fails, panic with message
7. Return result

#### Desugaring

```ori
run(
    pre_check: P,
    body,
    post_check: r -> Q
)

// Desugars to:
run(
    if !P then panic("pre_check failed: " + stringify(P)),
    let __result = body,
    if !Q(__result) then panic("post_check failed: " + stringify(Q)),
    __result
)
```

#### Multiple Conditions

```ori
run(
    pre_check: [A, B, C],
    body
)

// Desugars to:
run(
    if !A then panic("pre_check failed: " + stringify(A)),
    if !B then panic("pre_check failed: " + stringify(B)),
    if !C then panic("pre_check failed: " + stringify(C)),
    body
)
```

#### With Messages

```ori
run(
    pre_check: x > 0 | "x must be positive",
    body
)

// Desugars to:
run(
    if !(x > 0) then panic("x must be positive"),
    body
)
```

### Check Mode Configuration

Global configuration controls check behavior:

```ori
$check_mode = enforce  // Default
```

| Mode | Behavior |
|------|----------|
| `enforce` | Check condition, panic on failure |
| `observe` | Check condition, log warning on failure, continue |
| `ignore` | Skip all checks (production performance) |

#### Per-Function Override

```ori
@hot_path (x: int) -> int = run(
    pre_check: x > 0,
    check_mode: ignore,  // Override for this function
    x * 2
)
```

---

## Examples

### Basic Usage

```ori
@abs (x: int) -> int = run(
    if x < 0 then -x else x,
    post_check: r -> r >= 0
)

@sqrt (x: float) -> float = run(
    pre_check: x >= 0.0,
    newton_raphson(x),
    post_check: r -> r >= 0.0
)

@get (items: [T], index: int) -> T = run(
    pre_check: index >= 0 && index < len(items),
    items[index]
)
```

### Multiple Conditions

```ori
@binary_search (items: [T], target: T) -> Option<int> = run(
    pre_check: [
        len(items) > 0 | "items must not be empty",
        is_sorted(items) | "items must be sorted"
    ],
    binary_search_impl(items, target, 0, len(items)),
    post_check: r -> match(r,
        Some(i) -> items[i] == target,
        None -> !items.contains(target)
    )
)
```

### Financial Example

```ori
@transfer (from: Account, to: Account, amount: int) -> (Account, Account) = run(
    pre_check: [
        amount > 0 | "transfer amount must be positive",
        from.balance >= amount | "insufficient funds",
        from.id != to.id | "cannot transfer to same account"
    ],
    let new_from = Account { id: from.id, balance: from.balance - amount },
    let new_to = Account { id: to.id, balance: to.balance + amount },
    (new_from, new_to),
    post_check: (f, t) -> [
        f.balance == from.balance - amount,
        t.balance == to.balance + amount,
        f.balance + t.balance == from.balance + to.balance  // Conservation
    ]
)
```

### Composing with Other Patterns

```ori
// check + try
@safe_divide (a: int, b: int) -> Result<int, MathError> = run(
    pre_check: true,  // No precondition needed
    try(
        if b == 0 then Err(MathError.DivideByZero),
        Ok(a div b)
    ),
    post_check: r -> is_ok(r) || b == 0
)

// check + validate
@create_user (input: UserInput) -> Result<User, [str]> = run(
    pre_check: input.source == "trusted",
    validate(
        rules: [
            len(input.name) > 0 | "name required",
            input.age >= 0 | "invalid age"
        ],
        then: User { name: input.name, age: input.age },
    )
)
```

### Testing Checks

```ori
@test_divide tests @divide () -> void = run(
    // Normal cases
    assert_eq(divide(10, 2), 5),
    assert_eq(divide(7, 3), 2),

    // Pre-check violations
    assert_panics(divide(10, 0))
)

@test_sqrt tests @sqrt () -> void = run(
    // Normal cases
    assert_eq(sqrt(4.0), 2.0),
    assert_eq(sqrt(0.0), 0.0),

    // Pre-check violations
    assert_panics(sqrt(-1.0))
)
```

---

## Comparison

### vs. C++26 Contracts

| Aspect | C++26 | Ori |
|--------|-------|-------|
| New keywords | Yes (`pre`, `post`, `contract_assert`) | No |
| New grammar | Yes | No |
| Learning curve | Moderate | Zero (if you know `run`) |
| Visible in signature | Yes | No (in body) |
| Incremental adoption | Requires refactoring | Add properties to existing `run` |
| Evaluation modes | 4 (ignore, observe, enforce, quick-enforce) | 3 (ignore, observe, enforce) |

### vs. Manual Checks

```ori
// Before: manual, verbose
@sqrt (x: float) -> float = run(
    if x < 0.0 then panic("x must be non-negative"),
    let result = newton_raphson(x),
    if result < 0.0 then panic("result must be non-negative"),
    result
)

// After: declarative, clear
@sqrt (x: float) -> float = run(
    pre_check: x >= 0.0,
    newton_raphson(x),
    post_check: r -> r >= 0.0
)
```

### vs. Separate `check` Pattern

A dedicated `check` pattern was considered:

```ori
@sqrt (x: float) -> float = check(
    pre: x >= 0.0,
    body: newton_raphson(x),
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
map(over: items, transform: x -> x * 2)
filter(over: items, predicate: x -> x > 0)
run(pre_check: x > 0, body, post_check: r -> r > x)
```

### Why `pre_check` and `post_check`?

Alternatives considered:

| Name | Problem |
|------|---------|
| `pre:` / `post:` | Ambiguous - pre/post what? |
| `requires:` / `ensures:` | Longer, less obvious |
| `precondition:` / `postcondition:` | Too verbose |
| `pre_check:` / `post_check:` | Clear, explicit, action-oriented |

### Why Not in the Signature?

C++26 puts contracts on declarations:

```cpp
int sqrt(float x) pre(x >= 0) post(r: r >= 0);
```

Ori puts them in the body:

```ori
@sqrt (x: float) -> float = run(
    pre_check: x >= 0.0,
    ...
)
```

Tradeoffs:

| Signature (C++26) | Body (Ori) |
|-------------------|--------------|
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

Minimal. The `run` pattern parser accepts named properties. Add `pre_check:` and `post_check:` to recognized properties.

### Type Checking

- `pre_check:` expression must have type `bool` or `[bool]`
- `post_check:` must be a function from result type to `bool` or `[bool]`
- Message expressions (after `|`) must have type `str`

### Code Generation

Desugar to explicit conditional checks and panics during AST lowering.

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

### Invariant Checks

For types with invariants:

```ori
type BankAccount = { id: str, balance: int }
    invariant: self.balance >= 0

// All functions returning BankAccount automatically check invariant
```

### Check Inheritance

When a function calls another:

```ori
@wrapper (x: int) -> int = run(
    pre_check: x > 0,
    inner(x),  // inner's pre_checks also apply
    post_check: r -> r > 0
)
```

### Static Analysis

Tooling could verify that callers satisfy callees' preconditions:

```ori
@caller () -> int = run(
    let x = -5,
    sqrt(float(x))  // Warning: sqrt pre_check (x >= 0.0) may fail
)
```

---

## Summary

The `pre_check:` and `post_check:` properties for `run` provide contract-style defensive programming that:

1. **Requires zero new syntax** - Just named properties on existing pattern
2. **Has zero learning curve** - If you know `run`, you know this
3. **Enables incremental adoption** - Add checks to existing code without refactoring
4. **Is purely Ori** - Patterns + named properties, nothing foreign
5. **Solves real problems** - Clear preconditions, postconditions, better error messages

The design prioritizes consistency with Ori's existing patterns over visibility in function signatures, a tradeoff appropriate for a language with mandatory testing and short function bodies.
