# Proposal: Module Namespaces

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-01-29

---

## Summary

Formalize the relationship between modules and namespaces in Ori. A module IS a namespace — importing a module gives you access to its exports via qualified names.

```ori
// Import specific items
use std.net.http { Http, Response }

// Import module as namespace
use std.net.http as http
http.get(url: "...")  // Qualified access

// Re-export
pub use "./internal" { Widget, Button }
```

---

## Motivation

### Current State

The module system (Phase 4) has basic infrastructure:
- ✅ Relative imports: `use "./path" { item }`
- ✅ Module imports: `use std.math { sqrt }`
- ✅ Aliases: `use "./math" { add as plus }`
- ✅ Module aliases: `use std.net.http as http` (parsing done)
- ✅ Visibility: `pub` keyword
- 🔶 Qualified access: `module.item` (runtime works, type checker partial)
- 🔶 Re-exports: `pub use` (parsing done, full resolution pending)

### What's Missing

1. **Clear mental model** — When is a module a namespace vs a collection of items?
2. **Qualified access semantics** — How does `http.get(...)` resolve?
3. **Namespace vs item import** — `use mod { item }` vs `use mod as ns`
4. **Re-export resolution** — How do re-exports chain?
5. **Interaction with capabilities** — How do `def impl` defaults work with namespaces?

### Goals

- Simple, consistent model: module = namespace
- Both import styles work: items or namespace
- Clear qualified access semantics
- Works well with `def impl` capability defaults

---

## Design

### Core Principle: Module = Namespace

Every `.ori` file defines one module. That module IS a namespace containing its public exports.

```
src/
  math/
    mod.ori       → namespace: math
    vectors.ori   → namespace: math.vectors
  main.ori        → namespace: main
```

### Import Styles

#### Style 1: Import Items

Import specific items into local scope:

```ori
use std.math { sqrt, abs, pow }

let x = sqrt(n: 16)  // Direct access
```

#### Style 2: Import Namespace

Import module as a namespace:

```ori
use std.math as math

let x = math.sqrt(n: 16)  // Qualified access
```

#### Style 3: Mixed

Combine both:

```ori
use std.math { sqrt }
use std.collections as collections

let x = sqrt(n: 16)
let list = collections.List.new()
```

### Qualified Access

When you import a module as a namespace, access its exports with dot notation:

```ori
use std.net.http as http

http.get(url: "...")           // Function
http.Response                   // Type
http.Http                       // Trait (and its def impl default)
```

Resolution:
1. Look up `http` in scope → finds namespace binding
2. Look up `get` in that namespace → finds exported function
3. Call the function

### Namespace Contents

A namespace contains all `pub` items from the module:

| Item Kind | Accessible As |
|-----------|---------------|
| `pub @fn` | `ns.fn(...)` |
| `pub type T` | `ns.T` |
| `pub trait T` | `ns.T` |
| `pub def impl T` | `ns.T` (bound to default) |
| `pub let $x` | `ns.$x` |

Private items (no `pub`) are not accessible via namespace.

### Re-exports

Re-export items from other modules:

```ori
// std/prelude.ori
pub use "./option" { Option, Some, None }
pub use "./result" { Result, Ok, Err }
```

Consumers see re-exported items as if defined in the re-exporting module:

```ori
use std.prelude { Option, Result }  // Works
```

#### Chained Re-exports

Re-exports can chain:

```ori
// a.ori
pub @helper () -> int = 42

// b.ori
pub use "./a" { helper }

// c.ori
pub use "./b" { helper }

// main.ori
use "./c" { helper }  // Gets helper from a, via b, via c
```

#### Namespace Re-export

Re-export an entire module as a namespace:

```ori
// std/mod.ori
pub use "./math" as math
pub use "./net" as net

// user code
use std { math, net }
math.sqrt(n: 16)
net.http.get(url: "...")
```

### Interaction with `def impl`

When a module has `pub def impl Trait`:

**Item import** — Gets trait + default bound:
```ori
use std.net.http { Http }
Http.get(url: "...")  // Uses default
```

**Namespace import** — Access via qualified name:
```ori
use std.net.http as http
http.Http.get(url: "...")  // Uses default
```

Both work identically — the default is bound to the trait name.

### Shadowing and Conflicts

#### Local Shadows Import

Local definitions shadow imports:

```ori
use std.math { sqrt }

@sqrt (n: int) -> int = n  // Shadows imported sqrt

sqrt(n: 16)  // Calls local sqrt, returns 16
```

