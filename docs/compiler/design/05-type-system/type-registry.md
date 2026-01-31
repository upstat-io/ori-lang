---
title: "Type Registry"
description: "Ori Compiler Design — Type Registry"
order: 503
section: "Type System"
---

# Type Registry

The TypeRegistry stores user-defined types (structs, enums, newtypes). It enables looking up type definitions by name or `TypeId`.

## Location

```
compiler/ori_typeck/src/registry/
├── mod.rs                    # TypeRegistry struct, TypeKind, TypeEntry, re-exports
├── trait_registry.rs         # TraitRegistry core (method_cache)
├── trait_types.rs            # TraitMethodDef, TraitAssocTypeDef, TraitEntry
├── impl_types.rs             # ImplMethodDef, ImplAssocTypeDef, ImplEntry, CoherenceError
├── method_lookup.rs          # MethodLookup result type
└── tests/                    # Test modules
    ├── mod.rs
    ├── trait_registry_tests.rs
    └── type_registry_tests.rs
```

## Structure

```rust
/// Registry for user-defined types.
pub struct TypeRegistry {
    /// Types indexed by name.
    types_by_name: HashMap<Name, TypeEntry>,
    /// Types indexed by TypeId.
    types_by_id: HashMap<TypeId, TypeEntry>,
    /// Next available TypeId for compound types.
    next_type_id: u32,
    /// Type interner for Type↔TypeId conversions.
    interner: SharedTypeInterner,
}

/// Entry for a user-defined type.
pub struct TypeEntry {
    pub name: Name,
    pub type_id: TypeId,
    pub kind: TypeKind,
    pub span: Span,
    pub type_params: Vec<Name>,
}

/// Kind of user-defined type.
pub enum TypeKind {
    /// Struct type with named fields.
    Struct { fields: Vec<(Name, TypeId)> },
    /// Sum type (enum) with variants.
    Enum { variants: Vec<VariantDef> },
    /// Newtype: nominally distinct wrapper around an existing type.
    Newtype { underlying: TypeId },
}

/// Variant definition for enum types.
pub struct VariantDef {
    pub name: Name,
    /// Variant fields (empty for unit variants, multiple for multi-field variants).
    pub fields: Vec<(Name, TypeId)>,
}
```

## Registration

Types are registered via specific methods that generate unique `TypeId`s:

```rust
impl TypeRegistry {
    /// Register a struct type.
    pub fn register_struct(
        &mut self,
        name: Name,
        fields: Vec<(Name, Type)>,
        span: Span,
        type_params: Vec<Name>,
    ) -> TypeId {
        let field_ids = fields.into_iter()
            .map(|(name, ty)| (name, ty.to_type_id(&self.interner)))
            .collect();
        self.register_entry(name, TypeKind::Struct { fields: field_ids }, span, type_params)
    }

    /// Register an enum type.
    pub fn register_enum(
        &mut self,
        name: Name,
        variants: Vec<(Name, Vec<(Name, Type)>)>,
        span: Span,
        type_params: Vec<Name>,
    ) -> TypeId {
        let variant_defs = variants.into_iter()
            .map(|(vname, vfields)| {
                let field_ids = vfields.into_iter()
                    .map(|(fname, ty)| (fname, ty.to_type_id(&self.interner)))
                    .collect();
                VariantDef { name: vname, fields: field_ids }
            })
            .collect();
        self.register_entry(name, TypeKind::Enum { variants: variant_defs }, span, type_params)
    }

    /// Register a newtype (nominally distinct type wrapper).
    pub fn register_newtype(
        &mut self,
        name: Name,
        underlying: &Type,
        span: Span,
        type_params: Vec<Name>,
    ) -> TypeId {
        let underlying_id = underlying.to_type_id(&self.interner);
        self.register_entry(name, TypeKind::Newtype { underlying: underlying_id }, span, type_params)
    }
}
```

## Lookup

### Type Definition

```rust
impl TypeRegistry {
    /// Look up a type entry by name.
    pub fn get_by_name(&self, name: Name) -> Option<&TypeEntry> {
        self.types_by_name.get(&name)
    }

    /// Look up a type entry by TypeId.
    pub fn get_by_id(&self, type_id: TypeId) -> Option<&TypeEntry> {
        self.types_by_id.get(&type_id)
    }

    /// Check if a type name is already registered.
    pub fn contains(&self, name: Name) -> bool {
        self.types_by_name.contains_key(&name)
    }
}
```

### Field Lookup

