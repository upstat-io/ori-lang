---
title: "Modules"
description: "Ori Language Specification — Modules"
order: 12
---

# Modules

Every source file defines one module.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § SOURCE STRUCTURE (import, extension_def, extension_import)

## Module Names

| File Path | Module Name |
|-----------|-------------|
| `src/main.ori` | `main` |
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

## Visibility

Items are private by default. `pub` exports:

```ori
pub @add (a: int, b: int) -> int = a + b
pub type User = { id: int, name: str }
pub $timeout = 30s
```

## Re-exports

```ori
pub use "./client" { get, post }
```

## Extensions

### Definition

```ori
extend Iterator {
    @count (self) -> int = ...
}

extend Iterator where Self.Item: Add {
    @sum (self) -> Self.Item = ...
}
```

### Import

```ori
extension std.iter.extensions { Iterator.count, Iterator.last }
extension "./my_ext" { Iterator.sum }
```

Method-level granularity required; no wildcards.

## Resolution

1. Local bindings (inner first)
2. Function parameters
3. Module-level items
4. Imports
5. Prelude

Circular dependencies prohibited.

## Prelude

Available without import:

**Types**: `int`, `float`, `bool`, `str`, `char`, `byte`, `void`, `Never`, `Duration`, `Size`, `Option<T>`, `Result<T, E>`, `Ordering`, `Error`, `TraceEntry`, `Range<T>`, `Set<T>`, `Channel<T>`, `[T]`, `{K: V}`

**Functions**: `print`, `len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`, `int`, `float`, `str`, `byte`, `compare`, `min`, `max`, `panic`, all assertions

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

## Package Manifest

```toml
[project]
name = "my_project"
version = "0.1.0"

[dependencies]
some_lib = "1.0.0"
```

Entry point: `@main` function.
