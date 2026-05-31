//! Function signature output types.
//!
//! Function signatures, const-generic parameter info, where-clause
//! constraints, and effect classification used by downstream phases for
//! call checking, ABI computation, and incremental test intelligence.

use ori_ir::{ExprId, Name, Span};

use crate::Idx;

/// Info about a const generic parameter (e.g., `$N: int`).
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ConstParamInfo {
    /// Parameter name (e.g., `N`).
    pub name: Name,
    /// The type of this const param (INT or BOOL).
    pub const_type: Idx,
    /// Optional default value expression.
    pub default_value: Option<ori_ir::ExprId>,
}

/// Function signature.
///
/// Contains all information needed to type-check calls to this function
/// from other modules.
///
/// # Generic Parameters
///
/// Generics are represented as type variables in the `type_params` field.
/// When calling a generic function, fresh variables are instantiated for
/// each type parameter.
#[allow(
    clippy::struct_excessive_bools,
    reason = "flags represent independent orthogonal properties"
)]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FunctionSig {
    /// Function name.
    pub name: Name,

    /// Generic type parameter names (e.g., `T`, `U` in `fn foo<T, U>`).
    pub type_params: Vec<Name>,

    /// Const generic parameters (e.g., `$N: int` in `@f<$N: int>`).
    /// Empty for non-const-generic functions.
    pub const_params: Vec<ConstParamInfo>,

    /// Parameter names.
    pub param_names: Vec<Name>,

    /// Parameter types.
    pub param_types: Vec<Idx>,

    /// Return type.
    pub return_type: Idx,

    /// Capabilities required by this function (`uses` clause).
    pub capabilities: Vec<Name>,

    /// Whether this function is public.
    pub is_public: bool,

    /// Whether this is a test function.
    pub is_test: bool,

    /// Whether this is the main entry point.
    pub is_main: bool,

    /// Whether this function is annotated `#fbip` for constructor-reuse enforcement.
    pub is_fbip: bool,

    /// Trait bounds for each generic type parameter (parallel to `type_params`).
    ///
    /// For `@foo<C: Container, T: Eq + Clone>`, this would be
    /// `[["Container"], ["Eq", "Clone"]]`.
    pub type_param_bounds: Vec<Vec<Name>>,

    /// Where-clause constraints.
    pub where_clauses: Vec<FnWhereClause>,

    /// Maps each generic type param to a function param index (if directly used).
    ///
    /// Parallel to `type_params`. For `@foo<C: Container>(c: C)`, this is `[Some(0)]`.
    /// For `@bar<T>(items: [T])`, this is `[None]` since T isn't a direct param type.
    pub generic_param_mapping: Vec<Option<usize>>,

    /// Pool `var_ids` for the scheme's quantified type variables.
    ///
    /// Parallel to `type_params`. Needed by the monomorphizer to build
    /// the `var_id` → `concrete_type` substitution map at call sites.
    /// Empty for non-generic functions.
    pub scheme_var_ids: Vec<u32>,

    /// Number of required parameters (those without default values).
    ///
    /// A call is valid if `required_params <= num_args <= param_types.len()`.
    pub required_params: usize,

    /// Default expressions for each parameter (parallel to `param_names`/`param_types`).
    ///
    /// `Some(expr_id)` if the parameter has a default value expression in the source AST,
    /// `None` if the parameter is required. Used by the canonicalizer to fill in omitted
    /// arguments when desugaring `CallNamed` to positional `Call`.
    pub param_defaults: Vec<Option<ExprId>>,

    /// Merkle hashes for parameter types — stable across Pool instances.
    ///
    /// `param_hashes[i]` is the content-addressed hash of `param_types[i]`.
    /// Used for cross-module type identity: receiving modules can look up
    /// types by hash in their own `intern_map` without AST re-walking.
    /// Always `param_hashes.len() == param_types.len()`.
    ///
    /// Zero when the signature was constructed without pool access (e.g.,
    /// test helpers, dummy signatures). Use [`populate_hashes()`](Self::populate_hashes)
    /// to fill in after construction.
    pub param_hashes: Vec<u64>,

    /// Merkle hash for the return type — stable across Pool instances.
    ///
    /// Zero when constructed without pool access.
    pub return_hash: u64,
}

/// A where-clause constraint on a function.
///
/// Represents `where C.Item: Eq` — a constraint on an associated type projection.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FnWhereClause {
    /// The type parameter being constrained (e.g., `C`).
    pub param: Name,
    /// Optional associated type projection (e.g., `Item` in `C.Item: Eq`).
    pub projection: Option<Name>,
    /// The trait bounds that must be satisfied.
    pub bounds: Vec<Name>,
    /// Source span.
    pub span: Span,
}

