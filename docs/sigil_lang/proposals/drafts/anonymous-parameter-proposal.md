# Proposal: Anonymous Parameters for Positional Arguments

**Status:** Draft
**Author:** Claude (with Eric)
**Created:** 2026-01-25

---

## Summary

Allow function authors to opt into positional argument syntax by using `_` as the parameter name.

```sigil
// Standard: named arguments required
@double (n: int) -> int = n * 2
double(n: 5)

// With anonymous parameter: positional allowed
@double (_ n: int) -> int = n * 2
double(5)
```

---

## Motivation

### Named Arguments Are Great, But Sometimes Verbose

Sigil requires named arguments for self-documentation:

```sigil
fetch_user(id: 1)           // Clear: what is 1? It's the id
send_email(to: a, subject: b, body: c)  // Clear: which string is which
```

But for simple single-parameter functions, the name adds ceremony without clarity:

```sigil
wrap(value: 42)       // Obvious what 42 is
double(n: 5)          // Obvious what 5 is
Some(value: x)        // Verbose for a simple wrapper
negate(x: true)       // The function name says it all
```

### Library Authors Need an Escape Hatch

Library authors building utility functions may want cleaner call sites:

```sigil
// Current: verbose
items.map(transform: x -> x * 2)
Option.from(value: maybe_null)
Result.ok(value: 42)

// Desired: concise
items.map(x -> x * 2)
Option.from(maybe_null)
Result.ok(42)
```

### The `_` Convention Is Familiar

The underscore as "anonymous" or "unused" is established:
- **Rust:** `let _ = value` (discard), `_foo` (unused but named)
- **Swift:** `func double(_ n: Int)` (no external label)
- **Go:** `_ = value` (discard)
- **Python:** `_` as throwaway variable

Using `_` to signal "no external name required" follows this convention.

---

## Design

### Syntax

```
parameter := '_' IDENTIFIER ':' type    // anonymous (positional allowed)
           | IDENTIFIER ':' type        // named (positional required)
```

The `_` prefix on a parameter name indicates:
1. The call site may use positional arguments
2. The internal name (after `_`) is used in the function body

### Examples

```sigil
// Anonymous single parameter
@double (_ n: int) -> int = n * 2

double(5)        // OK: positional
double(n: 5)     // OK: named still works

// Anonymous with multiple parameters (only first is anonymous)
@add (_ a: int, b: int) -> int = a + b

add(1, b: 2)     // OK: first positional, second named
add(a: 1, b: 2)  // OK: all named

// All anonymous (rare, but allowed)
@point (_ x: int, _ y: int) -> Point = Point { x, y }

point(1, 2)          // OK: all positional
point(x: 1, y: 2)    // OK: all named
point(1, y: 2)       // OK: mixed
```

### Rules

1. **`_` prefix makes a parameter anonymous** — positional allowed at call site
2. **Internal name follows `_`** — used in function body
3. **Named arguments always work** — `_` just adds positional as an option
4. **Order matters for positional** — must match declaration order
5. **Mixing allowed** — positional for anonymous, named for non-anonymous

### Method Syntax

Works the same for methods:

```sigil
impl Option<T> {
    @map (_ transform: (T) -> U) -> Option<U> = match self {
        Some(v) -> Some(transform(v)),
        None -> None,
    }
}

opt.map(x -> x * 2)      // Positional: clean!
opt.map(transform: fn)   // Named: still works
```

### Constructors and Wrappers

Particularly useful for simple constructors:

```sigil
// Standard library could define:
@Some (_ value: T) -> Option<T> = ...
@Ok (_ value: T) -> Result<T, E> = ...
@Err (_ error: E) -> Result<T, E> = ...

// Clean call sites:
Some(42)
Ok(result)
Err("failed")

// Named still works:
Some(value: 42)
Ok(value: result)
Err(error: "failed")
```

---

## Rationale

### Why `_ name` Instead of Just `_`?

Using `_ name` instead of bare `_`:

```sigil
// Option A: _ name (proposed)
@double (_ n: int) -> int = n * 2

// Option B: bare _ (rejected)
@double (_: int) -> int = _ * 2  // Can't reference _ in body
```

Option A lets you use the parameter in the function body. Option B would require special syntax to reference anonymous parameters.

### Why Not Swift's External/Internal Name Syntax?

Swift uses `func double(_ n: Int)` where `_` is the external name and `n` is internal. Sigil could adopt this:

```sigil
// Swift-style (not proposed)
@double (_ n: int) -> int = n * 2   // Same syntax, different interpretation
```

The proposed design is actually compatible with Swift's interpretation. The `_` is the "external name" (none), and what follows is the internal name.

### Why Allow Mixed Positional/Named?

Consider:

```sigil
@fetch (_ url: str, timeout: Duration = 30s) -> Response
```

