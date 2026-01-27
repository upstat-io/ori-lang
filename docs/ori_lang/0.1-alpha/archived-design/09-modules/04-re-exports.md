# Re-exports

This document covers Ori's re-export mechanism: using `pub use` to create facade modules and organize public APIs.

---

## Overview

Re-exports allow a module to expose items from other modules as part of its own public API. This is done with `pub use`.

```ori
// In http/mod.ori
pub use http.client { get, post }
pub use http.server { serve }

// Users can now do:
use http { get, post, serve }
```

### Design Rationale

| Benefit | Description |
|---------|-------------|
| Facade modules | Aggregate functionality from submodules |
| Cleaner APIs | Users import from one place, not many |
| Flexibility | Internal structure can change without breaking users |
| Library design | Expose curated public surface |

---

## Basic Syntax

### Re-exporting Specific Items

```ori
pub use module { item1, item2 }
```

### Re-exporting with Aliases

```ori
pub use module { original as renamed }
```

### Re-exporting Modules

```ori
// re-export for qualified access
pub use submodule
// re-export with alias
pub use submodule as alias
```

---

## Creating Facade Modules

### Problem: Deep Imports

Without re-exports, users must know internal structure:

```ori
// User code - tedious
use http.client { get, post }
use http.server { serve, listen }
use http.types { Request, Response, Header }
use http.errors { HttpError, TimeoutError }
```

### Solution: Facade Module

Create a facade that re-exports the public API:

```ori
// File: src/http/mod.ori

// Re-export from submodules
pub use http.client { get, post, put, delete }
pub use http.server { serve, listen }
pub use http.types { Request, Response, Header }
pub use http.errors { HttpError, TimeoutError }

// Can also add items defined here
pub type Method = Get | Post | Put | Delete
```

Now users have a clean interface:

```ori
// User code - simple
use http { get, post, serve, Request, Response }
```

---

## Organizing Submodules

### Directory Structure

```
src/
  http/
    mod.ori         -> facade module "http"
    client.ori      -> "http.client" (internal)
    server.ori      -> "http.server" (internal)
    types.ori       -> "http.types" (internal)
    errors.ori      -> "http.errors" (internal)
```

### Facade Module

```ori
// File: src/http/mod.ori

// Client operations
pub use http.client {
    get,
    post,
    put,
    delete,
    request
}

// Server operations
pub use http.server {
    serve,
    listen,
    Router
}

// Common types
pub use http.types {
    Request,
    Response,
    Header,
    Headers,
    Body,
    Method,
    StatusCode
}

// Error types
pub use http.errors {
    HttpError,
    TimeoutError,
    ConnectionError
}
```

### Internal Modules

Submodules focus on implementation:

```ori
// File: src/http/client.ori

use http.types { Request, Response }
use http.errors { HttpError }

pub @get (url: str) -> Result<Response, HttpError> = ...
pub @post (url: str, body: Body) -> Result<Response, HttpError> = ...
pub @put (url: str, body: Body) -> Result<Response, HttpError> = ...
pub @delete (url: str) -> Result<Response, HttpError> = ...
```

---

## Re-export Patterns

### Selective Re-export

Only expose what users need:

```ori
// File: src/database/mod.ori

// Public API - what users should use
pub use database.connection { connect, Connection }
pub use database.query { execute, Query }

// NOT re-exported - internal implementation
// database.pool
// database.cache
// database.wire_protocol
```

### Renamed Re-exports

Provide better names for the public API:

```ori
// File: src/json/mod.ori

pub use json.parser { parse_json as parse }
pub use json.writer { write_json as stringify }
pub use json.types { JsonValue as Value }
```

Users see the cleaner names:

```ori
use json { parse, stringify, Value }
```

### Grouping Related Items

```ori
// File: src/crypto/mod.ori

// Hashing
pub use crypto.hash { sha256, sha512, md5 }

// Encryption
pub use crypto.encrypt { aes_encrypt, aes_decrypt }
pub use crypto.encrypt { rsa_encrypt, rsa_decrypt }

// Utilities
pub use crypto.random { random_bytes, random_int }
pub use crypto.encoding { base64_encode, base64_decode }
```

---

## Re-export vs Import

### Import (private)

Regular `use` imports items for use within the current module only:

```ori
// File: src/utils.ori

// only available in this module
use std.string { split }

pub @parse_csv (line: str) -> [str] = split(line, ",")
// Users of utils cannot access split
```

### Re-export (public)

`pub use` makes items available to importers of this module:

```ori
// File: src/utils.ori

// available to users of utils
pub use std.string { split }

pub @parse_csv (line: str) -> [str] = split(line, ",")
// Users of utils CAN access split
```

