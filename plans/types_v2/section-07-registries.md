---
section: "07"
title: Registries
status: not-started
goal: Type, trait, and method registries for user-defined types
sections:
  - id: "07.1"
    title: TypeRegistry
    status: not-started
  - id: "07.2"
    title: TraitRegistry
    status: not-started
  - id: "07.3"
    title: MethodRegistry
    status: not-started
  - id: "07.4"
    title: Built-in Methods
    status: not-started
  - id: "07.5"
    title: Method Lookup Algorithm
    status: not-started
---

# Section 07: Registries

**Status:** Not Started
**Goal:** Unified registries for types, traits, and methods
**Source:** Current Ori implementation, improved design

---

## 07.1 TypeRegistry

**Goal:** Registry for user-defined types (structs, enums)

### Design

```rust
/// Registry for user-defined types.
pub struct TypeRegistry {
    /// Types by name for lookup.
    types_by_name: FxHashMap<Name, TypeEntry>,
    /// Types by pool index for reverse lookup.
    types_by_idx: FxHashMap<Idx, TypeEntry>,
    /// Variant name -> (type Idx, variant index).
    variants_by_name: FxHashMap<Name, (Idx, usize)>,
}

#[derive(Clone, Debug)]
pub struct TypeEntry {
    pub name: Name,
    pub idx: Idx,
    pub kind: TypeKind,
    pub span: Span,
    pub type_params: Vec<Name>,
    pub visibility: Visibility,
}

#[derive(Clone, Debug)]
pub enum TypeKind {
    Struct {
        fields: Vec<FieldDef>,
    },
    Enum {
        variants: Vec<VariantDef>,
    },
    Newtype {
        underlying: Idx,
    },
    Alias {
        target: Idx,
    },
}

#[derive(Clone, Debug)]
pub struct FieldDef {
    pub name: Name,
    pub ty: Idx,
    pub span: Span,
    pub visibility: Visibility,
}

#[derive(Clone, Debug)]
pub struct VariantDef {
    pub name: Name,
    pub fields: VariantFields,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum VariantFields {
    Unit,
    Tuple(Vec<Idx>),
    Record(Vec<FieldDef>),
}
```

### Tasks

- [ ] Create `ori_typeck/src/registry/types.rs`
- [ ] Define `TypeRegistry` with lookup methods
- [ ] Define `TypeEntry`, `TypeKind`, etc.
- [ ] Implement type registration
- [ ] Implement variant lookup
- [ ] Add tests for type registry

---

## 07.2 TraitRegistry

**Goal:** Registry for traits and implementations

### Design

```rust
/// Registry for traits and their implementations.
pub struct TraitRegistry {
    /// Traits by name.
    traits_by_name: FxHashMap<Name, TraitEntry>,
    /// Implementations indexed by (trait, self_type).
    impls: Vec<ImplEntry>,
    /// Quick lookup: self_type -> impl indices.
    impls_by_type: FxHashMap<Idx, Vec<usize>>,
}

#[derive(Clone, Debug)]
pub struct TraitEntry {
    pub name: Name,
    pub idx: Idx,
    pub type_params: Vec<Name>,
    pub methods: FxHashMap<Name, TraitMethodDef>,
    pub assoc_types: FxHashMap<Name, TraitAssocTypeDef>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct TraitMethodDef {
    pub name: Name,
    pub signature: Idx, // Function type
    pub has_default: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct TraitAssocTypeDef {
    pub name: Name,
    pub bounds: Vec<Idx>,
    pub default: Option<Idx>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct ImplEntry {
    pub trait_idx: Option<Idx>, // None for inherent impls
    pub self_type: Idx,
    pub type_params: Vec<Name>,
    pub methods: FxHashMap<Name, ImplMethodDef>,
    pub assoc_types: FxHashMap<Name, Idx>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct ImplMethodDef {
    pub name: Name,
    pub signature: Idx,
    pub body: ExprId,
    pub span: Span,
}
```

