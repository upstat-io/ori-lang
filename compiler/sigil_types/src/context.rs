//! Type inference context and type deduplication.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

use sigil_ir::Name;
use crate::{Type, TypeVar, TypeScheme, TypeFolder, TypeVisitor, TypeError};

/// Type inference context.
pub struct InferenceContext {
    /// Next type variable ID.
    next_var: u32,
    /// Type variable substitutions.
    substitutions: HashMap<TypeVar, Type>,
    /// Type context for deduplicating generic instantiations.
    type_context: TypeContext,
}

impl InferenceContext {
    /// Create a new inference context.
    pub fn new() -> Self {
        InferenceContext {
            next_var: 0,
            substitutions: HashMap::new(),
            type_context: TypeContext::new(),
        }
    }

    /// Create a fresh type variable.
    pub fn fresh_var(&mut self) -> Type {
        let var = TypeVar::new(self.next_var);
        self.next_var += 1;
        Type::Var(var)
    }

    /// Unify two types, returning error if they can't be unified.
    pub fn unify(&mut self, t1: &Type, t2: &Type) -> Result<(), TypeError> {
        let t1 = self.resolve(t1);
        let t2 = self.resolve(t2);

        match (&t1, &t2) {
            // Same types unify
            _ if t1 == t2 => Ok(()),

            // Type variables unify with anything
            (Type::Var(v), t) | (t, Type::Var(v)) => {
                // Occurs check
                if self.occurs(*v, t) {
                    return Err(TypeError::InfiniteType);
                }
                self.substitutions.insert(*v, t.clone());
                Ok(())
            }

            // Error type unifies with anything (for error recovery)
            (Type::Error, _) | (_, Type::Error) => Ok(()),

            // Function types
            (
                Type::Function { params: p1, ret: r1 },
                Type::Function { params: p2, ret: r2 },
            ) => {
                if p1.len() != p2.len() {
                    return Err(TypeError::ArgCountMismatch {
                        expected: p1.len(),
                        found: p2.len(),
                    });
                }
                for (a, b) in p1.iter().zip(p2.iter()) {
                    self.unify(a, b)?;
                }
                self.unify(r1, r2)
            }

            // Tuple types
            (Type::Tuple(t1), Type::Tuple(t2)) => {
                if t1.len() != t2.len() {
                    return Err(TypeError::TupleLengthMismatch {
                        expected: t1.len(),
                        found: t2.len(),
                    });
                }
                for (a, b) in t1.iter().zip(t2.iter()) {
                    self.unify(a, b)?;
                }
                Ok(())
            }

            // Single-parameter container types: unify inner types
            (Type::List(a), Type::List(b))
            | (Type::Option(a), Type::Option(b))
            | (Type::Set(a), Type::Set(b))
            | (Type::Range(a), Type::Range(b))
            | (Type::Channel(a), Type::Channel(b)) => self.unify(a, b),

            // Map types
            (
                Type::Map { key: k1, value: v1 },
                Type::Map { key: k2, value: v2 },
            ) => {
                self.unify(k1, k2)?;
                self.unify(v1, v2)
            }

            // Result types
            (
                Type::Result { ok: o1, err: e1 },
                Type::Result { ok: o2, err: e2 },
            ) => {
                self.unify(o1, o2)?;
                self.unify(e1, e2)
            }

            // Projection types: unify if same trait/assoc_name and bases unify
            (
                Type::Projection { base: b1, trait_name: t1, assoc_name: a1 },
                Type::Projection { base: b2, trait_name: t2, assoc_name: a2 },
            ) if t1 == t2 && a1 == a2 => {
                self.unify(b1, b2)
            }

            // Applied generic types: unify if same base name and args unify
            (
                Type::Applied { name: n1, args: a1 },
                Type::Applied { name: n2, args: a2 },
            ) if n1 == n2 && a1.len() == a2.len() => {
                for (arg1, arg2) in a1.iter().zip(a2.iter()) {
                    self.unify(arg1, arg2)?;
                }
                Ok(())
            }

            // Incompatible types
            _ => Err(TypeError::TypeMismatch {
                expected: t1,
                found: t2,
            }),
        }
    }

    /// Resolve a type by following substitutions.
    pub fn resolve(&self, ty: &Type) -> Type {
        struct Resolver<'a> {
            substitutions: &'a HashMap<TypeVar, Type>,
        }

        impl TypeFolder for Resolver<'_> {
            fn fold_var(&mut self, var: TypeVar) -> Type {
                if let Some(resolved) = self.substitutions.get(&var) {
                    self.fold(resolved)
                } else {
                    Type::Var(var)
                }
            }
        }

        let mut resolver = Resolver {
            substitutions: &self.substitutions,
        };
        resolver.fold(ty)
    }

    /// Check if a type variable occurs in a type (for occurs check).
    fn occurs(&self, var: TypeVar, ty: &Type) -> bool {
        struct OccursChecker<'a> {
            target: TypeVar,
            substitutions: &'a HashMap<TypeVar, Type>,
            found: bool,
        }

        impl TypeVisitor for OccursChecker<'_> {
            fn visit_var(&mut self, var: TypeVar) {
                if var == self.target {
                    self.found = true;
                } else if let Some(resolved) = self.substitutions.get(&var) {
                    self.visit(resolved);
                }
            }
        }

        let mut checker = OccursChecker {
            target: var,
            substitutions: &self.substitutions,
            found: false,
        };
        checker.visit(ty);
        checker.found
    }

    /// Collect all free type variables in a type.
    ///
    /// Free variables are those that appear in the type but are not in the
    /// current substitution (i.e., they haven't been unified with anything).
    pub fn free_vars(&self, ty: &Type) -> Vec<TypeVar> {
        struct FreeVarCollector<'a> {
            substitutions: &'a HashMap<TypeVar, Type>,
            vars: Vec<TypeVar>,
        }

        impl TypeVisitor for FreeVarCollector<'_> {
            fn visit_var(&mut self, var: TypeVar) {
                if let Some(resolved) = self.substitutions.get(&var) {
                    self.visit(resolved);
                } else if !self.vars.contains(&var) {
                    self.vars.push(var);
                }
            }
        }

        let mut collector = FreeVarCollector {
            substitutions: &self.substitutions,
            vars: Vec::new(),
        };
        collector.visit(ty);
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
        struct VarSubstitutor<'a> {
            mapping: &'a HashMap<TypeVar, Type>,
            substitutions: &'a HashMap<TypeVar, Type>,
        }

        impl TypeFolder for VarSubstitutor<'_> {
            fn fold_var(&mut self, var: TypeVar) -> Type {
                if let Some(replacement) = self.mapping.get(&var) {
                    replacement.clone()
                } else if let Some(resolved) = self.substitutions.get(&var) {
                    self.fold(resolved)
                } else {
                    Type::Var(var)
                }
            }
        }

        let mut substitutor = VarSubstitutor {
            mapping,
            substitutions: &self.substitutions,
        };
        substitutor.fold(ty)
    }

    // =========================================================================
    // Type context convenience methods for deduplicating compound types
    // =========================================================================

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

// ===== Type Instantiation Deduplication =====

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