impl FunctionSig {
    /// Create a simple function signature with no generics or capabilities.
    ///
    /// Hash fields are initialized to zero. Call [`populate_hashes()`](Self::populate_hashes)
    /// to fill them from a Pool.
    pub fn simple(name: Name, param_types: Vec<Idx>, return_type: Idx) -> Self {
        let required_params = param_types.len();
        let param_hashes = vec![0; param_types.len()];
        Self {
            name,
            type_params: Vec::new(),
            const_params: Vec::new(),
            param_names: Vec::new(),
            param_types,
            return_type,
            capabilities: Vec::new(),
            is_public: false,
            is_test: false,
            is_main: false,
            is_fbip: false,
            type_param_bounds: Vec::new(),
            where_clauses: Vec::new(),
            generic_param_mapping: Vec::new(),
            scheme_var_ids: Vec::new(),
            required_params,
            param_defaults: Vec::new(),
            param_hashes,
            return_hash: 0,
        }
    }

    /// Create a synthetic function signature for compiler-generated methods.
    ///
    /// Like [`simple`](Self::simple) but includes parameter names, which are
    /// needed for ABI computation in derived trait methods. All other fields
    /// (generics, capabilities, flags) default to empty/false.
    ///
    /// Hash fields are initialized to zero. Call [`populate_hashes()`](Self::populate_hashes)
    /// to fill them from a Pool.
    pub fn synthetic(
        name: Name,
        param_names: Vec<Name>,
        param_types: Vec<Idx>,
        return_type: Idx,
    ) -> Self {
        let required_params = param_types.len();
        let param_hashes = vec![0; param_types.len()];
        Self {
            name,
            type_params: Vec::new(),
            const_params: Vec::new(),
            param_names,
            param_types,
            return_type,
            capabilities: Vec::new(),
            is_public: false,
            is_test: false,
            is_main: false,
            is_fbip: false,
            type_param_bounds: Vec::new(),
            where_clauses: Vec::new(),
            generic_param_mapping: Vec::new(),
            scheme_var_ids: Vec::new(),
            required_params,
            param_defaults: Vec::new(),
            param_hashes,
            return_hash: 0,
        }
    }

    /// Populate Merkle hash fields from a Pool.
    ///
    /// Computes content-addressed hashes for all parameter types and the return
    /// type, enabling cross-module type identity without pool access. Call this
    /// after construction when a Pool is available.
    ///
    /// # Panics
    ///
    /// Panics in debug mode if any `Idx` is out of bounds for the pool.
    pub fn populate_hashes(&mut self, pool: &crate::Pool) {
        self.param_hashes = self.param_types.iter().map(|&idx| pool.hash(idx)).collect();
        self.return_hash = pool.hash(self.return_type);
    }

    /// Get the function type as an `Idx`.
    ///
    /// Requires a mutable pool to create the function type.
    pub fn to_function_type(&self, pool: &mut crate::Pool) -> Idx {
        pool.function(&self.param_types, self.return_type)
    }

    /// Get the arity (number of parameters).
    pub fn arity(&self) -> usize {
        self.param_types.len()
    }

    /// Check if this function is generic.
    pub fn is_generic(&self) -> bool {
        !self.type_params.is_empty() || !self.const_params.is_empty()
    }

    /// Check if this function uses capabilities.
    pub fn has_capabilities(&self) -> bool {
        !self.capabilities.is_empty()
    }

    /// Classify this function's effect level based on its declared capabilities.
    ///
    /// Requires the `StringInterner` to resolve capability `Name`s to strings
    /// for classification against the known capability categories.
    pub fn effect_class(&self, interner: &ori_ir::StringInterner) -> EffectClass {
        if self.capabilities.is_empty() {
            return EffectClass::Pure;
        }

        for &cap in &self.capabilities {
            let cap_str = interner.lookup(cap);
            if !READ_ONLY_CAPABILITIES.contains(&cap_str) {
                return EffectClass::HasEffects;
            }
        }

        EffectClass::ReadsOnly
    }
}

/// Classification of a function's effect level based on its capabilities.
///
/// Used for incremental test intelligence: pure functions produce deterministic
/// results, enabling aggressive caching of their test outcomes.
///
/// # Ordering
///
/// `Pure < ReadsOnly < HasEffects` — more effects means less cacheability.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, Ord, PartialOrd)]
pub enum EffectClass {
    /// No capabilities — fully deterministic, safely parallelizable.
    Pure,
    /// Only reads external state (Env, Clock, Random) — may vary between runs
    /// but has no observable side effects.
    ReadsOnly,
    /// Performs I/O or mutation (`Http`, `FileSystem`, `Print`, etc.).
    HasEffects,
}

/// Capability names that are classified as read-only (no side effects).
///
/// These capabilities read external state but don't mutate it.
/// From the spec: Clock (time), Random (entropy), Env (environment variables).
const READ_ONLY_CAPABILITIES: &[&str] = &["Env", "Clock", "Random"];
