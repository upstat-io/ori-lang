//! Type system for Sigil.
//!
//! Per design spec 02-design-principlesmd:
//! - All types have Clone, Eq, Hash for Salsa compatibility
//! - Interned type representations for efficiency
//! - Flat structures for cache locality

use sigil_ir::{Name, Span, StringInterner};
use std::hash::Hash;
use std::collections::HashMap;

// Size assertions to prevent accidental regressions.
// Type is used throughout type checking and stored in query results.
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    use super::*;
    // Type enum: largest variant is Function with Vec<Type> (24) + Box<Type> (8) = 32 bytes
    // The enum discriminant fits within the alignment padding.
    sigil_ir::static_assert_size!(Type, 32);
    // TypeVar is just a u32 wrapper
    sigil_ir::static_assert_size!(TypeVar, 4);
}

/// Concrete type representation.
///
/// # Salsa Compatibility
/// Has Clone, Eq, Hash for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Type {
    // ===== Primitives =====
    /// Integer type
    Int,
    /// Floating point type
    Float,
    /// Boolean type
    Bool,
    /// String type
    Str,
    /// Character type
    Char,
    /// Byte type
    Byte,
    /// Unit type ()
    Unit,
    /// Never type (diverging)
    Never,

    // ===== Special types =====
    /// Duration type (30s, 100ms)
    Duration,
    /// Size type (4kb, 10mb)
    Size,

    // ===== Compound types =====
    /// Function type: (params) -> return
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
    },

    /// Tuple type: (T, U, V)
    Tuple(Vec<Type>),

    /// List type: [T]
    List(Box<Type>),

    /// Map type: {K: V}
    Map {
        key: Box<Type>,
        value: Box<Type>,
    },

    /// Set type: Set<T>
    Set(Box<Type>),

    /// Option type: Option<T>
    Option(Box<Type>),

    /// Result type: Result<T, E>
    Result {
        ok: Box<Type>,
        err: Box<Type>,
    },

    /// Range type: Range<T>
    Range(Box<Type>),

    /// Channel type: Channel<T>
    Channel(Box<Type>),

    // ===== Named types =====
    /// User-defined type reference
    Named(Name),

    /// Generic type variable (for inference)
    Var(TypeVar),

    /// Error type (for error recovery)
    Error,
}

impl Type {
    /// Check if this is a primitive type.
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            Type::Int | Type::Float | Type::Bool | Type::Str |
            Type::Char | Type::Byte | Type::Unit | Type::Never
        )
    }

    /// Check if this is the error type.
    pub fn is_error(&self) -> bool {
        matches!(self, Type::Error)
    }

    /// Check if this is a type variable.
    pub fn is_var(&self) -> bool {
        matches!(self, Type::Var(_))
    }

    /// Get inner type for Option, List, etc.
    pub fn inner(&self) -> Option<&Type> {
        match self {
            Type::List(t) | Type::Option(t) | Type::Set(t) |
            Type::Range(t) | Type::Channel(t) => Some(t),
            _ => None,
        }
    }

    /// Format type for display.
    pub fn display(&self, interner: &StringInterner) -> String {
        match self {
            Type::Int => "int".to_string(),
            Type::Float => "float".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Str => "str".to_string(),
            Type::Char => "char".to_string(),
            Type::Byte => "byte".to_string(),
            Type::Unit => "()".to_string(),
            Type::Never => "Never".to_string(),
            Type::Duration => "Duration".to_string(),
            Type::Size => "Size".to_string(),
            Type::Function { params, ret } => {
                let params_str: Vec<_> = params.iter()
                    .map(|p| p.display(interner))
                    .collect();
                format!("({}) -> {}", params_str.join(", "), ret.display(interner))
            }
            Type::Tuple(types) => {
                let types_str: Vec<_> = types.iter()
                    .map(|t| t.display(interner))
                    .collect();
                format!("({})", types_str.join(", "))
            }
            Type::List(t) => format!("[{}]", t.display(interner)),
            Type::Map { key, value } => {
                format!("{{{}: {}}}", key.display(interner), value.display(interner))
            }
            Type::Set(t) => format!("Set<{}>", t.display(interner)),
            Type::Option(t) => format!("Option<{}>", t.display(interner)),
            Type::Result { ok, err } => {
                format!("Result<{}, {}>", ok.display(interner), err.display(interner))
            }
            Type::Range(t) => format!("Range<{}>", t.display(interner)),
            Type::Channel(t) => format!("Channel<{}>", t.display(interner)),
            Type::Named(name) => interner.lookup(*name).to_string(),
            Type::Var(v) => format!("?{}", v.0),
            Type::Error => "<error>".to_string(),
        }
    }
}

