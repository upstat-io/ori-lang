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
}
```

Type IDs (`Idx`) are assigned by the `Pool` before registration — `TypeRegistry` stores entries under caller-provided indices, it does not allocate IDs itself.

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

### Visibility

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Visibility {
    #[default]
    Public,   // Visible everywhere
    Private,  // Visible only within the defining module
}
```

The default is `Public`. Fields, types, and methods can each carry their own `Visibility`.

### TypeKind

```rust
pub enum TypeKind {
    Struct(StructDef),
    Enum { variants: Vec<VariantDef> },
    Newtype { underlying: Idx },
    Alias { target: Idx },
}
```

**`StructDef`** contains `Vec<FieldDef>` where each field has a name, type `Idx`, span, and visibility. It also carries a `category: ValueCategory` field (defaulting to `Value`), reserved for future inline type support where small structs can be passed by value without heap allocation.

**`VariantDef`** has a name and `VariantFields`:

```rust
pub enum VariantFields {
    Unit,                         // Variant with no data
    Tuple(Vec<Idx>),              // Variant(int, str)
    Record(Vec<FieldDef>),        // Variant(x: int, y: str)
}
```

### Registration

Types are registered during Pass 0 of module checking. The caller provides the `Idx` (created in the pool before registration), and the registry stores the entry under both name and index:

```rust
impl TypeRegistry {
    pub fn register_struct(&mut self, name: Name, idx: Idx, type_params: Vec<Name>,
                           fields: Vec<FieldDef>, span: Span, visibility: Visibility);
    pub fn register_enum(&mut self, name: Name, idx: Idx, type_params: Vec<Name>,
                         variants: Vec<VariantDef>, span: Span, visibility: Visibility);
    pub fn register_newtype(&mut self, name: Name, idx: Idx, type_params: Vec<Name>,
                            underlying: Idx, span: Span, visibility: Visibility);
    pub fn register_alias(&mut self, name: Name, idx: Idx, type_params: Vec<Name>,
                          target: Idx, span: Span, visibility: Visibility);
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
    pub super_traits: Vec<Idx>,
    pub object_safety_violations: Vec<ObjectSafetyViolation>,
    pub span: Span,
}
```

**`super_traits`** lists the trait's supertrait `Idx` values (e.g., `Comparable` has `Eq` as a supertrait). `TraitRegistry::all_super_traits()` performs a transitive BFS walk to collect the full supertrait closure, used for verifying that an impl satisfies all inherited obligations.

**`object_safety_violations`** records why a trait cannot be used as a trait object. The `ObjectSafetyViolation` enum has three variants:

```rust
pub enum ObjectSafetyViolation {
    SelfReturn { method: Name, span: Span },   // Method returns Self
    SelfParam { method: Name, span: Span },     // Method takes Self as non-receiver param
    GenericMethod { method: Name, span: Span }, // Method has its own type parameters
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

The `MethodRegistry` delegates trait-based method lookup to `TraitRegistry`. Built-in method resolution is handled separately by `resolve_builtin_method()` in the `infer/expr/methods.rs` module, which dispatches on type tag and method name. The registry is reserved for future unification of inherent impls and trait impls with built-in methods into a single lookup path.

```rust
#[derive(Clone, Debug, Default)]
pub struct MethodRegistry;

impl MethodRegistry {
    pub fn lookup_trait_method<'a>(
        &self,
        receiver_ty: Idx,
        method_name: Name,
        trait_registry: &'a TraitRegistry,
    ) -> Option<MethodLookup<'a>>;
}
```

### Built-in Method Resolution

Built-in methods for primitive types (str, list, map, etc.) are resolved via direct function dispatch in `infer/expr/methods.rs`, not through the `MethodRegistry`. The manifest of all built-in methods is the `TYPECK_BUILTIN_METHODS` constant array (sorted by type and method name):

```rust
// Source of truth: ori_types/src/infer/expr/methods.rs
pub const TYPECK_BUILTIN_METHODS: &[(&str, &str)] = &[
    ("Iterator", "all"),
    ("Iterator", "any"),
    // ... ~100+ entries, sorted alphabetically by (type, method)
    ("str", "trim"),
];
```

Per-type resolver functions handle the actual type inference for each method:
- `resolve_list_method()` — `[T]` methods
- `resolve_str_method()` — `str` methods
- `resolve_map_method()` — `{K: V}` methods
- `resolve_option_method()` — `Option<T>` methods
- `resolve_result_method()` — `Result<T, E>` methods
- `resolve_iterator_method()` — `Iterator<T>` methods

### Method Resolution Order

Resolution order:
1. **Built-in methods** — Direct dispatch via `resolve_builtin_method()` (matches on type tag + method name)
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
