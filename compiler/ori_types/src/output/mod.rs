//! Type Checker Output Types.
//!
//! This module provides the output structures for the type checker.
//! All types are Salsa-compatible (`Clone, Eq, PartialEq, Hash, Debug`).
//!
//! # Key Types
//!
//! - [`TypedModule`]: Complete type information for a module
//! - [`FunctionSig`]: Function signature with parameter and return types
//! - [`TypeCheckResult`]: Wrapper with errors and guarantee
//!
//! Uses [`Idx`] (pool-based) instead of `TypeId` (legacy interning).

use ori_diagnostic::ErrorGuaranteed;
use ori_ir::{ExprId, Name, PatternKey, PatternResolution, Span};

use crate::registry::TypeEntry;
use crate::{Idx, TypeCheckError, TypeCheckWarning};

/// Type-checked module.
///
/// Contains all type information computed by the inference engine.
/// Uses `Idx` for O(1) type comparisons via the unified Pool.
///
/// # Salsa Compatibility
///
/// Derives all traits required for Salsa query results.
///
/// # Example
///
/// ```ignore
/// let result = type_check_module(db, file);
/// if result.has_errors() {
///     for err in &result.typed.errors {
///         // report error
///     }
/// }
/// // Get type of expression 42
/// let ty = result.typed.expr_type(42);
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct TypedModule {
    /// Type of each expression, indexed by expression ID.
    ///
    /// This is stored as a Vec for O(1) access. Expression IDs are
    /// sequential starting from 0 in each module.
    pub expr_types: Vec<Idx>,

    /// Function signatures by name.
    ///
    /// Sorted by name for deterministic output.
    pub functions: Vec<FunctionSig>,

    /// User-defined type definitions (structs, enums, newtypes, aliases).
    ///
    /// Exported from the module's `TypeRegistry` for cross-module type
    /// resolution. Sorted by name (from `BTreeMap` iteration order).
    pub types: Vec<TypeEntry>,

    /// Type errors accumulated during type checking.
    pub errors: Vec<TypeCheckError>,

    /// Type warnings accumulated during type checking.
    ///
    /// Warnings indicate suspicious but valid code (e.g., infinite iterator
    /// consumed without `.take()`). They do not prevent compilation.
    pub warnings: Vec<TypeCheckWarning>,

    /// Resolved patterns: `Binding` names disambiguated to unit variants.
    ///
    /// Sorted by `PatternKey` for O(log n) binary search via `resolve_pattern()`.
    /// Only patterns that were resolved are stored — unresolved bindings are
    /// normal variable bindings and have no entry.
    pub pattern_resolutions: Vec<(PatternKey, PatternResolution)>,

    /// Impl method signatures for codegen.
    ///
    /// Each entry maps a method name to its resolved `FunctionSig`. Codegen
    /// needs these to compute ABI (calling convention, sret, parameter passing)
    /// for impl methods, which are compiled separately from top-level functions.
    pub impl_sigs: Vec<(Name, FunctionSig)>,
}

impl TypedModule {
    /// Create a new empty typed module.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a typed module with pre-allocated capacity.
    pub fn with_capacity(expr_count: usize, function_count: usize) -> Self {
        Self {
            expr_types: Vec::with_capacity(expr_count),
            functions: Vec::with_capacity(function_count),
            types: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
            pattern_resolutions: Vec::new(),
            impl_sigs: Vec::new(),
        }
    }

    /// Check if this module has type errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get the type of an expression by index.
    ///
    /// Returns `None` if the expression index is out of bounds.
    pub fn expr_type(&self, expr_index: usize) -> Option<Idx> {
        self.expr_types.get(expr_index).copied()
    }

    /// Get a function signature by name.
    pub fn function(&self, name: Name) -> Option<&FunctionSig> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// Get a type definition by name.
    pub fn type_def(&self, name: Name) -> Option<&TypeEntry> {
        self.types.iter().find(|t| t.name == name)
    }

    /// Get the number of type definitions.
    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    /// Get the number of typed expressions.
    pub fn expr_count(&self) -> usize {
        self.expr_types.len()
    }

    /// Get the number of functions.
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    /// Look up a pattern resolution by key.
    ///
    /// Returns `Some(&PatternResolution)` if the pattern was resolved to a
    /// unit variant, `None` if it's a normal variable binding.
    ///
    /// Uses O(log n) binary search on the sorted `pattern_resolutions` vec.
    pub fn resolve_pattern(&self, key: PatternKey) -> Option<&PatternResolution> {
        self.pattern_resolutions
            .binary_search_by_key(&key, |(k, _)| *k)
            .ok()
            .map(|idx| &self.pattern_resolutions[idx].1)
    }
}

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
    pub fn simple(name: Name, param_types: Vec<Idx>, return_type: Idx) -> Self {
        let required_params = param_types.len();
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
            type_param_bounds: Vec::new(),
            where_clauses: Vec::new(),
            generic_param_mapping: Vec::new(),
            required_params,
            param_defaults: Vec::new(),
        }
    }

    /// Create a synthetic function signature for compiler-generated methods.
    ///
    /// Like [`simple`](Self::simple) but includes parameter names, which are
    /// needed for ABI computation in derived trait methods. All other fields
    /// (generics, capabilities, flags) default to empty/false.
    pub fn synthetic(
        name: Name,
        param_names: Vec<Name>,
        param_types: Vec<Idx>,
        return_type: Idx,
    ) -> Self {
        let required_params = param_types.len();
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
            type_param_bounds: Vec::new(),
            where_clauses: Vec::new(),
            generic_param_mapping: Vec::new(),
            required_params,
            param_defaults: Vec::new(),
        }
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

/// Type check result with typed module and error guarantee.
///
/// This is the top-level result returned by the type checker query.
/// It wraps `TypedModule` and provides an `ErrorGuaranteed` token
/// for cases where errors were emitted.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeCheckResult {
    /// The typed module.
    pub typed: TypedModule,

    /// Error guarantee token.
    ///
    /// `Some` if at least one error was emitted during type checking.
    /// This provides a compile-time proof that error reporting was not forgotten.
    pub error_guarantee: Option<ErrorGuaranteed>,
}

impl TypeCheckResult {
    /// Create a successful result (no errors).
    pub fn ok(typed: TypedModule) -> Self {
        debug_assert!(typed.errors.is_empty(), "ok() called with errors present");
        Self {
            typed,
            error_guarantee: None,
        }
    }

    /// Create an error result.
    pub fn err(typed: TypedModule, guarantee: ErrorGuaranteed) -> Self {
        debug_assert!(
            !typed.errors.is_empty(),
            "err() called with no errors present"
        );
        Self {
            typed,
            error_guarantee: Some(guarantee),
        }
    }

    /// Create a result, automatically determining if errors are present.
    pub fn from_typed(typed: TypedModule) -> Self {
        if typed.has_errors() {
            // Create ErrorGuaranteed from the error count
            Self {
                error_guarantee: ErrorGuaranteed::from_error_count(typed.errors.len()),
                typed,
            }
        } else {
            Self {
                typed,
                error_guarantee: None,
            }
        }
    }

    /// Check if this result has errors.
    pub fn has_errors(&self) -> bool {
        self.error_guarantee.is_some()
    }

    /// Get the errors.
    pub fn errors(&self) -> &[TypeCheckError] {
        &self.typed.errors
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
