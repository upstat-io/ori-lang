---
title: "Type Registry"
description: "Ori Compiler Design — Type Registry"
order: 503
section: "Type System"
---

# Type Registry

The type registry system stores user-defined types (structs, enums, newtypes), trait definitions and implementations, and built-in method declarations. Three registries work together: `TypeRegistry`, `TraitRegistry`, and `MethodRegistry`.

## Location

```
compiler/ori_types/src/registry/
├── mod.rs       # Re-exports
├── types.rs     # TypeRegistry — struct/enum/newtype storage
├── traits.rs    # TraitRegistry — trait definitions and implementations
└── methods.rs   # MethodRegistry — built-in method resolution
```

## TypeRegistry

The `TypeRegistry` stores user-defined type definitions and provides lookup by name, by `Idx`, and by variant name.

```rust
pub struct TypeRegistry {
    types_by_name: BTreeMap<Name, TypeEntry>,     // Deterministic iteration order
    types_by_idx: FxHashMap<Idx, TypeEntry>,       // Fast lookup by type Idx
    variants_by_name: FxHashMap<Name, (Idx, usize)>,  // Variant → (enum Idx, variant index)
    next_internal_id: u32,
}
```

`BTreeMap` is used for `types_by_name` to ensure deterministic iteration order, which matters for error messages and test stability.

### TypeEntry

```rust
pub struct TypeEntry {
    pub name: Name,
    pub idx: Idx,
    pub kind: TypeKind,
    pub span: Span,
    pub type_params: Vec<Name>,
    pub visibility: Visibility,
}
```

### TypeKind

```rust
pub enum TypeKind {
    Struct(StructDef),
    Enum { variants: Vec<VariantDef> },
    Newtype { underlying: Idx },
    Alias { target: Idx },
}
```

**`StructDef`** contains `Vec<FieldDef>` where each field has a name, type `Idx`, span, and visibility.

**`VariantDef`** has a name and `VariantFields`:

```rust
pub enum VariantFields {
    Unit,                         // Variant with no data
    Tuple(Vec<Idx>),              // Variant(int, str)
    Record(Vec<FieldDef>),        // Variant(x: int, y: str)
}
```

### Registration

Types are registered during Pass 0 of module checking. Each registration generates a unique `Idx` in the pool:

```rust
impl TypeRegistry {
    pub fn register_struct(&mut self, name: Name, fields: Vec<FieldDef>,
                           span: Span, type_params: Vec<Name>) -> Idx;
    pub fn register_enum(&mut self, name: Name, variants: Vec<VariantDef>,
                         span: Span, type_params: Vec<Name>) -> Idx;
    pub fn register_newtype(&mut self, name: Name, underlying: Idx,
                            span: Span, type_params: Vec<Name>) -> Idx;
}
```

### Lookup

```rust
impl TypeRegistry {
    pub fn get_by_name(&self, name: Name) -> Option<&TypeEntry>;
    pub fn get_by_idx(&self, idx: Idx) -> Option<&TypeEntry>;
    pub fn contains(&self, name: Name) -> bool;
}
```

#### Variant Lookup

Variants are indexed by name for O(1) constructor resolution:

```rust
impl TypeRegistry {
    /// Look up which enum a variant belongs to.
    /// Returns (enum Idx, variant index within the enum).
    pub fn lookup_variant(&self, variant_name: Name) -> Option<(Idx, usize)> {
        self.variants_by_name.get(&variant_name).copied()
    }
}
```

### Nominal Typing

All user-defined types have **nominal identity** — they are distinct from their underlying representation:

```ori
type UserId = str    // newtype: UserId ≠ str
type Point = { x: int, y: int }  // struct: Point ≠ (int, int)
```

`UserId` and `str` are different types in the type system. To access the inner value, use `.unwrap()`.

## TraitRegistry

The `TraitRegistry` stores trait definitions and their implementations.

```rust
pub struct TraitRegistry {
    traits_by_name: BTreeMap<Name, TraitEntry>,
    traits_by_idx: FxHashMap<Idx, TraitEntry>,
    impls: Vec<ImplEntry>,
    impls_by_type: FxHashMap<Idx, Vec<usize>>,    // type → impl indices
    impls_by_trait: FxHashMap<Idx, Vec<usize>>,   // trait → impl indices
}
```

### TraitEntry

```rust
pub struct TraitEntry {
    pub name: Name,
    pub idx: Idx,
    pub type_params: Vec<Name>,
    pub methods: FxHashMap<Name, TraitMethodDef>,
    pub assoc_types: FxHashMap<Name, TraitAssocTypeDef>,
    pub span: Span,
}
```

### ImplEntry

