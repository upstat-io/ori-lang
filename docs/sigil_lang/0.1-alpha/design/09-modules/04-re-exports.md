# Re-exports

This document covers Sigil's re-export mechanism: using `pub use` to create facade modules and organize public APIs.

---

## Overview

Re-exports allow a module to expose items from other modules as part of its own public API. This is done with `pub use`.

```sigil
// In http/mod.si
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

```sigil
pub use module { item1, item2 }
```

### Re-exporting with Aliases

```sigil
pub use module { original as renamed }
```

### Re-exporting Modules

```sigil
pub use submodule              // re-export for qualified access
pub use submodule as alias     // re-export with alias
```

---

## Creating Facade Modules

### Problem: Deep Imports

Without re-exports, users must know internal structure:

```sigil
// User code - tedious
use http.client { get, post }
use http.server { serve, listen }
use http.types { Request, Response, Header }
use http.errors { HttpError, TimeoutError }
```

### Solution: Facade Module

Create a facade that re-exports the public API:

```sigil
// File: src/http/mod.si

// Re-export from submodules
pub use http.client { get, post, put, delete }
pub use http.server { serve, listen }
pub use http.types { Request, Response, Header }
pub use http.errors { HttpError, TimeoutError }

// Can also add items defined here
pub type Method = Get | Post | Put | Delete
```

Now users have a clean interface:

```sigil
// User code - simple
use http { get, post, serve, Request, Response }
```

---

## Organizing Submodules

### Directory Structure

```
src/
  http/
    mod.si         -> facade module "http"
    client.si      -> "http.client" (internal)
    server.si      -> "http.server" (internal)
    types.si       -> "http.types" (internal)
    errors.si      -> "http.errors" (internal)
```

### Facade Module

```sigil
// File: src/http/mod.si

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

```sigil
// File: src/http/client.si

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

```sigil
// File: src/database/mod.si

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

```sigil
// File: src/json/mod.si

pub use json.parser { parse_json as parse }
pub use json.writer { write_json as stringify }
pub use json.types { JsonValue as Value }
```

Users see the cleaner names:

```sigil
use json { parse, stringify, Value }
```

### Grouping Related Items

```sigil
// File: src/crypto/mod.si

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

```sigil
// File: src/utils.si

use std.string { split }  // only available in this module

pub @parse_csv (line: str) -> [str] = split(line, ",")
// Users of utils cannot access split
```

### Re-export (public)

`pub use` makes items available to importers of this module:

```sigil
// File: src/utils.si

pub use std.string { split }  // available to users of utils

pub @parse_csv (line: str) -> [str] = split(line, ",")
// Users of utils CAN access split
```

```sigil
// User code
use utils { split, parse_csv }  // both available
```

---

## Library Design

### Public Surface

Design your library's public API with re-exports:

```sigil
// File: src/lib.si (library entry point)

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

```sigil
// Version 1.0: parse is in parser.si
pub use my_lib.parser { parse }

// Version 2.0: moved parse to new_parser.si
// But the public API stays the same:
pub use my_lib.new_parser { parse }  // users don't notice
```

### Deprecation

Re-exports can help with deprecation:

```sigil
// Old name still works (re-exported)
pub use my_lib.new_module { new_func as old_func }

// New name is primary
pub use my_lib.new_module { new_func }
```

---

## Transitive Re-exports

### Chain of Re-exports

Re-exports can be chained:

```sigil
// File: src/a.si
pub @helper () -> int = 42

// File: src/b.si
pub use a { helper }      // re-exports helper

// File: src/c.si
pub use b { helper }      // re-exports helper again

// User code
use c { helper }          // works through the chain
```

### Avoiding Deep Chains

Keep re-export chains short for clarity:

```sigil
// Good: one level
pub use internal.types { Type }

// Avoid: deep chains that obscure origin
// pub use a { x }  // a re-exports from b, which re-exports from c...
```

---

## Common Mistakes

### Circular Re-exports

```sigil
// File: src/a.si
pub use b { func_b }

// File: src/b.si
pub use a { func_a }  // ERROR: circular re-export
```

```
error: circular re-export detected
  --> src/a.si:1:1
  |
  = cycle: a -> b -> a
```

### Re-exporting Private Items

```sigil
// File: src/internal.si
@private_func () -> int = 42  // not pub

// File: src/public.si
pub use internal { private_func }  // ERROR: can't re-export private item
```

```
error: cannot re-export private item `private_func`
  --> src/public.si:1:16
  |
  = note: `private_func` is not public in `internal`
  = help: add `pub` to `private_func` in `internal.si`
```

### Duplicate Re-exports

```sigil
pub use module_a { helper }
pub use module_b { helper }  // ERROR: duplicate
```

```
error: duplicate re-export of `helper`
  --> src/lib.si:2:20
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

```sigil
// File: src/http/mod.si

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

```sigil
// File: src/db/mod.si

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

```sigil
// Think about what users need:
use my_lib { Config, process, Result, Error }

// Then organize internals to support it
```

### Keep Facades Focused

```sigil
// Good: focused facade
pub use http.client { get, post }
pub use http.server { serve }

// Avoid: kitchen sink
pub use everything { ... }  // 50 items
```

### Document Re-exports

```sigil
// #Primary client operations
pub use http.client { get, post, put, delete }

// #Server creation and routing
pub use http.server { serve, Router }
```

### Test Through Public API

```sigil
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
