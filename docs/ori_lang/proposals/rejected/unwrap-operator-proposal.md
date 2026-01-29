# Proposal: Unwrap Operator (`!`)

**Status:** Rejected
**Author:** Eric (with AI assistance)
**Created:** 2026-01-28
**Rejected:** 2026-01-28

---

## Rejection Reason

Making panicking code easier to write encourages unsafe patterns. The verbosity of `.unwrap()` is a feature, not a bug — it forces developers to acknowledge they're writing code that can panic. A single-character operator normalizes panicking and makes it too easy to sprinkle `!` throughout code without thinking.

Ori's philosophy favors explicit, safe code. If unwrapping feels tedious, that's a signal to use `?` propagation, `.unwrap_or()`, or pattern matching instead.

---

## Summary

Add `!` as a postfix operator for unwrapping `Option` and `Result` types. Panics on `None` or `Err`.

```ori
let name = user!.name           // unwrap Option<User>, access field
let len = maybe_list!.len()     // unwrap, then call method
let first = result!.items![0]   // chain multiple unwraps
```

The `!` operator is shorthand for `.unwrap()`. Both remain available.

---

## Motivation

### Chaining Ergonomics

The `.unwrap()` method works but is verbose when chaining:

```ori
// Current - verbose
let name = response.unwrap().name
let first = result.unwrap().items.unwrap()[0]

// Proposed - concise
let name = response!.name
let first = result!.items![0]
```

The `!` binds tightly and chains naturally with `.` for field access and method calls.

### Visual Assertion

`!` reads as an assertion: "I know this has a value!"

```ori
let config = load_config()!     // I assert config exists
let user = get_user(id: id)!    // I assert user is Ok
```

The exclamation mark conveys certainty and intent.

### Swift Precedent

Swift uses `!` for force unwrap:

```swift
let name = optionalUser!.name   // Swift
```

Developers familiar with Swift will recognize this pattern.

### Keeps Panicking Visible

Unlike some alternatives, `!` is visually distinct:
- Single character, but stands out
- Reads as emphatic/assertive
- Not easily confused with other operators

---

## Design

### Grammar

Add `!` as a postfix operator with high precedence (same level as `.`, `[]`, `()`):

```ebnf
postfix_expr = primary_expr { postfix_op } .
postfix_op   = '.' IDENTIFIER
             | '[' expression ']'
             | '(' arguments ')'
             | '?'
             | '!' .
```

### Semantics

For `Option<T>`:
- `Some(value)!` evaluates to `value`
- `None!` panics with "unwrap called on None"

For `Result<T, E>`:
- `Ok(value)!` evaluates to `value`
- `Err(e)!` panics with error message from `e`

### Precedence

`!` binds tighter than binary operators, same as other postfix operators:

```ori
value!.field      // (value!).field
value!.method()   // (value!).method()
value![0]         // (value!)[0]
a! + b!           // (a!) + (b!)
```

### Chaining

Multiple unwraps chain naturally:

```ori
// Nested Option
let inner = outer!.middle!.inner

// Result then Option
let name = fetch_user(id: 1)!.nickname!

// With indexing
let first = get_items()![0]
```

### Relationship to `.unwrap()`

Both `!` and `.unwrap()` remain available:

```ori
// Equivalent
value!
value.unwrap()
```

Use `!` for concise chaining. Use `.unwrap()` when you prefer explicit method style or for consistency with `.unwrap_or()`.

---

## Examples

### Simple Unwrap

```ori
let config = load_config()!
let port = config.port
```

### Chained Access

```ori
@get_user_email (id: UserId) -> str uses Database =
    run(
        let user = fetch_user(id: id)!,
        user.email!,  // Option<str> -> str
    )
```

### In Expressions

```ori
let total = items!.len() + extra_items!.len()

let message = if user!.is_admin then "Welcome, admin" else "Welcome"
```

### With Indexing

```ori
let first = list![0]
let value = map!["key"]!  // unwrap Option<V> from map lookup
```