```rust
pub struct ImplEntry {
    pub trait_idx: Option<Idx>,  // None for inherent impls
    pub self_type: Idx,
    pub type_params: Vec<Name>,
    pub methods: FxHashMap<Name, ImplMethodDef>,
    pub assoc_types: FxHashMap<Name, Idx>,
    pub where_clause: Vec<WhereConstraint>,
    pub span: Span,
}
```

### Default Type Parameters

Traits may have type parameters with defaults:

```ori
trait Add<Rhs = Self> {
    @add (self, rhs: Rhs) -> Self
}
```

Default types are stored as `ParsedType` rather than `Idx` because `Self` must be resolved at impl registration time, not at trait definition time.

### Default Associated Types

```ori
trait Add<Rhs = Self> {
    type Output = Self
    @add (self, rhs: Rhs) -> Self.Output
}
```

When an impl omits an associated type that has a default, the default is used with `Self` resolved to the implementing type.

### Impl Lookup

```rust
impl TraitRegistry {
    /// Find all impls for a given type.
    pub fn impls_for_type(&self, idx: Idx) -> &[usize];

    /// Find all impls of a given trait.
    pub fn impls_of_trait(&self, idx: Idx) -> &[usize];

    /// Look up a specific trait impl for a type.
    pub fn get_trait_impl(&self, trait_name: Name, self_type: Idx) -> Option<&ImplEntry>;
}
```

## MethodRegistry

The `MethodRegistry` stores compiler-defined methods for built-in types (str, list, map, etc.) and provides unified method lookup.

```rust
pub struct MethodRegistry {
    builtin: FxHashMap<(Tag, Name), BuiltinMethod>,
    builtin_by_tag: FxHashMap<Tag, Vec<Name>>,
}
```

Methods are keyed by `(Tag, Name)` — the receiver's type tag and the method name — for O(1) lookup.

### BuiltinMethod

```rust
pub struct BuiltinMethod {
    pub name: Name,
    pub receiver_tag: Tag,
    pub doc: &'static str,
    pub kind: BuiltinMethodKind,
}

pub enum BuiltinMethodKind {
    Fixed(Idx),                      // Fixed return type (e.g., len() → int)
    Element,                         // Returns element type (e.g., first() → T?)
    Transform(MethodTransform),      // Transforms receiver type
}
```

`MethodTransform` covers patterns like:
- `Identity` — Returns the same type as receiver
- `WrapOption` — Wraps element in option (e.g., `get()` → `T?`)
- `MapKey` / `MapValue` — Extracts key or value type from maps

### Method Resolution

The unified resolution chain tries methods in priority order:

```rust
pub enum MethodResolution<'a> {
    Builtin(&'a BuiltinMethod),
    Impl(MethodLookup<'a>),
}
```

Resolution order:
1. **Built-in methods** — Checked first via `MethodRegistry` (O(1) lookup by tag + name)
2. **Inherent methods** — `impl Type { ... }` blocks via `TraitRegistry`
3. **Trait methods** — `impl Trait for Type { ... }` blocks via `TraitRegistry`

### Built-in Method Coverage

| Receiver | Example Methods |
|----------|----------------|
| `str` | `len`, `split`, `trim`, `contains`, `starts_with`, `to_upper`, ... |
| `[T]` | `len`, `push`, `pop`, `get`, `map`, `filter`, `fold`, `sort`, ... |
| `{K: V}` | `len`, `get`, `insert`, `remove`, `keys`, `values`, `contains_key`, ... |
| `T?` | `map`, `unwrap_or`, `ok_or`, `and_then`, `is_some`, `is_none`, ... |
| `result<T,E>` | `map`, `map_err`, `unwrap_or`, `ok`, `err`, `is_ok`, ... |
| `int`, `float` | `abs`, `min`, `max`, `clamp`, numeric methods |

## Error Suggestions

The registries support "did you mean?" suggestions via Levenshtein distance:

```rust
impl TypeRegistry {
    pub fn suggest_similar(&self, name: Name) -> Vec<Name> {
        // Search types_by_name for names within edit distance 2
    }
}
```

This provides helpful error messages when users misspell type or variant names.

## Registration During Pass 0

The registration order during Pass 0 ensures dependencies are resolved correctly:

1. **Built-in types** (Ordering, etc.) — Always available
2. **User-defined types** — Struct/enum/newtype definitions
3. **Traits** — Trait definitions with methods and associated types
4. **Implementations** — `impl` blocks linking types to traits
5. **Derived implementations** — Auto-generated from `#derive` attributes
6. **Config variables** — `let $VAR` constant bindings

This ordering ensures that trait implementations can reference user-defined types, and derived implementations can reference trait definitions.