```rust
impl TypeRegistry {
    /// Get field types for a struct type.
    /// Returns the fields as (Name, Type) pairs by converting from TypeId.
    pub fn get_struct_fields(&self, type_id: TypeId) -> Option<Vec<(Name, Type)>> {
        self.get_by_id(type_id).and_then(|entry| match &entry.kind {
            TypeKind::Struct { fields } => Some(
                fields.iter()
                    .map(|(name, ty_id)| (*name, self.interner.to_type(*ty_id)))
                    .collect(),
            ),
            _ => None,
        })
    }

    /// Get field types for an enum variant.
    pub fn get_variant_fields(
        &self,
        type_id: TypeId,
        variant_name: Name,
    ) -> Option<Vec<(Name, Type)>> {
        self.get_by_id(type_id).and_then(|entry| match &entry.kind {
            TypeKind::Enum { variants } => {
                variants.iter().find(|v| v.name == variant_name).map(|v| {
                    v.fields.iter()
                        .map(|(name, ty_id)| (*name, self.interner.to_type(*ty_id)))
                        .collect()
                })
            }
            _ => None,
        })
    }
}
```

### Variant Constructor Lookup

```rust
/// Information about a variant constructor.
pub struct VariantConstructorInfo {
    pub enum_name: Name,
    pub variant_name: Name,
    pub field_types: Vec<Type>,
    pub type_params: Vec<Name>,
}

impl TypeRegistry {
    /// Look up a variant constructor by name.
    /// Searches all registered enum types for a variant with the given name.
    pub fn lookup_variant_constructor(&self, variant_name: Name) -> Option<VariantConstructorInfo> {
        for entry in self.types_by_name.values() {
            if let TypeKind::Enum { variants } = &entry.kind {
                for variant in variants {
                    if variant.name == variant_name {
                        let field_types = variant.fields.iter()
                            .map(|(_, ty_id)| self.interner.to_type(*ty_id))
                            .collect();
                        return Some(VariantConstructorInfo {
                            enum_name: entry.name,
                            variant_name,
                            field_types,
                            type_params: entry.type_params.clone(),
                        });
                    }
                }
            }
        }
        None
    }
}
```

### Newtype Lookup

```rust
/// Information about a newtype constructor.
pub struct NewtypeConstructorInfo {
    pub newtype_name: Name,
    pub underlying_type: Type,
    pub type_params: Vec<Name>,
}

impl TypeRegistry {
    /// Look up a newtype constructor by name.
    pub fn lookup_newtype_constructor(&self, name: Name) -> Option<NewtypeConstructorInfo> {
        self.types_by_name.get(&name).and_then(|entry| {
            if let TypeKind::Newtype { underlying } = &entry.kind {
                Some(NewtypeConstructorInfo {
                    newtype_name: entry.name,
                    underlying_type: self.interner.to_type(*underlying),
                    type_params: entry.type_params.clone(),
                })
            } else {
                None
            }
        })
    }

    /// Get the underlying type for a newtype.
    pub fn get_newtype_underlying(&self, type_id: TypeId) -> Option<Type> {
        self.get_by_id(type_id).and_then(|entry| match &entry.kind {
            TypeKind::Newtype { underlying } => Some(self.interner.to_type(*underlying)),
            _ => None,
        })
    }
}
```

## Type Identity

### Nominal vs Structural Types

Newtypes have **nominal identity** — they are distinct from their underlying type even if the representation is identical:

```rust
impl TypeRegistry {
    /// Convert a registered type to the type checker's Type representation.
    /// For all user-defined types (struct, enum, newtype), returns Type::Named(name).
    /// Newtypes are nominally distinct from their underlying type.
    pub fn to_type(&self, type_id: TypeId) -> Option<Type> {
        self.get_by_id(type_id).map(|entry| match &entry.kind {
            TypeKind::Struct { .. } | TypeKind::Enum { .. } | TypeKind::Newtype { .. } => {
                Type::Named(entry.name)
            }
        })
    }
}
```

This means:
- `type UserId = str` creates a distinct type `UserId`
- `UserId` is NOT equal to `str` in the type system
- To access the inner value, use `.unwrap()` method

## Trait Registry

### Trait Definition

```rust
pub struct TraitEntry {
    pub name: Name,
    pub span: Span,
    pub type_params: Vec<Name>,
    /// Default types for type parameters (parallel to type_params).
    /// Stored as ParsedType to preserve Self references for impl-time resolution.
    pub default_types: Vec<Option<ParsedType>>,
    pub super_traits: Vec<Name>,
    pub methods: Vec<TraitMethodDef>,
    pub assoc_types: Vec<TraitAssocTypeDef>,
    pub visibility: Visibility,
}

pub struct TraitMethodDef {
    pub name: Name,
    pub params: Vec<TypeId>,
    pub return_ty: TypeId,
    pub has_default: bool,
}
```

