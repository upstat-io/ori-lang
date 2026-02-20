# Proposal: Capability Composition Rules

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Approved:** 2026-01-29
**Affects:** Compiler, capability system, grammar

---

## Summary

This proposal specifies how capabilities compose, including partial provision with `with...in`, nested binding behavior, capability variance, and resolution rules when defaults and explicit bindings interact.

---

## Problem Statement

The spec defines capabilities but doesn't address:

1. **Partial provision**: Can you provide some capabilities but not others?
2. **Nested `with...in`**: What happens with multiple levels of binding?
3. **Capability variance**: Can a function requiring fewer capabilities call one requiring more?
4. **Default vs explicit**: When both `def impl` and `with...in` apply, which wins?

---

## Grammar Changes

### Multi-Binding Syntax

The `with` expression is extended to support multiple capability bindings:

```ebnf
with_expr = "with" capability_binding { "," capability_binding } "in" expression .
capability_binding = identifier "=" expression .
```

This allows:

```ori
with Http = mock_http, Cache = mock_cache in
    complex_operation()
```

As equivalent to the nested form:

```ori
with Http = mock_http in
    with Cache = mock_cache in
        complex_operation()
```

---

## Partial Capability Provision

### Multiple Capabilities

A function can require multiple capabilities:

```ori
@complex_operation () -> Result<Data, Error> uses Http, Cache, Logger = ...
```

### Partial with...in

You can provide some capabilities while letting others use defaults:

```ori
def impl Http { ... }
def impl Cache { ... }
def impl Logger { ... }

@test_with_mock_http () -> void = {
    let mock = MockHttp { ... }

    with Http = mock in
        complex_operation(),  // MockHttp + default Cache + default Logger
}
```

Only `Http` is overridden; `Cache` and `Logger` use their `def impl`.

### Multiple Bindings

Provide multiple capabilities in one `with`:

```ori
with Http = mock_http, Cache = mock_cache in
    complex_operation()  // Both overridden, Logger uses default
```

Or nested (equivalent result):

```ori
with Http = mock_http in
    with Cache = mock_cache in
        complex_operation()
```

---

## Nested with...in Semantics

### Shadowing Rule

Inner bindings shadow outer bindings:

```ori
with Http = OuterHttp in {
    use_http(),  // OuterHttp

    with Http = InnerHttp in
        use_http(),  // InnerHttp (shadows Outer)

    use_http(),  // OuterHttp again
}
```

### Multiple Capabilities Nesting

```ori
with Http = HttpA in
    with Cache = CacheX in
        with Http = HttpB in
            operation()  // HttpB + CacheX
        // Back to HttpA + CacheX
    // Back to HttpA + default Cache
```

### Scope Rules

`with` creates a lexical scope — bindings are visible only within:

```ori
let result = with Http = mock in fetch()
// mock is NOT bound here
fetch()  // Uses default Http, not mock
```

---

## Capability Variance

### Subtyping Question

Can a function requiring `uses Http` be called in a context that has `uses Http, Cache`?

**Answer: Yes.** A context with MORE capabilities can call functions requiring FEWER:

```ori
@needs_http () -> void uses Http = ...
@needs_both () -> void uses Http, Cache = ...

@caller () -> void uses Http, Cache = {
    needs_http(),  // OK: caller has Http
    needs_both(),  // OK: caller has both
}
```

### Reverse Not Allowed

A function requiring MORE capabilities cannot be called from one with FEWER:

```ori
@needs_both () -> void uses Http, Cache = ...

@caller () -> void uses Http = {
    needs_both(),  // ERROR: caller lacks Cache
}
```

Error message:
```
error[E1200]: missing capability `Cache`
  --> src/main.ori:4:5
   |
4  |     needs_both()
   |     ^^^^^^^^^^^^ requires `Cache` capability
   |
   = note: `caller` only has: Http
   = help: add `Cache` to caller's capability list: `uses Http, Cache`
```

### Capability Propagation

Capabilities propagate upward through the call chain:

```ori
@level3 () -> void uses Http = ...
@level2 () -> void uses Http = level3()  // Must declare Http
@level1 () -> void uses Http = level2()  // Must declare Http
@main () -> void = with Http = impl in level1()
```

---

## Explicit Declaration Requirement

Capability requirements must be explicitly declared in function signatures. The compiler does not infer capabilities from the function body:

