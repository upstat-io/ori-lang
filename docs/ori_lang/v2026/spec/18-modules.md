---
title: "Modules"
description: "Clause 18: Ori Language Specification — Modules"
order: 18
section: "Language"
---

# 18 Modules

Every source file defines one module.

> **Grammar:** See [grammar.ebnf](grammar.md) § SOURCE STRUCTURE (import, extension_def, extension_import)

## 18.1 Entry point files

| File | Purpose |
|------|---------|
| `main.ori` | Binary entry point (shall contain `@main`) |
| `lib.ori` | Library entry point (defines public API) |
| `mod.ori` | Directory module entry point (within a package) |

`lib.ori` is the package-level public interface. `mod.ori` is a directory-level public interface within a package. A package root cannot use `mod.ori` as its library entry point; `lib.ori` is required.

## 18.2 Module names

| File Path | Module Name |
|-----------|-------------|
| `src/main.ori` | `main` |
| `src/lib.ori` | (package name) |
| `src/math.ori` | `math` |
| `src/http/client.ori` | `http.client` |
| `src/http/mod.ori` | `http` |

## 18.3 Imports

### 18.3.1 Relative (local files)

```ori
use "./math" { add, subtract };
use "../utils/helpers" { format };
```

Paths start with `./` or `../`, resolve from current file, omit `.ori`.

### 18.3.2 Module (stdlib/packages)

```ori
use std.math { sqrt, abs };
use std.net.http as http;
```

### 18.3.3 Private access

```ori
use "./math" { ::internal_helper };
```

`::` prefix imports private (non-pub) items.

### 18.3.4 Aliases

```ori
use "./math" { add as plus };
use std.collections { HashMap as Map };
```

### 18.3.5 Default bindings

When importing a trait that has a `def impl` in its source module, the default implementation is automatically bound to the trait name:

```ori
use std.net.http { Http };  // Http bound to default impl
Http.get(url: "...")        // Uses default
```

Override with `with...in`:

```ori
with Http = MockHttp {} in
    Http.get(url: "...")   // Uses mock
```

To import a trait without its default:

```ori
use std.net.http { Http without def };  // Import trait, skip def impl
```