Callers can use:
```sigil
fetch("https://api.com")                    // Positional url, default timeout
fetch("https://api.com", timeout: 5s)       // Positional url, named timeout
fetch(url: "https://api.com", timeout: 5s)  // All named
```

This is flexible and follows the principle of least surprise.

---

## Examples

### Standard Library

```sigil
// Option constructors
@Some (_ value: T) -> Option<T>
@None () -> Option<T>

// Result constructors
@Ok (_ value: T) -> Result<T, E>
@Err (_ error: E) -> Result<T, E>

// Simple utilities
@not (_ b: bool) -> bool = !b
@negate (_ n: int) -> int = -n

// Usage
Some(42)
Ok(result)
not(flag)
negate(value)
```

### Collection Methods

```sigil
impl [T] {
    @map (_ transform: (T) -> U) -> [U]
    @filter (_ predicate: (T) -> bool) -> [T]
    @find (_ predicate: (T) -> bool) -> Option<T>
    @fold (_ initial: U, _ op: (U, T) -> U) -> U
}

// Clean call sites
items.map(x -> x * 2)
items.filter(x -> x > 0)
items.fold(0, (acc, x) -> acc + x)

// Named still available for clarity
items.fold(initial: 0, op: (acc, x) -> acc + x)
```

### Builder Patterns

```sigil
@Request (_ method: Method, _ url: str) -> RequestBuilder

Request(GET, "/api/users")
    .header(name: "Auth", value: token)
    .timeout(duration: 30s)
    .send()
```

---

## Interaction with Existing Rules

### Function Variables

Calls through function variables already allow positional (param names unknowable):

```sigil
let f = x -> x + 1
f(5)  // Already allowed
```

The `_` feature is for **direct calls** to make them optionally positional.

### Type Conversions

Type conversions (`int`, `float`, `str`, `byte`) already allow positional. They could be thought of as having anonymous parameters:

```sigil
// Conceptually:
@int (_ x: T) -> int
@str (_ x: T) -> str
```

### Adding Parameters

Adding a named parameter to a function with anonymous parameters is still non-breaking if it has a default:

```sigil
// v1
@fetch (_ url: str) -> Response

// v2 - non-breaking
@fetch (_ url: str, timeout: Duration = 30s) -> Response

// Existing calls still work:
fetch("https://api.com")
```

---

## Tradeoffs

| Cost | Mitigation |
|------|------------|
| Slightly more complex grammar | `_ name` is intuitive |
| Function author decides, not caller | Explicit opt-in is good API design |
| Could be overused | Code review; naming conventions |
| Multiple `_` params can be confusing | Discourage via style guide |

### When NOT to Use Anonymous Parameters

```sigil
// BAD: Multiple anonymous params with similar types
@send (_ from: str, _ to: str, _ body: str) -> void
send("alice", "bob", "hello")  // Which is which?!

// GOOD: Use named params for clarity
@send (from: str, to: str, body: str) -> void
send(from: "alice", to: "bob", body: "hello")
```

**Guideline:** Use `_` for single-parameter functions or when the meaning is obvious from context/position.

---

## Implementation

### Parser Changes

1. Update parameter parsing to recognize `'_' IDENTIFIER ':' type`
2. Store a flag on parameters indicating anonymous
3. During call resolution, check if positional is allowed

### Type Checker Changes

1. For `Call` (positional) expressions, check if the callee function has anonymous parameters
2. If all used positions have anonymous parameters, allow
3. Otherwise, emit E2011 "named arguments required"

### Files to Update

- `compiler/sigil_ir/src/ast/items/functions.rs` — Add `anonymous: bool` to `Param`
- `compiler/sigil_parse/src/grammar/item.rs` — Parse `_ name: type`
- `compiler/sigilc/src/typeck/infer/call.rs` — Check anonymous params
- `docs/sigil_lang/0.1-alpha/spec/08-declarations.md` — Document syntax
- `CLAUDE.md` — Update quick reference

---

## Future Considerations

### Shorthand for Single Anonymous Param?

Could allow omitting the internal name for single params:

```sigil
@double (_: int) -> int = _ * 2  // _ refers to the single param
```

This would require `_` to be a valid expression referring to the anonymous parameter. Deferred for simplicity.

### IDE Support

IDEs could show parameter hints even for anonymous params:

```sigil
double(▌5▐)
       └── n: int
```

---

## Summary

Allow `_ name: type` syntax for parameters to indicate positional arguments are accepted:

- `@double (_ n: int) -> int` — can call as `double(5)` or `double(n: 5)`
- Library authors opt into positional where it makes sense
- Named arguments always work as fallback
- Familiar `_` convention from Rust, Swift, Go
- Simple implementation: flag on parameters

The feature provides an escape hatch for verbosity while preserving Sigil's default of self-documenting named arguments.
