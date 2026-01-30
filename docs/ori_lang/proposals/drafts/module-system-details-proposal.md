# Proposal: Module System Details

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Compiler, module resolution

---

## Summary

This proposal specifies module system details including `mod.ori` conventions, circular dependency detection, re-export chains, and nested module structure.

---

## Problem Statement

The spec describes imports but doesn't specify:

1. **mod.ori convention**: How do directory-based modules work?
2. **Circular dependencies**: How are they detected and reported?
3. **Re-export chains**: What happens with `pub use` across multiple levels?
4. **Nested modules**: How does visibility work in nested directories?
5. **Package structure**: Library vs binary visibility distinctions

---

## Module Structure

### File-Based Modules

Each `.ori` file is a module:

```
src/
├── main.ori       # Module: main
├── utils.ori      # Module: utils
└── math.ori       # Module: math
```

Import:
```ori
use "./utils" { helper }
use "./math" { sqrt }
```

### Directory-Based Modules

A directory with `mod.ori` is also a module:

```
src/
├── main.ori
└── http/
    ├── mod.ori      # Module entry point
    ├── client.ori   # Submodule
    └── server.ori   # Submodule
```

The `mod.ori` file defines what the directory module exports.

### mod.ori Purpose

`mod.ori` serves as the public interface for a directory module:

```ori
// http/mod.ori
pub use "./client" { Client, get, post }
pub use "./server" { Server, listen }

// Private implementation detail
use "./internal" { ... }
```

Import the directory module:
```ori
use "./http" { Client, Server }  // Imports from mod.ori
```

### Implicit mod.ori

If a directory has no `mod.ori`, it cannot be imported as a module:

```ori
// ERROR: cannot import directory without mod.ori
use "./http" { ... }  // Error if http/mod.ori doesn't exist
```

Individual files can still be imported:
```ori
use "./http/client" { Client }  // OK: imports client.ori directly
```

---

## Circular Dependency Detection

### Definition

A circular dependency exists when module A imports module B, and B (directly or transitively) imports A:

```
A -> B -> C -> A  (circular)
```

### Detection Algorithm

The compiler builds a dependency graph during import resolution:

1. Start with the entry module (e.g., `main.ori`)
2. For each `use` statement, add an edge from current module to target
3. Detect cycles using depth-first traversal
4. Report the first cycle found

### Error Reporting

```
error[E1100]: circular dependency detected
  --> src/a.ori:1:1
   |
   = note: cycle: a.ori -> b.ori -> c.ori -> a.ori
   |
   ::: src/a.ori:1:1
   |
1  | use "./b" { ... }
   | ----------------- a.ori imports b.ori
   |
   ::: src/b.ori:1:1
   |
1  | use "./c" { ... }
   | ----------------- b.ori imports c.ori
   |
   ::: src/c.ori:1:1
   |
1  | use "./a" { ... }
   | ----------------- c.ori imports a.ori, completing the cycle
   |
   = help: consider extracting shared code to a separate module
```

### Breaking Cycles

Common strategies:
1. Extract shared types/functions to a third module
2. Use dependency injection (pass functions as parameters)
3. Reorganize to have one-way dependencies

```
Before (circular):
A -> B
B -> A

After (extracted):
A -> Common
B -> Common
```

---

## Re-export Chains

### Single-Level Re-export

```ori
// internal.ori
pub @helper () -> int = 42

// api.ori
pub use "./internal" { helper }

// main.ori
use "./api" { helper }  // Gets internal.helper via api
```

### Multi-Level Re-export

Re-exports can chain through multiple levels:

```ori
// level3.ori
pub @deep () -> str = "deep"

// level2.ori
pub use "./level3" { deep }

// level1.ori
pub use "./level2" { deep }

// main.ori
use "./level1" { deep }  // Works through the chain
```

### Visibility Through Chain

An item must be `pub` at every level of the chain:

```ori
// internal.ori
@private_fn () -> int = 42  // Not pub

// api.ori
pub use "./internal" { private_fn }  // ERROR: private_fn is not pub

// Correct:
// internal.ori
pub @helper () -> int = 42  // Must be pub

// api.ori
pub use "./internal" { helper }  // OK
```

### Re-export Aliasing

Aliases work through chains:

```ori
// math.ori
pub @square (x: int) -> int = x * x

// utils.ori
pub use "./math" { square as sq }

// main.ori
use "./utils" { sq }  // Gets math.square as sq
```

### Diamond Re-exports

When the same item is accessible through multiple paths:

```ori
// base.ori
pub type Value = int

// path_a.ori
pub use "./base" { Value }

// path_b.ori
pub use "./base" { Value }

// main.ori
use "./path_a" { Value }
use "./path_b" { Value }  // Same type, no conflict
```

The same underlying item imported multiple times is NOT an error — it's the same type/function.

---

## Nested Module Visibility

### Parent Cannot Access Child Private

```
src/
├── parent.ori
└── child/
    └── mod.ori
```