### Tasks

- [ ] Create `ori_typeck/src/registry/traits.rs`
- [ ] Define `TraitRegistry` with lookup methods
- [ ] Define `TraitEntry`, `ImplEntry`, etc.
- [ ] Implement trait registration
- [ ] Implement impl registration with coherence checking
- [ ] Add impl lookup by type and trait
- [ ] Add tests for trait registry

---

## 07.3 MethodRegistry

**Goal:** Unified method lookup across all sources

### Design

```rust
/// Unified method registry combining all method sources.
pub struct MethodRegistry {
    /// Built-in methods by (type_tag, method_name).
    builtin: FxHashMap<(Tag, Name), BuiltinMethod>,
    /// Reference to trait registry for trait methods.
    traits: Arc<TraitRegistry>,
    /// Reference to type registry for inherent methods.
    types: Arc<TypeRegistry>,
}

#[derive(Clone, Debug)]
pub struct BuiltinMethod {
    pub name: Name,
    pub receiver_tag: Tag,
    pub signature: fn(&Pool, Idx) -> Idx, // Computes signature from receiver
    pub doc: &'static str,
}

#[derive(Clone, Debug)]
pub enum MethodResolution {
    Builtin(BuiltinMethod),
    TraitMethod {
        trait_idx: Idx,
        impl_idx: usize,
        method: ImplMethodDef,
    },
    InherentMethod {
        type_idx: Idx,
        method: ImplMethodDef,
    },
}

impl MethodRegistry {
    /// Look up a method on a type.
    pub fn lookup(
        &self,
        pool: &Pool,
        receiver_ty: Idx,
        method_name: Name,
    ) -> Option<MethodResolution> {
        // 1. Check built-in methods first
        let tag = pool.tag(receiver_ty);
        if let Some(builtin) = self.builtin.get(&(tag, method_name)) {
            return Some(MethodResolution::Builtin(builtin.clone()));
        }

        // 2. Check inherent impls
        if let Some(type_entry) = self.types.get_by_idx(receiver_ty) {
            // Look for inherent impl with this method
        }

        // 3. Check trait impls
        for impl_idx in self.traits.impls_for_type(receiver_ty) {
            let impl_entry = &self.traits.impls[impl_idx];
            if let Some(method) = impl_entry.methods.get(&method_name) {
                return Some(MethodResolution::TraitMethod {
                    trait_idx: impl_entry.trait_idx.unwrap(),
                    impl_idx,
                    method: method.clone(),
                });
            }
        }

        None
    }
}
```

### Tasks

- [ ] Create `ori_typeck/src/registry/methods.rs`
- [ ] Define `MethodRegistry` combining all sources
- [ ] Define `MethodResolution` enum
- [ ] Implement unified `lookup()` method
- [ ] Handle method ambiguity
- [ ] Add tests for method resolution

---

## 07.4 Built-in Methods

**Goal:** Define built-in methods for primitive and collection types

### Design

