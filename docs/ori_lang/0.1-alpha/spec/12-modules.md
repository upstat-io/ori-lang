---
title: "Modules"
description: "Ori Language Specification — Modules"
order: 12
section: "Declarations"
---

# Modules

Every source file defines one module.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § SOURCE STRUCTURE (import, extension_def, extension_import)

## Entry Point Files

| File | Purpose |
|------|---------|
| `main.ori` | Binary entry point (must contain `@main`) |
| `lib.ori` | Library entry point (defines public API) |
| `mod.ori` | Directory module entry point (within a package) |

`lib.ori` is the package-level public interface. `mod.ori` is a directory-level public interface within a package. A package root cannot use `mod.ori` as its library entry point; `lib.ori` is required.

## Module Names

| File Path | Module Name |
|-----------|-------------|
| `src/main.ori` | `main` |
| `src/lib.ori` | (package name) |
| `src/math.ori` | `math` |
| `src/http/client.ori` | `http.client` |
| `src/http/mod.ori` | `http` |

## Imports

### Relative (Local Files)

```ori
use "./math" { add, subtract }
use "../utils/helpers" { format }
```

Paths start with `./` or `../`, resolve from current file, omit `.ori`.

### Module (Stdlib/Packages)

```ori
use std.math { sqrt, abs }
use std.net.http as http
```

### Private Access

```ori
use "./math" { ::internal_helper }
```

`::` prefix imports private (non-pub) items.

### Aliases

```ori
use "./math" { add as plus }
use std.collections { HashMap as Map }
```

### Default Bindings

When importing a trait that has a `def impl` in its source module, the default implementation is automatically bound to the trait name:

```ori
use std.net.http { Http }  // Http bound to default impl
Http.get(url: "...")       // Uses default
```

Override with `with...in`:

```ori
with Http = MockHttp {} in
    Http.get(url: "...")   // Uses mock
```

To import a trait without its default:

```ori
use std.net.http { Http without def }  // Import trait, skip def impl
```

