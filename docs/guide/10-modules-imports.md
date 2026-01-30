---
title: "Modules and Imports"
description: "File modules, imports, visibility, and re-exports."
order: 10
part: "Program Structure"
---

# Modules and Imports

As your Ori programs grow beyond a single file, you'll need to organize code into modules. This guide shows you how to structure projects that scale.

## Every File Is a Module

In Ori, there's no special syntax to declare a module. The file itself is the module, and its path determines its name:

| File Path | Module Name | Import Path |
|-----------|-------------|-------------|
| `src/main.ori` | `main` | N/A (entry point) |
| `src/math.ori` | `math` | `"./math"` |
| `src/utils/strings.ori` | `utils.strings` | `"./utils/strings"` |
| `src/http/client.ori` | `http.client` | `"./http/client"` |

## Import Syntax

### Relative Imports

Import from your project files with quoted paths:

```ori
// Same directory
use "./math" { add, subtract }

// Parent directory
use "../shared" { common_helper }

// Subdirectory
use "./utils/strings" { capitalize }

// Deeply nested
use "./services/api/v2/client" { fetch }
```

Relative imports:
- Start with `./` or `../`
- Use forward slashes (even on Windows)
- Omit the `.ori` extension
- Are always quoted

### Standard Library Imports

Import built-in modules with unquoted dot notation:

```ori
use std.math { sqrt, abs, pow, floor, ceil }
use std.time { Duration, now, today }
use std.collections { HashMap, HashSet }
use std.io { read_file, write_file }
```

Standard library imports:
- Use dot notation without quotes
- Don't start with `./`
- Are namespaced under `std`

### Importing Multiple Items

Import several items at once:

```ori
use "./math" { add, subtract, multiply, divide }
use std.math { sqrt, abs, pow, floor, ceil, round }
```

Or spread across multiple lines for readability:

```ori
use "./math" {
    add,
    subtract,
    multiply,
    divide,
}
```

### Import Aliases

Rename imports to avoid conflicts or improve clarity:

```ori
// Rename a single import
use "./math" { add as sum }
use "./strings" { split as split_string }

// Now use the alias
let result = sum(a: 1, b: 2)
let parts = split_string(text: "a,b,c", delimiter: ",")
```

### Module Aliases

Give a whole module a shorter name:

```ori
use std.collections.concurrent as cc
use std.net.http.client as http

// Use with dot notation
let map = cc.ConcurrentHashMap.new()
let response = http.get(url: "/api/data")
```

## Visibility

### Private by Default

Everything in Ori is private unless you explicitly make it public:

```ori
// PRIVATE — only usable within this file
@internal_helper (x: int) -> int = x * 2

type InternalState = { count: int }

let $INTERNAL_LIMIT = 100

// PUBLIC — can be imported by other modules
pub @process (x: int) -> int = internal_helper(x: x) + 1

pub type Config = { timeout: Duration }

pub let $MAX_RETRIES = 3
```

### Visibility Modifiers

| Declaration | Private | Public |
|-------------|---------|--------|
| Function | `@name ...` | `pub @name ...` |
| Type | `type Name = ...` | `pub type Name = ...` |
| Constant | `let $NAME = ...` | `pub let $NAME = ...` |
| Trait | `trait Name { ... }` | `pub trait Name { ... }` |

### Why Private by Default?

Private by default encourages encapsulation:

```ori
// In database.ori

// Internal implementation detail — could change
@build_connection_string (host: str, port: int, db: str) -> str =
    `postgres://{host}:{port}/{db}`

// Public interface — stable contract
pub @connect (config: DbConfig) -> Result<Connection, Error> uses Database = run(
    let conn_str = build_connection_string(
        host: config.host,
        port: config.port,
        db: config.database,
    ),
    Database.connect(connection_string: conn_str),
)
```

Other modules use `connect` without depending on `build_connection_string`. You can refactor the internal function freely.

### Accessing Private Items

Sometimes you need to access private items, especially for testing. Use the `::` prefix:

```ori
// In test file
use "./database" { ::build_connection_string }

@test_connection_string tests _ () -> void = run(
    let result = build_connection_string(host: "localhost", port: 5432, db: "test"),
    assert_eq(actual: result, expected: "postgres://localhost:5432/test"),
)
```

Use `::` sparingly — it's a signal that you're breaking encapsulation.

## Re-exports

### Building Clean APIs

Imagine this structure:

```
mylib/
├── internal/
│   ├── parser.ori      # pub type Parser, pub @parse
│   ├── lexer.ori       # pub type Lexer, pub @tokenize
│   └── optimizer.ori   # pub @optimize
└── lib.ori
```

Without re-exports, users must know your internal structure:

```ori
// Ugly — exposes internal organization
use "mylib/internal/parser" { Parser, parse }
use "mylib/internal/lexer" { Lexer, tokenize }
use "mylib/internal/optimizer" { optimize }
```

### Using `pub use`

Use `pub use` to expose items through a public interface:

```ori
// In lib.ori
pub use "./internal/parser" { Parser, parse }
pub use "./internal/lexer" { Lexer, tokenize }
pub use "./internal/optimizer" { optimize }
```

Now users have a clean API:

```ori
// Clean — single import point
use "mylib" { Parser, Lexer, parse, tokenize, optimize }
```

### Re-export Patterns

**Selective re-export:**

```ori
// parser.ori has Parser, ParserConfig, ParserState, parse, parse_partial
// Only expose the stable interface
pub use "./internal/parser" { Parser, parse }
```

**Re-export with alias:**

```ori
pub use "./internal/parser" { InternalParser as Parser }
```

**Re-export types only:**

```ori
pub use "./internal/parser" { Parser, ParserConfig }
// parse function stays internal
```

## Organizing Imports

### Import Order Convention

```ori
// Standard library first
use std.math { sqrt, abs }
use std.time { Duration }

