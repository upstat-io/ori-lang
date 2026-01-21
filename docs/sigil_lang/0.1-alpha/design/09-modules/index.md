# Modules

This section covers Sigil's module system: file-based modules, imports, visibility, and re-exports.

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

In Sigil, each file is a module:

```
src/
  math.si        -> module "math"
  utils.si       -> module "utils"
  http/
    client.si    -> module "http.client"
    server.si    -> module "http.server"
```

### Import Syntax

```sigil
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

### Visibility

```sigil
// Public (accessible from other modules)
pub @add (a: int, b: int) -> int = a + b
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

## See Also

- [Main Index](../00-index.md)
- [Basic Syntax](../02-syntax/01-basic-syntax.md)
