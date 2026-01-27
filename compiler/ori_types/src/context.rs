//! Type inference context and type deduplication.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

use ori_ir::{Name, TypeId};
use crate::core::{Type, TypeScheme};
use crate::data::{TypeData, TypeVar};
use crate::traverse::{TypeIdFolder, TypeIdVisitor};
use crate::type_interner::{SharedTypeInterner, TypeInterner};
use crate::error::TypeError;

/// Type inference context.
///
/// Internally uses `TypeId` for O(1) type equality comparisons.
/// The public API accepts and returns `Type` for compatibility with
/// existing code, converting at the boundaries.
pub struct InferenceContext {
    /// Next type variable ID.
    next_var: u32,
    /// Type variable substitutions (stored as TypeId for efficiency).
    substitutions: HashMap<TypeVar, TypeId>,
    /// Type context for deduplicating generic instantiations.
    type_context: TypeContext,
    /// Type interner for converting between Type and TypeId.
    interner: SharedTypeInterner,
}

impl InferenceContext {
    /// Create a new inference context with a new type interner.
    pub fn new() -> Self {
        InferenceContext {
            next_var: 0,
            substitutions: HashMap::new(),
            type_context: TypeContext::new(),
            interner: SharedTypeInterner::new(),
        }
    }

    /// Create a new inference context with a shared type interner.
    ///
    /// Use this when you want to share the interner with other compiler phases.
    pub fn with_interner(interner: SharedTypeInterner) -> Self {
        InferenceContext {
            next_var: 0,
            substitutions: HashMap::new(),
            type_context: TypeContext::new(),
            interner,
        }
    }

    /// Get a reference to the type interner.
    pub fn interner(&self) -> &TypeInterner {
        &self.interner
    }

    /// Get the shared type interner handle.
    pub fn shared_interner(&self) -> SharedTypeInterner {
        self.interner.clone()
    }

    /// Create a fresh type variable.
    pub fn fresh_var(&mut self) -> Type {
        let var = TypeVar::new(self.next_var);
        self.next_var += 1;
        Type::Var(var)
    }

    /// Create a fresh type variable and return its TypeId.
    pub fn fresh_var_id(&mut self) -> TypeId {
        let var = TypeVar::new(self.next_var);
        self.next_var += 1;
        self.interner.intern(TypeData::Var(var))
    }

    /// Unify two types, returning error if they can't be unified.
    ///
    /// This is the public API that accepts `Type` references.
    /// Internally converts to `TypeId` for O(1) equality fast-paths.
    pub fn unify(&mut self, t1: &Type, t2: &Type) -> Result<(), TypeError> {
        let id1 = t1.to_type_id(&self.interner);
        let id2 = t2.to_type_id(&self.interner);
        self.unify_ids(id1, id2)
    }