// External packages second
use http_client { get, post }

// Local imports last, grouped by proximity
use "../shared" { Error, Result }
use "./models" { User, Order }
use "./utils" { format_date }
```

### One Import per Line (Optional)

For complex modules:

```ori
use std.collections { HashMap }
use std.collections { HashSet }
use std.collections { BTreeMap }
```

## Project Structure

### Small Projects

```
project/
├── main.ori          # Entry point and most code
├── utils.ori         # Helpers if needed
└── config.ori        # Configuration constants
```

### Medium Projects

```
project/
├── ori.toml              # Project manifest
├── src/
│   ├── main.ori          # Entry point
│   ├── lib.ori           # Public API (re-exports)
│   ├── config.ori        # Configuration
│   ├── models/           # Data types
│   │   ├── user.ori
│   │   ├── order.ori
│   │   └── product.ori
│   ├── services/         # Business logic
│   │   ├── auth.ori
│   │   ├── payment.ori
│   │   └── shipping.ori
│   └── _test/            # Test files
│       ├── auth.test.ori
│       └── payment.test.ori
└── library/              # Dependencies
```

### Large Projects

```
project/
├── ori.toml
├── src/
│   ├── main.ori
│   ├── lib.ori           # Main public API
│   ├── core/             # Core business logic
│   │   ├── lib.ori       # Core public API
│   │   ├── domain/
│   │   └── services/
│   ├── api/              # HTTP API layer
│   │   ├── lib.ori       # API public API
│   │   ├── routes/
│   │   ├── middleware/
│   │   └── handlers/
│   ├── infrastructure/   # External integrations
│   │   ├── database/
│   │   ├── cache/
│   │   └── messaging/
│   └── shared/           # Shared utilities
│       ├── errors.ori
│       └── types.ori
└── _test/                # Integration tests
    └── api.test.ori
```

## Complete Example

```
calculator/
├── main.ori
├── math.ori
└── format.ori
```

**math.ori:**

```ori
// Basic arithmetic functions
pub @add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    assert_eq(actual: add(a: 2, b: 3), expected: 5),
    assert_eq(actual: add(a: -1, b: 1), expected: 0),
)

pub @subtract (a: int, b: int) -> int = a - b

@test_subtract tests @subtract () -> void =
    assert_eq(actual: subtract(a: 5, b: 3), expected: 2)

pub @multiply (a: int, b: int) -> int = a * b

@test_multiply tests @multiply () -> void =
    assert_eq(actual: multiply(a: 4, b: 5), expected: 20)

pub @divide (a: int, b: int) -> int = run(
    pre_check: b != 0 | "division by zero",
    a div b,
)

@test_divide tests @divide () -> void = run(
    assert_eq(actual: divide(a: 10, b: 2), expected: 5),
    assert_panics(f: () -> divide(a: 1, b: 0)),
)
```

**format.ori:**

```ori
pub @format_result (operation: str, a: int, b: int, result: int) -> str =
    `{a} {operation} {b} = {result}`

@test_format tests @format_result () -> void =
    assert_eq(
        actual: format_result(operation: "+", a: 2, b: 3, result: 5),
        expected: "2 + 3 = 5",
    )
```

**main.ori:**

```ori
use "./math" { add, subtract, multiply, divide }
use "./format" { format_result }

@main () -> void = run(
    let a = 10,
    let b = 3,

    print(msg: format_result(operation: "+", a: a, b: b, result: add(a: a, b: b))),
    print(msg: format_result(operation: "-", a: a, b: b, result: subtract(a: a, b: b))),
    print(msg: format_result(operation: "*", a: a, b: b, result: multiply(a: a, b: b))),
    print(msg: format_result(operation: "/", a: a, b: b, result: divide(a: a, b: b))),
)
```

## Quick Reference

### Import Syntax

```ori
// Local imports (quoted, relative path)
use "./file" { item1, item2 }
use "./subdir/file" { item }
use "../parent/file" { item }

// Standard library (unquoted, dot notation)
use std.module { item }
use std.nested.module { item }

// Aliases
use "./file" { original as alias }
use std.module as alias

// Private access
use "./file" { ::private_item }

// Re-exports
pub use "./file" { item }
```

### Visibility

```ori
// Public
pub @function_name ...
pub type TypeName = ...
pub let $CONSTANT = ...
pub trait TraitName { ... }

// Private (no keyword)
@function_name ...
type TypeName = ...
let $CONSTANT = ...
```

## What's Next

Now that you understand modules:

- **[Constants](/guide/11-constants)** — Module-level constants and const functions
- **[Testing](/guide/12-testing)** — Comprehensive testing strategies