#### Import Conflicts

Importing the same name from multiple sources is an error:

```ori
use "./a" { helper }
use "./b" { helper }  // ERROR: helper already imported
```

Fix with alias:

```ori
use "./a" { helper }
use "./b" { helper as helper_b }
```

#### Namespace Doesn't Conflict

Namespace imports don't conflict with item imports:

```ori
use std.math { sqrt }
use std.math as math

sqrt(n: 16)       // OK - uses item import
math.sqrt(n: 16)  // OK - uses namespace
```

### Nested Namespaces

Modules can be nested via directory structure:

```
std/
  net/
    mod.ori        → std.net
    http/
      mod.ori      → std.net.http
    tcp/
      mod.ori      → std.net.tcp
```

Access nested namespaces:

```ori
use std.net.http as http
use std.net.tcp as tcp

// Or import parent namespace
use std.net as net
net.http.get(url: "...")
net.tcp.connect(host: "...")
```

### `mod.ori` Convention

A directory with `mod.ori` is treated as a module:

```
math/
  mod.ori       // This file defines the `math` module
  vectors.ori   // Submodule: math.vectors
  matrices.ori  // Submodule: math.matrices
```

`mod.ori` typically re-exports submodules:

```ori
// math/mod.ori
pub use "./vectors" as vectors
pub use "./matrices" as matrices

pub @common_helper () -> int = 42
```

### Visibility Rules

| Declaration | Accessible From |
|-------------|-----------------|
| `pub @fn` | Anywhere that imports it |
| `@fn` (no pub) | Same module only |
| `pub type T` | Anywhere that imports it |
| `type T` | Same module only |
| Items in `_test/` | Can access parent's private items |

### Standard Library Structure

```
library/std/
  prelude.ori           // Auto-imported: Option, Result, traits
  mod.ori               // Re-exports submodules
  math/
    mod.ori             // std.math
  collections/
    mod.ori             // std.collections
  net/
    mod.ori             // std.net
    http/
      mod.ori           // std.net.http (with def impl Http)
    tcp/
      mod.ori           // std.net.tcp
  fs/
    mod.ori             // std.fs (with def impl FileSystem)
  time/
    mod.ori             // std.time (with def impl Clock)
```

---

## Examples

### Basic Namespace Usage

```ori
use std.collections as collections

@main () -> void = run(
    let list = collections.List.from(items: [1, 2, 3]),
    let set = collections.Set.from(items: [1, 2, 3]),
    print(msg: `List: {list}, Set: {set}`),
)
```

### Mixed Import Styles

```ori
use std.math { sqrt, PI }
use std.collections as collections
use std.net.http { Http }

@calculate () -> float =
    sqrt(n: PI * 2.0)

@fetch_items () -> Result<[Item], Error> uses Http =
    run(
        let response = Http.get(url: "/items")?,
        parse_items(json: response.body),
    )
```

### Re-exporting a Facade

```ori
// mylib/mod.ori - Public API
pub use "./internal/parser" { parse, ParseError }
pub use "./internal/compiler" { compile, CompileError }
pub use "./internal/runtime" { run, RuntimeError }

// Hide internal structure
// Users just: use mylib { parse, compile, run }
```

### Capability Module Pattern

```ori
// std/net/http/mod.ori

pub type Response = {
    status: int,
    headers: {str: str},
    body: str,
}

pub type Request = {
    method: str,
    url: str,
    headers: {str: str},
    body: str,
}

pub trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

pub def impl Http {
    @get (url: str) -> Result<Response, Error> = ...
    @post (url: str, body: str) -> Result<Response, Error> = ...
}

// Additional utilities (not part of capability)
pub @url_encode (s: str) -> str = ...
pub @url_decode (s: str) -> str = ...
```

Usage options:

```ori
// Option 1: Import just what you need
use std.net.http { Http, Response }

// Option 2: Import as namespace
use std.net.http as http
http.Http.get(url: "...")
http.url_encode(s: "hello world")

// Option 3: Mixed
use std.net.http { Http }
use std.net.http as http
Http.get(url: "...")  // Capability
http.url_encode(s: "hello world")  // Utility
```

---

## Resolution Algorithm

### Item Import: `use path { item }`

1. Resolve `path` to a module
2. Look up `item` in module's public exports
3. If found, bind `item` in current scope
4. If `item` is a trait with `def impl`, also bind the default

### Namespace Import: `use path as name`

