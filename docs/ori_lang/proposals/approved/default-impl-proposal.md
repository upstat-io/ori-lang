# Proposal: Default Implementations (`def impl`)

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-01-29
**Approved:** 2026-01-29

---

## Summary

Introduce `def impl` syntax to declare a default implementation for a trait. When a module exports both a trait and its `def impl`, importing the trait automatically binds the default implementation to the trait name.

```ori
// Definition
pub trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

pub def impl Http {
    @get (url: str) -> Result<Response, Error> = ...
    @post (url: str, body: str) -> Result<Response, Error> = ...
}

// Usage - no with...in needed
use std.net.http { Http }

@fetch () -> Result<str, Error> uses Http =
    Http.get(url: "https://api.example.com/data")
```

---

## Motivation

### The Problem

Currently, using a capability requires verbose `with...in` boilerplate:

```ori
use std.net.http { Http, StdHttp }

@main () -> void =
    with Http = StdHttp {} in
        run(
            let user = fetch_user(id: 1)?,
            print(msg: `Got user: {user.name}`),
        )
```

Issues:
1. **Verbose** — Every entry point needs `with...in` for each capability
2. **Confusing types** — Users must know about `StdHttp`, `RealHttp`, etc.
3. **Documentation burden** — Guide must explain trait vs implementation distinction
4. **Boilerplate** — The common case (use the standard impl) requires the most code

### The Solution

`def impl` provides a default that's automatically bound when importing:

```ori
use std.net.http { Http }

@main () -> void =
    run(
        let user = fetch_user(id: 1)?,  // Http "just works"
        print(msg: `Got user: {user.name}`),
    )
```

- No `StdHttp` type to know about
- No `with...in` for the common case
- `with...in` still available for testing/custom config

### Design Philosophy

This is "classes without classes" — compositional behavior without inheritance:

| OOP Concept | Ori Equivalent |
|-------------|----------------|
| Interface | `trait` |
| Default class | `def impl` |
| Dependency injection | `with...in` override |
| Constructor | Importing brings the default |

Benefits of traditional classes (encapsulation, polymorphism, defaults) without the baggage (inheritance hierarchies, hidden state, `this` confusion).

---

## Design

### Syntax

```
def_impl = "def" "impl" trait_name "{" { impl_method } "}" .
impl_method = "@" identifier "(" parameters ")" "->" type "=" expression .
```

The `def` keyword marks this as the default implementation. No `for Type` clause — the implementation is anonymous.

### Visibility

`def impl` can be `pub` or private:

```ori
pub def impl Http { ... }   // Exported with trait
def impl Http { ... }       // Module-internal only
```

### Semantics

1. **One default per trait per module** — Multiple `def impl Http` in the same module is a compile error
2. **One default per trait globally is NOT enforced** — Different modules can have different defaults; the importer chooses
3. **Import binds the default** — `use mod { Trait }` binds `Trait` to the default if one exists
4. **`with...in` overrides** — Can always override with explicit `with Trait = other in`
5. **Same-module usage** — Within the defining module, the trait name resolves to the default

### Name Resolution

When resolving a capability name:

1. Check for `with...in` binding (innermost first)
2. Check for imported default
3. Check for module-local `def impl`
4. Error: capability not provided

### Method Dispatch

`Http.get(url: "...")` calls the method on the bound default. This is identical to how `with Http = impl in Http.get(...)` works today.

### No Anonymous Type Needed

The `def impl` doesn't create a named type. It's an anonymous implementation. Users never see a type name — they just use the trait name.

If state is needed, the implementation can use module-level bindings. This is the recommended pattern for configurable defaults:

```ori
// Configuration via module-level immutable bindings
let $timeout = 30s
let $base_url = "https://api.example.com"

pub def impl Http {
    @get (url: str) -> Result<Response, Error> =
        __http_get(url: $base_url + url, timeout: $timeout)
}
```

This pattern keeps the default stateless while allowing configuration through module-level constants.

---

## Examples

### Standard Library Capability

