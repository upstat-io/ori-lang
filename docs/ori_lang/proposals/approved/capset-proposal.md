# Proposal: Named Capability Sets (`capset`)

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-02-15
**Approved:** 2026-02-15
**Affects:** Compiler (parser, name resolution, type checker), grammar, capabilities
**Related:** `capability-sets-proposal.md` (draft, complementary — addresses binding ergonomics)

---

## Summary

This proposal introduces `capset`, a named capability set declaration that groups multiple capabilities under a single name. Capsets are transparent aliases — they expand to their constituent capabilities before type checking. This reduces signature noise, creates stable dependency surfaces, and enables domain-driven capability modeling.

---

## Problem Statement

As applications grow, capability lists in function signatures become verbose:

```ori
@handle_request (req: Request) -> Response
    uses Http, Cache, Logger, Clock, Metrics, Database, Suspend = ...

@process_payment (order: Order) -> Result<Receipt, PaymentError>
    uses Http, Logger, Clock, Crypto, Database, Suspend = ...
```

This causes three problems:

1. **Signature noise**: Long `uses` clauses obscure what a function does
2. **Signature churn**: Adding a capability to a layer (e.g., adding `Metrics` to the infrastructure layer) requires updating every function in that layer
3. **No domain vocabulary**: Developers think in terms of "this function needs network access" or "this function needs the runtime environment," not "this function needs Http, Dns, Tls"

The capability composition proposal (approved 2026-01-29) addresses *binding* ergonomics (`with A = ..., B = ... in ...`) but not *declaration* ergonomics. Wiring functions help with test setup, but don't help with signature readability.

---

## Design

### Core Concept

A `capset` is a **transparent alias** for a set of capabilities. It is expanded to its constituent capabilities during name resolution, before type checking. A capset is not a trait, not a type, and has no runtime representation.

This follows Koka's approach to effect aliases (`alias io = <console,net,fsys>`), which is the most mature model for this pattern among production effect systems.

### Declaration

```ori
capset Net = Http, Dns, Tls
capset Observability = Logger, Metrics, Tracing
capset Runtime = Clock, Random, Env
capset WebService = Net, Observability, Runtime, Database, Suspend
```

A `capset` declaration:
- Is a module-level declaration (like `type` or `trait`)
- Contains one or more capabilities or other capsets (comma-separated)
- Supports visibility modifiers (`pub`, `pub(package)`)
- Uses PascalCase names (same convention as traits)

### Usage in `uses` Clauses

Capsets can be used anywhere a capability name is accepted in a `uses` clause:

```ori
@handle_request (req: Request) -> Response uses WebService =
    ...

@pure_transform (data: Data) -> Data uses Runtime =
    ...
```

Capsets and individual capabilities can be mixed:

```ori
@send_email (to: str, body: str) -> Result<void, Error> uses Net, Logger =
    ...
```

### Expansion

The compiler expands capsets transitively and deduplicates:

```ori
capset Net = Http, Dns
capset Infra = Net, Logger, Cache

// This:
@fn () -> void uses Infra, Http = ...

// Expands to:
@fn () -> void uses Http, Dns, Logger, Cache = ...
```

Expansion is a set operation — duplicates are eliminated, order is irrelevant.

---

## Grammar Changes

### New Declaration

```ebnf
capset_decl = [ visibility ] "capset" IDENTIFIER "=" capset_member { "," capset_member } .
capset_member = IDENTIFIER .
```

### `uses` Clause (Unchanged)

The `uses` clause grammar does not change:

```ebnf
uses_clause = "uses" identifier { "," identifier } .
```

The change is in name resolution: identifiers in a `uses` clause may resolve to either a capability trait or a capset. Capsets are expanded before type checking.

### `capset` as Keyword

`capset` becomes a reserved keyword.

---

## Semantic Rules

### Transparency

Capsets are expanded before type checking. The type system never sees capset names — only individual capability traits. Two functions with equivalent expanded capability sets are interchangeable regardless of whether they used capsets:

```ori
capset Net = Http, Dns

@fn_a () -> void uses Net = ...
@fn_b () -> void uses Http, Dns = ...

// fn_a and fn_b have identical capability requirements
```

### Set Semantics

Capability sets use set semantics:
- **Deduplication**: A capability appearing multiple times (directly or via capsets) is counted once
- **Order independence**: `uses A, B` is equivalent to `uses B, A`
- **Flattening**: Nested capsets are fully flattened before comparison

### Cycle Prohibition

Capset definitions must not form cycles:

```ori
capset A = B, Http
capset B = A, Cache  // ERROR: cyclic capset definition
```

### Non-Empty Requirement

A capset must contain at least one member:

```ori
capset Empty =  // ERROR: capset must contain at least one capability
```

### No Shadowing of Traits