    /// Unify two TypeIds, returning error if they can't be unified.
    ///
    /// This is the internal implementation using interned types.
    /// Provides O(1) fast-path when types are identical.
    pub fn unify_ids(&mut self, id1: TypeId, id2: TypeId) -> Result<(), TypeError> {
        // O(1) fast path: identical TypeIds always unify
        if id1 == id2 {
            return Ok(());
        }

        let id1 = self.resolve_id(id1);
        let id2 = self.resolve_id(id2);

        // Check again after resolution
        if id1 == id2 {
            return Ok(());
        }

        let data1 = self.interner.lookup(id1);
        let data2 = self.interner.lookup(id2);

        match (&data1, &data2) {
            // Type variables unify with anything
            (TypeData::Var(v), _) => {
                // Occurs check
                if self.occurs_id(*v, id2) {
                    return Err(TypeError::InfiniteType);
                }
                self.substitutions.insert(*v, id2);
                Ok(())
            }
            (_, TypeData::Var(v)) => {
                // Occurs check
                if self.occurs_id(*v, id1) {
                    return Err(TypeError::InfiniteType);
                }
                self.substitutions.insert(*v, id1);
                Ok(())
            }

            // Error type unifies with anything (for error recovery)
            (TypeData::Error, _) | (_, TypeData::Error) => Ok(()),

            // Function types
            (
                TypeData::Function { params: p1, ret: r1 },
                TypeData::Function { params: p2, ret: r2 },
            ) => {
                if p1.len() != p2.len() {
                    return Err(TypeError::ArgCountMismatch {
                        expected: p1.len(),
                        found: p2.len(),
                    });
                }
                for (&a, &b) in p1.iter().zip(p2.iter()) {
                    self.unify_ids(a, b)?;
                }
                self.unify_ids(*r1, *r2)
            }

            // Tuple types
            (TypeData::Tuple(t1), TypeData::Tuple(t2)) => {
                if t1.len() != t2.len() {
                    return Err(TypeError::TupleLengthMismatch {
                        expected: t1.len(),
                        found: t2.len(),
                    });
                }
                for (&a, &b) in t1.iter().zip(t2.iter()) {
                    self.unify_ids(a, b)?;
                }
                Ok(())
            }

            // Single-parameter container types: unify inner types
            (TypeData::List(a), TypeData::List(b))
            | (TypeData::Option(a), TypeData::Option(b))
            | (TypeData::Set(a), TypeData::Set(b))
            | (TypeData::Range(a), TypeData::Range(b))
            | (TypeData::Channel(a), TypeData::Channel(b)) => self.unify_ids(*a, *b),

            // Map types
            (
                TypeData::Map { key: k1, value: v1 },
                TypeData::Map { key: k2, value: v2 },
            ) => {
                self.unify_ids(*k1, *k2)?;
                self.unify_ids(*v1, *v2)
            }

            // Result types
            (
                TypeData::Result { ok: o1, err: e1 },
                TypeData::Result { ok: o2, err: e2 },
            ) => {
                self.unify_ids(*o1, *o2)?;
                self.unify_ids(*e1, *e2)
            }

            // Projection types: unify if same trait/assoc_name and bases unify
            (
                TypeData::Projection { base: b1, trait_name: t1, assoc_name: a1 },
                TypeData::Projection { base: b2, trait_name: t2, assoc_name: a2 },
            ) if t1 == t2 && a1 == a2 => {
                self.unify_ids(*b1, *b2)
            }

            // Applied generic types: unify if same base name and args unify
            (
                TypeData::Applied { name: n1, args: a1 },
                TypeData::Applied { name: n2, args: a2 },
            ) if n1 == n2 && a1.len() == a2.len() => {
                for (&arg1, &arg2) in a1.iter().zip(a2.iter()) {
                    self.unify_ids(arg1, arg2)?;
                }
                Ok(())
            }

            // Incompatible types - convert back to Type for error message
            _ => Err(TypeError::TypeMismatch {
                expected: self.interner.to_type(id1),
                found: self.interner.to_type(id2),
            }),
        }
    }

    /// Resolve a type by following substitutions.
    ///
    /// This is the public API that accepts and returns `Type`.
    pub fn resolve(&self, ty: &Type) -> Type {
        let id = ty.to_type_id(&self.interner);
        let resolved_id = self.resolve_id(id);
        self.interner.to_type(resolved_id)
    }

    /// Resolve a TypeId by following substitutions.
    ///
    /// This is the internal implementation using interned types.
    pub fn resolve_id(&self, id: TypeId) -> TypeId {
        struct IdResolver<'a> {
            substitutions: &'a HashMap<TypeVar, TypeId>,
            interner: &'a TypeInterner,
        }

        impl TypeIdFolder for IdResolver<'_> {
            fn interner(&self) -> &TypeInterner {
                self.interner
            }

