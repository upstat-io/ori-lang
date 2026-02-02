//! Type inference context and type deduplication.

use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use std::hash::{Hash, Hasher};

use crate::core::{Type, TypeScheme};
use crate::data::{TypeData, TypeVar};
use crate::error::TypeError;
use crate::traverse::{TypeIdFolder, TypeIdVisitor};
use crate::type_interner::{SharedTypeInterner, TypeInterner};
use ori_ir::{Name, TypeId};

/// Type inference context.
///
/// Internally uses `TypeId` for O(1) type equality comparisons.
/// The public API accepts and returns `Type` for compatibility with
/// existing code, converting at the boundaries.
pub struct InferenceContext {
    /// Next type variable ID.
    next_var: u32,
    /// Type variable substitutions (stored as `TypeId` for efficiency).
    substitutions: FxHashMap<TypeVar, TypeId>,
    /// Type context for deduplicating generic instantiations.
    type_context: TypeContext,
    /// Type interner for converting between Type and `TypeId`.
    interner: SharedTypeInterner,
}

impl InferenceContext {
    /// Create a new inference context with a new type interner.
    pub fn new() -> Self {
        InferenceContext {
            next_var: 0,
            substitutions: FxHashMap::default(),
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
            substitutions: FxHashMap::default(),
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

    /// Create a fresh type variable and return its `TypeId`.
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

    /// Unify two `TypeIds`, returning error if they can't be unified.
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
            // --- Type Variables ---
            // Type variables unify with anything (after occurs check)
            (TypeData::Var(v), _) => {
                if self.occurs_id(*v, id2) {
                    return Err(TypeError::InfiniteType);
                }
                self.substitutions.insert(*v, id2);
                Ok(())
            }
            (_, TypeData::Var(v)) => {
                if self.occurs_id(*v, id1) {
                    return Err(TypeError::InfiniteType);
                }
                self.substitutions.insert(*v, id1);
                Ok(())
            }

            // --- Error Recovery & Never Type ---
            // Error unifies with anything to allow continued type checking after errors.
            // Never is the bottom type: it coerces to any type T (the coercion never
            // actually executes since Never has no values - the expression diverges).
            (TypeData::Error | TypeData::Never, _) | (_, TypeData::Error | TypeData::Never) => {
                Ok(())
            }

            // --- Compound Types ---
            // Functions: params and return must unify
            (
                TypeData::Function {
                    params: p1,
                    ret: r1,
                },
                TypeData::Function {
                    params: p2,
                    ret: r2,
                },
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

            // Tuples: same length and elements must unify
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

            // --- Container Types ---
            // Single-type containers: inner types must unify
            (TypeData::List(a), TypeData::List(b))
            | (TypeData::Option(a), TypeData::Option(b))
            | (TypeData::Set(a), TypeData::Set(b))
            | (TypeData::Range(a), TypeData::Range(b))
            | (TypeData::Channel(a), TypeData::Channel(b)) => self.unify_ids(*a, *b),

            // Two-type containers: both inner types must unify
            (TypeData::Map { key: k1, value: v1 }, TypeData::Map { key: k2, value: v2 }) => {
                self.unify_ids(*k1, *k2)?;
                self.unify_ids(*v1, *v2)
            }
            (TypeData::Result { ok: o1, err: e1 }, TypeData::Result { ok: o2, err: e2 }) => {
                self.unify_ids(*o1, *o2)?;
                self.unify_ids(*e1, *e2)
            }

            // --- Generic Types ---
            // Projections: same trait/assoc_name and bases must unify
            (
                TypeData::Projection {
                    base: b1,
                    trait_name: t1,
                    assoc_name: a1,
                },
                TypeData::Projection {
                    base: b2,
                    trait_name: t2,
                    assoc_name: a2,
                },
            ) if t1 == t2 && a1 == a2 => self.unify_ids(*b1, *b2),

            // Applied generics: same name and all args must unify
            (
                TypeData::Applied { name: n1, args: a1 },
                TypeData::Applied { name: n2, args: a2 },
            ) if n1 == n2 && a1.len() == a2.len() => {
                for (&arg1, &arg2) in a1.iter().zip(a2.iter()) {
                    self.unify_ids(arg1, arg2)?;
                }
                Ok(())
            }

            // --- Mismatch ---
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

    /// Resolve a `TypeId` by following substitutions.
    ///
    /// This is the internal implementation using interned types.
    pub fn resolve_id(&self, id: TypeId) -> TypeId {
        struct IdResolver<'a> {
            substitutions: &'a FxHashMap<TypeVar, TypeId>,
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

    /// Check if a type variable occurs in a `TypeId` (for occurs check).
    ///
    /// This is the internal implementation using interned types.
    fn occurs_id(&self, var: TypeVar, id: TypeId) -> bool {
        struct OccursChecker<'a> {
            target: TypeVar,
            substitutions: &'a FxHashMap<TypeVar, TypeId>,
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

    /// Collect all free type variables in a `TypeId`.
    ///
    /// This is the internal implementation using interned types.
    pub fn free_vars_id(&self, id: TypeId) -> Vec<TypeVar> {
        struct FreeVarCollector<'a> {
            substitutions: &'a FxHashMap<TypeVar, TypeId>,
            interner: &'a TypeInterner,
            vars: FxHashSet<TypeVar>,
        }

        impl TypeIdVisitor for FreeVarCollector<'_> {
            fn interner(&self) -> &TypeInterner {
                self.interner
            }

            fn visit_var(&mut self, var: TypeVar) {
                if let Some(&resolved) = self.substitutions.get(&var) {
                    self.visit(resolved);
                } else {
                    self.vars.insert(var); // O(1) instead of O(n)
                }
            }
        }

        let mut collector = FreeVarCollector {
            substitutions: &self.substitutions,
            interner: &self.interner,
            vars: FxHashSet::default(),
        };
        collector.visit(id);
        collector.vars.into_iter().collect()
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

        // Convert to HashSet for O(1) lookup instead of O(n)
        let env_set: FxHashSet<TypeVar> = env_free_vars.iter().copied().collect();

        // Only quantify variables that are free in the type but not in the environment
        let quantified: Vec<TypeVar> = free.into_iter().filter(|v| !env_set.contains(v)).collect();

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
        let fresh_vars: FxHashMap<TypeVar, Type> =
            scheme.vars.iter().map(|v| (*v, self.fresh_var())).collect();

        // Substitute quantified variables with fresh ones
        self.substitute_vars(&scheme.ty, &fresh_vars)
    }

    /// Substitute type variables according to a mapping.
    fn substitute_vars(&self, ty: &Type, mapping: &FxHashMap<TypeVar, Type>) -> Type {
        // Convert mapping to TypeId-based
        let id_mapping: FxHashMap<TypeVar, TypeId> = mapping
            .iter()
            .map(|(&v, t)| (v, t.to_type_id(&self.interner)))
            .collect();
        let id = ty.to_type_id(&self.interner);
        let result_id = self.substitute_vars_id(id, &id_mapping);
        self.interner.to_type(result_id)
    }

    /// Substitute type variables according to a `TypeId` mapping.
    ///
    /// This is the internal implementation using interned types.
    fn substitute_vars_id(&self, id: TypeId, mapping: &FxHashMap<TypeVar, TypeId>) -> TypeId {
        struct VarSubstitutor<'a> {
            mapping: &'a FxHashMap<TypeVar, TypeId>,
            substitutions: &'a FxHashMap<TypeVar, TypeId>,
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

    /// Get or create a Function type from a slice, avoiding caller allocation.
    ///
    /// This is more efficient than `make_function` when you have a `&[Type]`
    /// because it avoids the caller needing to call `.to_vec()`.
    pub fn make_function_from_slice(&mut self, params: &[Type], ret: Type) -> Type {
        self.type_context.function_type(params.to_vec(), ret)
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
///
/// # Memory Behavior
///
/// The cache grows as types are instantiated and is never automatically cleared.
/// For long-running sessions, create a fresh `TypeContext` per compilation unit
/// or query to prevent unbounded growth. The cache is cheap to recreate since
/// it's just bookkeeping - the actual type data lives in the `TypeInterner`.
#[derive(Clone, Debug, Default)]
pub struct TypeContext {
    /// `hash(origin_id` + targs) -> list of entries with that hash
    type_map: FxHashMap<u64, Vec<TypeContextEntry>>,
    /// Origin type scheme -> stable ID for hashing
    origin_ids: FxHashMap<TypeScheme, u32>,
    /// Next origin ID to assign
    next_origin_id: u32,
}

impl TypeContext {
    // Built-in type origin IDs for deduplication cache keys.
    const LIST_ORIGIN: u32 = 0;
    const OPTION_ORIGIN: u32 = 1;
    const RESULT_ORIGIN: u32 = 2;
    const MAP_ORIGIN: u32 = 3;
    const SET_ORIGIN: u32 = 4;
    const RANGE_ORIGIN: u32 = 5;
    const CHANNEL_ORIGIN: u32 = 6;
    const TUPLE_ORIGIN: u32 = 7;
    const FUNCTION_ORIGIN: u32 = 8;

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
    ///
    /// Uses `FxHasher` for speed (non-cryptographic, optimized for small keys).
    fn instance_hash(&mut self, origin: &TypeScheme, targs: &[Type]) -> u64 {
        let origin_id = self.get_origin_id(origin);
        let mut hasher = FxHasher::default();
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
        // Compute hash once and reuse for both lookup and insert
        let hash = self.instance_hash(&origin, &targs);

        // Check if already exists using precomputed hash
        if let Some(entries) = self.type_map.get(&hash) {
            for entry in entries {
                if entry.origin == origin && entry.targs == targs {
                    return entry.instance.clone();
                }
            }
        }

        let entry = TypeContextEntry {
            origin,
            targs,
            instance: instance.clone(),
        };

        self.type_map.entry(hash).or_default().push(entry);
        instance
    }

    /// Deduplicate a type instantiation. Returns the existing instance if one
    /// matches, otherwise creates and caches a new one via `make_type`.
    fn deduplicate_type(
        &mut self,
        origin_id: u32,
        targs: Vec<Type>,
        make_type: impl FnOnce() -> Type,
    ) -> Type {
        let origin = TypeScheme::mono(Type::Named(Name::new(0, origin_id)));
        if let Some(existing) = self.lookup(&origin, &targs) {
            return existing.clone();
        }
        let instance = make_type();
        self.insert(origin, targs, instance)
    }

    /// Get or create a List<elem> type.
    pub fn list_type(&mut self, elem: Type) -> Type {
        let targs = vec![elem.clone()];
        self.deduplicate_type(Self::LIST_ORIGIN, targs, || Type::List(Box::new(elem)))
    }

    /// Get or create an Option<inner> type.
    pub fn option_type(&mut self, inner: Type) -> Type {
        let targs = vec![inner.clone()];
        self.deduplicate_type(Self::OPTION_ORIGIN, targs, || Type::Option(Box::new(inner)))
    }

    /// Get or create a Result<ok, err> type.
    pub fn result_type(&mut self, ok: Type, err: Type) -> Type {
        let targs = vec![ok.clone(), err.clone()];
        self.deduplicate_type(Self::RESULT_ORIGIN, targs, || Type::Result {
            ok: Box::new(ok),
            err: Box::new(err),
        })
    }

    /// Get or create a Map<key, value> type.
    pub fn map_type(&mut self, key: Type, value: Type) -> Type {
        let targs = vec![key.clone(), value.clone()];
        self.deduplicate_type(Self::MAP_ORIGIN, targs, || Type::Map {
            key: Box::new(key),
            value: Box::new(value),
        })
    }

    /// Get or create a Set<elem> type.
    pub fn set_type(&mut self, elem: Type) -> Type {
        let targs = vec![elem.clone()];
        self.deduplicate_type(Self::SET_ORIGIN, targs, || Type::Set(Box::new(elem)))
    }

    /// Get or create a Range<elem> type.
    pub fn range_type(&mut self, elem: Type) -> Type {
        let targs = vec![elem.clone()];
        self.deduplicate_type(Self::RANGE_ORIGIN, targs, || Type::Range(Box::new(elem)))
    }

    /// Get or create a Channel<elem> type.
    pub fn channel_type(&mut self, elem: Type) -> Type {
        let targs = vec![elem.clone()];
        self.deduplicate_type(Self::CHANNEL_ORIGIN, targs, || {
            Type::Channel(Box::new(elem))
        })
    }

    /// Get or create a Tuple type.
    pub fn tuple_type(&mut self, types: Vec<Type>) -> Type {
        let targs = types.clone();
        self.deduplicate_type(Self::TUPLE_ORIGIN, targs, || Type::Tuple(types))
    }

    /// Get or create a Function type.
    pub fn function_type(&mut self, params: Vec<Type>, ret: Type) -> Type {
        let mut targs = params.clone();
        targs.push(ret.clone());
        self.deduplicate_type(Self::FUNCTION_ORIGIN, targs, || Type::Function {
            params,
            ret: Box::new(ret),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_never_unifies_with_any_type() {
        let mut ctx = InferenceContext::new();

        // Never should unify with int
        assert!(ctx.unify(&Type::Never, &Type::Int).is_ok());

        // int should unify with Never
        assert!(ctx.unify(&Type::Int, &Type::Never).is_ok());

        // Never should unify with str
        assert!(ctx.unify(&Type::Never, &Type::Str).is_ok());

        // Never should unify with complex types
        assert!(ctx
            .unify(&Type::Never, &Type::List(Box::new(Type::Int)))
            .is_ok());

        // Never should unify with Option<int>
        assert!(ctx
            .unify(&Type::Never, &Type::Option(Box::new(Type::Int)))
            .is_ok());

        // Never should unify with Result<int, str>
        assert!(ctx
            .unify(
                &Type::Never,
                &Type::Result {
                    ok: Box::new(Type::Int),
                    err: Box::new(Type::Str)
                }
            )
            .is_ok());
    }

    #[test]
    fn test_never_unifies_with_never() {
        let mut ctx = InferenceContext::new();
        assert!(ctx.unify(&Type::Never, &Type::Never).is_ok());
    }

    #[test]
    fn test_never_in_function_return() {
        let mut ctx = InferenceContext::new();

        // A function returning Never should unify with a function returning int
        // (common in diverging functions like panic)
        let fn_never = Type::Function {
            params: vec![],
            ret: Box::new(Type::Never),
        };
        let fn_int = Type::Function {
            params: vec![],
            ret: Box::new(Type::Int),
        };

        assert!(ctx.unify(&fn_never, &fn_int).is_ok());
    }
}
