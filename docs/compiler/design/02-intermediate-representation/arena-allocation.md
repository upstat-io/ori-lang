---
title: "Arena Allocation"
description: "Ori Compiler Design — Arena Allocation"
order: 201
section: "Intermediate Representation"
---

# Arena Allocation

The Ori compiler uses arena allocation for expressions. This document explains the implementation and rationale.

## What is Arena Allocation?

Arena allocation (also called "region-based allocation") allocates objects in a contiguous block of memory. Objects are freed all at once when the arena is dropped, rather than individually.

```rust
pub struct ExprArena {
    expr_kinds: Vec<ExprKind>,  // 24 bytes each — the primary data
    expr_spans: Vec<Span>,      // 8 bytes each — parallel to kinds
    // ... 15+ side-table vectors for variable-length data
}
```

The arena uses a struct-of-arrays (SoA) layout. Expression kinds and spans are stored in separate parallel `Vec`s for cache efficiency — most operations only need the 24-byte kind, keeping spans out of the cache line. Additional side-table vectors store variable-length data (parameters, match arms, list elements, etc.) indexed by range types.

## Implementation

### ExprId

```rust
/// Index into ExprArena
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct ExprId(pub u32);

impl ExprId {
    pub const INVALID: ExprId = ExprId(u32::MAX);

    pub fn index(self) -> usize {
        self.0 as usize
    }
}
```

`ExprId::INVALID` is used as a placeholder during parsing when the actual expression isn't known yet.

### ExprArena

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct ExprArena {
    // Parallel arrays (indexed by ExprId)
    expr_kinds: Vec<ExprKind>,   // Primary expression data (24 bytes each)
    expr_spans: Vec<Span>,       // Source locations (8 bytes each)

    // Side tables (indexed by range types like ExprRange, StmtRange, etc.)
    expr_lists: Vec<ExprId>,     // Flattened expression lists
    stmts: Vec<Stmt>,            // Statements
    params: Vec<Param>,          // Function parameters
    arms: Vec<Arm>,              // Match arms
    map_entries: Vec<MapEntry>,  // Map literal entries
    // ... and more for field_inits, struct_lit_fields, generic_params,
    //     parsed_types, match_patterns, binding_patterns, function_seqs, etc.
}

impl ExprArena {
    /// Pre-allocate based on source length heuristic
    pub fn with_capacity(source_len: usize) -> Self { ... }

    /// Allocate an expression (kind + span in parallel arrays)
    pub fn alloc(&mut self, kind: ExprKind, span: Span) -> ExprId { ... }

    /// Direct-append API for zero-allocation list building
    pub fn start_params(&mut self) -> u32 { ... }
    pub fn push_param(&mut self, param: Param) { ... }
    pub fn finish_params(&mut self, start: u32) -> ParamRange { ... }
}
```

The `with_capacity()` constructor uses `source_len / 20` as a heuristic for estimated expression count, reducing reallocations during parsing. The `reset()` method clears all vectors without deallocating, enabling arena reuse across compilation units. `SharedArena(Arc<ExprArena>)` wraps the arena for cross-module function references (e.g., when imported functions reference expressions in another module's arena).

**Capacity limits:** The maximum number of expressions is `u32::MAX` (ExprId is a `u32` index). Expression lists use `ExprRange { start: u32, len: u16 }`, so the maximum range length is `u16::MAX` (65,535 elements per list).

### Usage During Parsing

```rust
impl Parser {
    fn parse_if(&mut self) -> ExprId {
        self.expect(Token::If);
        let condition = self.parse_expr();

        self.expect(Token::Then);
        let then_branch = self.parse_expr();

        let else_branch = if self.check(Token::Else) {
            self.advance();
            Some(self.parse_expr())
        } else {
            None
        };

        // Allocate the If expression
        self.arena.alloc(Expr {
            kind: ExprKind::If {
                condition,
                then_branch,
                else_branch,
            },
            span: self.current_span(),
        })
    }
}
```

## Memory Characteristics

### Allocation

- **O(1) amortized** - Just a vector push
- **No fragmentation** - All expressions contiguous
- **No individual deallocation** - Arena freed all at once

### Access

- **O(1) lookup** - Direct index into vector
- **Good cache locality** - Sequential traversal is fast
- **No pointer chasing** - IDs are indices, not pointers

### Memory Overhead

Per expression:
- `ExprKind`: Variable, typically 16-48 bytes
- `Span`: 8 bytes
- No allocation overhead (compared to ~16 bytes for Box)

## Comparison with Alternatives

### Box<Expr>

```rust
// Box-based
struct Expr {
    kind: ExprKind,
}
enum ExprKind {
    Binary { left: Box<Expr>, right: Box<Expr> },
}
```

Pros: Simpler code, no arena parameter
Cons: Heap allocation per expr, poor locality, not Salsa-compatible

### Typed Arenas (typed-arena crate)

```rust
// typed-arena based
let arena = Arena::new();
let expr: &Expr = arena.alloc(Expr { ... });
```

Pros: Returns references, familiar API
Cons: References have lifetimes, harder with Salsa

### Our Approach (ID-based)

```rust
// ID-based
let id: ExprId = arena.alloc(Expr { ... });
let expr: &Expr = arena.get(id);
```

Pros: IDs are Copy, Salsa-compatible, explicit ownership
Cons: Must pass arena around, indirect access

## Best Practices

### 1. Prefer ExprId Over References

```rust
// Good: Store IDs
struct Function {
    body: ExprId,
}

// Avoid: Store references (lifetime issues)
struct Function<'a> {
    body: &'a Expr,
}
```

### 2. Pass Arena Explicitly

```rust
// Good: Explicit arena parameter
fn eval(arena: &ExprArena, id: ExprId) -> Value

// Avoid: Global arena (testing issues)
fn eval(id: ExprId) -> Value
```

### 3. Allocate Bottom-Up

When building expressions, allocate children first:

```rust
// Correct order for `1 + 2`
let one = arena.alloc(literal(1));     // ID 0
let two = arena.alloc(literal(2));     // ID 1
let add = arena.alloc(binary(one, two)); // ID 2

// Children (0, 1) allocated before parent (2)
```

### 4. Use INVALID Sparingly

```rust
// Use for temporary placeholders only
let placeholder = ExprId::INVALID;
// Then fill in real value later
```

## Parallel Arrays

The arena pattern enables parallel arrays for additional data:

```rust
struct TypedModule {
    // Types parallel to ExprArena
    expr_types: Vec<Type>,
}

// Access type for expression
let ty = &typed.expr_types[expr_id.index()];
```

This keeps the AST immutable while allowing separate type information.

## Iteration Patterns

### Iterate All Expressions

```rust
for (id, expr) in arena.iter() {
    println!("{:?}: {:?}", id, expr.kind);
}
```

### Recursive Traversal

```rust
fn visit(arena: &ExprArena, id: ExprId) {
    let expr = arena.get(id);
    match &expr.kind {
        ExprKind::Binary { left, right, .. } => {
            visit(arena, *left);
            visit(arena, *right);
        }
        // ...
    }
}
```

### Collecting IDs

```rust
fn collect_literals(arena: &ExprArena, id: ExprId) -> Vec<ExprId> {
    let mut result = Vec::new();
    collect_literals_inner(arena, id, &mut result);
    result
}
```