            fn fold_var(&mut self, var: TypeVar) -> TypeId {
                if let Some(&resolved) = self.substitutions.get(&var) {
                    self.fold(resolved)
                } else {
                    self.interner.intern(TypeData::Var(var))
                }
            }
        }

        let mut resolver = IdResolver {
            substitutions: &self.substitutions,
            interner: &self.interner,
        };
        resolver.fold(id)
    }

    /// Check if a type variable occurs in a type (for occurs check).
    ///
    /// This is the public API that accepts `Type`.
    #[allow(dead_code)]
    fn occurs(&self, var: TypeVar, ty: &Type) -> bool {
        let id = ty.to_type_id(&self.interner);
        self.occurs_id(var, id)
    }

    /// Check if a type variable occurs in a TypeId (for occurs check).
    ///
    /// This is the internal implementation using interned types.
    fn occurs_id(&self, var: TypeVar, id: TypeId) -> bool {
        struct OccursChecker<'a> {
            target: TypeVar,
            substitutions: &'a HashMap<TypeVar, TypeId>,
            interner: &'a TypeInterner,
            found: bool,
        }

        impl TypeIdVisitor for OccursChecker<'_> {
            fn interner(&self) -> &TypeInterner {
                self.interner
            }

            fn visit_var(&mut self, var: TypeVar) {
                if var == self.target {
                    self.found = true;
                } else if let Some(&resolved) = self.substitutions.get(&var) {
                    self.visit(resolved);
                }
            }
        }

        let mut checker = OccursChecker {
            target: var,
            substitutions: &self.substitutions,
            interner: &self.interner,
            found: false,
        };
        checker.visit(id);
        checker.found
    }

    /// Collect all free type variables in a type.
    ///
    /// Free variables are those that appear in the type but are not in the
    /// current substitution (i.e., they haven't been unified with anything).
    pub fn free_vars(&self, ty: &Type) -> Vec<TypeVar> {
        let id = ty.to_type_id(&self.interner);
        self.free_vars_id(id)
    }

    /// Collect all free type variables in a TypeId.
    ///
    /// This is the internal implementation using interned types.
    pub fn free_vars_id(&self, id: TypeId) -> Vec<TypeVar> {
        struct FreeVarCollector<'a> {
            substitutions: &'a HashMap<TypeVar, TypeId>,
            interner: &'a TypeInterner,
            vars: Vec<TypeVar>,
        }

        impl TypeIdVisitor for FreeVarCollector<'_> {
            fn interner(&self) -> &TypeInterner {
                self.interner
            }

            fn visit_var(&mut self, var: TypeVar) {
                if let Some(&resolved) = self.substitutions.get(&var) {
                    self.visit(resolved);
                } else if !self.vars.contains(&var) {
                    self.vars.push(var);
                }
            }
        }

        let mut collector = FreeVarCollector {
            substitutions: &self.substitutions,
            interner: &self.interner,
            vars: Vec::new(),
        };
        collector.visit(id);
        collector.vars
    }

    /// Generalize a type to a type scheme by quantifying over free variables
    /// that are not in the environment.
    ///
    /// This implements the "generalization" step of Hindley-Milner:
    /// `Gen(Γ, τ) = ∀(FV(τ) - FV(Γ)). τ`
    ///
    /// The `env_free_vars` parameter contains free variables from the
    /// environment that should NOT be generalized.
    pub fn generalize(&self, ty: &Type, env_free_vars: &[TypeVar]) -> TypeScheme {
        let ty = self.resolve(ty);
        let free = self.free_vars(&ty);

        // Only quantify variables that are free in the type but not in the environment
        let quantified: Vec<TypeVar> = free
            .into_iter()
            .filter(|v| !env_free_vars.contains(v))
            .collect();

        if quantified.is_empty() {
            TypeScheme::mono(ty)
        } else {
            TypeScheme::poly(quantified, ty)
        }
    }

    /// Instantiate a type scheme by replacing quantified variables with fresh ones.
    ///
    /// This implements the "instantiation" step of Hindley-Milner:
    /// Create fresh type variables for each quantified variable and substitute.
    pub fn instantiate(&mut self, scheme: &TypeScheme) -> Type {
        if scheme.is_mono() {
            return scheme.ty.clone();
        }

        // Create fresh variables for each quantified variable
        let fresh_vars: HashMap<TypeVar, Type> = scheme
            .vars
            .iter()
            .map(|v| (*v, self.fresh_var()))
            .collect();

        // Substitute quantified variables with fresh ones
        self.substitute_vars(&scheme.ty, &fresh_vars)
    }

    /// Substitute type variables according to a mapping.
    fn substitute_vars(&self, ty: &Type, mapping: &HashMap<TypeVar, Type>) -> Type {
        // Convert mapping to TypeId-based
        let id_mapping: HashMap<TypeVar, TypeId> = mapping
            .iter()
            .map(|(&v, t)| (v, t.to_type_id(&self.interner)))
            .collect();
        let id = ty.to_type_id(&self.interner);
        let result_id = self.substitute_vars_id(id, &id_mapping);
        self.interner.to_type(result_id)
    }

    /// Substitute type variables according to a TypeId mapping.
    ///
    /// This is the internal implementation using interned types.
    fn substitute_vars_id(&self, id: TypeId, mapping: &HashMap<TypeVar, TypeId>) -> TypeId {
        struct VarSubstitutor<'a> {
            mapping: &'a HashMap<TypeVar, TypeId>,
            substitutions: &'a HashMap<TypeVar, TypeId>,
            interner: &'a TypeInterner,
        }

        impl TypeIdFolder for VarSubstitutor<'_> {
            fn interner(&self) -> &TypeInterner {
                self.interner
            }

            fn fold_var(&mut self, var: TypeVar) -> TypeId {
                if let Some(&replacement) = self.mapping.get(&var) {
                    replacement
                } else if let Some(&resolved) = self.substitutions.get(&var) {
                    self.fold(resolved)
                } else {
                    self.interner.intern(TypeData::Var(var))
                }
            }
        }

        let mut substitutor = VarSubstitutor {
            mapping,
            substitutions: &self.substitutions,
            interner: &self.interner,
        };
        substitutor.fold(id)
    }

    /// Get or create a List<elem> type, deduplicating identical instantiations.
    pub fn make_list(&mut self, elem: Type) -> Type {
        self.type_context.list_type(elem)
    }

    /// Get or create an Option<inner> type, deduplicating identical instantiations.
    pub fn make_option(&mut self, inner: Type) -> Type {
        self.type_context.option_type(inner)
    }

    /// Get or create a Result<ok, err> type, deduplicating identical instantiations.
    pub fn make_result(&mut self, ok: Type, err: Type) -> Type {
        self.type_context.result_type(ok, err)
    }

    /// Get or create a Map<key, value> type, deduplicating identical instantiations.
    pub fn make_map(&mut self, key: Type, value: Type) -> Type {
        self.type_context.map_type(key, value)
    }

    /// Get or create a Set<elem> type, deduplicating identical instantiations.
    pub fn make_set(&mut self, elem: Type) -> Type {
        self.type_context.set_type(elem)
    }

    /// Get or create a Range<elem> type, deduplicating identical instantiations.
    pub fn make_range(&mut self, elem: Type) -> Type {
        self.type_context.range_type(elem)
    }

    /// Get or create a Channel<elem> type, deduplicating identical instantiations.
    pub fn make_channel(&mut self, elem: Type) -> Type {
        self.type_context.channel_type(elem)
    }

    /// Get or create a Tuple type, deduplicating identical instantiations.
    pub fn make_tuple(&mut self, types: Vec<Type>) -> Type {
        self.type_context.tuple_type(types)
    }

    /// Get or create a Function type, deduplicating identical instantiations.
    pub fn make_function(&mut self, params: Vec<Type>, ret: Type) -> Type {
        self.type_context.function_type(params, ret)
    }

    /// Access the underlying type context directly.
    pub fn type_context(&mut self) -> &mut TypeContext {
        &mut self.type_context
    }
}