See [Declarations § Default Implementations](08-declarations.md#default-implementations).

## Visibility

Items are private by default. `pub` exports:

```ori
pub @add (a: int, b: int) -> int = a + b
pub type User = { id: int, name: str }
pub $timeout = 30s
```

### Nested Module Visibility

Parent modules cannot access child private items. Child modules cannot access parent private items. Sibling modules cannot access each other's private items.

The `::` prefix allows importing private items for testing:

```ori
use "./internal" { ::private_helper }  // Explicit private access
```

## Re-exports

```ori
pub use "./client" { get, post }
```

Re-exporting a trait includes its `def impl` if both are public:

```ori
pub use std.logging { Logger }  // Re-exports trait AND def impl
```

To re-export a trait without its default:

```ori
pub use std.logging { Logger without def }  // Strips def impl permanently
```

When a trait is re-exported `without def`, consumers cannot access the original default through that export path — they must import from the original source.

### Re-export Chains

Re-exports can chain through multiple levels. An item must be `pub` at every level of the chain:

```ori
// level3.ori
pub @deep () -> str = "deep"

// level2.ori
pub use "./level3" { deep }

// level1.ori
pub use "./level2" { deep }

// main.ori
use "./level1" { deep }  // Works through the chain
```

Aliases propagate through chains. The same underlying item imported through multiple paths is not an error.

## Extensions

Extensions add methods to existing types without modifying their definition.

### Definition

```ori
extend Iterator {
    @count (self) -> int = ...
}
```

Constrained extensions use angle brackets or where clauses (equivalent):

```ori
// Angle bracket form
extend<T: Clone> [T] {
    @duplicate_all (self) -> [T] = self.map(transform: x -> x.clone())
}

// Where clause form
extend [T] where T: Clone {
    @duplicate_all (self) -> [T] = self.map(transform: x -> x.clone())
}

extend Iterator where Self.Item: Add {
    @sum (self) -> Self.Item = ...
}
```

Extensions may be defined for concrete types, generic types, trait implementors, and constrained generics.

Extensions cannot:
- Add fields to types
- Implement traits (use `impl Trait for Type`)
- Override existing methods
- Add static methods (methods without `self`)

### Visibility

Visibility is block-level. All methods in a `pub extend` block are public; all in a non-pub block are module-private.

```ori
pub extend Iterator {
    @count (self) -> int = ...  // Publicly importable
}

extend Iterator {
    @internal (self) -> int = ...  // Module-private
}
```

### Import

```ori
extension std.iter.extensions { Iterator.count, Iterator.last }
extension "./my_ext" { Iterator.sum }
```

Method-level granularity required; wildcards prohibited.

### Resolution Order

When calling `value.method()`:

1. **Inherent methods** — methods in `impl Type { }`
2. **Trait methods** — methods from implemented traits
3. **Extension methods** — methods from imported extensions

If multiple imported extensions define the same method, the call is ambiguous. Use qualified syntax:

```ori
use "./ext_a" as ext_a
ext_a.Point.distance(p)  // Calls ext_a's implementation
```

### Scoping

Extension imports are file-scoped. To re-export:

```ori
pub extension std.iter.extensions { Iterator.count }
```

### Orphan Rules

An extension must be in the same package as either the type being extended OR at least one trait bound in a constrained extension.

## Resolution

1. Local bindings (inner first)
2. Function parameters
3. Module-level items
4. Imports
5. Prelude

Circular dependencies prohibited. The compiler detects cycles using depth-first traversal of the import graph and reports all cycles found.

## Import Path Resolution

When processing a `use` statement, the compiler determines the target module:

1. **Relative path** (`"./..."`, `"../..."`): Resolve relative to current file's directory
2. **Package path** (`"pkg_name"`): Look up in `ori.toml` dependencies
3. **Standard library** (`std.xxx`): Built-in stdlib modules

This is distinct from *name resolution* within a module (see Resolution above).

## Prelude

Available without import:

**Types**: `int`, `float`, `bool`, `str`, `char`, `byte`, `void`, `Never`, `Duration`, `Size`, `Option<T>`, `Result<T, E>`, `Ordering`, `Error`, `TraceEntry`, `Range<T>`, `Set<T>`, `Channel<T>`, `[T]`, `{K: V}`

**Functions**: `print`, `len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`, `int`, `float`, `str`, `byte`, `compare`, `min`, `max`, `panic`, `todo`, `unreachable`, `dbg`, `hash_combine`, all assertions

**Traits**: `Eq`, `Comparable`, `Hashable`, `Printable`, `Debug`, `Clone`, `Default`, `Iterator`, `DoubleEndedIterator`, `Iterable`, `Collect`, `Into`, `Traceable`

| Trait | Method | Description |
|-------|--------|-------------|
| `Eq` | `==`, `!=` | Equality comparison |
| `Comparable` | `.compare()` | Ordering comparison |
| `Hashable` | `.hash()` | Hash value for map keys |
| `Printable` | `.to_str()` | String representation |
| `Debug` | `.debug()` | Developer-facing representation |
| `Clone` | `.clone()` | Explicit value duplication |
| `Default` | `.default()` | Default value construction |
| `Iterator` | `.next()` | Iterate forward |
| `DoubleEndedIterator` | `.next_back()` | Iterate both directions |
| `Iterable` | `.iter()` | Produce an iterator |
| `Collect` | `.from_iter()` | Build from iterator |
| `Into` | `.into()` | Type conversion |
| `Traceable` | `.with_trace()`, `.trace()` | Error trace propagation |

**Functions**: `repeat`

## Standard Library

| Module | Description |
|--------|-------------|
| `std.math` | Mathematical functions |
| `std.io` | I/O traits |
| `std.fs` | Filesystem |
| `std.net.http` | HTTP |
| `std.time` | Date/time |
| `std.json` | JSON |
| `std.crypto` | Cryptography |

## Test Modules

Tests in `_test/` with `.test.ori` extension can access private items:

```
src/
  math.ori
  _test/
    math.test.ori
```

## Package Structure

### Library Package

A library package exports its public API via `lib.ori`:

```
my_lib/
├── ori.toml
├── src/
│   ├── lib.ori      # Library entry point
│   └── internal.ori # Internal implementation
```

### Binary Package

A binary package has `main.ori` with an `@main` function:

```
my_app/
├── ori.toml
├── src/
│   ├── main.ori    # Binary entry point
│   └── utils.ori
```

### Library + Binary

A package can contain both. The binary imports from the library using the package name and can only access public items:

```ori
// lib.ori
pub @exported () -> int = 42
@internal () -> int = 1  // Private

// main.ori
use "my_pkg" { exported }      // OK: public
use "my_pkg" { ::internal }    // ERROR: private access not allowed
```

This enforces clean API boundaries.

## Package Manifest

```toml
[project]
name = "my_project"
version = "0.1.0"

[dependencies]
some_lib = "1.0.0"
```

Entry point: `@main` function for binaries, `lib.ori` for libraries.