### Multiple Unwraps

```ori
// Parse nested JSON structure
let city = response!.body!.user!.address!.city
```

---

## Error Messages

When `!` panics, the error message should be helpful:

```
panic: unwrap called on None
  --> src/main.ori:42:15
   |
42 |     let name = user!.name
   |                    ^ Option was None

panic: unwrap called on Err
  --> src/main.ori:58:20
   |
58 |     let data = fetch_data()!
   |                            ^ Result was Err: connection timeout
```

---

## Tradeoffs

| Consideration | Assessment |
|---------------|------------|
| **Hides danger** | `!` is visually distinct; panic risk is clear |
| **Cryptic to newcomers** | Swift precedent helps; easy to learn |
| **Redundant with `.unwrap()`** | Intentional — `!` for chaining, method for explicitness |
| **Could conflict with future syntax** | `!` as postfix is well-established |

### When to Use Which

| Situation | Recommendation |
|-----------|----------------|
| Chaining field access | `value!.field` |
| Chaining method calls | `value!.method()` |
| Standalone unwrap | Either works; preference |
| With `.unwrap_or()` family | Use method: `value.unwrap_or(default: x)` |

---

## Alternatives Considered

### 1. `?>` Operator

```ori
value?>.name  // proposed alternative
```

Rejected:
- Awkward before `.` — two characters don't close the expression
- Weak symmetry argument — `?` propagates, doesn't "wrap"
- Less familiar than `!`

### 2. Only `.unwrap()` Method

Keep status quo.

Rejected:
- Verbose for chaining
- `response.unwrap().name` is noisier than `response!.name`

### 3. `!!` Double Bang

```ori
value!!.name
```

Rejected:
- More to type
- No precedent
- Single `!` is sufficient

### 4. Implicit Unwrap Types

Like Swift's `T!` implicitly unwrapped optionals.

Rejected:
- Adds type system complexity
- Hidden unwrap points
- Against Ori's explicit philosophy

---

## Implementation

### Lexer Changes

Add `!` as a token (already exists for logical not — context determines meaning).

### Parser Changes

In `parse_postfix_expr`, after primary expression, check for `!`:

```rust
fn parse_postfix_expr(&mut self) -> Expr {
    let mut expr = self.parse_primary_expr();
    loop {
        match self.current_token() {
            Token::Dot => { /* field access */ }
            Token::LBracket => { /* index */ }
            Token::LParen => { /* call */ }
            Token::Question => { /* propagate */ }
            Token::Bang => {
                self.advance();
                expr = Expr::Unwrap(Box::new(expr), self.span());
            }
            _ => break,
        }
    }
    expr
}
```

### Type Checker

For `Expr::Unwrap(inner)`:
1. Check `inner` is `Option<T>` or `Result<T, E>`
2. Result type is `T`
3. If neither, emit error: "cannot unwrap type X"

### Code Generation

Desugar to match:

```ori
// Source
value!

// Desugars to (for Option)
match(value,
    Some(v) -> v,
    None -> panic(msg: "unwrap called on None"),
)

// Desugars to (for Result)
match(value,
    Ok(v) -> v,
    Err(e) -> panic(msg: `unwrap called on Err: {e}`),
)
```

### Files to Update

- `compiler/oric/src/parser/` — Add postfix `!` parsing
- `compiler/oric/src/types/` — Type check unwrap expression
- `compiler/oric/src/interpreter/` — Evaluate unwrap
- `docs/ori_lang/0.1-alpha/spec/` — Document operator
- `CLAUDE.md` — Quick reference

---

## Summary

Add `!` as a postfix unwrap operator:

- `value!` unwraps `Option<T>` or `Result<T, E>` to `T`
- Panics on `None` or `Err`
- Chains naturally: `response!.user!.name`
- Complements `.unwrap()` — both remain available
- Familiar from Swift
- Visually distinct — panicking is intentional and visible

```ori
// Before
let name = response.unwrap().user.unwrap().name

// After
let name = response!.user!.name
```