```ori
// Correct: capabilities declared
@caller () -> void uses Http, Cache = {
    Http.get(url: "/data")
    Cache.set(key: "k", value: "v")
}

// Error: Http used but not declared
@caller () -> void uses Cache = {
    Http.get(url: "/data"),  // ERROR: uses Http without declaring it
    Cache.set(key: "k", value: "v")
}
```

Error:
```
error[E0600]: function uses `Http` without declaring it
  --> src/main.ori:3:5
   |
3  |     Http.get(url: "/data"),
   |     ^^^^^^^^^^^^^^^^^^^^^^ requires `Http` capability
   |
   = help: add `Http` to the function signature: `uses Cache, Http`
```

This ensures:
- Function signatures are complete contracts
- Callers know required capabilities from the signature alone
- No hidden capability requirements

---

## Default vs Explicit Resolution

### Priority Order

When resolving a capability, the compiler checks in order:

1. **Innermost `with...in` binding** — highest priority
2. **Outer `with...in` bindings** — in reverse nesting order
3. **Imported `def impl`** — from the module where the trait is defined
4. **Module-local `def impl`** — defined in the current module
5. **Error** — capability not provided

```ori
def impl Http { ... }  // Priority 4 (local)

with Http = MiddleHttp in {  // Priority 2
    with Http = InnerHttp in
        fetch(),  // Uses InnerHttp (priority 1)

    fetch(),  // Uses MiddleHttp (priority 2)
}

fetch()  // Uses def impl (priority 3 or 4)
```

### Imported vs Local Defaults

When both an imported `def impl` and a module-local `def impl` exist for the same capability, imported takes precedence:

```ori
// std/net/http.ori
pub def impl Http { ... }  // Imported default

// my_module.ori
use std.net.http { Http }
def impl Http { ... }  // Local default (lower priority)

@fetch () -> Result uses Http = Http.get(url: "/data")
// Uses imported def impl from std.net.http
```

### No Implicit Fallback

If no binding or `def impl` exists, the capability is "unbound":

```ori
// No def impl for Database

@query () -> Result uses Database = ...

query()  // ERROR: Database capability not bound
```

Error:
```
error[E1201]: unbound capability `Database`
  --> src/main.ori:5:1
   |
5  | query()
   | ^^^^^^^ `Database` capability is required but not provided
   |
   = help: provide with `with Database = impl in query()`
   = help: or add a `def impl Database` to bring a default into scope
```

---

## Capability Compatibility

### Trait Requirements

A capability binding must implement the capability trait:

```ori
trait Http {
    @get (url: str) -> Result<Response, Error> uses Suspend
    @post (url: str, body: str) -> Result<Response, Error> uses Suspend
}

type MockHttp = { responses: {str: Response} }

impl Http for MockHttp {
    @get (url: str) -> Result<Response, Error> uses Suspend =
        Ok(self.responses[url])
    // ...
}

with Http = MockHttp { responses: ... } in
    fetch()  // OK: MockHttp implements Http
```

### Type Mismatch Error

```ori
type NotHttp = { foo: int }

with Http = NotHttp { foo: 1 } in
    fetch()  // ERROR: NotHttp does not implement Http
```

Error:
```
error[E1202]: type `NotHttp` does not implement trait `Http`
  --> src/main.ori:3:14
   |
3  | with Http = NotHttp { foo: 1 } in
   |             ^^^^^^^^^^^^^^^^^^ expected implementation of `Http`
   |
   = note: `Http` requires methods: get, post
```

---

## Async Capability Interaction

### Async Binding Prohibition

`Async` is a marker capability — it has no methods and cannot be provided via `with...in`. Attempting to bind `Async` is a compile-time error:

```ori
// ERROR: Async cannot be bound with `with...in`
with Async = SomeImpl in
    async_fn()
```

Error:
```
error[E1203]: `Async` capability cannot be explicitly bound
  --> src/main.ori:1:6
   |
1  | with Async = SomeImpl in
   |      ^^^^^ `Async` is a marker capability
   |
   = note: `Async` context is provided by the runtime or concurrency patterns
   = help: use `parallel`, `spawn`, or `nursery` to create async contexts
```

### Async Context Creation

`Async` context is provided by:
- The runtime for `@main () uses Suspend`
- Concurrency patterns: `parallel`, `spawn`, `nursery`

```ori
@main () -> void uses Suspend = {
    // Async context exists here
    parallel(tasks: [...]),  // Creates sub-contexts for tasks
}
```

---

## Capability Calling Convention

Capabilities are called using the trait name as a namespace:

```ori
@fetch (url: str) -> Result<Response, Error> uses Http =
    Http.get(url: url)

@log_and_fetch (url: str) -> Result<Response, Error> uses Http, Logger = {
    Logger.info(message: `Fetching {url}`)
    Http.get(url: url)
}
```

The compiler resolves `Http.get(...)` to the currently bound implementation based on the resolution priority order.

---

## Capability Sets

### Combining Capabilities

Function signatures declare capability sets:

```ori
@fn1 () uses Http, Cache = ...
@fn2 () uses Cache, Logger = ...
@fn3 () uses Http, Cache, Logger = {fn1(), fn2()}  // Must declare union
```

### Set Operations

| Operation | Result |
|-----------|--------|
| Caller has `A, B`, callee needs `A` | OK |
| Caller has `A`, callee needs `A, B` | ERROR |
| `fn1: A, B` + `fn2: B, C` in same body | Body must declare `A, B, C` |

---

## Examples

### Complete Capability Wiring

```ori
// Define capabilities
trait Logger { @info (message: str) -> void }
trait Database { @query (sql: str) -> Result<Rows, Error> uses Suspend }
trait Http { @get (url: str) -> Result<Response, Error> uses Suspend }

// Default implementations
def impl Logger { @info (message: str) -> void = print(msg: message) }

// Production implementations (no def impl — must be explicitly provided)
type ProdDatabase = { connection: Connection }
impl Database for ProdDatabase { ... }

type ProdHttp = { client: HttpClient }
impl Http for ProdHttp { ... }

// Application code
@fetch_and_store (url: str) -> Result<void, Error> uses Http, Database, Logger =
    {
        Logger.info(message: `Fetching {url}`)
        let response = Http.get(url: url)?
        Database.query(sql: `INSERT INTO cache VALUES ('{url}', '{response}')`)?
        Ok(())
    }

// Main wiring
@main () -> void uses Suspend = {
    let db = ProdDatabase { connection: connect() }
    let http = ProdHttp { client: create_client() }

    with Database = db, Http = http in
        fetch_and_store(url: "https://example.com")
    // Logger uses def impl automatically
}
```

### Testing with Mocks

```ori
@test_fetch_and_store tests @fetch_and_store () -> void = {
    let mock_http = MockHttp { responses: {"https://example.com": "data"} }
    let mock_db = MockDatabase { queries: [] }
    let mock_logger = MockLogger { messages: [] }

    with Http = mock_http, Database = mock_db, Logger = mock_logger in
        fetch_and_store(url: "https://example.com")

    assert_eq(actual: mock_db.queries.len(), expected: 1)
    assert_eq(actual: mock_logger.messages.len(), expected: 1)
}
```

---

## Implementation

### Compiler Changes

1. **Parser**: Extend `with_expr` grammar to support comma-separated bindings
2. **Type checker**: Implement capability resolution with priority order
3. **Type checker**: Add variance checking for capability requirements
4. **Type checker**: Prohibit `Async` in `with...in` bindings
5. **Error reporting**: Add error codes E1200-E1203 with helpful messages

### Test Cases

1. Partial provision with multiple capabilities
2. Nested `with...in` shadowing
3. Capability variance (more can call fewer)
4. Missing capability error
5. Type mismatch for capability binding
6. Async binding prohibition
7. Resolution priority (inner > outer > imported > local)

---

## Spec Changes Required

### Update `14-capabilities.md`

1. Add grammar reference for updated `with_expr`
2. Add partial provision rules
3. Add nested binding semantics
4. Update priority resolution order (5 levels)
5. Add capability variance rules
6. Add explicit declaration requirement
7. Add Async binding prohibition

### Update `grammar.ebnf`

Change:
```ebnf
with_expr = "with" identifier "=" expression "in" expression .
```

To:
```ebnf
with_expr = "with" capability_binding { "," capability_binding } "in" expression .
capability_binding = identifier "=" expression .
```

---

## Summary

| Aspect | Rule |
|--------|------|
| Partial provision | Provide some, rest use defaults |
| Nested `with` | Inner shadows outer |
| Resolution order | Inner `with` → Outer `with` → Imported `def impl` → Local `def impl` → Error |
| Variance | More caps can call fewer (not reverse) |
| Declaration | Must be explicit, no inference |
| Unbound | Error if no `with` or `def impl` |
| Async | Marker capability, cannot be bound via `with...in` |
| Type check | Binding must implement capability trait |
| Calling | `CapabilityName.method(...)` namespace syntax |
