---
title: "Type Registry"
description: "Ori Compiler Design — Type Registry"
order: 503
section: "Type System"
---

# Type Registry

The TypeRegistry stores user-defined types (structs, enums, type aliases). It enables looking up type definitions by name.

## Location

```
compiler/ori_typeck/src/registry/
├── mod.rs                    # TypeRegistry struct, re-exports
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
pub struct TypeRegistry {
    /// Type name -> Definition
    types: HashMap<Name, TypeDef>,

    /// Trait name -> Definition
    traits: HashMap<Name, TraitDef>,

    /// Type -> Trait implementations
    impls: HashMap<Type, Vec<ImplDef>>,
}

pub enum TypeDef {
    Struct(StructDef),
    Enum(EnumDef),
    Alias(Type),
}

pub struct StructDef {
    pub name: Name,
    pub generics: Vec<Name>,
    pub fields: Vec<Field>,
}

pub struct EnumDef {
    pub name: Name,
    pub generics: Vec<Name>,
    pub variants: Vec<Variant>,
}
```

## Registration

Types are registered during an initial pass:

```rust
impl TypeRegistry {
    pub fn register_types(&mut self, module: &Module) {
        for type_def in &module.types {
            match type_def {
                TypeDecl::Struct { name, generics, fields } => {
                    self.types.insert(*name, TypeDef::Struct(StructDef {
                        name: *name,
                        generics: generics.clone(),
                        fields: fields.clone(),
                    }));
                }

                TypeDecl::Enum { name, generics, variants } => {
                    self.types.insert(*name, TypeDef::Enum(EnumDef {
                        name: *name,
                        generics: generics.clone(),
                        variants: variants.clone(),
                    }));
                }

                TypeDecl::Alias { name, ty } => {
                    self.types.insert(*name, TypeDef::Alias(ty.clone()));
                }
            }
        }
    }
}
```

## Lookup

### Type Definition

```rust
impl TypeRegistry {
    pub fn get(&self, name: Name) -> Option<&TypeDef> {
        self.types.get(&name)
    }

    pub fn get_struct(&self, name: Name) -> Option<&StructDef> {
        match self.types.get(&name)? {
            TypeDef::Struct(s) => Some(s),
            _ => None,
        }
    }

    pub fn get_enum(&self, name: Name) -> Option<&EnumDef> {
        match self.types.get(&name)? {
            TypeDef::Enum(e) => Some(e),
            _ => None,
        }
    }
}
```

### Field Lookup

```rust
impl TypeRegistry {
    pub fn field_type(&self, ty: &Type, field: Name) -> Option<Type> {
        match ty {
            Type::Named(name) => {
                let struct_def = self.get_struct(*name)?;
                struct_def.fields
                    .iter()
                    .find(|f| f.name == field)
                    .map(|f| f.ty.clone())
            }

            Type::Generic { base, args } => {
                // Substitute generic arguments
                let struct_def = match base.as_ref() {
                    Type::Named(name) => self.get_struct(*name)?,
                    _ => return None,
                };

                let field_ty = struct_def.fields
                    .iter()
                    .find(|f| f.name == field)?
                    .ty.clone();

                // Build substitution from generic params to args
                let subst: HashMap<Name, Type> = struct_def.generics
                    .iter()
                    .zip(args.iter())
                    .map(|(p, a)| (*p, a.clone()))
                    .collect();

                Some(field_ty.substitute(&subst))
            }

            _ => None,
        }
    }
}
```

### Variant Lookup

```rust
impl TypeRegistry {
    pub fn variant_type(&self, ty: &Type, variant: Name) -> Option<VariantType> {
        match ty {
            Type::Named(name) => {
                let enum_def = self.get_enum(*name)?;
                enum_def.variants
                    .iter()
                    .find(|v| v.name() == variant)
                    .map(|v| v.to_type())
            }

            Type::Generic { base, args } => {
                // Similar substitution logic...
            }

            _ => None,
        }
    }
}
```

## Generic Instantiation

```rust
impl TypeRegistry {
    pub fn instantiate(&self, name: Name, args: &[Type]) -> Result<Type, TypeError> {
        let def = self.get(name).ok_or(TypeError::UndefinedType(name))?;

        match def {
            TypeDef::Struct(s) => {
                if args.len() != s.generics.len() {
                    return Err(TypeError::WrongGenericArgCount {
                        expected: s.generics.len(),
                        found: args.len(),
                    });
                }

                Ok(Type::Generic {
                    base: Box::new(Type::Named(name)),
                    args: args.to_vec(),
                })
            }

            TypeDef::Alias(ty) => {
                // Expand alias and substitute
                Ok(ty.clone())
            }

            _ => todo!(),
        }
    }
}
```

## Trait Registry

### Trait Definition

```rust
pub struct TraitDef {
    pub name: Name,
    pub methods: Vec<TraitMethod>,
    pub associated_types: Vec<AssociatedType>,
}

pub struct TraitMethod {
    pub name: Name,
    pub params: Vec<Type>,
    pub ret: Type,
    pub default: Option<ExprId>,
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
