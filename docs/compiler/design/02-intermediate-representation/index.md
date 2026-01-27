# Intermediate Representation Overview

The Ori compiler uses a carefully designed intermediate representation (IR) optimized for:

- **Memory efficiency** via arena allocation
- **Fast comparison** via string interning
- **Salsa compatibility** via flat, comparable types
- **Visitor pattern** for AST traversal

## IR Components

The IR lives in its own crate `ori_ir`, which has no dependencies and is used by all other compiler crates.

```
compiler/ori_ir/src/
├── lib.rs          # Module exports, static_assert_size! macro
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
├── derives.rs      # DerivedTrait, DerivedMethodInfo (~103 lines)
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

Ori uses arena allocation:

```rust
// Ori (arena-allocated)
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

Types have a dual representation for different use cases:

**External API (`Type`)** - Uses `Box<Type>` for recursive types:

```rust
enum Type {
    Int, Float, Bool, String, Char, Void,
    List(Box<Type>),
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),
    Function { params: Vec<Type>, ret: Box<Type> },
    TypeVar(TypeVar),
    Named(Name),
    // ...
}
```

**Internal Representation (`TypeData`)** - Uses `TypeId` for O(1) equality:

```rust
enum TypeData {
    Int, Float, Bool, Str, Char, Byte, Unit, Never,
    List(TypeId),
    Option(TypeId),
    Result { ok: TypeId, err: TypeId },
    Function { params: Box<[TypeId]>, ret: TypeId },
    Var(TypeVar),
    Named(Name),
    // ...
}
```

See [Type Representation](type-representation.md) for details.

### 5. Type Interning

Types are interned for O(1) equality comparison using `TypeInterner`:

```rust
pub struct TypeInterner {
    shards: [RwLock<TypeShard>; 16],  // 16 shards for concurrent access
    next_var: AtomicU32,
}

impl TypeInterner {
    pub fn intern(&self, data: TypeData) -> TypeId;    // Intern a type
    pub fn lookup(&self, id: TypeId) -> TypeData;      // Look up by ID
    pub fn to_type(&self, id: TypeId) -> Type;         // Convert to external Type
}
```

**Sharded Layout:**
- TypeId is 32 bits: 4 bits shard index + 28 bits local index
- Supports up to 268 million types per shard (16 shards total)
- Primitives are pre-interned in shard 0 with fixed indices

```rust
// TypeId layout: [shard:4][local:28]
impl TypeId {
    pub const INT: TypeId = TypeId(0);    // Shard 0, local 0
    pub const FLOAT: TypeId = TypeId(1);  // Shard 0, local 1
    // ...

    pub fn shard(self) -> usize { (self.0 >> 28) as usize }
    pub fn local(self) -> usize { (self.0 & 0x0FFF_FFFF) as usize }
}
```

Benefits:
- O(1) type equality (compare TypeIds, not structure)
- Pre-interned primitives for fast primitive checks
- Concurrent access via sharded locking
- Memory sharing (same type = same TypeId)

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
| `String` | `Name(u32)` | `StringInterner` |
| `Type` | `TypeId(u32)` | `TypeInterner` |
| `TypeVar` | `TypeVar(u32)` | `InferenceContext` |

Benefits:
- O(1) comparison (compare IDs, not contents)
- Memory sharing (same ID = same content)
- Salsa-friendly (IDs are hashable)

### TypeId Layout

TypeId uses a sharded layout for concurrent type interning:

```
Bits: [31-28: shard][27-0: local index]
```

| Field | Bits | Range |
|-------|------|-------|
| Shard | 4 | 0-15 |
| Local | 28 | 0-268,435,455 |

Pre-interned primitives (shard 0):
- `TypeId::INT` = 0, `TypeId::FLOAT` = 1, `TypeId::BOOL` = 2
- `TypeId::STR` = 3, `TypeId::CHAR` = 4, `TypeId::BYTE` = 5
- `TypeId::VOID` = 6, `TypeId::NEVER` = 7
- `TypeId::INFER` = 8, `TypeId::SELF_TYPE` = 9

## Derived Trait Definitions

The `derives.rs` module contains types used by both the type checker and evaluator for `#[derive(...)]` support:

```rust
/// A derived trait that can be auto-implemented.
pub enum DerivedTrait {
    Eq,        // Structural equality
    Clone,     // Field-by-field cloning
    Hashable,  // Hash computation
    Printable, // String representation
    Default,   // Default value construction
}

/// Information about a derived method.
pub struct DerivedMethodInfo {
    pub trait_kind: DerivedTrait,
    pub field_names: Vec<Name>,  // Struct fields (in order)
}
```

These types live in `ori_ir` (rather than `ori_typeck` or `ori_eval`) to avoid circular dependencies—both the type checker and evaluator need these definitions, and `ori_ir` has no dependencies.

## Size Assertions

To prevent accidental size regressions in frequently-allocated types, the compiler uses compile-time size assertions. The `static_assert_size!` macro is defined in `ori_ir` and used across all crates:

```rust
// In ori_ir/src/lib.rs
#[macro_export]
macro_rules! static_assert_size {
    ($ty:ty, $size:expr) => {
        const _: [(); $size] = [(); ::std::mem::size_of::<$ty>()];
    };
}

// In ori_ir type files
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    ori_ir::static_assert_size!(Span, 8);
    ori_ir::static_assert_size!(Token, 24);
    ori_ir::static_assert_size!(TokenKind, 16);
    ori_ir::static_assert_size!(Expr, 88);
    ori_ir::static_assert_size!(ExprKind, 80);
}

// In ori_types
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    ori_ir::static_assert_size!(Type, 32);
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
