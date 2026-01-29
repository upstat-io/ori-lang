# Proposal: Positional Lambdas for Single-Parameter Functions

**Status:** Approved
**Approved:** 2026-01-28
**Author:** Claude (with Eric)
**Created:** 2026-01-28

---

## Summary

When a function has exactly one parameter and the argument is a lambda, allow omitting the parameter name:

```ori
// Current (required)
items.map(transform: x -> x * 2)
items.filter(predicate: x -> x > 0)

// Proposed (allowed)
items.map(x -> x * 2)
items.filter(x -> x > 0)
```

---

## Motivation

### The Problem

Ori requires named arguments for clarity:

```ori
send_email(to: alice, subject: title, body: content)  // Clear
```

But for higher-order functions taking a single lambda, the name adds ceremony without clarity:

```ori
items.map(transform: x -> x * 2)      // "transform" obvious from context
items.filter(predicate: x -> x > 0)   // "predicate" obvious from context
items.find(where: x -> x.id == id)    // "where" obvious from context
tasks.any(predicate: t -> t.done)     // "predicate" obvious from context
```

The lambda itself is visually distinct from regular values. When you see `x -> x * 2`, you know it's a transformation. The parameter name is redundant.

### Prior Art

Every mainstream language with lambdas allows this:

```javascript
// JavaScript
items.map(x => x * 2)

// Python
list(map(lambda x: x * 2, items))

// Rust
items.iter().map(|x| x * 2)

// Swift
items.map { $0 * 2 }

// Kotlin
items.map { it * 2 }

// Haskell
map (*2) items
```

Ori is the only language requiring `items.map(transform: x -> x * 2)`.

### Why This Matters

Higher-order functions are idiomatic in Ori:

```ori
// Current: verbose
users
    .filter(predicate: u -> u.active)
    .map(transform: u -> u.name)
    .find(where: n -> n.starts_with(prefix: "A"))

// Proposed: clean
users
    .filter(u -> u.active)
    .map(u -> u.name)
    .find(n -> n.starts_with(prefix: "A"))
```

The named arguments add 40+ characters without improving clarity.

---

## Design

### The Rule

**When ALL of the following are true:**
1. Function has exactly one explicit parameter (excluding `self` for methods)
2. The argument expression is a lambda

**THEN:** The parameter name may be omitted.

### What Counts as a Lambda?

A lambda expression is:
- `x -> expr` (single parameter)
- `(a, b) -> expr` (multiple parameters)
- `() -> expr` (no parameters)
- `(x: int) -> int = expr` (typed lambda)

A lambda expression is NOT:
- A variable holding a function: `let f = x -> x + 1; list.map(f)` — named arg required
- A function reference: `list.map(double)` — named arg required
- Any other expression type

### Examples

**Allowed (single param + lambda):**
```ori
items.map(x -> x * 2)
items.filter(x -> x > 0)
items.find(x -> x.id == id)
items.fold(0, (acc, x) -> acc + x)  // NOT allowed: 2 params
tasks.any(t -> t.done)
tasks.all(t -> t.valid)
```

**Not allowed (not a lambda literal):**
```ori
let double = x -> x * 2
items.map(double)              // Error: named arg required
items.map(transform: double)   // OK

@double (n: int) -> int = n * 2
items.map(double)              // Error: named arg required
items.map(transform: double)   // OK
```

**Not allowed (multiple params):**
```ori
items.fold(0, (acc, x) -> acc + x)      // Error: 2 params
items.fold(initial: 0, op: (acc, x) -> acc + x)  // OK
```

**Named always works:**
```ori
items.map(transform: x -> x * 2)  // Still valid
items.filter(predicate: x -> x > 0)  // Still valid
```

### Edge Cases

**Chained methods:**
```ori
items
    .filter(x -> x > 0)      // OK: single param lambda
    .map(x -> x * 2)         // OK: single param lambda
    .fold(initial: 0, op: (acc, x) -> acc + x)  // Named required: 2 params
```

**Nested lambdas:**
```ori
matrix.map(row -> row.map(x -> x * 2))  // OK: both single param lambda
```

**Lambda returning lambda:**
```ori
@curry (f: (int, int) -> int) -> (int) -> (int) -> int
curry((a, b) -> a + b)  // OK: single param is lambda `(a, b) -> a + b`
```

---

## Rationale

### Why Only Lambdas?

Lambdas are visually distinct. When you see:
```ori
items.map(x -> x * 2)
```

The `x -> x * 2` syntax immediately signals "this is a transformation function." No label needed.

But for regular values:
```ori
send(message)      // What is message? A recipient? A channel?
send(to: message)  // Ah, it's the destination
```

Variables don't carry their purpose in their syntax. Named arguments provide that context.

### Why Not Allow Function References?

```ori
@double (n: int) -> int = n * 2
items.map(double)  // Why require name here?
```

Function references look like any other identifier. Without IDE support, you can't tell if `double` is a function, a value, or a type. The named argument `transform: double` provides context.

But `x -> x * 2` is unambiguous — it can only be a function.

### Why Single Parameter Only?

