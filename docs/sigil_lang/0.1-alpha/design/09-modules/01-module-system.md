# Module System

This document covers Sigil's file-based module system: how files map to modules, directory structure, and module paths.

---

## Overview

In Sigil, every source file is automatically a module. The file path determines the module name. No declaration is needed.

```
src/
  math.si        -> module "math"
  utils.si       -> module "utils"
  http/
    client.si    -> module "http.client"
    server.si    -> module "http.server"
```

### Design Rationale

This design was chosen for AI code generation:

| Benefit | Why It Matters |
|---------|----------------|
| No boilerplate | File path *is* the module name |
| No sync issues | AI doesn't keep declarations and file paths in sync |
| Simple mental model | "Where is this code?" -> look at the path |
| Proven approach | Works for Python, Go, TypeScript ESM |

---

## File = Module

### Basic Mapping

Each `.si` file is a module named after its path:

```sigil
// File: src/math.si
// Module: math

pub @add (a: int, b: int) -> int = a + b
pub @subtract (a: int, b: int) -> int = a - b
```

```sigil
// File: src/string_utils.si
// Module: string_utils

pub @capitalize (s: str) -> str = ...
pub @truncate (s: str, max: int) -> str = ...
```

### File Names

Module names follow file naming rules:

| File | Module Name | Valid |
|------|-------------|-------|
| `math.si` | `math` | Yes |
| `string_utils.si` | `string_utils` | Yes |
| `HTTP.si` | `HTTP` | Yes (but not recommended) |
| `my-module.si` | - | No (hyphens not allowed) |
| `123start.si` | - | No (must start with letter) |

**Convention:** Use `snake_case` for file and module names.

### No Declaration Required

Unlike some languages, Sigil doesn't require a module declaration:

```sigil
// NOT needed in Sigil:
// module math;  // Java
// package math  // Go
// mod math;     // Rust (in parent)

// Just write your code
pub @add (a: int, b: int) -> int = a + b
```

The compiler infers the module name from the file path.

---

## Directory Structure

### Nested Modules

Directories create nested module paths:

```
src/
  http/
    client.si    -> module "http.client"
    server.si    -> module "http.server"
    request.si   -> module "http.request"
    response.si  -> module "http.response"
```

```sigil
// File: src/http/client.si
// Module: http.client

use http.request { Request }
use http.response { Response }

pub @get (url: str) -> Result<Response, Error> = ...
pub @post (url: str, body: str) -> Result<Response, Error> = ...
```

### Deep Nesting

Any level of nesting is supported:

```
src/
  database/
    sql/
      postgres/
        connection.si -> module "database.sql.postgres.connection"
        query.si      -> module "database.sql.postgres.query"
      mysql/
        connection.si -> module "database.sql.mysql.connection"
```

**Recommendation:** Limit nesting to 3-4 levels. Deeper nesting often indicates the need to reorganize.

### Directory Index Files

A `mod.si` file serves as the directory's index module:

```
src/
  http/
    mod.si       -> module "http"
    client.si    -> module "http.client"
    server.si    -> module "http.server"
```

```sigil
// File: src/http/mod.si
// Module: http

// Re-export commonly used items
pub use http.client { get, post }
pub use http.server { serve }

// Shared types
pub type Method = Get | Post | Put | Delete
pub type StatusCode = int
```

Users can now import from `http` directly:

```sigil
use http { get, post, serve, Method }
```

---

## Module Paths

### Path Components

Module paths use dot (`.`) as the separator:

```
http.client.get
^^^^ ^^^^^^ ^^^
 |     |     |
 |     |     +-- function name
 |     +-------- submodule
 +-------------- root module
```

### Absolute Paths

All module paths are absolute from the source root:

```sigil
use http.client { get }           // absolute path
use database.sql.query { execute } // absolute path
```

### No Relative Paths

Sigil does not support relative imports:

```sigil
// NOT supported:
// use ./sibling { func }
// use ../parent { func }
// use super.module { func }

// Always use absolute paths:
use my_project.sibling { func }
use my_project.parent { func }
```

**Rationale:** Absolute paths are easier to understand and refactor. AI can always know the full path without tracking the current file's location.

---

## Source Root

### Default Structure

The source root is where module resolution starts:

```
my_project/
  src/              <- source root
    main.si         -> module "main"
    lib.si          -> module "lib"
    utils/
      helpers.si    -> module "utils.helpers"
```

### Project Configuration

The source root can be configured in `sigil.toml`:

```toml
[project]
name = "my_project"
src = "src"        # source root (default: "src")
```

### Multiple Source Roots

For larger projects:

```toml
[project]
name = "my_project"
src = ["src", "gen"]  # multiple source roots
```

```
my_project/
  src/
    main.si       -> module "main"
    app.si        -> module "app"
  gen/
    proto.si      -> module "proto"  # generated code
```

---

## Module Organization Patterns

### Flat Structure

For small projects:

```
src/
  main.si
  lib.si
  utils.si
  config.si
```

### Feature-Based Structure

For medium projects:

```
src/
  main.si
  auth/
    mod.si
    login.si
    session.si
  api/
    mod.si
    routes.si
    handlers.si
  db/
    mod.si
    connection.si
    queries.si
```

### Layer-Based Structure

For larger projects:

```
src/
  main.si
  domain/           # business logic
    user.si
    order.si
  service/          # application services
    user_service.si
    order_service.si
  repository/       # data access
    user_repo.si
    order_repo.si
  api/              # HTTP interface
    handlers.si
    routes.si
```