See [Declarations § Default Implementations](10-declarations.md#105-default-implementations).

## 18.4 Visibility

Items are private by default. `pub` exports:

```ori
pub @add (a: int, b: int) -> int = a + b;
pub type User = { id: int, name: str }
pub $timeout = 30s;
```

### 18.4.1 Nested module visibility

Parent modules cannot access child private items. Child modules cannot access parent private items. Sibling modules cannot access each other's private items.

The `::` prefix allows any module to explicitly import private items from another module:

```ori
use "./internal" { ::private_helper };  // Explicit private access
```

The `::` prefix makes the boundary crossing visible at the import site. The compiler does not restrict where `::` imports may appear.

## 18.5 Re-exports

```ori
pub use "./client" { get, post };
```

Re-exporting a trait includes its `def impl` if both are public:

```ori
pub use std.logging { Logger };  // Re-exports trait AND def impl
```

To re-export a trait without its default:

```ori
pub use std.logging { Logger without def };  // Strips def impl permanently
```

When a trait is re-exported `without def`, consumers cannot access the original default through that export path — they shall import from the original source.

### 18.5.1 Re-export chains

Re-exports can chain through multiple levels. An item shall be `pub` at every level of the chain:

```ori
// level3.ori
pub @deep () -> str = "deep";

// level2.ori
pub use "./level3" { deep };

// level1.ori
pub use "./level2" { deep };

// main.ori
use "./level1" { deep };  // Works through the chain
```

Aliases propagate through chains. The same underlying item imported through multiple paths is not an error.

## 18.6 Extensions

Extensions add methods to existing types without modifying their definition.

### 18.6.1 Definition

```ori
extend Iterator {
    @count (self) -> int = ...;
}
```

Constrained extensions use angle brackets or where clauses (equivalent):

```ori
// Angle bracket form
extend<T: Clone> [T] {
    @duplicate_all (self) -> [T] = self.map(transform: x -> x.clone());
}

// Where clause form
extend [T] where T: Clone {
    @duplicate_all (self) -> [T] = self.map(transform: x -> x.clone());
}

extend Iterator where Self.Item: Add {
    @sum (self) -> Self.Item = ...;
}
```

Extensions may be defined for concrete types, generic types, trait implementors, and constrained generics.

Extensions cannot:
- Add fields to types
- Implement traits (use `impl Trait for Type`)
- Override existing methods
- Add static methods (methods without `self`)

### 18.6.2 Visibility

Visibility is block-level. All methods in a `pub extend` block are public; all in a non-pub block are module-private.

```ori
pub extend Iterator {
    @count (self) -> int = ...;  // Publicly importable
}

extend Iterator {
    @internal (self) -> int = ...;  // Module-private
}
```

### 18.6.3 Import

```ori
extension std.iter.extensions { Iterator.count, Iterator.last };
extension "./my_ext" { Iterator.sum };
```

Method-level granularity required; wildcards prohibited.

### 18.6.4 Resolution order

When calling `value.method()`:

1. **Inherent methods** — methods in `impl Type { }`
2. **Trait methods** — methods from implemented traits
3. **Extension methods** — methods from imported extensions

If multiple imported extensions define the same method, the call is ambiguous. Use qualified syntax:

```ori
use "./ext_a" as ext_a;
ext_a.Point.distance(p)  // Calls ext_a's implementation
```

### 18.6.5 Scoping

Extension imports are file-scoped. To re-export:

```ori
pub extension std.iter.extensions { Iterator.count };
```

### 18.6.6 Orphan rules

An extension shall be in the same package as either the type being extended OR at least one trait bound in a constrained extension.

## 18.7 Resolution

1. Local bindings (inner first)
2. Function parameters
3. Module-level items
4. Imports
5. Prelude

Circular dependencies prohibited. The compiler detects cycles using depth-first traversal of the import graph and reports all cycles found.

## 18.8 Import path resolution

When processing a `use` statement, the compiler determines the target module:

1. **Relative path** (`"./..."`, `"../..."`): Resolve relative to current file's directory
2. **Package path** (`"pkg_name"`): Look up in `ori.toml` dependencies
3. **Standard library** (`std.xxx`): Built-in stdlib modules

This is distinct from *name resolution* within a module (see Resolution above).

## 18.9 Prelude

Available without import:

**Types**: `int`, `float`, `bool`, `str`, `char`, `byte`, `void`, `Never`, `Duration`, `Size`, `Option<T>`, `Result<T, E>`, `Ordering`, `Error`, `TraceEntry`, `Range<T>`, `Set<T>`, `Channel<T>`, `[T]`, `{K: V}`, `FormatSpec`, `Alignment`, `Sign`, `FormatType`

**Functions**: `print`, `len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`, `int`, `float`, `str`, `byte`, `compare`, `min`, `max`, `panic`, `todo`, `unreachable`, `dbg`, `hash_combine`, all assertions

**Traits**: `Eq`, `Comparable`, `Hashable`, `Printable`, `Formattable`, `Debug`, `Clone`, `Default`, `Iterator`, `DoubleEndedIterator`, `Iterable`, `Collect`, `Into`, `Traceable`

| Trait | Method | Description |
|-------|--------|-------------|
| `Eq` | `==`, `!=` | Equality comparison |
| `Comparable` | `.compare()` | Ordering comparison |
| `Hashable` | `.hash()` | Hash value for map keys |
| `Printable` | `.to_str()` | String representation |
| `Formattable` | `.format()` | Formatted string with spec |
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

## 18.10 Standard library

| Module | Description |
|--------|-------------|
| `std.math` | Mathematical functions |
| `std.io` | I/O traits |
| `std.fs` | Filesystem |
| `std.net.http` | HTTP |
| `std.time` | Date/time |
| `std.json` | JSON |
| `std.crypto` | Cryptography |

## 18.11 Test modules

Tests in `_test/` with `.test.ori` extension can access private items:

```
src/
  math.ori
  _test/
    math.test.ori
```

## 18.12 Package structure

### 18.12.1 Library package

A library package exports its public API via `lib.ori`:

```
my_lib/
├── ori.toml
├── src/
│   ├── lib.ori      # Library entry point
│   └── internal.ori # Internal implementation
```

### 18.12.2 Binary package

A binary package has `main.ori` with an `@main` function:

```
my_app/
├── ori.toml
├── src/
│   ├── main.ori    # Binary entry point
│   └── utils.ori
```

### 18.12.3 Library + binary

A package can contain both. The binary imports from the library using the package name and can only access public items:

```ori
// lib.ori
pub @exported () -> int = 42;
@internal () -> int = 1;  // Private

// main.ori
use "my_pkg" { exported };      // OK: public
use "my_pkg" { ::internal };    // ERROR: private access not allowed
```

This enforces clean API boundaries.

## 18.13 Package manifest

```toml
[project]
name = "my_project"
version = "0.1.0"

[dependencies]
some_lib = "1.0.0"
```

Entry point: `@main` function for binaries, `lib.ori` for libraries.
