# Imports

This document covers Sigil's import system: the `use` keyword, specific imports, qualified imports, aliases, and import organization.

---

## Overview

Imports bring items from other modules into scope. All imports use the `use` keyword.

```sigil
use math { add, subtract }       // import specific items
use math { add as plus }         // import with alias
use http.client                  // import module for qualified access
use http.client as http          // import module with alias
```

### Design Rationale

| Feature | Benefit |
|---------|---------|
| Specific imports `{ a, b }` | Dependencies explicit, clear what's used |
| Qualified imports | Avoid name collisions |
| Aliases `as` | Flexibility without ambiguity |
| Top-of-file only | Dependencies visible at a glance |

---

## Basic Import Syntax

### Importing Specific Items

Import named items from a module:

```sigil
use std.math { sqrt, abs, pow }
```

Items are now available by their names:

```sigil
@distance (x: float, y: float) -> float = sqrt(pow(x, 2) + pow(y, 2))
```

### What Can Be Imported?

| Item Type | Example |
|-----------|---------|
| Functions | `use math { add }` |
| Types | `use types { User, Order }` |
| Variants | `use types { Some, None }` |
| Config | `use config { $timeout }` |

### Multiple Items

Import multiple items in one statement:

```sigil
use std.json { parse, stringify, JsonValue }
use std.http { get, post, Request, Response }
```

### Single Item

For a single item, braces are still required:

```sigil
use std.math { sqrt }  // correct
// use std.math sqrt   // NOT valid
```

---

## Qualified Imports

### Module-Level Import

Import a module for qualified access:

```sigil
use http.client

@fetch (url: str) -> Result<Response, Error> = http.client.get(url)
```

### When to Use Qualified Imports

| Use Case | Reason |
|----------|--------|
| Name collision | Two modules export `get` |
| Clarity | Make origin obvious: `json.parse` vs `xml.parse` |
| Large modules | Don't want many names in scope |
| Exploration | Using module before knowing all items |

### Example: Avoiding Collision

```sigil
use http.client
use database

@handle_request () -> Result<Data, Error> = try(
    response = http.client.get(url),   // http get
    data = database.get(id),            // db get
    Ok(data)
)
```

---

## Import Aliases

### Item Aliases

Rename an item on import:

```sigil
use math { add as plus, subtract as minus }

@calculate () -> int = plus(5, minus(10, 3))
```

### Module Aliases

Rename a module on import:

```sigil
use http.client as http

@fetch (url: str) -> Result<Response, Error> = http.get(url)
```

### When to Use Aliases

| Use Case | Example |
|----------|---------|
| Shorten long paths | `use database.sql.postgres.connection as pg` |
| Clarify meaning | `use legacy.api { fetch as legacy_fetch }` |
| Resolve collision | `use lib_a { Item as ItemA }` |

### Multiple Aliases

```sigil
use http.client {
    get as http_get,
    post as http_post,
    Response as HttpResponse
}
```

---

## Import Location

### Top of File Only

All imports **must** be at the top of the file, before any definitions:

```sigil
// Imports first
use std.math { sqrt, abs }
use std.string { split }
use my_module { helper }

// Then definitions
pub type Data = { value: int }

pub @process (x: int) -> int = abs(x)

@test_process tests @process () -> void = ...
```

### Why Top-Only?

1. **Visibility** - Dependencies visible at a glance
2. **Predictability** - No hidden imports buried in code
3. **AI efficiency** - Scan top of file to understand dependencies
4. **Standard practice** - Matches almost all languages

### Invalid: Imports Mid-File

```sigil
@first_func () -> int = 1

use std.math { sqrt }  // ERROR: imports must be at top

@second_func () -> float = sqrt(2.0)
```

```
error: imports must appear at the top of the file
  --> src/example.si:3:1
  |
3 | use std.math { sqrt }
  | ^^^
  |
  = help: move this import before any function definitions
```

---

## Import Grouping

### Recommended Order

Group imports by source:

```sigil
// 1. Standard library
use std.math { sqrt, abs }
use std.string { split, join }
use std.io { read_file, write_file }

// 2. External dependencies
use serde { serialize, deserialize }
use http { Client, Response }

// 3. Internal modules
use my_app.config { $timeout, $max_retries }
use my_app.utils { format, validate }
use my_app.types { User, Order }
```

### Single Blank Line Between Groups

```sigil
use std.math { sqrt }
use std.string { split }

use external_lib { process }

use my_module { helper }
```

---

## Import Semantics

### Name Binding

Imports create bindings in the current module's scope:

```sigil
use math { add }

// "add" is now bound in this module
@double_add (a: int, b: int) -> int = add(a, b) + add(a, b)
```

### No Re-Export by Default

Imported items are **not** automatically re-exported:

```sigil
// File: src/utils.si
use std.math { sqrt }  // sqrt is available here

pub @distance (x: float, y: float) -> float = sqrt(x*x + y*y)
// sqrt is NOT exported from utils, only distance is
```

To re-export, use `pub use` (see [Re-exports](04-re-exports.md)).

### Shadowing

Local definitions shadow imports:

```sigil
use math { add }

// This shadows the imported add
@add (a: int, b: int) -> int = a + b + 1  // custom implementation

@main () -> void = run(
    result = add(2, 3),  // uses local add, result = 6
    print(str(result))
)
```

**Warning:** Shadowing imports is usually a mistake. Rename the local function or use an alias.

---

## Visibility and Imports

### Public vs Private

Only `pub` items can be imported from other modules:

```sigil
// File: src/math.si

pub @add (a: int, b: int) -> int = a + b        // can be imported
@internal_helper () -> int = 42                  // cannot be imported
pub type Point = { x: int, y: int }             // can be imported
type InternalData = { value: int }               // cannot be imported
```