A capset name must not collide with a trait name in the same scope:

```ori
trait Http { ... }
capset Http = Logger, Cache  // ERROR: `Http` already defined as a trait
```

### Capsets Are Not Traits

Because a capset is not a trait:

- **No `impl`**: You cannot `impl SomeCapset for SomeType`
- **No `def impl`**: You cannot `def impl SomeCapset { ... }`
- **No `with` binding**: You cannot `with SomeCapset = expr in ...`
- **No method calls**: You cannot `SomeCapset.method(...)`

Capsets participate only in `uses` clauses and other `capset` declarations.

### Variance Interaction

Capability variance works on the expanded set:

```ori
capset Runtime = Clock, Random, Env

@needs_clock () -> void uses Clock = ...
@needs_runtime () -> void uses Runtime = ...

@caller () -> void uses Runtime = {
    needs_clock(),    // OK: Runtime includes Clock
    needs_runtime(),  // OK: same set
}
```

A function `uses Runtime` can call any function whose expanded capability set is a subset.

---

## Visibility

Capsets follow standard Ori visibility rules:

```ori
// Public: usable by any module that imports it
pub capset Net = Http, Dns, Tls

// Package-private: usable within the package
pub(package) capset Infra = Net, Logger, Cache

// Private (default): usable only within the defining module
capset Internal = Logger, Metrics
```

When a capset is `pub`, all its constituent capabilities must also be accessible to importers. Referencing a non-accessible capability in a public capset is an error:

```ori
capset Private = SomeInternalTrait  // private

pub capset Broken = Private  // ERROR: `Broken` is pub but `Private` is not accessible
```

---

## Error Messages

### Cyclic Definition

```
error[E1220]: cyclic capset definition
  --> src/capabilities.ori:1:1
   |
1  | capset A = B, Http
   | ^^^^^^^^^^^^^^^^^^ `A` is defined in terms of `B`
2  | capset B = A, Cache
   | ^^^^^^^^^^^^^^^^^^ `B` is defined in terms of `A`
   |
   = note: capset definitions must be acyclic
```

### Empty Capset

```
error[E1221]: empty capset
  --> src/capabilities.ori:1:1
   |
1  | capset Empty =
   |              ^ expected at least one capability
```

### Name Collision

```
error[E1222]: `Http` is already defined as a trait
  --> src/capabilities.ori:5:8
   |
2  | trait Http { ... }
   | ---------- trait defined here
5  | capset Http = Logger, Cache
   |        ^^^^ cannot redefine as capset
```

### Missing Capability via Capset

When a function requires capabilities via a capset and the caller is missing some, the existing E1200 error is enhanced with capset expansion context:

```
error[E1200]: missing capability `Dns`
  --> src/main.ori:8:5
   |
8  |     net_operation()
   |     ^^^^^^^^^^^^^^^ requires `Dns` capability
   |
   = note: `net_operation` uses `Net` which expands to: Http, Dns, Tls
   = note: `caller` has: Http, Tls
   = help: add `Dns` to caller's capability list, or add `Net`
```

### Redundant Capability Warning

When a capability is listed both individually and via a capset:

```
warning[W1220]: redundant capability `Http`
  --> src/main.ori:3:1
   |
3  | @fn () -> void uses Net, Http = ...
   |                          ^^^^ `Http` is already included via `Net`
   |
   = note: `Net` expands to: Http, Dns, Tls
   = help: remove `Http` from the uses clause
```

### Non-Capability in Capset

When a capset references something that is neither a capability trait nor another capset:

```
error[E1223]: `MyStruct` is not a capability or capset
  --> src/capabilities.ori:1:16
   |
1  | capset Bad = MyStruct, Http
   |              ^^^^^^^^ expected a capability trait or capset
   |
   = note: capset members must be capability traits or other capsets
```

---

## Standard Library Capsets

> **Note:** The following capsets are illustrative examples showing the intended usage pattern. Actual standard library capsets will be defined when the capabilities they reference (e.g., `Dns`, `Tls`, `Metrics`, `Tracing`) are added to the standard capabilities table.

The standard library may define common capsets for convenience. These are not special — they follow the same rules as user-defined capsets:

```ori
// std/capabilities.ori (illustrative — not yet defined)

/// Network operations (HTTP, DNS, TLS)
pub capset Net = Http, Dns, Tls

/// Observability (logging, metrics, tracing)
pub capset Observability = Logger, Metrics, Tracing

/// Time and randomness
pub capset Runtime = Clock, Random, Env

/// All I/O capabilities
pub capset IO = Net, FileSystem, Database, Suspend
```

Users would import and use them like any other declaration:

```ori
use std.capabilities { Net, Runtime }

@fetch_data (url: str) -> Result<str, Error> uses Net, Runtime, Suspend = ...
```