### Hybrid Structure

Combining feature and layer approaches:

```
src/
  main.si
  user/
    domain.si       # User type, business rules
    service.si      # UserService
    repository.si   # UserRepository
    api.si          # HTTP handlers for user
  order/
    domain.si
    service.si
    repository.si
    api.si
  shared/
    types.si
    utils.si
```

---

## Standard Library Structure

The standard library follows these conventions:

```
std/
  mod.si            -> module "std"
  math.si           -> module "std.math"
  string.si         -> module "std.string"
  list.si           -> module "std.list"
  io/
    mod.si          -> module "std.io"
    file.si         -> module "std.io.file"
    stream.si       -> module "std.io.stream"
  net/
    mod.si          -> module "std.net"
    http.si         -> module "std.net.http"
    tcp.si          -> module "std.net.tcp"
```

```sigil
// Importing from std
use std.math { sqrt, abs }
use std.io.file { read_file, write_file }
use std.net.http { get, post }
```

---

## Module Discovery

### Compiler Resolution

The compiler resolves modules in this order:

1. **Standard library** (`std.*`)
2. **Project source** (configured source roots)
3. **Dependencies** (from `sigil.toml`)

```sigil
use std.math { sqrt }           // 1. standard library
use my_app.utils { format }     // 2. project source
use external_lib { process }    // 3. dependency
```

### Ambiguity Resolution

If the same module name exists in multiple locations, an error is raised:

```
error: ambiguous module "utils"
  --> src/main.si:1:5
  |
1 | use utils { helper }
  |     ^^^^^
  |
  = found in: src/utils.si
  = found in: dependency "common-utils"
  = help: use a qualified path or rename one module
```

---

## Test Modules

### Test Directory Convention

Test files live in `_test/` directories:

```
src/
  math.si           -> module "math"
  _test/
    math.test.si    -> test module for "math"
```

### Test Module Structure

```sigil
// File: src/_test/math.test.si

use math { add, subtract, multiply }

@test_add tests @add () -> void = run(
    assert_eq(add(2, 3), 5),
    assert_eq(add(-1, 1), 0),
)

@test_subtract tests @subtract () -> void = run(
    assert_eq(subtract(5, 3), 2),
    assert_eq(subtract(0, 5), -5),
)
```

### Test Discovery

The compiler finds test modules by:

1. Looking for `_test/` directories
2. Finding files matching `*.test.si`
3. Running all `@test_*` functions

---

## Module Dependencies

### Dependency Graph

The compiler builds a dependency graph from imports:

```
main.si
  |-> api.si
  |     |-> handlers.si
  |     |-> routes.si
  |-> db.si
        |-> connection.si
        |-> queries.si
```

### Circular Dependencies

Circular imports are **not allowed**:

```sigil
// File: src/a.si
use b { func_b }  // ERROR: circular dependency
pub @func_a () -> int = func_b()

// File: src/b.si
use a { func_a }  // a uses b, b uses a -> cycle
pub @func_b () -> int = func_a()
```

```
error: circular dependency detected
  --> src/a.si:1:5
  |
  = cycle: a -> b -> a
  = help: refactor to break the cycle, consider extracting shared code
```

**Rationale:** Circular dependencies indicate poor architecture. The compiler forces clean, layered structure.

### Breaking Cycles

Extract shared code into a third module:

```sigil
// File: src/shared.si
pub type SharedData = { ... }
pub @shared_func () -> int = ...

// File: src/a.si
use shared { SharedData, shared_func }
pub @func_a (data: SharedData) -> int = ...

// File: src/b.si
use shared { SharedData, shared_func }
pub @func_b (data: SharedData) -> int = ...
```

---

## Best Practices

### Module Size

Keep modules focused:

```sigil
// Good: single responsibility
// File: src/user.si - User type and operations

pub type User = { id: int, name: str, email: str }

pub @create_user (name: str, email: str) -> User = ...
pub @validate_email (email: str) -> bool = ...
pub @format_user (user: User) -> str = ...
```

```sigil
// Bad: too many unrelated things
// File: src/everything.si

pub type User = { ... }
pub type Product = { ... }
pub type Order = { ... }
pub @create_user () -> User = ...
pub @fetch_product () -> Product = ...
pub @process_order () -> void = ...
```

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Modules | `snake_case` | `user_service.si` |
| Directories | `snake_case` | `http_client/` |
| Types | `PascalCase` | `type UserProfile = ...` |
| Functions | `snake_case` | `@get_user` |

### Export Strategy

Export only what's needed:

```sigil
// Internal helper - not exported
@validate_internal (s: str) -> bool = ...

// Public API - exported
pub @process (input: str) -> Result<Output, Error> = ...
```

### Documentation

Document modules at the top:

```sigil
// #HTTP client for making web requests
// #
// #Provides functions for GET, POST, PUT, DELETE requests.
// #
// #Example:
// #```
// #use http { get }
// #response = get("https://api.example.com")
// #```

pub @get (url: str) -> Result<Response, Error> = ...
pub @post (url: str, body: str) -> Result<Response, Error> = ...
```

---

## See Also

- [Imports](02-imports.md)
- [Prelude](03-prelude.md)
- [Re-exports](04-re-exports.md)
- [Basic Syntax](../02-syntax/01-basic-syntax.md)