```ori
// child/mod.ori
@private_fn () -> int = 42  // Private to child
pub @public_fn () -> int = 84

// parent.ori
use "./child" { public_fn }   // OK
use "./child" { private_fn }  // ERROR: not visible
```

### Child Cannot Access Parent Private

```ori
// parent.ori
@parent_private () -> int = 42

// child/mod.ori
use "../parent" { parent_private }  // ERROR: not visible
```

### Sibling Visibility

Siblings cannot access each other's private items:

```
src/
├── a.ori
└── b.ori
```

```ori
// a.ori
@a_private () -> int = 1

// b.ori
use "./a" { a_private }  // ERROR: a_private is private
```

### Private Access via ::

The `::` prefix allows importing private items for testing:

```ori
// In test file or same module
use "./internal" { ::private_helper }  // Explicit private access
```

This is intentional — tests need access to internals.

---

## Package Structure

### Library Package

A library package exports its public API:

```
my_lib/
├── ori.toml         # Package manifest
├── src/
│   ├── lib.ori      # Library entry point
│   └── internal.ori # Internal implementation
```

```ori
// lib.ori
pub use "./internal" { PublicType, public_fn }
// Only pub items are visible to consumers
```

### Binary Package

A binary package has an entry point:

```
my_app/
├── ori.toml
├── src/
│   ├── main.ori    # Binary entry point (@main)
│   └── utils.ori
```

### Library + Binary

A package can be both:

```
my_pkg/
├── ori.toml
├── src/
│   ├── lib.ori     # Library API
│   ├── main.ori    # Binary using the library
│   └── internal.ori
```

The binary can import from the library:
```ori
// main.ori
use "my_pkg" { exported_fn }  // Uses library's public API
```

---

## Import Resolution Order

### Resolution Steps

1. **Relative path** (`"./..."`, `"../..."`): Resolve relative to current file
2. **Package name** (`"my_pkg"`): Look up in dependencies
3. **Standard library** (`std.xxx`): Built-in modules

### Path Resolution

```ori
// In src/utils/helpers.ori:
use "./sibling"        // src/utils/sibling.ori
use "../parent"        // src/parent.ori
use "../../other"      // other.ori (outside src)
```

### Package Dependencies

Defined in `ori.toml`:

```toml
[dependencies]
some_lib = "1.0.0"
```

```ori
use "some_lib" { Thing }  // From dependency
```

---

## Error Messages

### Missing Module

```
error[E1101]: cannot find module
  --> src/main.ori:1:1
   |
1  | use "./nonexistent" { helper }
   |     ^^^^^^^^^^^^^^^ module not found
   |
   = note: looked for: src/nonexistent.ori, src/nonexistent/mod.ori
```

### Missing Export

```
error[E1102]: item `foo` is not exported from module
  --> src/main.ori:1:1
   |
1  | use "./utils" { foo }
   |                 ^^^ not found in utils
   |
   = note: available exports: helper, process, transform
   = help: did you mean `bar`?
```

### Private Item

```
error[E1103]: `secret` is private
  --> src/main.ori:1:1
   |
1  | use "./internal" { secret }
   |                    ^^^^^^ cannot import private item
   |
   = help: use `::secret` for explicit private access (testing)
   = help: or make `secret` public with `pub`
```

---

## Examples

### Organizing a Library

```
my_lib/
├── ori.toml
└── src/
    ├── lib.ori           # Public API
    ├── types/
    │   ├── mod.ori       # Type exports
    │   ├── user.ori
    │   └── post.ori
    ├── services/
    │   ├── mod.ori
    │   ├── auth.ori
    │   └── db.ori
    └── internal/
        └── helpers.ori   # Not re-exported
```

```ori
// lib.ori
pub use "./types" { User, Post }
pub use "./services" { authenticate, query }
// internal not re-exported — implementation detail

// types/mod.ori
pub use "./user" { User }
pub use "./post" { Post }
```

### Avoiding Circular Dependencies

```ori
// BAD: circular
// user.ori
use "./post" { Post }
type User = { posts: [Post] }

// post.ori
use "./user" { User }
type Post = { author: User }

// GOOD: extract to shared
// types.ori
type UserId = int
type PostId = int

// user.ori
use "./types" { UserId, PostId }
type User = { id: UserId, post_ids: [PostId] }

// post.ori
use "./types" { UserId, PostId }
type Post = { id: PostId, author_id: UserId }
```

---

## Spec Changes Required

### Update `12-modules.md`

Add:
1. `mod.ori` convention specification
2. Circular dependency detection algorithm
3. Re-export chain rules
4. Visibility in nested modules

### Add Package Section

Document:
1. Package structure (library vs binary)
2. `ori.toml` manifest format
3. Dependency resolution

---

## Summary

| Aspect | Specification |
|--------|--------------|
| mod.ori | Required for directory-as-module |
| Circular deps | Compile error with path shown |
| Re-export chains | All levels must be `pub` |
| Diamond imports | Same item = no conflict |
| Private access | `::` prefix for explicit access |
| Siblings | Cannot access each other's private items |
| Resolution order | Relative → Package → Stdlib |
