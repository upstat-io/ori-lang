# Module System

This document covers Ori's file-based module system: how files map to modules, directory structure, and module paths.

---

## Overview

In Ori, every source file is automatically a module. The file path determines the module name. No declaration is needed.

```
src/
  math.ori        -> module "math"
  utils.ori       -> module "utils"
  http/
    client.ori    -> module "http.client"
    server.ori    -> module "http.server"
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

Each `.ori` file is a module named after its path:

```ori
// File: src/math.ori
// Module: math

pub @add (left: int, right: int) -> int = left + right
pub @subtract (left: int, right: int) -> int = left - right
```

```ori
// File: src/string_utils.ori
// Module: string_utils

pub @capitalize (text: str) -> str = ...
pub @truncate (text: str, max_length: int) -> str = ...
```

### File Names

Module names follow file naming rules:

| File | Module Name | Valid |
|------|-------------|-------|
| `math.ori` | `math` | Yes |
| `string_utils.ori` | `string_utils` | Yes |
| `HTTP.ori` | `HTTP` | Yes (but not recommended) |
| `my-module.ori` | - | No (hyphens not allowed) |
| `123start.ori` | - | No (must start with letter) |

**Convention:** Use `snake_case` for file and module names.

### No Declaration Required

Unlike some languages, Ori doesn't require a module declaration:

```ori
// NOT needed in Ori:
// Java
// module math;
// Go
// package math
// Rust (in parent)
// mod math;

// Just write your code
pub @add (left: int, right: int) -> int = left + right
```

The compiler infers the module name from the file path.

---

## Directory Structure

### Nested Modules

Directories create nested module paths:

```
src/
  http/
    client.ori    -> module "http.client"
    server.ori    -> module "http.server"
    request.ori   -> module "http.request"
    response.ori  -> module "http.response"
```

```ori
// File: src/http/client.ori
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
        connection.ori -> module "database.sql.postgres.connection"
        query.ori      -> module "database.sql.postgres.query"
      mysql/
        connection.ori -> module "database.sql.mysql.connection"
```

**Recommendation:** Limit nesting to 3-4 levels. Deeper nesting often indicates the need to reorganize.

### Directory Index Files

A `mod.ori` file serves as the directory's index module:

```
src/
  http/
    mod.ori       -> module "http"
    client.ori    -> module "http.client"
    server.ori    -> module "http.server"
```

```ori
// File: src/http/mod.ori
// Module: http

// Re-export commonly used items
pub use http.client { get, post }
pub use http.server { serve }

// Shared types
pub type Method = Get | Post | Put | Delete
pub type StatusCode = int
```

Users can now import from `http` directly:

```ori
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

```ori
// absolute path
use http.client { get }
// absolute path
use database.sql.query { execute }
```

### No Relative Paths

Ori does not support relative imports:

```ori
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
    main.ori         -> module "main"
    lib.ori          -> module "lib"
    utils/
      helpers.ori    -> module "utils.helpers"
```

### Project Configuration

The source root can be configured in `ori.toml`:

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
    main.ori       -> module "main"
    app.ori        -> module "app"
  gen/
    proto.ori      -> module "proto"  # generated code
```

---

## Module Organization Patterns

### Flat Structure

For small projects:

```
src/
  main.ori
  lib.ori
  utils.ori
  config.ori
```

### Feature-Based Structure

For medium projects:

```
src/
  main.ori
  auth/
    mod.ori
    login.ori
    session.ori
  api/
    mod.ori
    routes.ori
    handlers.ori
  db/
    mod.ori
    connection.ori
    queries.ori
```

### Layer-Based Structure

For larger projects:

```
src/
  main.ori
  domain/           # business logic
    user.ori
    order.ori
  service/          # application services
    user_service.ori
    order_service.ori
  repository/       # data access
    user_repo.ori
    order_repo.ori
  api/              # HTTP interface
    handlers.ori
    routes.ori
```

### Hybrid Structure

Combining feature and layer approaches:

```
src/
  main.ori
  user/
    domain.ori       # User type, business rules
    service.ori      # UserService
    repository.ori   # UserRepository
    api.ori          # HTTP handlers for user
  order/
    domain.ori
    service.ori
    repository.ori
    api.ori
  shared/
    types.ori
    utils.ori
