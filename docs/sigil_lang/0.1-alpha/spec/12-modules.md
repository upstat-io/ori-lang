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

```ebnf
import        = "use" import_path [ import_list | "as" identifier ] .
import_path   = relative_path | module_path .
relative_path = string_literal .
module_path   = identifier { "." identifier } .
import_list   = "{" import_item { "," import_item } [ "," ] "}" .
import_item   = import_name [ "as" identifier ] .
import_name   = [ "::" ] identifier | "$" identifier .
```

## Extension Imports

Extension imports bring trait extension methods into scope. Unlike regular imports, extension imports use the `extension` keyword and specify methods at the trait-method level.

### Syntax

```ebnf
extension_import = "extension" import_path "{" extension_item { "," extension_item } [ "," ] "}" .
extension_item   = identifier "." identifier .
```

The first identifier is the trait name, the second is the method name.

### From Local Files

```sigil
extension './my_extensions' { Iterator.count, Iterator.sum }
extension '../utils/iter_extensions' { Iterator.take, Iterator.skip }
```

### From Standard Library

```sigil
extension std.iter.extensions { Iterator.count, Iterator.last }
extension std.fmt.extensions { Display.print, Display.println }
```

### Method-Level Granularity

Extension imports require specifying individual methods, not entire traits:

```sigil
// Correct: specify each method
extension std.iter.extensions { Iterator.count, Iterator.last }

// Invalid: wildcard imports not supported
extension std.iter.extensions { Iterator.* }  // ERROR
```

This ensures:
1. Explicit visibility of which methods are added to types
2. No implicit namespace pollution
3. Self-documenting imports

### Constraints

- It is an error if the specified trait does not exist in the extension module.
- It is an error if the specified method does not exist in the trait's extension.
- It is an error to import the same extension method twice in the same module.
- Extension methods are only available in the importing module's scope.

### Relative Imports (Local Files)

Local project files are imported using relative paths in quotes:

```sigil
use './math' { add, subtract }
use '../utils/helpers' { format }
use './http/client' { get, post }
```

Relative paths:
- Start with `./` (same directory) or `../` (parent directory)
- Are enclosed in single quotes
- Resolve relative to the current file's location
- Omit the `.si` extension

This makes file locations explicit and unambiguous for both humans and AI.

### Module Imports (Standard Library & Packages)

External modules use dot-separated paths without quotes:

```sigil
use std.math { sqrt, abs, pow }
use std.time { Time, Duration }
use std.net.http { get, post }
```

### Private Imports

Private items (not marked `pub`) can be imported using the `::` prefix:

```sigil
use './math' { add, ::internal_helper }
use './utils' { ::validate, ::parse }
```

The `::` prefix explicitly requests access to a non-public item. This works from any file.

```sigil
// In math.si
@add (a: int, b: int) -> int = a + b           // private
pub @subtract (a: int, b: int) -> int = a - b  // public

// In another file
use './math' { subtract }       // OK - public
use './math' { add }            // ERROR - add is private
use './math' { ::add }          // OK - explicit private access
```

### Import with Aliases

Rename imports to avoid conflicts or improve clarity:

```sigil
use './math' { add as plus, subtract as minus }
use std.collections { HashMap as Map }
use './config' { $timeout, $max_retries as $retries }
use './internal' { ::helper as h }  // private with alias
```

### Module Alias

Import an entire module under an alias:

```sigil
use std.net.http as http
use './utilities/string_helpers' as strings
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

| Declaration | Import Syntax | Notes |
|-------------|---------------|-------|
| No modifier (private) | `use '...' { ::name }` | Requires `::` prefix |
| `pub` | `use '...' { name }` | Normal import |

Private items can be imported from any file using explicit `::` syntax. This enables testing private internals without magic visibility rules.

## Re-exports

A module may re-export items from other modules:

```sigil
// In http/mod.si
pub use './client' { get, post }
pub use './server' { serve }
```

Consumers can then import from the parent module:

```sigil
use './http' { get, post, serve }
```

## Module Path Resolution

### Resolution Order

The compiler resolves module paths in this order:

1. Standard library (`std.*`)
2. Project source (from source root)
3. Dependencies (from project manifest)

### Source Root

The source root is the directory from which relative module paths are resolved. Default is `src/`.

### Path Types

| Path Type | Syntax | Resolves To |
|-----------|--------|-------------|
| Relative | `'./foo'` | Same directory |
| Relative | `'../foo'` | Parent directory |
| Module | `std.foo` | Standard library |
| Module | `pkg.foo` | External package |

### Relative Path Resolution

Relative paths resolve from the importing file's directory:

```
src/
  math.si
  utils/
    helpers.si
  _test/
    math.test.si