```rust
impl MethodRegistry {
    fn register_builtins(&mut self, pool: &Pool) {
        // List methods
        self.register_builtin(Tag::List, "len", |pool, recv| {
            Idx::INT
        });
        self.register_builtin(Tag::List, "is_empty", |pool, recv| {
            Idx::BOOL
        });
        self.register_builtin(Tag::List, "first", |pool, recv| {
            let elem = Idx(pool.data(recv));
            pool.option(elem)
        });
        self.register_builtin(Tag::List, "push", |pool, recv| {
            let elem = Idx(pool.data(recv));
            pool.function(&[elem], recv)
        });
        self.register_builtin(Tag::List, "map", |pool, recv| {
            // <T, U> (self, (T) -> U) -> [U]
            // Returns a fresh function type with generics
            let elem = Idx(pool.data(recv));
            let fresh_out = pool.fresh_var(Rank::FIRST);
            let mapper = pool.function(&[elem], fresh_out);
            let result = pool.list(fresh_out);
            pool.function(&[mapper], result)
        });

        // Option methods
        self.register_builtin(Tag::Option, "is_some", |pool, recv| {
            Idx::BOOL
        });
        self.register_builtin(Tag::Option, "is_none", |pool, recv| {
            Idx::BOOL
        });
        self.register_builtin(Tag::Option, "unwrap", |pool, recv| {
            Idx(pool.data(recv)) // Inner type
        });

        // String methods
        self.register_builtin(Tag::Str, "len", |pool, recv| Idx::INT);
        self.register_builtin(Tag::Str, "is_empty", |pool, recv| Idx::BOOL);
        self.register_builtin(Tag::Str, "to_upper", |pool, recv| Idx::STR);
        self.register_builtin(Tag::Str, "to_lower", |pool, recv| Idx::STR);
        self.register_builtin(Tag::Str, "trim", |pool, recv| Idx::STR);
        self.register_builtin(Tag::Str, "split", |pool, recv| {
            pool.function(&[Idx::STR], pool.list(Idx::STR))
        });

        // Int methods
        self.register_builtin(Tag::Int, "abs", |pool, recv| Idx::INT);
        self.register_builtin(Tag::Int, "to_float", |pool, recv| Idx::FLOAT);
        self.register_builtin(Tag::Int, "to_str", |pool, recv| Idx::STR);

        // Float methods
        self.register_builtin(Tag::Float, "abs", |pool, recv| Idx::FLOAT);
        self.register_builtin(Tag::Float, "floor", |pool, recv| Idx::INT);
        self.register_builtin(Tag::Float, "ceil", |pool, recv| Idx::INT);
        self.register_builtin(Tag::Float, "round", |pool, recv| Idx::INT);
        self.register_builtin(Tag::Float, "to_int", |pool, recv| Idx::INT);
        self.register_builtin(Tag::Float, "to_str", |pool, recv| Idx::STR);

        // ... more built-in methods
    }
}
```

### Tasks

- [ ] Create `ori_typeck/src/registry/builtin.rs`
- [ ] Implement all List methods
- [ ] Implement all Option methods
- [ ] Implement all Result methods
- [ ] Implement all String methods
- [ ] Implement all Int/Float methods
- [ ] Implement Map methods
- [ ] Add tests for built-in method resolution

---

## 07.5 Method Lookup Algorithm

**Goal:** Define the complete method resolution algorithm

### Algorithm

```
lookup_method(receiver_ty, method_name):
    1. Resolve receiver_ty (follow type aliases, resolve vars)

    2. Check BUILT-IN methods:
       - Get tag of receiver_ty
       - Look up (tag, method_name) in builtin registry
       - If found, return BuiltinMethod

    3. Check INHERENT methods:
       - If receiver_ty is a user-defined type (Struct, Enum):
         - Look up inherent impl for that type
         - Check if impl has method with method_name
         - If found, return InherentMethod

    4. Check TRAIT methods:
       - For each impl where self_type matches receiver_ty:
         - Check if impl has method with method_name
         - If found, return TraitMethod

    5. Check AUTO-DEREF:
       - If receiver_ty is Option<T> or Result<T, E>:
         - Try lookup_method(T, method_name)
         - If found, mark as needs_unwrap

    6. Return None (method not found)
```

### Tasks

- [ ] Implement full method lookup algorithm
- [ ] Handle auto-deref for Option/Result
- [ ] Handle method ambiguity (multiple matches)
- [ ] Add caching for frequently used lookups
- [ ] Add tests for edge cases

---

## 07.6 Completion Checklist

- [ ] `TypeRegistry` complete with all operations
- [ ] `TraitRegistry` complete with coherence checking
- [ ] `MethodRegistry` unifying all method sources
- [ ] All built-in methods registered
- [ ] Method lookup algorithm working
- [ ] Auto-deref working for Option/Result
- [ ] All tests passing

**Exit Criteria:** Method calls resolve correctly through the unified registry, finding built-in methods, inherent methods, and trait methods in the correct priority order.