```ori
// std/net/http/mod.ori

pub type Response = {
    status: int,
    headers: {str: str},
    body: str,
}

pub trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
    @put (url: str, body: str) -> Result<Response, Error>
    @delete (url: str) -> Result<Response, Error>
}

pub def impl Http {
    @get (url: str) -> Result<Response, Error> =
        __http_request(method: "GET", url: url, body: "")

    @post (url: str, body: str) -> Result<Response, Error> =
        __http_request(method: "POST", url: url, body: body)

    @put (url: str, body: str) -> Result<Response, Error> =
        __http_request(method: "PUT", url: url, body: body)

    @delete (url: str) -> Result<Response, Error> =
        __http_request(method: "DELETE", url: url, body: "")
}
```

### User Code

```ori
use std.net.http { Http, Response }

@fetch_user (id: int) -> Result<User, Error> uses Http =
    run(
        let response = Http.get(url: `https://api.example.com/users/{id}`)?,
        parse_user(json: response.body),
    )

@main () -> void =
    run(
        let user = fetch_user(id: 1)?,
        print(msg: `Got user: {user.name}`),
    )
```

No `with...in` needed. `Http` is automatically bound to the default from `std.net.http`.

### Testing with Mocks

```ori
type MockHttp = { responses: {str: str} }

impl Http for MockHttp {
    @get (self, url: str) -> Result<Response, Error> =
        match self.responses[url] {
            Some(body) -> Ok(Response { status: 200, headers: {}, body: body }),
            None -> Err(Error.new(msg: `Not found: {url}`)),
        }

    @post (self, url: str, body: str) -> Result<Response, Error> =
        Ok(Response { status: 201, headers: {}, body: "" })

    // ... other methods
}

@test_fetch_user tests @fetch_user () -> void =
    with Http = MockHttp { responses: {
        "https://api.example.com/users/1": `{"id": 1, "name": "Alice"}`,
    }} in run(
        let result = fetch_user(id: 1),
        assert_ok(result: result),
    )
```

The mock overrides the default via `with...in`.

### Custom Configuration

When you need custom config, use `with...in`:

```ori
type ConfiguredHttp = { timeout: Duration, base_url: str }

impl Http for ConfiguredHttp {
    @get (self, url: str) -> Result<Response, Error> =
        __http_request(
            method: "GET",
            url: self.base_url + url,
            timeout: self.timeout,
        )
    // ...
}

@main () -> void =
    with Http = ConfiguredHttp { timeout: 5s, base_url: "https://api.example.com" } in
        run(
            let user = fetch_user(id: 1)?,
            print(msg: `Got user: {user.name}`),
        )
```

### Multiple Capabilities in One Module

A module can provide defaults for multiple related traits:

```ori
// std/net/mod.ori

pub trait Http { ... }
pub def impl Http { ... }

pub trait Tcp { ... }
pub def impl Tcp { ... }
```

Both defaults are exported. Importers get whichever they import:

```ori
use std.net { Http }        // Just Http default
use std.net { Http, Tcp }   // Both defaults
```

### Name Collision

If you import a trait with a default but already have that name defined:

```ori
pub trait Http { ... }
pub def impl Http { ... }

use other.module { Http }  // ERROR: Http already defined
```

Fix with rename:

```ori
use other.module { Http as OtherHttp }  // OK
```

---

## Constraints

### One Default Per Trait Per Module

```ori
pub def impl Http { ... }
pub def impl Http { ... }  // ERROR: duplicate default for Http
```

### Default Must Implement All Methods

```ori
pub trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

pub def impl Http {
    @get (url: str) -> Result<Response, Error> = ...
    // ERROR: missing implementation for @post
}
```

### No `self` Parameter

Unlike `impl Trait for Type`, `def impl` methods don't have `self`:

```ori
// Regular impl - has self
impl Http for MyHttp {
    @get (self, url: str) -> Result<Response, Error> = ...
}