---

## Relationship to Capability Sets Proposal

The `capability-sets-proposal.md` (draft, 2026-02-03) addresses a complementary problem: **binding-time ergonomics** (`let $testing = with { Http = MockHttp, ... }` → `with $testing in ...`). That proposal reduces wiring repetition in `with...in` expressions; this proposal reduces signature noise in `uses` clauses. The two are independent and can coexist.

| | `capset` (this proposal) | Capability sets (other draft) |
|--|---|---|
| **Problem** | Verbose `uses` clauses | Repetitive `with...in` wiring |
| **Syntax** | `capset Net = Http, Dns` | `let $testing = with { Http = MockHttp }` |
| **Usage** | `@fn () uses Net = ...` | `with $testing in expr` |
| **Mechanism** | Transparent name expansion | Compile-time binding constant |

---

## Interaction with Other Features

### `with...in` Bindings

Capsets cannot be used in `with...in` because they are not traits:

```ori
capset Net = Http, Dns

with Net = something in ...  // ERROR: `Net` is a capset, not a capability trait
```

Instead, bind individual capabilities or use a wiring function:

```ori
// Wiring function pattern (already supported)
@with_net<R> (body: () -> R) -> R =
    with Http = prod_http, Dns = prod_dns in
        body()

// Usage
with_net(body: () -> Response = handle_request(req))
```

### `def impl`

Capsets cannot have default implementations (they are not traits):

```ori
def impl Net { ... }  // ERROR: `Net` is a capset, not a capability trait
```

### Capability Propagation

Propagation works on the expanded set. If a callee `uses Net` and the caller doesn't declare `Http` (a member of `Net`), the error points to the specific missing capability:

```ori
capset Net = Http, Dns

@callee () -> void uses Net = ...

@caller () -> void uses Http =
    callee()  // ERROR: missing capability `Dns` (required by `Net`)
```

### Documentation

Tools (LSP, `ori doc`) should display both the capset name and its expansion:

```
fn handle_request(req: Request) -> Response
    uses WebService [= Net, Observability, Runtime, Database, Suspend]
```

---

## Examples

### Web Service Module

```ori
use std.capabilities { Net, Observability }

capset ServiceDeps = Net, Observability, Database, Suspend

@handle_get_user (id: int) -> Result<Response, Error> uses ServiceDeps = {
    Logger.info(message: `GET /users/{id}`)
    let user = Database.query(sql: `SELECT * FROM users WHERE id = {id}`)?
    Ok(Response.json(body: user))
}

@handle_create_user (req: Request) -> Result<Response, Error> uses ServiceDeps = {
    Logger.info(message: "POST /users")
    let body = req.json()?
    Database.query(sql: `INSERT INTO users ...`)?
    Ok(Response.created())
}
```

Adding `Metrics` to the service layer requires changing only the capset:

```ori
capset ServiceDeps = Net, Observability, Database, Metrics, Suspend
// All functions using ServiceDeps now include Metrics — no signature changes needed
```

### Testing with Capsets

```ori
capset ServiceDeps = Net, Observability, Database, Suspend

@with_test_service<R> (body: () -> R) -> R = {
    let http = MockHttp { responses: {} }
    let dns = MockDns {}
    let tls = MockTls {}
    let logger = MockLogger { messages: [] }
    let db = MockDatabase { queries: [] }

    with Http = http, Dns = dns, Tls = tls, Logger = logger, Database = db in
        body()
}

@test_get_user tests @handle_get_user () -> void =
    with_test_service(body: () -> void = {
        let response = handle_get_user(id: 1)?
        assert_eq(actual: response.status, expected: 200)
    })
```

### Layered Capsets

```ori
// Small, focused capsets
capset Net = Http, Dns, Tls
capset Storage = Database, Cache, FileSystem
capset Observe = Logger, Metrics

// Composed into larger groups
capset Backend = Net, Storage, Observe, Suspend

// Domain-specific subsets
capset ReadOnly = Net, Cache, Observe
capset WriteOnly = Storage, Observe
```

---

## Implementation

### Compiler Changes

1. **Lexer**: Add `capset` as a reserved keyword
2. **Parser**: Parse `capset_decl` as a new declaration form
3. **Name resolution**: When resolving names in `uses` clauses, check for capset declarations and expand transitively
4. **Cycle detection**: Detect cycles in capset definitions during name resolution (topological sort)
5. **Deduplication**: After expansion, deduplicate the capability set
6. **Type checker**: No changes — capsets are fully expanded before type checking sees them
7. **Error reporting**: Add error codes E1220-E1223; enhance E1200 to show capset expansion context
8. **LSP**: Show capset expansion on hover; autocomplete capset names in `uses` clauses
9. **Warnings**: Detect redundant capabilities (W1220)

### IR Representation

