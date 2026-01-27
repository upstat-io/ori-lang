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
    exprs: Vec<Expr>,
}
```

The `Vec<Expr>` is the arena. Expressions are allocated by pushing to the vector and returning an index.

## Implementation

### ExprId

```rust
/// Index into ExprArena
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct ExprId(pub u32);

impl ExprId {
    pub const DUMMY: ExprId = ExprId(u32::MAX);

    pub fn index(self) -> usize {
        self.0 as usize
    }
}
```

`ExprId::DUMMY` is used as a placeholder during parsing when the actual expression isn't known yet.

### ExprArena

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct ExprArena {
    exprs: Vec<Expr>,
}

impl ExprArena {
    pub fn new() -> Self {
        Self { exprs: Vec::new() }
    }

    /// Allocate an expression, returning its ID
    pub fn alloc(&mut self, expr: Expr) -> ExprId {
        let id = ExprId(self.exprs.len() as u32);
        self.exprs.push(expr);
        id
    }

    /// Get expression by ID
    pub fn get(&self, id: ExprId) -> &Expr {
        &self.exprs[id.index()]
    }

    /// Get mutable expression by ID
    pub fn get_mut(&mut self, id: ExprId) -> &mut Expr {
        &mut self.exprs[id.index()]
    }

    /// Number of expressions
    pub fn len(&self) -> usize {
        self.exprs.len()
    }

    /// Iterate over all expressions
    pub fn iter(&self) -> impl Iterator<Item = (ExprId, &Expr)> {
        self.exprs
            .iter()
            .enumerate()
            .map(|(i, e)| (ExprId(i as u32), e))
    }
}
```

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

### 4. Use DUMMY Sparingly

```rust
// Use for temporary placeholders only
let placeholder = ExprId::DUMMY;
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