### Importing Private Items

Attempting to import private items is an error:

```sigil
use math { internal_helper }  // ERROR
```

```
error: `internal_helper` is private
  --> src/main.si:1:12
  |
1 | use math { internal_helper }
  |            ^^^^^^^^^^^^^^^^
  |
  = note: `internal_helper` is defined in `math` but not exported
  = help: mark it as `pub` to make it accessible
```

---

## Selective Imports

### Import What You Need

Import only the items you actually use:

```sigil
// Good: import only what's used
use std.math { sqrt }

@distance (x: float, y: float) -> float = sqrt(x*x + y*y)
```

```sigil
// Avoid: importing everything
use std.math { sin, cos, tan, sqrt, abs, pow, log, exp, ... }

@distance (x: float, y: float) -> float = sqrt(x*x + y*y)
// Only sqrt is used
```

### Wildcard Imports

Sigil does **not** support wildcard imports:

```sigil
// NOT supported:
// use std.math.*
// use std.math { * }
```

**Rationale:** Wildcards make dependencies unclear. AI needs explicit imports to understand what's available.

---

## Complex Import Patterns

### Multi-Level Modules

```sigil
use database.sql.postgres { connect, query }
use database.sql.mysql { connect as mysql_connect }
use database.nosql.mongodb { Collection }
```

### Mixing Styles

Combine specific and qualified imports:

```sigil
use std.math { sqrt, abs }      // specific: commonly used
use std.string as string         // qualified: occasional use
use http.client                  // qualified: multiple methods

@process (x: float) -> str = run(
    result = sqrt(abs(x)),
    formatted = string.format("{:.2}", result),
    http.client.post("/result", formatted),
    formatted
)
```

### Importing Types and Functions Together

```sigil
use http {
    Client,           // type
    Response,         // type
    Error,            // type
    get,              // function
    post              // function
}

@fetch (client: Client, url: str) -> Result<Response, Error> =
    get(url)
```

---

## Import Errors

### Unknown Module

```sigil
use unknown_module { func }
```

```
error: module not found: `unknown_module`
  --> src/main.si:1:5
  |
1 | use unknown_module { func }
  |     ^^^^^^^^^^^^^^
  |
  = help: check the module path and ensure the file exists
```

### Unknown Item

```sigil
use std.math { squareroot }
```

```
error: `squareroot` not found in `std.math`
  --> src/main.si:1:16
  |
1 | use std.math { squareroot }
  |                ^^^^^^^^^^
  |
  = help: did you mean `sqrt`?
```

### Duplicate Import

```sigil
use math { add }
use other { add }  // ERROR: add already imported
```

```
error: `add` is already imported
  --> src/main.si:2:13
  |
1 | use math { add }
  |            --- first imported here
2 | use other { add }
  |             ^^^
  |
  = help: use an alias: `use other { add as other_add }`
```

---

## Examples

### HTTP Client Module

```sigil
// File: src/api/client.si

use std.string { join }
use std.json { parse, stringify }
use http { Client, Response, Error }
use http.client { get, post, put, delete }

use api.config { $base_url, $timeout }
use api.auth { get_token }
use api.types { User, ApiError }

pub @fetch_user (id: int) -> Result<User, ApiError> = try(
    token = get_token(),
    url = join([$base_url, "/users/", str(id)]),
    response = get(url, .headers: {"Authorization": token}),
    user = parse(response.body),
    Ok(user)
)
```

### Math Utilities Module

```sigil
// File: src/math_utils.si

use std.math { sqrt, pow, abs, sin, cos }
use std.math as math

pub type Point = { x: float, y: float }
pub type Vector = { dx: float, dy: float }

pub @distance (p1: Point, p2: Point) -> float = run(
    dx = p2.x - p1.x,
    dy = p2.y - p1.y,
    sqrt(pow(dx, 2) + pow(dy, 2))
)

pub @angle (v: Vector) -> float = math.atan2(v.dy, v.dx)

pub @magnitude (v: Vector) -> float = sqrt(pow(v.dx, 2) + pow(v.dy, 2))
```

### Test File Imports

```sigil
// File: src/_test/math_utils.test.si

use math_utils { Point, Vector, distance, angle, magnitude }
use std.math { abs }

@test_distance tests @distance () -> void = run(
    p1 = Point { x: 0.0, y: 0.0 },
    p2 = Point { x: 3.0, y: 4.0 },
    assert_eq(distance(p1, p2), 5.0)
)

@test_magnitude tests @magnitude () -> void = run(
    v = Vector { dx: 3.0, dy: 4.0 },
    assert_eq(magnitude(v), 5.0)
)
```

---

## Best Practices

### Keep Imports Organized

```sigil
// Good: grouped and ordered
use std.math { sqrt, abs }
use std.string { split, join }

use external.http { get, post }

use my_app.types { User }
use my_app.utils { validate }
```

### Use Aliases for Clarity

```sigil
// When module name adds context
use crypto.hash as hash
use crypto.encrypt as encrypt

hash.sha256(data)
encrypt.aes(data, key)
```

### Prefer Specific Imports

```sigil
// Good: explicit about what's used
use std.math { sqrt }

// Less clear: what functions are actually used?
use std.math as math
```

### Avoid Import Pollution

```sigil
// Bad: importing many unused items
use std.math { sin, cos, tan, sqrt, abs, pow, log, exp, floor, ceil }

// Good: import only what you need
use std.math { sqrt, abs }
```

---

## See Also

- [Module System](01-module-system.md)
- [Prelude](03-prelude.md)
- [Re-exports](04-re-exports.md)
- [Basic Syntax](../02-syntax/01-basic-syntax.md)