### Default Type Parameters

Traits may have type parameters with default values:

```ori
trait Add<Rhs = Self> {
    @add (self, rhs: Rhs) -> Self
}
```

Default types are stored as `ParsedType` rather than `TypeId` because `Self` must be resolved at impl registration time, not trait definition time. When an impl omits type arguments, the defaults are substituted with proper `Self` resolution:

```rust
/// Resolve trait type arguments, filling in defaults for missing args.
fn resolve_trait_type_args(
    &mut self,
    trait_entry: &TraitEntry,
    explicit_args: ParsedTypeRange,
    self_type: &Type,
) -> Vec<TypeId> {
    let explicit = self.arena.get_parsed_types(explicit_args);
    let mut resolved = Vec::new();

    for (i, param_name) in trait_entry.type_params.iter().enumerate() {
        if i < explicit.len() {
            // Explicit argument provided
            resolved.push(self.parsed_type_to_type(&explicit[i]));
        } else if let Some(Some(default)) = trait_entry.default_types.get(i) {
            // Use default, substituting Self with implementing type
            let substituted = self.resolve_parsed_type_with_self_substitution(default, self_type);
            resolved.push(substituted);
        } else {
            // Missing required argument - error E2016
        }
    }
    resolved
}
```

### Ordering Constraint

Parameters with defaults must appear after all parameters without defaults. This is validated at trait registration (error E2015):

```rust
fn validate_default_type_param_ordering(generics: &[GenericParam]) -> Result<(), Span> {
    let mut seen_default = false;
    for param in generics {
        if param.default_type.is_some() {
            seen_default = true;
        } else if seen_default {
            return Err(param.span);  // Non-default after default
        }
    }
    Ok(())
}
```

### Trait Implementation

```rust
pub struct ImplDef {
    pub trait_name: Name,
    pub for_type: Type,
    pub methods: HashMap<Name, ExprId>,
    pub associated_types: HashMap<Name, Type>,
}
```

### Checking Trait Bounds

```rust
impl TypeRegistry {
    pub fn implements(&self, ty: &Type, trait_name: Name) -> bool {
        // Check for explicit impl
        if let Some(impls) = self.impls.get(ty) {
            if impls.iter().any(|i| i.trait_name == trait_name) {
                return true;
            }
        }

        // Check built-in implementations
        match trait_name.as_str() {
            "Eq" => self.is_eq(ty),
            "Clone" => self.is_clone(ty),
            "Default" => self.is_default(ty),
            _ => false,
        }
    }

    fn is_eq(&self, ty: &Type) -> bool {
        match ty {
            // Primitives are Eq
            Type::Int | Type::Float | Type::Bool | Type::String => true,

            // Compound types are Eq if elements are Eq
            Type::List(elem) => self.is_eq(elem),
            Type::Option(inner) => self.is_eq(inner),
            Type::Tuple(elems) => elems.iter().all(|e| self.is_eq(e)),

            // Check for derived Eq
            Type::Named(name) => {
                self.has_derived(name, "Eq")
            }

            _ => false,
        }
    }
}
```

## Error Suggestions

```rust
impl TypeRegistry {
    pub fn suggest_similar(&self, name: Name) -> Vec<Name> {
        let name_str = self.interner.resolve(name);

        self.types
            .keys()
            .filter(|&n| {
                let s = self.interner.resolve(*n);
                levenshtein_distance(name_str, s) <= 2
            })
            .copied()
            .collect()
    }
}
```

## Built-in Types

Some types are built-in but still registered:

```rust
impl TypeRegistry {
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();

        // Option<T>
        registry.types.insert(option_name, TypeDef::Enum(EnumDef {
            name: option_name,
            generics: vec![t_name],
            variants: vec![
                Variant::Tuple(some_name, vec![Type::Generic(t_name)]),
                Variant::Unit(none_name),
            ],
        }));

        // Result<T, E>
        registry.types.insert(result_name, TypeDef::Enum(EnumDef {
            name: result_name,
            generics: vec![t_name, e_name],
            variants: vec![
                Variant::Tuple(ok_name, vec![Type::Generic(t_name)]),
                Variant::Tuple(err_name, vec![Type::Generic(e_name)]),
            ],
        }));

        registry
    }
}
```