impl Default for InferenceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry in the type context cache.
#[derive(Clone, Debug)]
struct TypeContextEntry {
    /// The origin type scheme.
    origin: TypeScheme,
    /// The type arguments used.
    targs: Vec<Type>,
    /// The resulting instantiated type.
    instance: Type,
}

/// Type instantiation context for deduplication.
///
/// Ensures identical generic instantiations share the same Type instance
/// within a single type-checking pass. This reduces allocations and
/// can make equality checks faster.
///
/// Note: Salsa handles cross-query memoization, but `TypeContext` deduplicates
/// **within** a single type-checking pass.
#[derive(Clone, Debug, Default)]
pub struct TypeContext {
    /// `hash(origin_id` + targs) -> list of entries with that hash
    type_map: HashMap<u64, Vec<TypeContextEntry>>,
    /// Origin type scheme -> stable ID for hashing
    origin_ids: HashMap<TypeScheme, u32>,
    /// Next origin ID to assign
    next_origin_id: u32,
}

impl TypeContext {
    /// Create a new empty type context.
    pub fn new() -> Self {
        TypeContext::default()
    }

    /// Get or assign a stable ID for an origin type scheme.
    fn get_origin_id(&mut self, origin: &TypeScheme) -> u32 {
        if let Some(&id) = self.origin_ids.get(origin) {
            return id;
        }
        let id = self.next_origin_id;
        self.next_origin_id += 1;
        self.origin_ids.insert(origin.clone(), id);
        id
    }