```

```sigil
// In src/_test/math.test.si
use '../math' { add }           // resolves to src/math.si
use '../utils/helpers' { fmt }  // resolves to src/utils/helpers.si
```

### Module Path Resolution

Module paths resolve in this order:

1. Standard library (`std.*`)
2. Project dependencies (from manifest)

```sigil
use std.time { Duration }       // standard library
use some_package { Widget }     // external dependency
```

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

## Extension Definitions

Extensions add methods to existing traits without modifying the original trait definition.

### Syntax

```ebnf
extension_def = "extend" identifier [ where_clause ] "{" { method_def } "}" .
```

### Basic Extension

```sigil
extend Iterator {
    @count (self) -> int = run(
        let n = 0,
        while self.next().is_some() do n = n + 1,
        n,
    )

    @last (self) -> Option<Self.Item> = run(
        let result = None,
        for item in self do result = Some(item),
        result,
    )
}
```

### Constrained Extension

Extensions may include constraints via `where` clauses:

```sigil
extend Iterator where Self.Item: Add {
    @sum (self) -> Self.Item =
        fold(self, Self.Item.default(), (acc, x) -> acc + x)
}

extend Iterator where Self.Item = int {
    @average (self) -> float = run(
        let sum = 0,
        let count = 0,
        for item in self do run(
            sum = sum + item,
            count = count + 1,
        ),
        float(sum) / float(count),
    )
}
```

### Extension with Capabilities

Extension methods may use capabilities:

```sigil
extend Display {
    @print (self) -> void uses Console =
        Console.write(self.display())

    @println (self) -> void uses Console =
        Console.writeln(self.display())
}
```

### Constraints

- It is an error to define an extension for a trait that does not exist.
- It is an error to define an extension method with the same name as an existing method on the trait.
- Extension methods cannot be overridden by implementors; they apply uniformly to all implementations.
- Extensions must be explicitly imported to use; they have no effect without an `extension` import.

## Module Items

A module may contain:

- Function declarations (`@name`)
- Type definitions (`type Name = ...`)
- Trait definitions (`trait Name { ... }`)
- Implementation blocks (`impl ... { ... }`)
- Extension definitions (`extend Trait { ... }`)
- Config variables (`$name = ...`)
- Test declarations (`@name tests @target`)
- Imports (`use ...`)
- Extension imports (`extension ...`)

## Prelude

Certain items are available in all modules without explicit import:

**Primitive types:**
- `int`, `float`, `bool`, `str`, `char`, `byte`, `void`, `Never`
- `Duration`, `Size`

**Built-in generic types:**
- `Option<T>` with variants `Some(T)`, `None`
- `Result<T, E>` with variants `Ok(T)`, `Err(E)`
- `Ordering` with variants `Less`, `Equal`, `Greater`
- `Error` — standard error type
- `Range<T>` — range type
- `Set<T>` — set collection
- `Channel<T>` — async communication

**Collection types:**
- `[T]` — list
- `{K: V}` — map

**Functions:**
- `print` — output to stdout
- `len` — collection length
- `str`, `int`, `float` — type conversions
- `compare` — value comparison
- `panic` — terminate with error
- `assert`, `assert_eq`, `assert_ne` — runtime assertions
- `assert_some`, `assert_none` — option assertions
- `assert_ok`, `assert_err` — result assertions
- `assert_panics`, `assert_panics_with` — panic assertions

**Traits:**
- `Eq`, `Comparable`, `Hashable`, `Printable`, `Clone`, `Default`

These are defined in the implicit prelude module. See [Standard Library § Prelude](../modules/prelude.md) for details.

## Standard Library

The standard library is a collection of modules that ship with every Sigil implementation. Standard library modules are imported using the `std` prefix:

```sigil
use std.time { Date, Time, DateTime }
use std.fs { read_file, write_file }
use std.net.http { get, post }
use std.json { parse, stringify }
```

### Module Path Convention

Standard library modules use dot-separated paths under the `std` namespace:

| Module | Description |
|--------|-------------|
| `std.io` | I/O traits and operations |
| `std.fs` | Filesystem operations |
| `std.net` | Networking (TCP, UDP) |
| `std.net.http` | HTTP client and server |
| `std.time` | Date, time, and duration |
| `std.json` | JSON encoding/decoding |
| `std.math` | Mathematical functions |
| `std.crypto` | Cryptographic functions |
| `std.encoding` | Data encoding (base64, hex) |
| `std.env` | Environment variables |
| `std.process` | Process management |
| `std.log` | Logging utilities |
| `std.testing` | Test utilities |
| `std.async` | Async utilities |
| `std.collections` | Additional collections |
| `std.compress` | Compression |

See [Standard Library Documentation](../modules/README.md) for complete reference.

### Capability Requirements

Some standard library modules require capabilities:

| Module | Capability |
|--------|------------|
| `std.io` | `IO` |
| `std.fs` | `FileSystem` |
| `std.net` | `Network` |
| `std.env` | `Env` |
| `std.process` | `Process` |

Modules without listed capabilities (e.g., `std.math`, `std.json`, `std.time`) are pure and require no capabilities.

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
