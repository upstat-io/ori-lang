# Modules

This section defines the module system.

## Module Definition

Every source file is a module. The file path determines the module name.

| File Path | Module Name |
|-----------|-------------|
| `src/main.si` | `main` |
| `src/math.si` | `math` |
| `src/http/client.si` | `http.client` |
| `src/http/mod.si` | `http` |

A file named `mod.si` represents the directory's index module.

## Imports

### Syntax

```
import        = "use" module_path [ import_list | "as" identifier ] .
module_path   = identifier { "." identifier } .
import_list   = "{" identifier { "," identifier } [ "," ] "}" .
```

### Selective Import

Import specific items from a module:

```sigil
use std.math { sqrt, abs, pow }
use http.client { get, post }
```

### Module Alias

Import a module under an alias:

```sigil
use http.client as http
use std.collections.hash_map as HashMap
```

### Qualified Access

Items may be accessed using qualified paths:

```sigil
use std.math

result = std.math.sqrt(x)
```

### Import Location

All imports must appear at the beginning of the source file, before any other declarations.

## Visibility

### Private by Default

All items (functions, types, config) are private by default and visible only within their module.

### Public Items

The `pub` modifier exports an item:

```sigil
pub @add (a: int, b: int) -> int = a + b
pub type User = { id: int, name: str }
pub $timeout = 30s
```

### Visibility Rules

| Declaration | Visible To |
|-------------|------------|
| No modifier | Same module only |
| `pub` | Any importing module |

## Re-exports

A module may re-export items from other modules:

```sigil
// In http/mod.si
pub use http.client { get, post }
pub use http.server { serve }
```

Consumers can then import from the parent module:

```sigil
use http { get, post, serve }
```

## Module Path Resolution

### Resolution Order

The compiler resolves module paths in this order:

1. Standard library (`std.*`)
2. Project source (from source root)
3. Dependencies (from project manifest)

### Source Root

The source root is the directory from which relative module paths are resolved. Default is `src/`.

### Circular Dependencies

Circular module dependencies are prohibited. If module A imports from B, and B imports from A, it is an error.

```
error: circular dependency detected
  = cycle: a -> b -> a
```

## Test Modules

### Convention

Test files reside in `_test/` subdirectories:

```
src/
  math.si
  _test/
    math.test.si
```

### Test Module Access

Files in `_test/` directories with the `.test.si` extension have special access to private items in their parent module.

| Access | Regular Import | Test Import (`_test/*.test.si`) |
|--------|----------------|--------------------------------|
| `pub` items | Yes | Yes |
| Private items | No | Yes |

This allows testing private implementation details without exposing them.

## Module Items

A module may contain:

- Function declarations (`@name`)
- Type definitions (`type Name = ...`)
- Trait definitions (`trait Name { ... }`)
- Implementation blocks (`impl ... { ... }`)
- Config variables (`$name = ...`)
- Test declarations (`@name tests @target`)
- Imports (`use ...`)

## Prelude

Certain items are available in all modules without explicit import:

- Primitive types: `int`, `float`, `bool`, `str`, `byte`, `void`
- Built-in types: `Option`, `Result`, `Duration`, `Size`
- Variants: `Some`, `None`, `Ok`, `Err`
- Functions: `len`, `print`, `assert`, `assert_eq`, `panic`
- Type conversions: `int()`, `float()`, `str()`

These are defined in the implicit prelude module.

## Package Structure

### Project Manifest

A project is configured via `sigil.toml`:

```toml
[project]
name = "my_project"
version = "0.1.0"
src = "src"

[dependencies]
some_lib = "1.0.0"
```

### Entry Point

The entry point is the `@main` function in the root module or a module specified in the manifest.

```sigil
@main () -> void = run(
    print("Hello, World!"),
)
```

## Name Resolution

### Scoping

Names are resolved in this order:

1. Local bindings (innermost scope first)
2. Function parameters
3. Module-level items
4. Imported items
5. Prelude items

### Shadowing

A local binding may shadow an outer binding or import:

```sigil
use std.math { sqrt }

@example () -> float = run(
    let sqrt = 42,  // shadows imported sqrt
    float(sqrt),    // refers to local sqrt
)
```

### Ambiguity

If an unqualified name could refer to multiple items, it is an error:

```
error: ambiguous name `sqrt`
  = imported from: std.math
  = imported from: custom.math
  = help: use qualified path or rename import
```
