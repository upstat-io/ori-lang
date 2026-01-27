# Imports

This document covers Ori's import system: the `use` keyword, specific imports, qualified imports, aliases, and import organization.

---

## Overview

Imports bring items from other modules into scope. All imports use the `use` keyword.

```ori
// import specific items
use math { add, subtract }
// import with alias
use math { add as plus }
// import module for qualified access
use http.client
// import module with alias
use http.client as http
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

```ori
use std.math { sqrt, abs, pow }
```

Items are now available by their names:

```ori
@distance (delta_x: float, delta_y: float) -> float = sqrt(
    .value: pow(
        .base: delta_x,
        .exponent: 2,
    ) + pow(
        .base: delta_y,
        .exponent: 2,
    ),
)
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

```ori
use std.json { parse, stringify, JsonValue }
use std.http { get, post, Request, Response }
```

### Single Item

For a single item, braces are still required:

```ori
// correct
use std.math { sqrt }
// NOT valid
// use std.math sqrt
```

---

## Qualified Imports

### Module-Level Import

Import a module for qualified access:

```ori
use http.client

@fetch (url: str) -> Result<Response, Error> = http.client.get(
    .url: url,
)
```

### When to Use Qualified Imports

| Use Case | Reason |
|----------|--------|
| Name collision | Two modules export `get` |
| Clarity | Make origin obvious: `json.parse` vs `xml.parse` |
| Large modules | Don't want many names in scope |
| Exploration | Using module before knowing all items |

### Example: Avoiding Collision

```ori
use http.client
use database

@handle_request () -> Result<Data, Error> = try(
    // http get
    let response = http.client.get(
        .url: url,
    )?,
    // db get
    let data = database.get(
        .id: id,
    )?,
    Ok(data),
)
```

---

## Import Aliases

### Item Aliases

Rename an item on import:

```ori
use math { add as plus, subtract as minus }

@calculate () -> int = plus(
    .left: 5,
    .right: minus(
        .left: 10,
        .right: 3,
    ),
)
```

### Module Aliases

Rename a module on import:

```ori
use http.client as http

@fetch (url: str) -> Result<Response, Error> = http.get(
    .url: url,
)
```

### When to Use Aliases

| Use Case | Example |
|----------|---------|
| Shorten long paths | `use database.sql.postgres.connection as pg` |
| Clarify meaning | `use legacy.api { fetch as legacy_fetch }` |
| Resolve collision | `use lib_a { Item as ItemA }` |

### Multiple Aliases

```ori
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

```ori
// Imports first
use std.math { sqrt, abs }
use std.string { split }
use my_module { helper }

// Then definitions
pub type Data = { value: int }

pub @process (value: int) -> int = abs(
    .value: value,
)

@test_process tests @process () -> void = ...
```

### Why Top-Only?

1. **Visibility** - Dependencies visible at a glance
2. **Predictability** - No hidden imports buried in code
3. **AI efficiency** - Scan top of file to understand dependencies
4. **Standard practice** - Matches almost all languages

### Invalid: Imports Mid-File

```ori
@first_func () -> int = 1

// ERROR: imports must be at top
use std.math { sqrt }

@second_func () -> float = sqrt(
    .value: 2.0,
)
```

```
error: imports must appear at the top of the file
  --> src/example.ori:3:1
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

```ori
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

```ori
use std.math { sqrt }
use std.string { split }

use external_lib { process }

use my_module { helper }
```

---

## Import Semantics

### Name Binding

Imports create bindings in the current module's scope:

```ori
use math { add }

// "add" is now bound in this module
@double_add (left: int, right: int) -> int = add(
    .left: left,
    .right: right,
) + add(
    .left: left,
    .right: right,
)
```

### No Re-Export by Default

Imported items are **not** automatically re-exported:

```ori
// File: src/utils.ori
// sqrt is available here
use std.math { sqrt }

pub @distance (delta_x: float, delta_y: float) -> float = sqrt(
    .value: delta_x * delta_x + delta_y * delta_y,
)
// sqrt is NOT exported from utils, only distance is
```

To re-export, use `pub use` (see [Re-exports](04-re-exports.md)).

### Shadowing

Local definitions shadow imports:

```ori
use math { add }

// This shadows the imported add
// custom implementation
@add (left: int, right: int) -> int = left + right + 1

