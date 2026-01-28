# Proposal: No Circular Imports

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-22
**Approved:** 2026-01-25
**Affects:** Compiler, module system

---

## Summary

The Ori compiler must reject circular import dependencies between modules. Import cycles are a compile-time error.

```ori
// file: a.ori
use './b' { foo }  // ERROR if b.ori imports from a.ori

// file: b.ori
use './a' { bar }  // Creates cycle: a -> b -> a
```

```
error[E0401]: circular import detected
  --> a.ori:1:1
   |
 1 | use './b' { foo }
   | ^^^^^^^^^^^^^^^^^
   |
   = note: import cycle: a.ori -> b.ori -> a.ori
   = help: break the cycle by extracting shared code to a third module
```

---

## Motivation

### The Problem

Circular dependencies between modules create problems:

1. **Compilation complexity** - Compiler must resolve chicken-and-egg initialization
2. **Harder reasoning** - Can't understand A without understanding B, and vice versa
3. **Tight coupling** - Modules become inseparable, violating modularity
4. **Build performance** - Incremental compilation becomes difficult
5. **Initialization order** - Which module initializes first? Undefined or fragile.

### Example of Problematic Cycle

```ori
// file: user.ori
use './order' { Order }

type User = {
    name: str,
    orders: [Order],
}

@get_user_orders (user: User) -> [Order] = user.orders

// file: order.ori
use './user' { User }

type Order = {
    id: int,
    owner: User,
}

@get_order_owner (order: Order) -> User = order.owner
```

This creates a cycle: `user.ori` needs `Order` from `order.ori`, but `order.ori` needs `User` from `user.ori`.

### Languages That Reject Cycles

| Language | Circular Imports | Notes |
|----------|------------------|-------|
| Go | Rejected | Compile error |
| Rust | Rejected | Compile error |
| Java | Allowed | Can cause initialization issues |
| Python | Allowed | Runtime errors possible |
| JavaScript | Allowed | Hoisting issues, undefined values |
| C/C++ | N/A | Headers, not modules |

Languages that allow cycles often suffer from subtle bugs and initialization issues.

---

## Design

### The Rule

**Import cycles are a compile-time error.**

A cycle exists when module A imports from B, and B (directly or transitively) imports from A.

```
A -> B -> A           // Direct cycle (error)
A -> B -> C -> A      // Transitive cycle (error)
A -> B, A -> C        // No cycle (OK - A imports both)
A -> B, C -> B        // No cycle (OK - multiple importers)
```

### Error Message

Clear, actionable error message:

```
error[E0401]: circular import detected
  --> src/user.ori:1:1
   |
 1 | use './order' { Order }
   | ^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: import cycle detected:
           src/user.ori
        -> src/order.ori
        -> src/user.ori
   |
   = help: break the cycle by:
           - extracting shared types to a common module
           - using dependency inversion (traits)
           - restructuring module boundaries
```

### Detection Algorithm

Standard cycle detection in directed graph:

1. Build import graph (modules are nodes, imports are edges)
2. Run DFS-based cycle detection
3. If cycle found, report error with full cycle path
4. Process continues to find all cycles (not just first)

### What Counts as an Import

All of these create edges in the import graph:

```ori
use './other' { foo }           // Named import
use './other' { foo as bar }    // Aliased import
pub use './other' { foo }       // Re-export
use './subdir/mod' { thing }    // Subdirectory import
```

---

## Breaking Cycles

### Strategy 1: Extract Common Types

**Before (cycle):**
```ori
// user.ori
use './order' { Order }
type User = { orders: [Order] }

// order.ori
use './user' { User }
type Order = { owner: User }
```

**After (no cycle):**
```ori
// types.ori (new shared module)
type UserId = int
type OrderId = int

// user.ori
use './types' { OrderId }
type User = { order_ids: [OrderId] }

// order.ori
use './types' { UserId }
type Order = { owner_id: UserId }
```

### Strategy 2: Dependency Inversion

**Before (cycle):**
```ori
// renderer.ori
use './scene' { Scene }
@render (scene: Scene) -> Image = ...

// scene.ori
use './renderer' { render }
@preview (self: Scene) -> Image = render(self)
```

**After (no cycle):**
```ori
// renderer.ori
trait Renderable {
    @to_render_data (self) -> RenderData
}
@render<T: Renderable> (obj: T) -> Image = ...

// scene.ori
use './renderer' { Renderable, render }
impl Renderable for Scene { ... }
@preview (self: Scene) -> Image = render(self)
```

### Strategy 3: Restructure Boundaries

Sometimes cycles indicate modules are too granular:

**Before (cycle):**
```ori
// parser.ori
use './ast' { Node }
// ast.ori
use './parser' { parse_child }
```

**After (combined):**
```ori
// parser.ori (contains both parsing and AST)
type Node = ...
@parse () -> Node = ...
```