Multiple parameters benefit from names:
```ori
items.fold(0, (acc, x) -> acc + x)          // Which is initial? Which is op?
items.fold(initial: 0, op: (acc, x) -> acc + x)  // Clear
```

Single parameter has no ambiguity:
```ori
items.map(x -> x * 2)  // Only one thing it could be
```

### Why Not Trailing Closure Syntax?

Some languages (Swift, Kotlin) have special "trailing closure" syntax:

```swift
// Swift
items.map { $0 * 2 }
```

This requires new syntax. The proposal reuses existing lambda syntax — just removes the label requirement in specific cases.

### Comparison with Anonymous Parameters Proposal

The [Anonymous Parameters proposal](./anonymous-parameter-proposal.md) allows function authors to opt-in to positional arguments via `_ name` syntax:

```ori
@map (_ transform: (T) -> U) -> [U]  // Explicit opt-in
```

This proposal is **complementary** — it's automatic based on call-site syntax:

| Approach | Scope | Opt-in | Applies to |
|----------|-------|--------|------------|
| Anonymous params | Any function | Author declares `_` | Any argument type |
| This proposal | Single-param functions | Automatic | Lambda literals only |

Both could coexist. This proposal handles the common HOF case automatically; anonymous params handle other cases explicitly.

**Resolution order:** When both features apply (single-param function with `_ param` and a lambda argument), this proposal's automatic rule takes precedence — the lambda can be passed positionally regardless of whether the function declared the parameter as anonymous.

---

## Implementation

### Parser Changes

No grammar changes needed. The parser already accepts:
```
call_expression := expression "(" argument_list ")"
argument := identifier ":" expression | expression
```

The second form (positional) is currently rejected by the type checker for direct calls.

### Type Checker Changes

In call resolution (`compiler/ori_typeck/src/infer/call.rs`):

```
When resolving a call with a positional argument:
1. If callee has exactly 1 parameter
2. AND argument is a LambdaExpr
3. THEN allow positional
4. ELSE require named argument (existing error E2011)
```

### Error Messages

When the rule doesn't apply:

```
error[E2011]: named arguments required for direct function calls
  --> src/main.ori:5:12
   |
5  |     items.map(double)
   |               ^^^^^^
   |
   = help: use named argument syntax: `map(transform: double)`
   = note: positional arguments are only allowed for inline lambda
           expressions, not function references
```

### Files to Update

- `compiler/ori_typeck/src/infer/call.rs` — Add lambda-literal check
- `docs/ori_lang/0.1-alpha/spec/09-expressions.md` — Document exception
- `CLAUDE.md` — Update call syntax section

---

## Examples

### Before and After

**Iterator chains:**
```ori
// Before
users
    .filter(predicate: u -> u.active && u.verified)
    .map(transform: u -> u.email)
    .filter(predicate: e -> e.ends_with(suffix: "@company.com"))

// After
users
    .filter(u -> u.active && u.verified)
    .map(u -> u.email)
    .filter(e -> e.ends_with(suffix: "@company.com"))
```

**Option/Result methods:**
```ori
// Before
maybe_user
    .map(transform: u -> u.profile)
    .and_then(transform: p -> p.avatar)
    .unwrap_or(default: $default_avatar)

// After
maybe_user
    .map(u -> u.profile)
    .and_then(p -> p.avatar)
    .unwrap_or(default: $default_avatar)
```

**Event handlers:**
```ori
// Before
button.on_click(handler: () -> save_document())
input.on_change(handler: value -> update_state(value: value))

// After
button.on_click(() -> save_document())
input.on_change(value -> update_state(value: value))
```

**Parallel operations:**
```ori
// Before (parallel has multiple params, so names still required)
parallel(
    tasks: [
        () -> fetch_users(),
        () -> fetch_posts(),
        () -> fetch_comments(),
    ],
    max_concurrent: 3,
)

// After (unchanged — multiple params)
parallel(
    tasks: [
        () -> fetch_users(),
        () -> fetch_posts(),
        () -> fetch_comments(),
    ],
    max_concurrent: 3,
)
```

---

## Tradeoffs

| Benefit | Cost |
|---------|------|
| Cleaner HOF chains | Slightly less consistent (named args sometimes required) |
| Matches industry conventions | Special case in type checker |
| No new syntax needed | Lambda vs function ref distinction matters |
| Backward compatible | - |

### When Names Are Still Valuable

Even with this feature, authors may prefer explicit names for documentation:

```ori
// Self-documenting
items.sort(by: (a, b) -> a.name.compare(other: b.name))
items.group(by: item -> item.category)
items.partition(where: item -> item.active)
```

Named arguments remain valid and are encouraged when they add clarity.

---

## Summary

Allow omitting parameter names when:
1. Function has exactly one parameter
2. Argument is an inline lambda expression

This eliminates the most common source of verbosity (`items.map(transform: x -> x * 2)` → `items.map(x -> x * 2)`) while preserving named arguments everywhere they add value.

The feature is:
- **Automatic** — no author opt-in required
- **Narrow** — only lambdas, only single-param functions
- **Backward compatible** — named arguments always work
- **Industry-aligned** — matches JavaScript, Python, Rust, Swift, Kotlin, etc.