@main () -> void = run(
    // uses local add, result = 6
    let result = add(
        .left: 2,
        .right: 3,
    ),
    print(
        .msg: str(result),
    ),
)
```

**Warning:** Shadowing imports is usually a mistake. Rename the local function or use an alias.

---

## Visibility and Imports

### Public vs Private

Only `pub` items can be imported from other modules:

```ori
// File: src/math.ori

// can be imported
pub @add (left: int, right: int) -> int = left + right
// cannot be imported
@internal_helper () -> int = 42
// can be imported
pub type Point = { x: int, y: int }
// cannot be imported
type InternalData = { value: int }
```

### Importing Private Items

Attempting to import private items is an error:

```ori
// ERROR
use math { internal_helper }
```

```
error: `internal_helper` is private
  --> src/main.ori:1:12
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

```ori
// Good: import only what's used
use std.math { sqrt }

@distance (delta_x: float, delta_y: float) -> float = sqrt(
    .value: delta_x * delta_x + delta_y * delta_y,
)
```

```ori
// Avoid: importing everything
use std.math { sin, cos, tan, sqrt, abs, pow, log, exp, ... }

@distance (delta_x: float, delta_y: float) -> float = sqrt(
    .value: delta_x * delta_x + delta_y * delta_y,
)
// Only sqrt is used
```

### Wildcard Imports

Ori does **not** support wildcard imports:

```ori
// NOT supported:
// use std.math.*
// use std.math { * }
```

**Rationale:** Wildcards make dependencies unclear. AI needs explicit imports to understand what's available.

---

## Extension Imports

Extension imports are separate from regular imports. They use the `extension` keyword (not `use`) and bring trait extension methods into scope.

### Why a Separate Keyword?

| Aspect | `use` | `extension` |
|--------|-------|-------------|
| Purpose | Import types, functions, values | Import trait extension methods |
| Syntax | `use path { item }` | `extension path { Trait.method }` |
| Granularity | Item-level | Method-level |

The separation makes it explicit that you're adding methods to types, not importing standalone items.

### Basic Syntax

```ori
extension std.iter.extensions { Iterator.count, Iterator.last }
extension std.fmt.extensions { Display.print, Display.println }
```

### From Local Files

```ori
extension './my_extensions' { Iterator.sum, Iterator.average }
extension '../utils/iter_helpers' { Iterator.take, Iterator.skip }
```

### Method-Level Granularity

You must specify individual methods, not entire traits:

```ori
// Correct: specify each method
extension std.iter.extensions { Iterator.count, Iterator.last }

// NOT supported: trait wildcards
// ERROR
extension std.iter.extensions { Iterator.* }
// ERROR
extension std.iter.extensions { Iterator }
```

**Rationale:** This maximizes explicitness:
- Clear which methods are added to which types
- No hidden method pollution
- Self-documenting imports

### Combining with Regular Imports

Extension imports go alongside regular imports at the top of the file:

```ori
// Regular imports
use std.collections { HashMap }
use './types' { User, Order }

// Extension imports
extension std.iter.extensions { Iterator.count, Iterator.sum }
extension './display_helpers' { Display.pretty_print }

// Definitions below...
```

### Extension vs Blanket Implementations

