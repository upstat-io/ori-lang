---
title: "Intermediate Representation Overview"
description: "Ori Compiler Design — Intermediate Representation Overview"
order: 200
section: "Intermediate Representation"
---

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
├── lib.rs              # Module exports, static_assert_size! macro
├── ast/                # Expression and statement types
│   ├── mod.rs              # Module re-exports
│   ├── expr.rs             # ExprKind variants
│   ├── stmt.rs             # Statement types
│   ├── operators.rs        # Operator enums
│   ├── ranges.rs           # Range types for arena allocation
│   ├── collections.rs      # Collection literals
│   ├── items/              # Top-level item definitions
│   │   ├── function.rs         # Function, TestDef
│   │   ├── imports.rs          # UseDef, ImportPath
│   │   ├── types.rs            # TypeDecl, TypeDeclKind
│   │   └── traits.rs          # TraitDef, ImplDef, ExtendDef
│   └── patterns/           # Pattern constructs
│       ├── seq.rs              # FunctionSeq (run, try, match)
│       ├── exp.rs              # FunctionExp (recurse, parallel, etc.)
│       └── binding.rs          # Match patterns and arms
├── canon/              # Canonical IR (sugar-free, type-annotated)
│   ├── mod.rs              # CanExpr, CanId, CanRange
│   └── tree.rs             # DecisionTree, FlatPattern, PatternMatrix
├── pattern_resolution.rs # PatternKey, PatternResolution (type-checker → evaluator bridge)
├── arena.rs            # Expression arena
├── builtin_constants.rs # Built-in constant definitions
├── builtin_type.rs     # Built-in type definitions
├── builtin_methods.rs  # Built-in method definitions
├── expr_id.rs          # ExprId, StmtId, ParsedTypeId, MatchPatternId
├── type_id.rs          # TypeId (flat u32 index, no sharding)
├── name.rs             # Name interning
├── parsed_type.rs      # ParsedType for type annotations
├── derives.rs          # DerivedTrait, DerivedMethodInfo
├── token.rs            # Token definitions
├── visitor.rs          # AST visitor pattern
├── interner.rs         # String interning (16-shard RwLock)
├── comment.rs          # Comment handling
├── metadata.rs         # ModuleExtra for formatter/IDE metadata
├── incremental.rs      # Incremental parsing support
├── traits.rs           # Spanned, Named, Typed traits
└── span.rs             # Source location tracking
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

### 3. Expression Ranges

All expression lists use `ExprRange` (8 bytes), a compact range type pointing into the arena's side tables:

```rust
/// Range into an arena side table.
pub struct ExprRange {
    start: u32,
    len: u16,
}
```

**Memory layout:** 8 bytes total. The `define_range!` macro generates `.new()`, `.is_empty()`, `.len()`, and `EMPTY` for all range types.

### 4. String Interning

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

### 5. TypeId in ori_ir

The `ori_ir` crate contains only `TypeId`, a flat `u32` index used as the parser-level type representation. It has ~14 pre-interned constants for primitive types:

```rust
#[repr(transparent)]
pub struct TypeId(u32);

impl TypeId {
    pub const INT: TypeId = TypeId(0);
    pub const FLOAT: TypeId = TypeId(1);
    pub const BOOL: TypeId = TypeId(2);
    pub const STR: TypeId = TypeId(3);
    // ... through ORDERING = 11

    pub const INFER: TypeId = TypeId(12);     // Placeholder during inference
    pub const SELF_TYPE: TypeId = TypeId(13); // Self type in trait/impl contexts

    pub const FIRST_COMPOUND: u32 = 64;      // First dynamically allocated compound type
}
```

The actual `Type` enum, `TypeData`, type interning, `StructType`, `EnumType`, and `TypeRegistry` all live in the `ori_types` crate, not in `ori_ir`. See [Type Representation](type-representation.md) for details.

### 6. Canonical IR

The `canon/` module contains the canonical IR (`CanExpr`), a sugar-free, type-annotated intermediate representation consumed by both the interpreter (`ori_eval`) and the ARC/LLVM backend (`ori_arc`):

- `canon/mod.rs` — `CanExpr`, `CanId`, `CanRange` (distinct index space from `ExprId`/`ExprRange`)
- `canon/tree.rs` — `DecisionTree`, `FlatPattern`, `PatternMatrix` for compiled pattern matching

### 7. Pattern Resolution

The `pattern_resolution.rs` module provides types that bridge the type checker and evaluator:

- `PatternKey` — identifies a match pattern in the AST (either a top-level arm or nested pattern)
- `PatternResolution` — type-checker resolution of ambiguous `Binding` patterns (e.g., resolving `Pending` to a unit variant)

These live in `ori_ir` because both `ori_types` and `ori_eval` need them.

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
| `Stmt` | `StmtId(u32)` | `ExprArena` (stmt_list) |
| `ParsedType` | `ParsedTypeId(u32)` | `ExprArena` (type_list) |
| `MatchPattern` | `MatchPatternId(u32)` | `ExprArena` (pattern_list) |
| `String` | `Name(u32)` | `StringInterner` |
| `Type` | `TypeId(u32)` | `ori_types` type pool |
| `TypeVar` | `TypeVar(u32)` | `InferenceContext` |

All ID types use `INVALID = u32::MAX` as their sentinel value:
```rust
impl ExprId {
    pub const INVALID: ExprId = ExprId(u32::MAX);
    pub fn is_valid(self) -> bool { self.0 != u32::MAX }
}
```

Benefits:
- O(1) comparison (compare IDs, not contents)
- Memory sharing (same ID = same content)
- Salsa-friendly (IDs are hashable)

### TypeId Layout

TypeId is a flat `u32` index with no sharding. Primitive types have fixed indices that match `ori_types::Idx`:

| Index | Constant | Description |
|-------|----------|-------------|
| 0 | `INT` | Signed integer (range: [-2⁶³, 2⁶³-1]) |
| 1 | `FLOAT` | IEEE 754 double-precision |
| 2 | `BOOL` | Boolean |
| 3 | `STR` | UTF-8 string |
| 4 | `CHAR` | Unicode scalar value |
| 5 | `BYTE` | Unsigned integer (range: [0, 255]) |
| 6 | `UNIT` (alias: `VOID`) | Unit type |
| 7 | `NEVER` | Bottom type |
| 8 | `ERROR` | Error placeholder |
| 9 | `DURATION` | Duration (nanoseconds) |
| 10 | `SIZE` | Size (bytes/count) |
| 11 | `ORDERING` | Less / Equal / Greater |
| 12 | `INFER` | Inference placeholder (not stored in type pool) |
| 13 | `SELF_TYPE` | Self type marker (not stored in type pool) |
| 64+ | `FIRST_COMPOUND` | Dynamically allocated compound types |

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

These types live in `ori_ir` (rather than `ori_typeck` or `ori_eval`) to avoid circular dependencies---both the type checker and evaluator need these definitions, and `ori_ir` has no dependencies.

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
| `Type` | 40 bytes | Vec<Type> + Box<Type> for Function variant |
| `TypeVar` | 4 bytes | Just a u32 wrapper |

If any of these sizes change, compilation fails with a clear error message, allowing intentional review of the change.

## Related Documents

- [Flat AST](flat-ast.md) - Why we avoid boxing
- [Arena Allocation](arena-allocation.md) - How expressions are stored
- [String Interning](string-interning.md) - Identifier deduplication
- [Type Representation](type-representation.md) - How types are encoded
