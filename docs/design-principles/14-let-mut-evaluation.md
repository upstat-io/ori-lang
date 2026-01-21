# Evaluation: `let` and `let mut` Addition

Evaluation of the new `let` / `let mut` syntax against syntax design principles.

---

## What Was Added

```sigil
// Immutable binding
let x = compute()

// Mutable binding
let mut counter = 0

// Reassignment (mutable only)
counter = counter + 1
```

---

## Evaluation Against Principles

### 1. Parser-Friendliness

**Rating: Pass**

`let` is a leading keyword - immediately distinguishes binding from expression.

```sigil
run(
    let x = compute(),    // Clearly a binding
    x + 1                 // Clearly an expression
)
```

Before (bare `=`), the parser had to disambiguate `x = ...` in context. Now `let` makes it unambiguous.

### 2. Explicitness / Visible Mutability

**Rating: Pass (Major Improvement)**

This directly addresses the "Partial" rating from the previous evaluation.

```sigil
let x = 5           // Immutable - explicit
let mut y = 5       // Mutable - explicit and visible

// vs. before (shadowing only)
x = 5               // Was this mutable? Immutable? Shadowing?
x = x + 1           // New binding or mutation?
```

**Research cited:**
> "Mutation should be syntactically obvious" — Rust's `let mut` is frequently praised for making mutability visible.

### 3. Consistency with Other Languages

**Rating: Pass**

| Language | Immutable | Mutable |
|----------|-----------|---------|
| Rust | `let x = ...` | `let mut x = ...` |
| **Sigil** | `let x = ...` | `let mut x = ...` |
| Swift | `let x = ...` | `var x = ...` |
| Kotlin | `val x = ...` | `var x = ...` |
| JS | `const x = ...` | `let x = ...` |

Sigil now matches Rust's syntax exactly. This is good for:
- Developers coming from Rust
- AI models trained heavily on Rust code
- Consistent mental model

### 4. Clarity of Intent

**Rating: Pass**

```sigil
// Clear: I intend to mutate this
let mut total = 0
for item in items do
    total = total + item.price

// Clear: I don't intend to mutate this
let config = load_config()
```

Before, "shadowing as mutation" was clever but confusing:
```sigil
// Old style: Is this mutation or shadowing?
config = load_config()
config = validate(.config: config)   // New binding? Same binding? Who knows!
```

### 5. AI Code Generation

**Rating: Pass**

For AI generating code:
- `let` is explicit intent marker
- `let mut` signals "this will change"
- Reassignment without `let` is clearly mutation, not new binding

**Reduces ambiguity errors:**
```sigil
// AI knows exactly what to generate
let result = compute()           // New immutable binding
let mut buffer = []              // New mutable binding
buffer = buffer + [item]         // Mutation of existing
```

### 6. Binding Syntax

**Rating: Pass**

All bindings require explicit `let` or `let mut`:

```sigil
run(
    let x = 5,           // Immutable binding
    let mut y = 10,      // Mutable binding
    y = y + x,           // Reassignment (only valid for mut bindings)
    y,
)
```

See [Syntax Improvements § 11](13-syntax-improvements.md) for rationale.

---

## Examples

### Pipeline Transformation with Shadowing

Shadowing is allowed with `let`. Each `let` creates a new binding:

```sigil
@process (data: Data) -> Data = run(
    let data = step1(.data: data),
    let data = step2(.data: data),
    let data = step3(.data: data),
    data,
)
```

### Mutation with `let mut`

Use `let mut` when you need actual mutation:

```sigil
@sum (items: [int]) -> int = run(
    let mut total = 0,
    for item in items do
        total = total + item,
    total,
)
```

This enables imperative algorithms when appropriate.

---

## Potential Concerns

### 1. Verbosity Increase

Every binding now needs `let`:

```sigil
// Before: 4 tokens
x = compute()

// After: 5 tokens
let x = compute()
```

**Mitigation:** This is minimal verbosity for significant clarity gain. AI doesn't care about token count. Humans benefit from explicit markers.

### 2. Mutation Creep

With `let mut` available, code might become mutation-heavy:

```sigil
// Bad pattern that's now possible
let mut x = 0
let mut y = 0
let mut z = 0
x = compute_x()
y = compute_y(.x: x)
z = compute_z(.x: x, .y: y)
```

**Mitigation:**
- Linter rule: "Prefer immutable bindings"
- Formatter: Warn on `let mut` when shadowing would work
- Documentation: Emphasize immutable-first philosophy

### 3. Shadowing + Mutation Confusion

```sigil
let mut x = 5
let x = 10      // Is this shadowing or error?
x = 15          // Does this affect which x?
```

**Recommendation:** Clear rules:
- `let x` always creates new binding (can shadow)
- `let mut x` always creates new mutable binding (can shadow)
- `x = value` reassigns the innermost mutable `x`
- Error if reassigning an immutable binding

---

## Formatter Rules Update

```sigil
// Immutable binding - single line OK
let x = compute()

// Mutable declaration - flag for review
let mut counter = 0   // Consider: is mutation necessary?

// Multi-step with shadowing - stack
let data = fetch(.url: url)
let data = parse(.data: data)
let data = validate(.data: data)

// Mutation loop - acceptable
let mut total = 0
for item in items do
    total = total + item.price
```

---

## Summary

| Aspect | Rating | Notes |
|--------|--------|-------|
| Parser-friendliness | Pass | `let` is unambiguous leading keyword |
| Visible mutability | Pass | `let mut` makes mutation explicit |
| Consistency | Pass | Matches Rust exactly |
| AI-friendliness | Pass | Clear intent markers |
| Beauty | Pass | More keywords but clearer intent |
| Backward compat | Needs work | Deprecate bare `=` bindings |

**Overall: Good addition.**

The `let` / `let mut` syntax:
1. Fixes the "Partial" rating on visible mutability
2. Aligns with Rust (huge corpus for AI training)
3. Makes intent explicit
4. Enables true mutation when needed

**Recommendations:**
1. Require `let` for all new bindings (deprecate bare `=`)
2. Add linter warning for unnecessary `let mut`
3. Update docs to emphasize immutable-first
4. Ensure shadowing rules are clear and documented

---

## Updated Beauty Evaluation

The language is now **more beautiful** because:

1. **Honesty** — `let mut` explicitly shows "this will change"
2. **Consistency** — Same syntax as Rust, familiar pattern
3. **Predictability** — Can tell at a glance what's mutable
4. **Scanability** — `mut` keyword stands out visually

```sigil
@process_items (items: [Item]) -> Summary = run(
    let validated = validate_all(.items: items),
    let grouped = group_by(
        .items: validated,
        .key: .category,
    ),

    let mut total = 0,
    for group in grouped.values() do
        total = total + sum_prices(.items: group),

    Summary {
        items: validated,
        groups: grouped,
        total: total,
    },
)
```

The mutation (`let mut total`) stands out. The immutable bindings are clearly transformation steps. The code's intent is visible.