Ori uses explicit extension imports instead of blanket implementations (like Rust's `impl<T: A> B for T`):

| Aspect | Blanket Impls | Extension Imports |
|--------|---------------|-------------------|
| Activation | Implicit, always active | Explicit import required |
| Visibility | Hidden, "where did this come from?" | Clear in import statement |
| Conflicts | Can conflict silently | Must choose which to import |
| Side effects | Methods appear without asking | You ask for what you want |

Extension imports are explicit over implicit—no surprises about what methods are available.

### See Also

- [Trait Extensions](../04-traits/06-extensions.md) - Defining extensions
- [Re-exports](04-re-exports.md) - Re-exporting extensions

---

## Complex Import Patterns

### Multi-Level Modules

```ori
use database.sql.postgres { connect, query }
use database.sql.mysql { connect as mysql_connect }
use database.nosql.mongodb { Collection }
```

### Mixing Styles

Combine specific and qualified imports:

```ori
// specific: commonly used
use std.math { sqrt, abs }
// qualified: occasional use
use std.string as string
// qualified: multiple methods
use http.client

@process (value: float) -> str = run(
    let result = sqrt(
        .value: abs(
            .value: value,
        ),
    ),
    let formatted = string.format(
        .template: "{:.2}",
        .value: result,
    ),
    http.client.post(
        .url: "/result",
        .body: formatted,
    ),
    formatted,
)
```

### Importing Types and Functions Together

```ori
use http {
    // type
    Client,
    // type
    Response,
    // type
    Error,
    // function
    get,
    // function
    post
}

@fetch (client: Client, url: str) -> Result<Response, Error> = get(
    .url: url,
)
```

---

## Import Errors

### Unknown Module

```ori
use unknown_module { func }
```

```
error: module not found: `unknown_module`
  --> src/main.ori:1:5
  |
1 | use unknown_module { func }
  |     ^^^^^^^^^^^^^^
  |
  = help: check the module path and ensure the file exists
```

### Unknown Item

```ori
use std.math { squareroot }
```

```
error: `squareroot` not found in `std.math`
  --> src/main.ori:1:16
  |
1 | use std.math { squareroot }
  |                ^^^^^^^^^^
  |
  = help: did you mean `sqrt`?
```

### Duplicate Import

```ori
use math { add }
// ERROR: add already imported
use other { add }
```

```
error: `add` is already imported
  --> src/main.ori:2:13
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

```ori
// File: src/api/client.ori

use std.string { join }
use std.json { parse, stringify }
use http { Client, Response, Error }
use http.client { get, post, put, delete }

use api.config { $base_url, $timeout }
use api.auth { get_token }
use api.types { User, ApiError }

pub @fetch_user (id: int) -> Result<User, ApiError> = try(
    let token = get_token()?,
    let url = join(
        .parts: [$base_url, "/users/", str(id)],
    ),
    let response = get(
        .url: url,
        .headers: {"Authorization": token},
    )?,
    let user = parse(
        .input: response.body,
    )?,
    Ok(user),
)
```

### Math Utilities Module

```ori
// File: src/math_utils.ori

use std.math { sqrt, pow, abs, sin, cos }
use std.math as math

pub type Point = { x: float, y: float }
pub type Vector = { dx: float, dy: float }

pub @distance (p1: Point, p2: Point) -> float = run(
    let dx = p2.x - p1.x,
    let dy = p2.y - p1.y,
    sqrt(
        .value: pow(
            .base: dx,
            .exponent: 2,
        ) + pow(
            .base: dy,
            .exponent: 2,
        ),
    ),
)

pub @angle (v: Vector) -> float = math.atan2(
    .y: v.dy,
    .x: v.dx,
)

pub @magnitude (v: Vector) -> float = sqrt(
    .value: pow(
        .base: v.dx,
        .exponent: 2,
    ) + pow(
        .base: v.dy,
        .exponent: 2,
    ),
)
```

### Test File Imports

```ori
// File: src/_test/math_utils.test.ori

use math_utils { Point, Vector, distance, angle, magnitude }
use std.math { abs }

@test_distance tests @distance () -> void = run(
    let p1 = Point { x: 0.0, y: 0.0 },
    let p2 = Point { x: 3.0, y: 4.0 },
    assert_eq(
        .actual: distance(
            .p1: p1,
            .p2: p2,
        ),
        .expected: 5.0,
    ),
)

@test_magnitude tests @magnitude () -> void = run(
    let v = Vector { dx: 3.0, dy: 4.0 },
    assert_eq(
        .actual: magnitude(
            .v: v,
        ),
        .expected: 5.0,
    ),
)
```

---

## Best Practices

### Keep Imports Organized

```ori
// Good: grouped and ordered
use std.math { sqrt, abs }
use std.string { split, join }

use external.http { get, post }

use my_app.types { User }
use my_app.utils { validate }
```

### Use Aliases for Clarity

```ori
// When module name adds context
use crypto.hash as hash
use crypto.encrypt as encrypt

hash.sha256(
    .data: data,
)
encrypt.aes(
    .data: data,
    .key: key,
)
```

### Prefer Specific Imports

```ori
// Good: explicit about what's used
use std.math { sqrt }

// Less clear: what functions are actually used?
use std.math as math
```

### Avoid Import Pollution

```ori
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