Or split differently:

```ori
// ast.ori (types only, no logic)
type Node = ...

// parser.ori (logic, depends on ast)
use './ast' { Node }
@parse () -> Node = ...

// ast_utils.ori (logic for AST, depends on ast)
use './ast' { Node }
@transform (n: Node) -> Node = ...
```

---

## Examples

### Valid Import Structures

**Linear chain:**
```
main.ori -> app.ori -> db.ori -> types.ori
```

**Tree:**
```
main.ori -> app.ori -> db.ori
        -> api.ori -> auth.ori
        -> cli.ori
```

**Diamond (allowed):**
```
main.ori -> app.ori -> types.ori
        -> api.ori -> types.ori
```

All modules can import `types.ori` - no cycle.

### Invalid Import Structures

**Direct cycle:**
```
a.ori -> b.ori -> a.ori
```

**Transitive cycle:**
```
a.ori -> b.ori -> c.ori -> a.ori
```

**Self-import:**
```
a.ori -> a.ori
```

---

## Edge Cases

### Re-exports

Re-exports create import edges:

```ori
// lib.ori
pub use './internal' { helper }  // lib imports internal
```

If `internal.ori` imports `lib.ori`, that's a cycle.

### Conditional Imports

Conditional imports still create edges (conservatively):

```ori
#[target(os: "linux")]
use './linux_impl' { native_call }
```

Even if not compiled on Windows, the edge exists in the import graph.

### Test Files

Test files can import the module they test:

```ori
// math.ori
@add (a: int, b: int) -> int = a + b

// math.test.ori
use './math' { add }

@test_add tests @add () -> void = assert_eq(add(1, 2), 3)
```

But the main module cannot import test files. Test files are leaves in the import graph.

---

## Implementation

### Compiler Changes

1. **Build import graph** during parsing phase
2. **Detect cycles** before type checking
3. **Report all cycles** (not just first)
4. **Fail fast** - no point continuing with cycles

### Algorithm

```
function detect_cycles(modules):
    graph = build_import_graph(modules)
    visited = {}
    rec_stack = {}
    cycles = []

    for module in modules:
        if module not in visited:
            dfs(module, visited, rec_stack, cycles, graph)

    return cycles

function dfs(node, visited, rec_stack, cycles, graph):
    visited[node] = true
    rec_stack[node] = true

    for neighbor in graph[node]:
        if neighbor not in visited:
            dfs(neighbor, visited, rec_stack, cycles, graph)
        else if neighbor in rec_stack:
            cycles.append(extract_cycle(node, neighbor))

    rec_stack[node] = false
```

### Build System Integration

The build system should:
1. Parse all files to extract imports
2. Check for cycles before full compilation
3. Cache import graph for incremental builds
4. Re-check only affected subgraph on file changes

---

## Rationale

### Why Not Allow Cycles?

Some languages allow cycles with caveats:

**Java:** Allows but can cause `ClassNotFoundException` at runtime if initialization order is wrong.

**Python:** Allows but can result in `ImportError` or `AttributeError` if modules access each other during import.

**JavaScript (ESM):** Allows but exports may be `undefined` if accessed before initialization.

These issues are:
- Hard to debug
- Non-deterministic (depend on import order)
- Violate the principle of least surprise

Ori's philosophy: **Compile-time errors are better than runtime surprises.**

### Why Not Lazy Imports?

Some languages solve cycles with lazy/deferred imports:

```python
def get_order():
    from order import Order  # Import inside function
    return Order()
```

This:
- Hides dependencies
- Makes code harder to analyze
- Defers errors to runtime
- Goes against Ori's explicitness

### Why Not Type-Only Imports?

TypeScript allows type-only imports that don't create runtime dependencies:

```typescript
import type { User } from './user';
```

This could work but:
- Adds complexity (two kinds of imports)
- Still indicates design smell (types are coupled)
- Better to extract shared types to common module

---

## Migration

For existing codebases with cycles:

1. **Identify cycles** - Compiler reports all cycles
2. **Prioritize** - Fix direct cycles first, then transitive
3. **Apply strategies** - Extract types, use traits, restructure
4. **Incremental** - Can be done module by module

### Tooling Support

```bash
# Check for cycles without full compilation
ori check --cycles

# Visualize import graph
ori graph --imports > imports.dot
dot -Tpng imports.dot -o imports.png
```

---

## Summary

- **Circular imports are compile-time errors**
- **Clear error messages** with full cycle path
- **Strategies provided** for breaking cycles
- **Enforces clean architecture** and modularity
- **Matches Go, Rust** and other modern languages

```
error[E0401]: circular import detected
  --> src/a.ori:1:1
   |
   = note: import cycle: a.ori -> b.ori -> a.ori
   = help: extract shared code to a common module
```
