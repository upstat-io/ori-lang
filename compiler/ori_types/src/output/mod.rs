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

use crate::pool::TypeDescriptor;
use crate::registry::TypeEntry;
use crate::{Idx, TypeCheckError, TypeCheckWarning};

/// A compile-time value used as a const generic argument.
///
/// Phase 1: unused (only [`GenericArg::Type`] variants).
/// Phase 2+: const generics (`$N: int`, `$B: bool`).
/// Phase 3+: any type `with Eq, Hashable` (`$C: Color`, `$S: [int]`).
///
/// Each variant must be `Eq + Hash`, mirroring Ori's requirement that
/// const-eligible types implement `Eq + Hashable`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ConstValue {
    /// Integer constant (`$N: int → 42`).
    Int(i64),
    /// Boolean constant (`$B: bool → true`).
    Bool(bool),
    // Future phases add variants as const generic eligibility expands:
    // Str(Name), Char(char), Byte(u8),
    // Enum { type_name: Name, variant: Name },
    // List(Vec<ConstValue>), Tuple(Vec<ConstValue>),
}

/// A concrete argument to a generic parameter.
///
/// Unifies type substitution (`T → int`) and const value substitution
/// (`$N → 42`). Parallel to the function's generic parameter list.
///
/// This design matches the convergent pattern across reference compilers:
/// - Rust: `GenericArgKind::Type | Const | Lifetime`
/// - Zig: uniform `InternPool.Index` for types and comptime values
/// - Lean 4: `Expr`-based key (types and values are both expressions)
///
/// Using a single enum avoids impedance mismatches when generic parameter
/// lists contain both type and const params (e.g., `@f<T with Clone, $N: int>`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GenericArg {
    /// Type parameter substitution: `T → int`.
    Type(Idx),
    /// Const generic value substitution: `$N → 42`, `$C → Color.Red`.
    Const(ConstValue),
}

/// A concrete instantiation of a generic function discovered during type checking.
///
/// Recorded when a generic function like `@identity<T>(x: T) -> T` is called
/// with concrete types (e.g., `identity(x: 42)` produces `T = int`). The LLVM
/// monomorphizer stamps out one specialized function per unique `MonoInstance`.
///
/// Identity is determined by `(fn_name, generic_args)` — two instances with
/// the same function and arguments are the same specialization, regardless of
/// where the call site is. This matches Rust's `(DefId, GenericArgsRef)` and
/// Zig's `(generic_owner, comptime_args[])` caching keys.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MonoInstance {
    /// The generic function being instantiated.
    pub fn_name: Name,
    /// Concrete generic arguments (parallel to function's generic params).
    ///
    /// Phase 1: all `GenericArg::Type`. Phase 2+: mixed `Type` and `Const`.
    pub generic_args: Vec<GenericArg>,
    /// Substituted parameter types (all type variables replaced with concrete types).
    pub concrete_param_types: Vec<Idx>,
    /// Substituted return type.
    pub concrete_return_type: Idx,
    /// Maps generic `Idx` → concrete `Idx` for body expression types.
    ///
    /// Sorted by key for deterministic `Eq`/`Hash` (required by Salsa early
    /// cutoff). The ARC lowerer converts this to `FxHashMap` for O(1) lookup
    /// when lowering the shared canonical IR body into a monomorphized ARC
    /// function (matching Swift's clone-and-substitute strategy).
    pub body_type_map: Vec<(Idx, Idx)>,
}

/// How a callee's type variable binds during a generic-calling-generic call.
#[derive(Clone, Debug)]
pub enum DeferredVarBinding {
    /// Maps to the caller's scheme var at this position (deferred until the
    /// caller is instantiated). E.g., `identity`'s `T` → `apply_identity`'s position 0.
    CallerSchemeVar(usize),
    /// Already resolved to a concrete type. E.g., `make_pair`'s `B` → `int` when
    /// called as `make_pair(a: x, b: 99)` inside a generic function.
    Concrete(Idx),
}

/// A deferred monomorphization call: generic function calling another generic.
///
/// Recorded when a generic function's body calls another generic function
/// and at least one type argument is still a type variable. Resolved later
/// via [`resolve_deferred_mono_calls`] when the caller is instantiated.
///
/// # Examples
///
/// Simple chain: `apply_identity<T>(x: T) = identity(x: x)` records
/// `identity`'s `T` → `CallerSchemeVar(0)`.
///
/// Mixed: `wrap_with_int<T>(x: T) = make_pair(a: x, b: 99)` records
/// `make_pair`'s `A` → `CallerSchemeVar(0)`, `B` → `Concrete(Idx::INT)`.
#[derive(Clone, Debug)]
pub struct DeferredMonoCall {
    /// The generic function that contains the call.
    pub caller: Name,
    /// The generic function being called.
    pub callee: Name,
    /// The callee's scheme var IDs (in declaration order).
    pub callee_scheme_var_ids: Vec<u32>,
    /// Maps callee scheme var ID → binding (caller scheme var position or
    /// concrete type). This semantic mapping avoids dependence on pool
    /// union-find state.
    pub var_subst: Vec<(u32, DeferredVarBinding)>,
    /// The callee's parameter types (from its generic signature).
    pub callee_param_types: Vec<Idx>,
    /// The callee's return type (from its generic signature).
    pub callee_return_type: Idx,
}

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

    /// Monomorphization instances discovered during type checking.
    ///
    /// Each entry represents a unique `(fn_name, generic_args)` combination
    /// found at a call site. The LLVM backend uses these to stamp out
    /// concrete specializations of generic functions.
    pub mono_instances: Vec<MonoInstance>,

    /// Portable type descriptors for all types referenced in exported signatures.
    ///
    /// Topologically sorted: leaves first. Each entry is `(merkle_hash, descriptor)`.
    /// Importing modules can reconstruct any exported type from these descriptors
    /// without accessing the originating Pool or AST.
    ///
    /// Only includes types from public function signatures to minimize size.
    pub type_descriptors: Vec<(u64, TypeDescriptor)>,
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
            mono_instances: Vec::new(),
            type_descriptors: Vec::new(),
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
