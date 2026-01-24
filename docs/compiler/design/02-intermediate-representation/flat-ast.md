# Flat AST Design

The Sigil compiler uses a "flat" AST where expressions are stored in an arena and referenced by ID, rather than using traditional heap-allocated tree structures.

## Traditional vs Flat AST

### Traditional AST (Box-based)

```rust
// Traditional recursive structure
enum Expr {
    Literal(Literal),
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Call {
        func: Box<Expr>,
        args: Vec<Box<Expr>>,
    },
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
}
```

Problems:
- Each expression requires heap allocation
- Poor cache locality (expressions scattered in memory)
- Deep trees can cause stack overflow during traversal
- Difficult to serialize for Salsa caching

### Flat AST (Arena-based)

```rust
// Flat structure with arena allocation
struct Expr {
    kind: ExprKind,
    span: Span,
}

#[derive(Clone, Copy)]
struct ExprId(u32);

enum ExprKind {
    Literal(Literal),
    Binary {
        left: ExprId,
        op: BinaryOp,
        right: ExprId,
    },
    Call {
        func: ExprId,
        args: Vec<ExprId>,
    },
    If {
        condition: ExprId,
        then_branch: ExprId,
        else_branch: Option<ExprId>,
    },
}

struct ExprArena {
    exprs: Vec<Expr>,
}
```

Benefits:
- All expressions in contiguous memory
- Excellent cache locality
- No heap allocation per expression
- ExprId is Copy, cheap to pass around
- Easy to serialize (just a Vec)

## Building the AST

During parsing, expressions are allocated into the arena:

```rust
impl Parser {
    fn parse_binary(&mut self) -> ExprId {
        let left = self.parse_unary();

        if self.check_operator() {
            let op = self.advance_operator();
            let right = self.parse_binary();

            // Allocate into arena, get back an ID
            self.arena.alloc(Expr {
                kind: ExprKind::Binary { left, op, right },
                span: self.span_from(left),
            })
        } else {
            left
        }
    }
}
```

## Accessing Expressions

To access an expression by ID:

```rust
impl ExprArena {
    pub fn get(&self, id: ExprId) -> &Expr {
        &self.exprs[id.0 as usize]
    }

    pub fn get_mut(&mut self, id: ExprId) -> &mut Expr {
        &mut self.exprs[id.0 as usize]
    }
}

// Usage
let expr = arena.get(some_id);
match &expr.kind {
    ExprKind::Binary { left, op, right } => {
        let left_expr = arena.get(*left);
        let right_expr = arena.get(*right);
        // ...
    }
    // ...
}
```

## Traversal

Traversal requires passing the arena along:

```rust
fn eval_expr(arena: &ExprArena, id: ExprId, env: &mut Environment) -> Value {
    let expr = arena.get(id);
    match &expr.kind {
        ExprKind::Literal(lit) => Value::from_literal(lit),

        ExprKind::Binary { left, op, right } => {
            let left_val = eval_expr(arena, *left, env);
            let right_val = eval_expr(arena, *right, env);
            apply_op(op, left_val, right_val)
        }

        ExprKind::If { condition, then_branch, else_branch } => {
            let cond_val = eval_expr(arena, *condition, env);
            if cond_val.is_truthy() {
                eval_expr(arena, *then_branch, env)
            } else if let Some(else_id) = else_branch {
                eval_expr(arena, *else_id, env)
            } else {
                Value::Void
            }
        }
        // ...
    }
}
```

## Type Annotations

Type information is stored parallel to the arena:

```rust
struct TypedModule {
    // expr_types[expr_id] = type of that expression
    expr_types: Vec<Type>,
}

// Access type by ExprId
let ty = &typed_module.expr_types[expr_id.0 as usize];
```

This parallel array pattern:
- Keeps AST immutable after parsing
- Allows type info to be computed separately
- Enables Salsa caching of type results

## ExprId Properties

`ExprId` is designed for efficiency:

```rust
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct ExprId(pub u32);
```

- `Copy` - No overhead passing around
- `Eq, Hash` - Can be used in HashMaps
- Small (4 bytes vs 8 bytes for Box on 64-bit)
- Salsa-compatible

## Memory Layout

Example memory layout for `1 + 2 * 3`:

```
ExprArenaexprs:
┌─────────────────────────────────────────────────────────────┐
│ [0] Literal(1) │ [1] Literal(2) │ [2] Literal(3) │ ...    │
│ [3] Binary(*,1,2) │ [4] Binary(+,0,3) │                    │
└─────────────────────────────────────────────────────────────┘

All expressions contiguous in memory!
```

Compare to Box-based:
```
Heap (scattered):
┌───────────┐     ┌───────────┐     ┌───────────┐
│ Literal(1)│     │ Literal(2)│     │ Literal(3)│
└───────────┘     └───────────┘     └───────────┘
      ↑                 ↑                 ↑
      └────────────┬────┴────────┬────────┘
              ┌────┴────┐   ┌────┴────┐
              │ Binary* │   │ Binary+ │
              └─────────┘   └─────────┘

Expressions scattered across heap!
```

## Limitations

1. **Cannot easily delete expressions** - Arena only grows
2. **Requires passing arena everywhere** - Extra parameter
3. **Indirect access** - Must go through arena.get()

These are acceptable tradeoffs for:
- Better memory performance
- Salsa compatibility
- Simpler lifetime management