```ori
// User code
// both available
use utils { split, parse_csv }
```

---

## Library Design

### Public Surface

Design your library's public API with re-exports:

```ori
// File: src/lib.ori (library entry point)

// Core types
pub use my_lib.core { Config, Context, Result }

// Primary operations
pub use my_lib.api { process, validate, transform }

// Utilities
pub use my_lib.utils { format, parse }

// Error types
pub use my_lib.errors { Error, ValidationError }
```

### Version Stability

Re-exports let you refactor internals without breaking users:

```ori
// Version 1.0: parse is in parser.ori
pub use my_lib.parser { parse }

// Version 2.0: moved parse to new_parser.ori
// But the public API stays the same:
// users don't notice
pub use my_lib.new_parser { parse }
```

### Deprecation

Re-exports can help with deprecation:

```ori
// Old name still works (re-exported)
pub use my_lib.new_module { new_func as old_func }

// New name is primary
pub use my_lib.new_module { new_func }
```

---

## Transitive Re-exports

### Chain of Re-exports

Re-exports can be chained:

```ori
// File: src/a.ori
pub @helper () -> int = 42

// File: src/b.ori
// re-exports helper
pub use a { helper }

// File: src/c.ori
// re-exports helper again
pub use b { helper }

// User code
// works through the chain
use c { helper }
```

### Avoiding Deep Chains

Keep re-export chains short for clarity:

```ori
// Good: one level
pub use internal.types { Type }

// Avoid: deep chains that obscure origin
// a re-exports from b, which re-exports from c...
// pub use a { x }
```

---

## Common Mistakes

### Circular Re-exports

```ori
// File: src/a.ori
pub use b { func_b }

// File: src/b.ori
// ERROR: circular re-export
pub use a { func_a }
```

```
error: circular re-export detected
  --> src/a.ori:1:1
  |
  = cycle: a -> b -> a
```

### Re-exporting Private Items

```ori
// File: src/internal.ori
// not pub
@private_func () -> int = 42

// File: src/public.ori
// ERROR: can't re-export private item
pub use internal { private_func }
```

```
error: cannot re-export private item `private_func`
  --> src/public.ori:1:16
  |
  = note: `private_func` is not public in `internal`
  = help: add `pub` to `private_func` in `internal.ori`
```

### Duplicate Re-exports

```ori
pub use module_a { helper }
// ERROR: duplicate
pub use module_b { helper }
```

```
error: duplicate re-export of `helper`
  --> src/lib.ori:2:20
  |
1 | pub use module_a { helper }
  |                    ------ first re-export
2 | pub use module_b { helper }
  |                    ^^^^^^
  |
  = help: use an alias: `pub use module_b { helper as helper_b }`
```

---

## Examples

### HTTP Library Facade

```ori
// File: src/http/mod.ori

// #HTTP client and server library
// #
// #Example:
// #```
// #use http { get, serve }
// #```

// Client API
pub use http.client {
    get,
    post,
    put,
    delete,
    Client,
    ClientConfig
}

// Server API
pub use http.server {
    serve,
    Router,
    route,
    middleware
}

// Shared types
pub use http.types {
    Request,
    Response,
    Headers,
    Body,
    Method,
    StatusCode
}

// Errors
pub use http.errors {
    HttpError,
    TimeoutError,
    NetworkError
}
```

### Database Library Facade

```ori
// File: src/db/mod.ori

// Connection management
pub use db.connection {
    connect,
    Connection,
    ConnectionPool,
    PoolConfig
}

// Query execution
pub use db.query {
    execute,
    query,
    Query,
    QueryResult
}

// Transactions
pub use db.transaction {
    begin,
    Transaction
}

// Schema types
pub use db.schema {
    Table,
    Column,
    Index
}

// Error types
pub use db.errors {
    DbError,
    ConnectionError,
    QueryError
}
```

---

## Best Practices

### Design the Public API First

```ori
// Think about what users need:
use my_lib { Config, process, Result, Error }

// Then organize internals to support it
```

### Keep Facades Focused

```ori
// Good: focused facade
pub use http.client { get, post }
pub use http.server { serve }

// Avoid: kitchen sink
// 50 items
pub use everything { ... }
```

### Document Re-exports

```ori
// #Primary client operations
pub use http.client { get, post, put, delete }

// #Server creation and routing
pub use http.server { serve, Router }
```

### Test Through Public API

```ori
// Test file imports from facade, not internals
use http { get, serve, Request, Response }

@test_get tests @get () -> void = ...
```

---

## See Also

- [Module System](01-module-system.md)
- [Imports](02-imports.md)
- [Prelude](03-prelude.md)
- [Basic Syntax](../02-syntax/01-basic-syntax.md)
