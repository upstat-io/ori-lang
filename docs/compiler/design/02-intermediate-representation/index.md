# Intermediate Representation Overview

The Sigil compiler uses a carefully designed intermediate representation (IR) optimized for:

- **Memory efficiency** via arena allocation
- **Fast comparison** via string interning
- **Salsa compatibility** via flat, comparable types
- **Visitor pattern** for AST traversal

## IR Components

```
compiler/sigilc/src/ir/
├── mod.rs          # Module exports
├── ast/            # Expression and statement types (~1,570 lines total)
│   ├── mod.rs          # Module re-exports (~110 lines)
│   ├── expr.rs         # ExprKind variants (~364 lines)
│   ├── stmt.rs         # Statement types (~51 lines)
│   ├── operators.rs    # Operator enums (~51 lines)
│   ├── ranges.rs       # Range types for arena allocation (~273 lines)
│   ├── collections.rs  # Collection literals (~53 lines)
│   ├── items/          # Top-level item definitions
│   │   ├── mod.rs          # Re-exports (~15 lines)
│   │   ├── function.rs     # Function, TestDef (~141 lines)
│   │   ├── imports.rs      # UseDef, ImportPath (~39 lines)
│   │   └── traits.rs       # TraitDef, ImplDef, ExtendDef (~205 lines)
│   └── patterns/       # Pattern constructs
│       ├── mod.rs          # Re-exports (~11 lines)
│       ├── seq.rs          # FunctionSeq (run, try, match) (~102 lines)
│       ├── exp.rs          # FunctionExp (map, filter, etc.) (~72 lines)
│       └── binding.rs      # Match patterns and arms (~83 lines)
├── arena.rs        # Expression arena (~475 lines)
├── token.rs        # Token definitions (~690 lines)
├── visitor.rs      # AST visitor pattern (~1,230 lines)
├── interner.rs     # String interning (~260 lines)
└── span.rs         # Source location tracking
```

## Key Design Decisions

### 1. Flat AST (No Boxing)

Traditional ASTs use heap allocation:

```rust
// Traditional (heap-allocated)
enum Expr {
    Binary { left: Box<Expr>, op: Op, right: Box<Expr> },
    Call { func: Box<Expr>, args: Vec<Box<Expr>> },
}
```

Sigil uses arena allocation:

```rust
// Sigil (arena-allocated)
struct Expr {
    kind: ExprKind,
    span: Span,
}

enum ExprKind {
    Binary { left: ExprId, op: Op, right: ExprId },
    Call { func: ExprId, args: Vec<ExprId> },
}

// ExprId is just a u32 index into ExprArena
struct ExprId(u32);
```

See [Flat AST](flat-ast.md) for details.

### 2. Arena Allocation

All expressions live in a contiguous `Vec<Expr>`:

```rust
struct ExprArena {
    exprs: Vec<Expr>,
}

impl ExprArena {
    fn alloc(&mut self, expr: Expr) -> ExprId {
        let id = ExprId(self.exprs.len() as u32);
        self.exprs.push(expr);
        id
    }

    fn get(&self, id: ExprId) -> &Expr {
        &self.exprs[id.0 as usize]
    }
}
```

See [Arena Allocation](arena-allocation.md) for details.

### 3. String Interning

All identifiers are interned to u32 indices:

```rust
struct Name(u32);

struct Interner {
    strings: Vec<String>,
    lookup: HashMap<String, Name>,
}

impl Interner {
    fn intern(&mut self, s: &str) -> Name { ... }
    fn resolve(&self, name: Name) -> &str { ... }
}
```

See [String Interning](string-interning.md) for details.

### 4. Type Representation

Types are represented as an enum:

```rust
enum Type {
    Int,
    Float,
    Bool,
    String,
    Char,
    Void,
    List(Box<Type>),
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),
    Function { params: Vec<Type>, ret: Box<Type> },
    TypeVar(TypeVarId),
    Named(Name),
    // ...
}
```

See [Type Representation](type-representation.md) for details.

## Salsa Compatibility

All IR types derive the traits required by Salsa:

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Module { ... }

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ExprArena { ... }

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TokenList { ... }
```

This enables:
- Memoization of query results
- Early cutoff when outputs unchanged
- Dependency tracking across queries

## Visitor Pattern

The IR includes a visitor pattern for AST traversal:

```rust
pub trait Visitor {
    fn visit_expr(&mut self, expr: &Expr, arena: &ExprArena);
    fn visit_function(&mut self, func: &Function);
    fn visit_type(&mut self, ty: &TypeDef);
    // ...
}

pub fn walk_module(visitor: &mut impl Visitor, module: &Module, arena: &ExprArena) {
    for func in &module.functions {
        visitor.visit_function(func);
        walk_expr(visitor, func.body, arena);
    }
    // ...
}
```

Used for:
- Type checking traversal
- Pretty printing
- Code analysis

## Type IDs

Several types use ID patterns for indirection:

| Type | ID | Storage |
|------|-----|---------|
| `Expr` | `ExprId(u32)` | `ExprArena` |
| `String` | `Name(u32)` | `Interner` |
| `TypeVar` | `TypeVarId(u32)` | `TypeChecker` |

Benefits:
- O(1) comparison (compare IDs, not contents)
- Memory sharing (same ID = same content)
- Salsa-friendly (IDs are hashable)

## Size Assertions

To prevent accidental size regressions in frequently-allocated types, the compiler uses compile-time size assertions:

```rust
// In lib.rs
#[macro_export]
macro_rules! static_assert_size {
    ($ty:ty, $size:expr) => {
        const _: [(); $size] = [(); ::std::mem::size_of::<$ty>()];
    };
}

// In type files
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    crate::static_assert_size!(Span, 8);
    crate::static_assert_size!(Token, 24);
    crate::static_assert_size!(TokenKind, 16);
    crate::static_assert_size!(Expr, 88);
    crate::static_assert_size!(ExprKind, 80);
    crate::static_assert_size!(Type, 32);
}
```

Current sizes (64-bit):

| Type | Size | Notes |
|------|------|-------|
| `Span` | 8 bytes | Two u32 offsets |
| `Token` | 24 bytes | TokenKind + Span |
| `TokenKind` | 16 bytes | Largest variant payload + discriminant |
| `Expr` | 88 bytes | ExprKind + Span |
| `ExprKind` | 80 bytes | Largest variants are FunctionSeq/FunctionExp |
| `Type` | 32 bytes | Vec<Type> + Box<Type> for Function variant |
| `TypeVar` | 4 bytes | Just a u32 wrapper |

If any of these sizes change, compilation fails with a clear error message, allowing intentional review of the change.

## Related Documents

- [Flat AST](flat-ast.md) - Why we avoid boxing
- [Arena Allocation](arena-allocation.md) - How expressions are stored
- [String Interning](string-interning.md) - Identifier deduplication
- [Type Representation](type-representation.md) - How types are encoded