/// Type variable for inference.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeVar(pub u32);

impl TypeVar {
    pub fn new(id: u32) -> Self {
        TypeVar(id)
    }
}

/// A type scheme (polymorphic type) with quantified type variables.
///
/// For example, the identity function `fn id<T>(x: T) -> T` has type scheme:
/// `TypeScheme { vars: [T], ty: T -> T }`
///
/// When used, we instantiate fresh type variables for each quantified variable.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeScheme {
    /// Quantified type variables (∀ these variables)
    pub vars: Vec<TypeVar>,
    /// The type with potentially free type variables
    pub ty: Type,
}

impl TypeScheme {
    /// Create a monomorphic scheme (no quantified variables).
    pub fn mono(ty: Type) -> Self {
        TypeScheme {
            vars: Vec::new(),
            ty,
        }
    }

    /// Create a polymorphic scheme with the given quantified variables.
    pub fn poly(vars: Vec<TypeVar>, ty: Type) -> Self {
        TypeScheme { vars, ty }
    }

    /// Check if this is a monomorphic type (no quantified variables).
    pub fn is_mono(&self) -> bool {
        self.vars.is_empty()
    }
}

/// Type environment for name resolution and scoping.
///
/// Supports both monomorphic types and polymorphic type schemes.
#[derive(Clone, Debug, Default)]
pub struct TypeEnv {
    /// Variable bindings: name -> type scheme
    bindings: HashMap<Name, TypeScheme>,
    /// Parent scope (for nested scopes)
    parent: Option<Box<TypeEnv>>,
}

impl TypeEnv {
    /// Create a new empty environment.
    pub fn new() -> Self {
        TypeEnv::default()
    }

    /// Create a child scope.
    pub fn child(&self) -> Self {
        TypeEnv {
            bindings: HashMap::new(),
            parent: Some(Box::new(self.clone())),
        }
    }

    /// Bind a name to a monomorphic type in the current scope.
    pub fn bind(&mut self, name: Name, ty: Type) {
        self.bindings.insert(name, TypeScheme::mono(ty));
    }

    /// Bind a name to a polymorphic type scheme in the current scope.
    pub fn bind_scheme(&mut self, name: Name, scheme: TypeScheme) {
        self.bindings.insert(name, scheme);
    }

    /// Look up a name, searching parent scopes.
    /// Returns the type scheme (use instantiate to get a concrete type).
    pub fn lookup_scheme(&self, name: Name) -> Option<&TypeScheme> {
        self.bindings.get(&name).or_else(|| {
            self.parent.as_ref().and_then(|p| p.lookup_scheme(name))
        })
    }

    /// Look up a name and return just the type (for monomorphic lookups).
    /// For polymorphic types, returns the uninstantiated type.
    pub fn lookup(&self, name: Name) -> Option<&Type> {
        self.lookup_scheme(name).map(|s| &s.ty)
    }

    /// Check if a name is bound in the current scope only.
    pub fn is_bound_locally(&self, name: Name) -> bool {
        self.bindings.contains_key(&name)
    }

    /// Collect all free type variables in the environment.
    ///
    /// This is used during generalization to avoid quantifying over
    /// variables that are free in the environment.
    pub fn free_vars(&self, ctx: &InferenceContext) -> Vec<TypeVar> {
        let mut vars = Vec::new();
        self.collect_env_free_vars(ctx, &mut vars);
        vars
    }