1. Resolve `path` to a module
2. Create a namespace value containing all public exports
3. Bind `name` to the namespace in current scope

### Qualified Access: `ns.item`

1. Look up `ns` in scope
2. If `ns` is a namespace, look up `item` in it
3. If `item` is a function, return callable
4. If `item` is a type/trait, return type reference
5. If `item` is a value, return it

### Name Lookup Priority

1. Local bindings (innermost scope first)
2. Function parameters
3. Module-level definitions
4. Imported items
5. Prelude

Namespaces are just bindings, so they follow the same priority.

---

## Implementation

### Current State (Phase 4)

| Component | Status |
|-----------|--------|
| `use path { item }` | ✅ Complete |
| `use path as name` | ✅ Parsing, ✅ Runtime, 🔶 Type checker |
| `pub use` re-exports | ✅ Parsing, 🔶 Resolution |
| Qualified access `ns.item` | ✅ Runtime, 🔶 Type checker |

### Remaining Work

#### Type Checker: Namespace Types

Add `ModuleNamespace` type that tracks available exports:

```rust
pub struct ModuleNamespace {
    pub module_path: ModulePath,
    pub exports: HashMap<String, ExportedItem>,
}

pub enum ExportedItem {
    Function(FunctionSignature),
    Type(TypeId),
    Trait(TraitId, Option<DefImplId>),  // Trait with optional default
    Constant(TypeId),
    Namespace(ModuleNamespace),  // Nested namespace
}
```

#### Type Checker: Qualified Access

When type-checking `ns.item`:

1. Infer type of `ns`
2. If `ModuleNamespace`, look up `item` in exports
3. Return appropriate type for the export kind

#### Module Resolver: Re-export Chains

Track re-export chains to resolve final source:

```rust
pub struct ResolvedImport {
    pub local_name: String,
    pub source_module: ModulePath,
    pub original_name: String,
    pub export_kind: ExportKind,
}
```

#### IR: Namespace AST Node

Add namespace binding to IR:

```rust
pub enum BindingKind {
    Value(ExprId),
    Function(FunctionId),
    Type(TypeId),
    Namespace(ModulePath),  // New
}
```

---

## Spec Changes Required

### `12-modules.md`

Update with namespace semantics:

```markdown
## Namespaces

A module defines a namespace. Import a module as a namespace to access its exports via qualified names:

\`\`\`ori
use std.math as math
math.sqrt(n: 16)
\`\`\`

### Qualified Access

Access namespace members with dot notation:

\`\`\`ori
namespace.function(args)
namespace.Type
namespace.Trait
namespace.SubNamespace.item
\`\`\`

### Import Styles

| Style | Syntax | Access |
|-------|--------|--------|
| Items | `use mod { a, b }` | `a`, `b` |
| Namespace | `use mod as ns` | `ns.a`, `ns.b` |
| Mixed | Both | Both |
```

### `grammar.ebnf`

Already has the productions:

```ebnf
use_decl = "use" import_path "{" use_items "}"
         | "use" import_path "as" identifier .
```

No changes needed.

---

## Interaction with Other Features

### With `def impl`

Namespace import preserves `def impl` binding:

```ori
use std.net.http as http
http.Http.get(url: "...")  // Uses default
```

The `Http` in the namespace is bound to the `def impl` default.

### With Capabilities

Capabilities work the same whether imported as items or via namespace:

```ori
// Item import
use std.net.http { Http }
@fetch () -> Result uses Http = Http.get(url: "...")

// Namespace import
use std.net.http as http
@fetch () -> Result uses http.Http = http.Http.get(url: "...")
```

Both declare `uses Http` (the trait), both use the default.

### With Extensions

Extensions are imported separately:

```ori
use std.iter as iter
extension std.iter.extensions { Iterator.count, Iterator.sum }

let nums = iter.range(start: 0, end: 10)
nums.count()  // Extension method
```

---

## Summary

| Aspect | Decision |
|--------|----------|
| Module = Namespace | Yes, every module is a namespace |
| Item import | `use mod { item }` |
| Namespace import | `use mod as name` |
| Qualified access | `ns.item` |
| Re-exports | `pub use path { items }` |
| Nested namespaces | Via directory structure |
| `mod.ori` convention | Directory module entry point |
| Conflict handling | Error on duplicate names, use aliases |
| `def impl` in namespace | Preserved, trait name bound to default |

This proposal formalizes existing behavior and fills gaps in the module system design.