```

---

## Standard Library Structure

The standard library follows these conventions:

```
std/
  mod.ori            -> module "std"
  math.ori           -> module "std.math"
  string.ori         -> module "std.string"
  list.ori           -> module "std.list"
  io/
    mod.ori          -> module "std.io"
    file.ori         -> module "std.io.file"
    stream.ori       -> module "std.io.stream"
  net/
    mod.ori          -> module "std.net"
    http.ori         -> module "std.net.http"
    tcp.ori          -> module "std.net.tcp"
```

```ori
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
3. **Dependencies** (from `ori.toml`)

```ori
// 1. standard library
use std.math { sqrt }
// 2. project source
use my_app.utils { format }
// 3. dependency
use external_lib { process }
```

### Ambiguity Resolution

If the same module name exists in multiple locations, an error is raised:

```
error: ambiguous module "utils"
  --> src/main.ori:1:5
  |
1 | use utils { helper }
  |     ^^^^^
  |
  = found in: src/utils.ori
  = found in: dependency "common-utils"
  = help: use a qualified path or rename one module
```

---

## Test Modules

### Test Directory Convention

Test files live in `_test/` directories:

```
src/
  math.ori           -> module "math"
  _test/
    math.test.ori    -> test module for "math"
```

### Test Module Structure

```ori
// File: src/_test/math.test.ori

use math { add, subtract, multiply }

@test_add tests @add () -> void = run(
    assert_eq(
        .actual: add(
            .a: 2,
            .b: 3,
        ),
        .expected: 5,
    ),
    assert_eq(
        .actual: add(
            .a: -1,
            .b: 1,
        ),
        .expected: 0,
    ),
)

@test_subtract tests @subtract () -> void = run(
    assert_eq(
        .actual: subtract(
            .a: 5,
            .b: 3,
        ),
        .expected: 2,
    ),
    assert_eq(
        .actual: subtract(
            .a: 0,
            .b: 5,
        ),
        .expected: -5,
    ),
)
```

### Test Discovery

The compiler finds test modules by:

1. Looking for `_test/` directories
2. Finding files matching `*.test.ori`
3. Running all `@test_*` functions

---

## Module Dependencies

### Dependency Graph

The compiler builds a dependency graph from imports:

```
main.ori
  |-> api.ori
  |     |-> handlers.ori
  |     |-> routes.ori
  |-> db.ori
        |-> connection.ori
        |-> queries.ori
```

### Circular Dependencies

Circular imports are **not allowed**:

```ori
// File: src/a.ori
// ERROR: circular dependency
use b { func_b }
pub @func_a () -> int = func_b()

// File: src/b.ori
// a uses b, b uses a -> cycle
use a { func_a }
pub @func_b () -> int = func_a()
```

```
error: circular dependency detected
  --> src/a.ori:1:5
  |
  = cycle: a -> b -> a
  = help: refactor to break the cycle, consider extracting shared code
```

**Rationale:** Circular dependencies indicate poor architecture. The compiler forces clean, layered structure.

### Breaking Cycles

Extract shared code into a third module:

```ori
// File: src/shared.ori
pub type SharedData = { ... }
pub @shared_func () -> int = ...

// File: src/a.ori
use shared { SharedData, shared_func }
pub @func_a (data: SharedData) -> int = ...

// File: src/b.ori
use shared { SharedData, shared_func }
pub @func_b (data: SharedData) -> int = ...
```

---

## Best Practices

### Module Size

Keep modules focused:

```ori
// Good: single responsibility
// File: src/user.ori - User type and operations

pub type User = { id: int, name: str, email: str }

pub @create_user (name: str, email: str) -> User = ...
pub @validate_email (email: str) -> bool = ...
pub @format_user (user: User) -> str = ...
```

```ori
// Bad: too many unrelated things
// File: src/everything.ori

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
| Modules | `snake_case` | `user_service.ori` |
| Directories | `snake_case` | `http_client/` |
| Types | `PascalCase` | `type UserProfile = ...` |
| Functions | `snake_case` | `@get_user` |

### Export Strategy

Export only what's needed:

```ori
// Internal helper - not exported
@validate_internal (text: str) -> bool = ...

// Public API - exported
pub @process (input: str) -> Result<Output, Error> = ...
```

### Documentation

Document modules at the top:

```ori
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