    fn collect_env_free_vars(&self, ctx: &InferenceContext, vars: &mut Vec<TypeVar>) {
        for scheme in self.bindings.values() {
            // Only collect free vars that are NOT quantified in the scheme
            let scheme_free = ctx.free_vars(&scheme.ty);
            for v in scheme_free {
                if !scheme.vars.contains(&v) && !vars.contains(&v) {
                    vars.push(v);
                }
            }
        }
        if let Some(parent) = &self.parent {
            parent.collect_env_free_vars(ctx, vars);
        }
    }
}

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

            // List types
            (Type::List(a), Type::List(b)) => self.unify(a, b),

            // Map types
            (
                Type::Map { key: k1, value: v1 },
                Type::Map { key: k2, value: v2 },
            ) => {
                self.unify(k1, k2)?;
                self.unify(v1, v2)
            }

            // Option types
            (Type::Option(a), Type::Option(b)) => self.unify(a, b),

            // Result types
            (
                Type::Result { ok: o1, err: e1 },
                Type::Result { ok: o2, err: e2 },
            ) => {
                self.unify(o1, o2)?;
                self.unify(e1, e2)
            }

            // Set types
            (Type::Set(a), Type::Set(b)) => self.unify(a, b),

            // Range types
            (Type::Range(a), Type::Range(b)) => self.unify(a, b),

            // Channel types
            (Type::Channel(a), Type::Channel(b)) => self.unify(a, b),

            // Incompatible types
            _ => Err(TypeError::TypeMismatch {
                expected: t1,
                found: t2,
            }),
        }
    }

    /// Resolve a type by following substitutions.
    pub fn resolve(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(v) => {
                if let Some(resolved) = self.substitutions.get(v) {
                    self.resolve(resolved)
                } else {
                    ty.clone()
                }
            }
            Type::Function { params, ret } => Type::Function {
                params: params.iter().map(|p| self.resolve(p)).collect(),
                ret: Box::new(self.resolve(ret)),
            },
            Type::Tuple(types) => {
                Type::Tuple(types.iter().map(|t| self.resolve(t)).collect())
            }
            Type::List(t) => Type::List(Box::new(self.resolve(t))),
            Type::Map { key, value } => Type::Map {
                key: Box::new(self.resolve(key)),
                value: Box::new(self.resolve(value)),
            },
            Type::Option(t) => Type::Option(Box::new(self.resolve(t))),
            Type::Result { ok, err } => Type::Result {
                ok: Box::new(self.resolve(ok)),
                err: Box::new(self.resolve(err)),
            },
            Type::Set(t) => Type::Set(Box::new(self.resolve(t))),
            Type::Range(t) => Type::Range(Box::new(self.resolve(t))),
            Type::Channel(t) => Type::Channel(Box::new(self.resolve(t))),
            _ => ty.clone(),
        }
    }

    /// Check if a type variable occurs in a type (for occurs check).
    fn occurs(&self, var: TypeVar, ty: &Type) -> bool {
        match ty {
            Type::Var(v) => {
                if *v == var {
                    return true;
                }
                if let Some(resolved) = self.substitutions.get(v) {
                    return self.occurs(var, resolved);
                }
                false
            }
            Type::Function { params, ret } => {
                params.iter().any(|p| self.occurs(var, p)) || self.occurs(var, ret)
            }
            Type::Tuple(types) => types.iter().any(|t| self.occurs(var, t)),
            Type::List(t) | Type::Option(t) | Type::Set(t) |
            Type::Range(t) | Type::Channel(t) => self.occurs(var, t),
            Type::Map { key, value } => self.occurs(var, key) || self.occurs(var, value),
            Type::Result { ok, err } => self.occurs(var, ok) || self.occurs(var, err),
            _ => false,
        }
    }

    /// Collect all free type variables in a type.
    ///
    /// Free variables are those that appear in the type but are not in the
    /// current substitution (i.e., they haven't been unified with anything).
    pub fn free_vars(&self, ty: &Type) -> Vec<TypeVar> {
        let mut vars = Vec::new();
        self.collect_free_vars(ty, &mut vars);
        vars
    }

    fn collect_free_vars(&self, ty: &Type, vars: &mut Vec<TypeVar>) {
        match ty {
            Type::Var(v) => {
                if let Some(resolved) = self.substitutions.get(v) {
                    self.collect_free_vars(resolved, vars);
                } else if !vars.contains(v) {
                    vars.push(*v);
                }
            }
            Type::Function { params, ret } => {
                for p in params {
                    self.collect_free_vars(p, vars);
                }
                self.collect_free_vars(ret, vars);
            }
            Type::Tuple(types) => {
                for t in types {
                    self.collect_free_vars(t, vars);
                }
            }
            Type::List(t) | Type::Option(t) | Type::Set(t) |
            Type::Range(t) | Type::Channel(t) => {
                self.collect_free_vars(t, vars);
            }
            Type::Map { key, value } => {
                self.collect_free_vars(key, vars);
                self.collect_free_vars(value, vars);
            }
            Type::Result { ok, err } => {
                self.collect_free_vars(ok, vars);
                self.collect_free_vars(err, vars);
            }
            _ => {}
        }
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
        match ty {
            Type::Var(v) => {
                if let Some(replacement) = mapping.get(v) {
                    replacement.clone()
                } else if let Some(resolved) = self.substitutions.get(v) {
                    self.substitute_vars(resolved, mapping)
                } else {
                    ty.clone()
                }
            }
            Type::Function { params, ret } => Type::Function {
                params: params.iter().map(|p| self.substitute_vars(p, mapping)).collect(),
                ret: Box::new(self.substitute_vars(ret, mapping)),
            },
            Type::Tuple(types) => {
                Type::Tuple(types.iter().map(|t| self.substitute_vars(t, mapping)).collect())
            }
            Type::List(t) => Type::List(Box::new(self.substitute_vars(t, mapping))),
            Type::Map { key, value } => Type::Map {
                key: Box::new(self.substitute_vars(key, mapping)),
                value: Box::new(self.substitute_vars(value, mapping)),
            },
            Type::Option(t) => Type::Option(Box::new(self.substitute_vars(t, mapping))),
            Type::Result { ok, err } => Type::Result {
                ok: Box::new(self.substitute_vars(ok, mapping)),
                err: Box::new(self.substitute_vars(err, mapping)),
            },
            Type::Set(t) => Type::Set(Box::new(self.substitute_vars(t, mapping))),
            Type::Range(t) => Type::Range(Box::new(self.substitute_vars(t, mapping))),
            Type::Channel(t) => Type::Channel(Box::new(self.substitute_vars(t, mapping))),
            _ => ty.clone(),
        }
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

use std::hash::Hasher;
use std::collections::hash_map::DefaultHasher;

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
/// Note: Salsa handles cross-query memoization, but TypeContext deduplicates
/// **within** a single type-checking pass.
#[derive(Clone, Debug, Default)]
pub struct TypeContext {
    /// hash(origin_id + targs) -> list of entries with that hash
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

/// Type error.
#[derive(Clone, Debug)]
pub enum TypeError {
    /// Type mismatch.
    TypeMismatch {
        expected: Type,
        found: Type,
    },
    /// Argument count mismatch.
    ArgCountMismatch {
        expected: usize,
        found: usize,
    },
    /// Tuple length mismatch.
    TupleLengthMismatch {
        expected: usize,
        found: usize,
    },
    /// Infinite type (occurs check failure).
    InfiniteType,
    /// Unknown identifier.
    UnknownIdent(Name),
    /// Cannot infer type.
    CannotInfer,
}

impl TypeError {
    /// Convert to a diagnostic with helpful suggestions.
    pub fn to_diagnostic(
        &self,
        span: Span,
        interner: &StringInterner,
    ) -> sigil_diagnostic::Diagnostic {
        use sigil_diagnostic::{Diagnostic, ErrorCode};

        match self {
            TypeError::TypeMismatch { expected, found } => {
                let exp_str = expected.display(interner);
                let found_str = found.display(interner);

                let mut diag = Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "type mismatch: expected `{}`, found `{}`",
                        exp_str, found_str,
                    ))
                    .with_label(span, format!("expected `{}`", exp_str));

                // Add helpful suggestions for common mistakes
                diag = match (expected, found) {
                    (Type::Bool, Type::Int) => {
                        diag.with_suggestion("use a comparison operator (e.g., `x != 0`) to convert int to bool")
                    }
                    (Type::Int, Type::Float) => {
                        diag.with_suggestion("use `int(x)` to convert float to int")
                    }
                    (Type::Float, Type::Int) => {
                        diag.with_suggestion("use `float(x)` to convert int to float")
                    }
                    (Type::Str, _) => {
                        diag.with_suggestion("use `str(x)` to convert to string")
                    }
                    (Type::List(_), t) if !matches!(t, Type::List(_)) => {
                        diag.with_suggestion("wrap the value in a list: `[x]`")
                    }
                    (Type::Option(_), t) if !matches!(t, Type::Option(_) | Type::Var(_)) => {
                        diag.with_suggestion("wrap the value in Some: `Some(x)`")
                    }
                    _ => diag,
                };

                diag
            }
            TypeError::ArgCountMismatch { expected, found } => {
                let plural = if *expected == 1 { "" } else { "s" };
                Diagnostic::error(ErrorCode::E2004)
                    .with_message(format!(
                        "wrong number of arguments: expected {}, found {}",
                        expected, found,
                    ))
                    .with_label(span, format!("expected {} argument{}", expected, plural))
                    .with_suggestion(if *found > *expected {
                        "remove extra arguments"
                    } else {
                        "add missing arguments"
                    })
            }
            TypeError::TupleLengthMismatch { expected, found } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "tuple length mismatch: expected {}-tuple, found {}-tuple",
                        expected, found,
                    ))
                    .with_label(span, format!("expected {} elements", expected))
            }
            TypeError::InfiniteType => {
                Diagnostic::error(ErrorCode::E2005)
                    .with_message("cannot construct infinite type (occurs check failed)")
                    .with_label(span, "this creates a self-referential type")
                    .with_suggestion("break the cycle by introducing an intermediate type")
            }
            TypeError::UnknownIdent(name) => {
                let name_str = interner.lookup(*name);
                Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown identifier `{}`", name_str))
                    .with_label(span, "not found in this scope")
                    .with_suggestion(format!(
                        "check spelling, or add a definition for `{}`", name_str
                    ))
            }
            TypeError::CannotInfer => {
                Diagnostic::error(ErrorCode::E2005)
                    .with_message("cannot infer type: insufficient context")
                    .with_label(span, "type annotation needed here")
                    .with_suggestion("add an explicit type annotation like `: int` or `: str`")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_ir::SharedInterner;

    #[test]
    fn test_type_primitives() {
        assert!(Type::Int.is_primitive());
        assert!(Type::Float.is_primitive());
        assert!(Type::Bool.is_primitive());
        assert!(!Type::List(Box::new(Type::Int)).is_primitive());
    }

    #[test]
    fn test_type_display() {
        let interner = SharedInterner::default();

        assert_eq!(Type::Int.display(&interner), "int");
        assert_eq!(Type::List(Box::new(Type::Int)).display(&interner), "[int]");
        assert_eq!(
            Type::Function {
                params: vec![Type::Int, Type::Bool],
                ret: Box::new(Type::Str),
            }.display(&interner),
            "(int, bool) -> str"
        );
    }

    #[test]
    fn test_type_env_scoping() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let y = interner.intern("y");

        let mut env = TypeEnv::new();
        env.bind(x, Type::Int);

        // x is visible
        assert_eq!(env.lookup(x), Some(&Type::Int));
        // y is not visible
        assert_eq!(env.lookup(y), None);

        // Create child scope
        let mut child = env.child();
        child.bind(y, Type::Bool);

        // x is still visible (from parent)
        assert_eq!(child.lookup(x), Some(&Type::Int));
        // y is now visible
        assert_eq!(child.lookup(y), Some(&Type::Bool));

        // y is not visible in parent
        assert_eq!(env.lookup(y), None);
    }

    #[test]
    fn test_unify_same_types() {
        let mut ctx = InferenceContext::new();

        assert!(ctx.unify(&Type::Int, &Type::Int).is_ok());
        assert!(ctx.unify(&Type::Bool, &Type::Bool).is_ok());
    }

    #[test]
    fn test_unify_different_types() {
        let mut ctx = InferenceContext::new();

        assert!(ctx.unify(&Type::Int, &Type::Bool).is_err());
    }

    #[test]
    fn test_unify_type_var() {
        let mut ctx = InferenceContext::new();
        let var = ctx.fresh_var();

        assert!(ctx.unify(&var, &Type::Int).is_ok());
        assert_eq!(ctx.resolve(&var), Type::Int);
    }

    #[test]
    fn test_unify_functions() {
        let mut ctx = InferenceContext::new();

        let f1 = Type::Function {
            params: vec![Type::Int],
            ret: Box::new(Type::Bool),
        };
        let f2 = Type::Function {
            params: vec![Type::Int],
            ret: Box::new(Type::Bool),
        };

        assert!(ctx.unify(&f1, &f2).is_ok());
    }

    #[test]
    fn test_unify_functions_mismatch() {
        let mut ctx = InferenceContext::new();

        let f1 = Type::Function {
            params: vec![Type::Int],
            ret: Box::new(Type::Bool),
        };
        let f2 = Type::Function {
            params: vec![Type::Int, Type::Int],
            ret: Box::new(Type::Bool),
        };

        assert!(matches!(
            ctx.unify(&f1, &f2),
            Err(TypeError::ArgCountMismatch { .. })
        ));
    }

    #[test]
    fn test_unify_lists() {
        let mut ctx = InferenceContext::new();
        let var = ctx.fresh_var();

        let list1 = Type::List(Box::new(var.clone()));
        let list2 = Type::List(Box::new(Type::Int));

        assert!(ctx.unify(&list1, &list2).is_ok());
        assert_eq!(ctx.resolve(&var), Type::Int);
    }

    #[test]
    fn test_occurs_check() {
        let mut ctx = InferenceContext::new();
        let var = ctx.fresh_var();

        // Try to unify ?0 with [?0] - should fail
        let list = Type::List(Box::new(var.clone()));

        assert!(matches!(
            ctx.unify(&var, &list),
            Err(TypeError::InfiniteType)
        ));
    }

    #[test]
    fn test_type_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        set.insert(Type::Int);
        set.insert(Type::Int); // duplicate
        set.insert(Type::Bool);
        set.insert(Type::List(Box::new(Type::Int)));

        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_type_error_to_diagnostic() {
        let interner = SharedInterner::default();
        let err = TypeError::TypeMismatch {
            expected: Type::Int,
            found: Type::Bool,
        };

        let diag = err.to_diagnostic(Span::new(0, 10), &interner);
        assert!(diag.message.contains("int"));
        assert!(diag.message.contains("bool"));
    }

    #[test]
    fn test_type_scheme_mono() {
        let scheme = TypeScheme::mono(Type::Int);
        assert!(scheme.is_mono());
        assert!(scheme.vars.is_empty());
        assert_eq!(scheme.ty, Type::Int);
    }

    #[test]
    fn test_type_scheme_poly() {
        let var = TypeVar::new(0);
        let scheme = TypeScheme::poly(
            vec![var],
            Type::Function {
                params: vec![Type::Var(var)],
                ret: Box::new(Type::Var(var)),
            },
        );
        assert!(!scheme.is_mono());
        assert_eq!(scheme.vars.len(), 1);
    }

    #[test]
    fn test_free_vars() {
        let mut ctx = InferenceContext::new();
        let var1 = ctx.fresh_var();
        let var2 = ctx.fresh_var();

        // Simple variable has itself as free var
        let free = ctx.free_vars(&var1);
        assert_eq!(free.len(), 1);

        // Unified variables resolve
        ctx.unify(&var1, &Type::Int).unwrap();
        let free = ctx.free_vars(&var1);
        assert!(free.is_empty()); // Int has no free vars

        // Compound type
        let fn_ty = Type::Function {
            params: vec![var2.clone()],
            ret: Box::new(var2.clone()),
        };
        let free = ctx.free_vars(&fn_ty);
        assert_eq!(free.len(), 1); // Only var2
    }

    #[test]
    fn test_generalize() {
        let mut ctx = InferenceContext::new();
        let var = ctx.fresh_var();

        // Function with free variable
        let fn_ty = Type::Function {
            params: vec![var.clone()],
            ret: Box::new(var.clone()),
        };

        // Generalize with empty environment free vars
        let scheme = ctx.generalize(&fn_ty, &[]);
        assert!(!scheme.is_mono());
        assert_eq!(scheme.vars.len(), 1);
    }

    #[test]
    fn test_generalize_with_env_vars() {
        let mut ctx = InferenceContext::new();
        let var1 = ctx.fresh_var();
        let var2 = ctx.fresh_var();

        // Extract the TypeVars
        let tv1 = if let Type::Var(v) = var1 { v } else { panic!() };
        let tv2 = if let Type::Var(v) = var2 { v } else { panic!() };

        // Function using both vars
        let fn_ty = Type::Function {
            params: vec![var1.clone()],
            ret: Box::new(var2.clone()),
        };

        // Generalize but tv1 is in the environment (shouldn't be quantified)
        let scheme = ctx.generalize(&fn_ty, &[tv1]);

        // Only tv2 should be quantified
        assert!(!scheme.is_mono());
        assert_eq!(scheme.vars.len(), 1);
        assert_eq!(scheme.vars[0], tv2);
    }

    #[test]
    fn test_instantiate() {
        let mut ctx = InferenceContext::new();

        // Create a polymorphic identity scheme: ∀a. a -> a
        let var = TypeVar::new(0);
        let scheme = TypeScheme::poly(
            vec![var],
            Type::Function {
                params: vec![Type::Var(var)],
                ret: Box::new(Type::Var(var)),
            },
        );

        // Instantiate twice - should get different fresh variables
        let ty1 = ctx.instantiate(&scheme);
        let ty2 = ctx.instantiate(&scheme);

        // Both should be function types
        assert!(matches!(ty1, Type::Function { .. }));
        assert!(matches!(ty2, Type::Function { .. }));

        // They should have different type variables
        if let (
            Type::Function { params: p1, ret: r1 },
            Type::Function { params: p2, ret: r2 },
        ) = (ty1, ty2)
        {
            // Each instantiation gets fresh variables
            assert_ne!(p1[0], p2[0]);
            // But within each instantiation, param and return should be same var
            assert_eq!(p1[0], *r1);
            assert_eq!(p2[0], *r2);
        } else {
            panic!("Expected function types");
        }
    }

    #[test]
    fn test_instantiate_mono() {
        let mut ctx = InferenceContext::new();
        let scheme = TypeScheme::mono(Type::Int);

        let ty = ctx.instantiate(&scheme);
        assert_eq!(ty, Type::Int);
    }

    #[test]
    fn test_type_context_list_dedup() {
        let mut ctx = TypeContext::new();

        let list1 = ctx.list_type(Type::Int);
        let list2 = ctx.list_type(Type::Int);
        let list3 = ctx.list_type(Type::Bool);

        // Same type args should return equal types
        assert_eq!(list1, list2);
        // Different type args should return different types
        assert_ne!(list1, list3);
    }

    #[test]
    fn test_type_context_option_dedup() {
        let mut ctx = TypeContext::new();

        let opt1 = ctx.option_type(Type::Str);
        let opt2 = ctx.option_type(Type::Str);
        let opt3 = ctx.option_type(Type::Int);

        assert_eq!(opt1, opt2);
        assert_ne!(opt1, opt3);
    }

    #[test]
    fn test_type_context_result_dedup() {
        let mut ctx = TypeContext::new();

        let res1 = ctx.result_type(Type::Int, Type::Str);
        let res2 = ctx.result_type(Type::Int, Type::Str);
        let res3 = ctx.result_type(Type::Bool, Type::Str);

        assert_eq!(res1, res2);
        assert_ne!(res1, res3);
    }

    #[test]
    fn test_type_context_map_dedup() {
        let mut ctx = TypeContext::new();

        let map1 = ctx.map_type(Type::Str, Type::Int);
        let map2 = ctx.map_type(Type::Str, Type::Int);
        let map3 = ctx.map_type(Type::Int, Type::Int);

        assert_eq!(map1, map2);
        assert_ne!(map1, map3);
    }

    #[test]
    fn test_type_context_lookup_insert() {
        let mut ctx = TypeContext::new();

        let var = TypeVar::new(100);
        let origin = TypeScheme::poly(
            vec![var],
            Type::Function {
                params: vec![Type::Var(var)],
                ret: Box::new(Type::Var(var)),
            },
        );
        let targs = vec![Type::Int];
        let instance = Type::Function {
            params: vec![Type::Int],
            ret: Box::new(Type::Int),
        };

        // Insert and get back
        let result = ctx.insert(origin.clone(), targs.clone(), instance.clone());
        assert_eq!(result, instance);

        // Lookup should find it
        let found = ctx.lookup(&origin, &targs);
        assert_eq!(found, Some(&instance));

        // Different targs should not find it
        let not_found = ctx.lookup(&origin, &[Type::Bool]);
        assert!(not_found.is_none());
    }
}
