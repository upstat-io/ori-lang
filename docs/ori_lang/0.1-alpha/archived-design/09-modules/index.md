# Modules

This section covers Ori's module system: file-based modules, imports, visibility, and re-exports.

---

## Documents

| Document | Description |
|----------|-------------|
| [Module System](01-module-system.md) | File = module, directory structure |
| [Imports](02-imports.md) | use syntax, qualified imports, aliases |
| [Prelude](03-prelude.md) | Auto-imported items |
| [Re-exports](04-re-exports.md) | pub use for facade modules |

---

## Overview

In Ori, each file is a module:

```
src/
  math.ori        -> module "math"
  utils.ori       -> module "utils"
  http/
    client.ori    -> module "http.client"
    server.ori    -> module "http.server"
```

### Import Syntax

```ori
// Import specific items
use math { add, subtract }

// Import with alias
use math { add as plus }

// Import module for qualified access
use http.client
// Use as: http.client.get()

// Import module with alias
use http.client as http
// Use as: http.get()
```

### Extension Imports

Trait extension methods use a separate `extension` keyword:

```ori
// Import extension methods (not use!)
extension std.iter.extensions { Iterator.count, Iterator.sum }
extension './my_extensions' { Display.pretty_print }

// Now available on any Iterator
range(1, 100).count()
```

See [Imports](02-imports.md#extension-imports) and [Trait Extensions](../04-traits/06-extensions.md) for details.

### Visibility

```ori
// Public (accessible from other modules)
pub @add (left: int, right: int) -> int = left + right
pub type Point = { x: int, y: int }
pub $default_timeout = 30

// Private (default, same module only)
@helper () -> int = 1
type Internal = { ... }
```

### Key Principles

1. **File = module** - No declaration needed
2. **Explicit imports** - Dependencies visible at top
3. **Private by default** - Use `pub` for public API
4. **No circular imports** - Compiler error if detected

---

## Standard Library

The standard library uses the `std` namespace:

```ori
use std.time { Date, Time }
use std.fs { read_file }
use std.net.http { get, post }
```

**Module organization:**

| Category | Modules |
|----------|---------|
| I/O | `std.io`, `std.fs` |
| Network | `std.net`, `std.net.http` |
| Data | `std.json`, `std.encoding` |
| Utilities | `std.time`, `std.math`, `std.fmt`, `std.text` |
| System | `std.env`, `std.process`, `std.log` |
| Async | `std.async` |

See [Standard Library Documentation](../../modules/README.md) for complete reference.

---

## See Also

- [Main Index](../00-index.md)
- [Basic Syntax](../02-syntax/01-basic-syntax.md)