// def impl - no self (stateless)
pub def impl Http {
    @get (url: str) -> Result<Response, Error> = ...
}
```

If state is needed, use module-level bindings or create a typed impl instead.

---

## Implementation

### Lexer

Add `def` as a keyword.

### Parser

Parse `def impl Trait { ... }` as a new AST node:

```rust
pub struct DefImpl {
    pub visibility: Visibility,
    pub trait_name: Name,
    pub methods: Vec<ImplMethod>,
    pub span: Span,
}
```

### IR

Add `DefImpl` to module items. Track which traits have defaults in module metadata.

### Module System

When exporting a trait:
- Check if module has `def impl` for that trait
- If so, mark the export as "has default"

When importing a trait:
- If source module has a default, create a binding for the trait name to the default impl
- The binding is a value that can be used for method dispatch

### Type Checker

- Verify `def impl` implements all trait methods
- Verify method signatures match trait
- Verify no duplicate `def impl` for same trait in module

### Evaluator

When evaluating capability method call (e.g., `Http.get(...)`):
1. Look up `Http` in scope
2. If bound to a `def impl`, dispatch to that implementation
3. If bound via `with...in`, dispatch to that implementation
4. Error if not bound

---

## Spec Changes Required

### `08-declarations.md`

Add new section:

```markdown
## Default Implementations

A default implementation provides the standard behavior for a trait:

\`\`\`ori
pub def impl Http {
    @get (url: str) -> Result<Response, Error> = ...
}
\`\`\`

When a module exports both a trait and its `def impl`, importing the trait automatically binds the default.
```

### `12-modules.md`

Update imports section:

```markdown
### Default Bindings

When importing a trait that has a `def impl` in its source module, the default implementation is automatically bound to the trait name:

\`\`\`ori
use std.net.http { Http }  // Http bound to default impl
Http.get(url: "...")       // Uses default
\`\`\`

Override with `with...in`:

\`\`\`ori
with Http = MockHttp {} in
    Http.get(url: "...")   // Uses mock
\`\`\`
```

### `14-capabilities.md`

Update to use `def impl` pattern:

```markdown
## Capability Traits

Capabilities are traits with default implementations:

\`\`\`ori
pub trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

pub def impl Http {
    @get (url: str) -> Result<Response, Error> = ...
    @post (url: str, body: str) -> Result<Response, Error> = ...
}
\`\`\`

Import the trait to use the default:

\`\`\`ori
use std.net.http { Http }

@fetch () -> Result<str, Error> uses Http =
    Http.get(url: "...")
\`\`\`
```

### `grammar.ebnf`

Add production:

```ebnf
def_impl = "def" "impl" identifier "{" { impl_method } "}" .
```

---

## Migration

### Guide Updates

The capabilities guide (`docs/guide/13-capabilities.md`) should be significantly simplified:

1. Remove "How Capabilities Connect to the Real World" section about runtime-provided implementations
2. Remove references to `RealHttp`, `StdHttp`, etc.
3. Show simple usage: import trait, use it
4. Show override pattern for testing

### Stdlib Updates

Update capability modules to use `def impl`:

- `std.net.http` — `def impl Http`
- `std.fs` — `def impl FileSystem`
- `std.time` — `def impl Clock`
- `std.math.rand` — `def impl Random`
- `std.log` — `def impl Logger`
- `std.env` — `def impl Env`

---

## Alternatives Considered

### 1. Namespace-based approach

```ori
use std.net.http as Http
Http.get(url: "...")
```

Rejected: Too implicit. Not clear `Http` is a capability. Conflates modules with capability instances.

### 2. Explicit default import syntax

```ori
use std.net.http { Http: default }
```

Rejected: Unnecessary verbosity. If there's a default, importing the trait should give it to you.

### 3. Module-level `with` bindings

```ori
with Http = std.net.http.default

@fetch () -> ... uses Http = ...
```

Rejected: Still requires knowing about a `default` export. More boilerplate than `def impl`.

### 4. Keep `StdHttp` pattern

```ori
use std.net.http { Http, StdHttp }
with Http = StdHttp {} in ...
```

Rejected: This is what we have today. It's verbose and confusing.

---

## Summary

| Aspect | Decision |
|--------|----------|
| Syntax | `def impl Trait { ... }` |
| Keyword | `def` (short for default) |
| Visibility | `pub def impl` or `def impl` |
| One per trait per module | Yes (compile error otherwise) |
| Global uniqueness | No (different modules can have different defaults) |
| Import behavior | Importing trait binds default automatically |
| Override | `with Trait = other in` still works |
| Self parameter | No (stateless default) |

This proposal eliminates capability boilerplate while maintaining explicitness and testability.