```rust
/// A capset declaration in the AST
struct CapsetDecl {
    name: Name,
    visibility: Visibility,
    members: Vec<Name>,  // capability traits or other capsets
    span: Span,
}
```

After expansion, the IR stores only the flat set of capability trait names. The capset name does not appear in the type-checked IR.

### Expansion Algorithm

```
expand(capset_name, visited = {}) -> Set<CapabilityTrait>:
    if capset_name in visited:
        error: cyclic capset definition
    visited.add(capset_name)

    result = {}
    for member in capset_name.members:
        if member is CapsetDecl:
            result = result | expand(member, visited)
        else if member is CapabilityTrait:
            result.add(member)
        else:
            error: not a capability or capset

    return result
```

### Test Cases

1. Basic capset declaration and usage in `uses`
2. Nested capsets (capset containing capsets)
3. Deeply nested capsets (3+ levels)
4. Duplicate elimination across overlapping capsets
5. Cycle detection (direct and indirect)
6. Empty capset error
7. Name collision with trait
8. Non-capability member error
9. Visibility rules (pub capset with private member)
10. Capability variance with capset-expanded sets
11. Error messages showing capset expansion context
12. Redundant capability warning
13. Capset in `with...in` error
14. Capset in `def impl` error
15. Mixed capset and individual capabilities in `uses`

---

## Spec Changes Required

### Update `14-capabilities.md`

Add section "Named Capability Sets" after "Capability Sets":
- Grammar reference for `capset_decl`
- Expansion semantics
- Transparency rule
- Restrictions (no cycles, no empty, no trait collision)
- Standard library capsets

### Update `grammar.ebnf`

Add to declarations section:

```ebnf
capset_decl = [ visibility ] "capset" IDENTIFIER "=" capset_member { "," capset_member } .
capset_member = IDENTIFIER .
```

### Add Error Codes

| Code | Description |
|------|-------------|
| E1220 | Cyclic capset definition |
| E1221 | Empty capset |
| E1222 | Capset name collides with trait name |
| E1223 | Capset member is not a capability trait or capset |
| W1220 | Redundant capability in `uses` clause |

---

## Design Decisions

1. **Transparent expansion over new type** — Capsets are aliases, not types. This follows Koka's mature model for effect aliases and avoids introducing a new concept into the type system. The type checker never sees capset names.

2. **`capset` keyword over `alias`** — `alias` is too generic (could mean type alias). `capset` is domain-specific and self-documenting. It also reserves `alias` for potential future use as a general type alias mechanism.

3. **No parametric capsets (for now)** — Koka supports parametric effect aliases (`alias try<a> = ...`). Ori could add `capset Net<Transport> = Transport, Dns` in the future, but the primary use case (grouping known capabilities) doesn't require it. Deferred to avoid unnecessary complexity.

4. **No `with` binding for capsets** — Since a capset is not a trait, it has no single implementation to bind. The wiring function pattern provides equivalent ergonomics without new language mechanics.

5. **Warning for redundancy, not error** — `uses Net, Http` is redundant but not harmful. A warning encourages clean code without breaking builds during refactoring when capsets change.

6. **Standard library capsets are not special** — `std.capabilities.Net` follows the same rules as any user-defined capset. This avoids privileged language constructs.

7. **Non-empty requirement** — An empty capset has no meaning and likely indicates a mistake. Unlike Koka's `alias pure = <>` (which represents the empty effect row), Ori's `uses` clause is simply absent for pure functions.

---

## Future Extensions

### Parametric Capsets

```ori
capset Service<DB> = Http, Logger, DB, Suspend
// Usage: uses Service<Postgres>
```

### Capset Subtraction

```ori
@pure_subset () -> void uses WebService - Suspend = ...
// WebService without Suspend
```

### Capset Providers (Bundled Binding)

A future proposal could introduce a way to bind all capabilities in a capset with a single provider:

```ori
type ProdNet = { http: ProdHttp, dns: ProdDns, tls: ProdTls }
impl Net for ProdNet { ... }  // Would require capsets to become trait-like

with Net = ProdNet { ... } in ...
```

This would require capsets to become more than aliases. Deferred pending real-world usage patterns.

---

## Summary

| Aspect | Rule |
|--------|------|
| What it is | Transparent alias for a set of capabilities |
| Syntax | `capset Name = Cap1, Cap2, ...` |
| Expansion | Transitive, deduplicated, before type checking |
| Visibility | Standard (`pub`, `pub(package)`, private) |
| In `uses` | Yes, mixed with individual capabilities |
| In `with...in` | No (not a trait) |
| In `def impl` | No (not a trait) |
| Cycles | Prohibited |
| Empty | Prohibited |
| Name collision | Prohibited with traits in same scope |
| Keyword | `capset` (reserved) |