    /// Compute a hash for (origin, targs) for lookup.
    fn instance_hash(&mut self, origin: &TypeScheme, targs: &[Type]) -> u64 {
        let origin_id = self.get_origin_id(origin);
        let mut hasher = DefaultHasher::new();
        origin_id.hash(&mut hasher);
        for t in targs {
            t.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Look up an existing instantiation.
    pub fn lookup(&mut self, origin: &TypeScheme, targs: &[Type]) -> Option<&Type> {
        let hash = self.instance_hash(origin, targs);
        if let Some(entries) = self.type_map.get(&hash) {
            for entry in entries {
                if &entry.origin == origin && entry.targs == targs {
                    return Some(&entry.instance);
                }
            }
        }
        None
    }

    /// Insert a new instantiation, returning the canonical instance.
    ///
    /// If an identical instantiation already exists, returns the existing one.
    pub fn insert(&mut self, origin: TypeScheme, targs: Vec<Type>, instance: Type) -> Type {
        // Check if already exists
        if let Some(existing) = self.lookup(&origin, &targs) {
            return existing.clone();
        }

        let hash = self.instance_hash(&origin, &targs);
        let entry = TypeContextEntry {
            origin,
            targs,
            instance: instance.clone(),
        };

        self.type_map.entry(hash).or_default().push(entry);
        instance
    }

    /// Get or create a List<elem> type.
    pub fn list_type(&mut self, elem: Type) -> Type {
        // Use a synthetic origin scheme for built-in List
        let origin = TypeScheme::mono(Type::Named(Name::new(0, 0))); // placeholder
        let targs = vec![elem.clone()];

        if let Some(existing) = self.lookup(&origin, &targs) {
            return existing.clone();
        }

        let instance = Type::List(Box::new(elem));
        self.insert(origin, targs, instance)
    }

    /// Get or create an Option<inner> type.
    pub fn option_type(&mut self, inner: Type) -> Type {
        let origin = TypeScheme::mono(Type::Named(Name::new(0, 1))); // placeholder
        let targs = vec![inner.clone()];

        if let Some(existing) = self.lookup(&origin, &targs) {
            return existing.clone();
        }

        let instance = Type::Option(Box::new(inner));
        self.insert(origin, targs, instance)
    }

    /// Get or create a Result<ok, err> type.
    pub fn result_type(&mut self, ok: Type, err: Type) -> Type {
        let origin = TypeScheme::mono(Type::Named(Name::new(0, 2))); // placeholder
        let targs = vec![ok.clone(), err.clone()];

        if let Some(existing) = self.lookup(&origin, &targs) {
            return existing.clone();
        }

        let instance = Type::Result {
            ok: Box::new(ok),
            err: Box::new(err),
        };
        self.insert(origin, targs, instance)
    }

    /// Get or create a Map<key, value> type.
    pub fn map_type(&mut self, key: Type, value: Type) -> Type {
        let origin = TypeScheme::mono(Type::Named(Name::new(0, 3))); // placeholder
        let targs = vec![key.clone(), value.clone()];

        if let Some(existing) = self.lookup(&origin, &targs) {
            return existing.clone();
        }

        let instance = Type::Map {
            key: Box::new(key),
            value: Box::new(value),
        };
        self.insert(origin, targs, instance)
    }

    /// Get or create a Set<elem> type.
    pub fn set_type(&mut self, elem: Type) -> Type {
        let origin = TypeScheme::mono(Type::Named(Name::new(0, 4))); // placeholder
        let targs = vec![elem.clone()];

        if let Some(existing) = self.lookup(&origin, &targs) {
            return existing.clone();
        }

        let instance = Type::Set(Box::new(elem));
        self.insert(origin, targs, instance)
    }

    /// Get or create a Range<elem> type.
    pub fn range_type(&mut self, elem: Type) -> Type {
        let origin = TypeScheme::mono(Type::Named(Name::new(0, 5))); // placeholder
        let targs = vec![elem.clone()];

        if let Some(existing) = self.lookup(&origin, &targs) {
            return existing.clone();
        }

        let instance = Type::Range(Box::new(elem));
        self.insert(origin, targs, instance)
    }

    /// Get or create a Channel<elem> type.
    pub fn channel_type(&mut self, elem: Type) -> Type {
        let origin = TypeScheme::mono(Type::Named(Name::new(0, 6))); // placeholder
        let targs = vec![elem.clone()];

        if let Some(existing) = self.lookup(&origin, &targs) {
            return existing.clone();
        }

        let instance = Type::Channel(Box::new(elem));
        self.insert(origin, targs, instance)
    }

    /// Get or create a Tuple type.
    pub fn tuple_type(&mut self, types: Vec<Type>) -> Type {
        let origin = TypeScheme::mono(Type::Named(Name::new(0, 7))); // placeholder
        let targs = types.clone();

        if let Some(existing) = self.lookup(&origin, &targs) {
            return existing.clone();
        }

        let instance = Type::Tuple(types);
        self.insert(origin, targs, instance)
    }

    /// Get or create a Function type.
    pub fn function_type(&mut self, params: Vec<Type>, ret: Type) -> Type {
        let origin = TypeScheme::mono(Type::Named(Name::new(0, 8))); // placeholder
        let mut targs = params.clone();
        targs.push(ret.clone());

        if let Some(existing) = self.lookup(&origin, &targs) {
            return existing.clone();
        }

        let instance = Type::Function {
            params,
            ret: Box::new(ret),
        };
        self.insert(origin, targs, instance)
    }
}
