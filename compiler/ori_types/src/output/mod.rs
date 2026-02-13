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
use crate::{Idx, TypeCheckError};

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
        !self.type_params.is_empty()
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
mod tests {
    use super::*;
    use crate::Pool;

    #[test]
    fn typed_module_basic() {
        let mut module = TypedModule::new();

        // Store expression types
        module.expr_types.push(Idx::INT);
        module.expr_types.push(Idx::STR);
        module.expr_types.push(Idx::BOOL);

        assert_eq!(module.expr_type(0), Some(Idx::INT));
        assert_eq!(module.expr_type(1), Some(Idx::STR));
        assert_eq!(module.expr_type(2), Some(Idx::BOOL));
        assert_eq!(module.expr_type(99), None);
        assert!(!module.has_errors());
    }

    #[test]
    fn function_sig_simple() {
        let mut pool = Pool::new();
        let name = Name::from_raw(1);

        let sig = FunctionSig::simple(name, vec![Idx::INT, Idx::STR], Idx::BOOL);

        assert_eq!(sig.name, name);
        assert_eq!(sig.arity(), 2);
        assert!(!sig.is_generic());
        assert!(!sig.has_capabilities());

        let func_ty = sig.to_function_type(&mut pool);
        assert_eq!(pool.tag(func_ty), crate::Tag::Function);
    }

    #[test]
    fn function_sig_generic() {
        let name = Name::from_raw(1);
        let t_param = Name::from_raw(2);

        let sig = FunctionSig {
            name,
            type_params: vec![t_param],
            param_names: vec![Name::from_raw(3)],
            param_types: vec![Idx::INT],
            return_type: Idx::INT,
            capabilities: vec![],
            is_public: true,
            is_test: false,
            is_main: false,
            type_param_bounds: vec![vec![]],
            where_clauses: vec![],
            generic_param_mapping: vec![None],
            required_params: 1,
            param_defaults: vec![],
        };

        assert!(sig.is_generic());
        assert!(sig.is_public);
    }

    #[test]
    fn type_check_result_ok() {
        let module = TypedModule::new();
        let result = TypeCheckResult::ok(module);

        assert!(!result.has_errors());
        assert!(result.error_guarantee.is_none());
    }

    #[test]
    fn type_check_result_from_typed() {
        // No errors
        let module = TypedModule::new();
        let result = TypeCheckResult::from_typed(module);
        assert!(!result.has_errors());

        // With errors
        let mut module_with_errors = TypedModule::new();
        module_with_errors
            .errors
            .push(TypeCheckError::undefined_identifier(
                Name::from_raw(1),
                ori_ir::Span::DUMMY,
            ));
        let result = TypeCheckResult::from_typed(module_with_errors);
        assert!(result.has_errors());
    }

    #[test]
    fn typed_module_with_capacity() {
        let module = TypedModule::with_capacity(100, 10);
        assert_eq!(module.expr_types.capacity(), 100);
        assert_eq!(module.functions.capacity(), 10);
    }

    #[test]
    fn effect_class_pure() {
        let interner = ori_ir::StringInterner::new();
        let name = Name::from_raw(1);
        let sig = FunctionSig::simple(name, vec![Idx::INT], Idx::BOOL);

        assert_eq!(sig.effect_class(&interner), EffectClass::Pure);
    }

    #[test]
    fn effect_class_reads_only() {
        let interner = ori_ir::StringInterner::new();
        let name = Name::from_raw(1);
        let clock = interner.intern("Clock");
        let env = interner.intern("Env");

        let sig = FunctionSig {
            name,
            type_params: vec![],
            param_names: vec![],
            param_types: vec![],
            return_type: Idx::STR,
            capabilities: vec![clock, env],
            is_public: false,
            is_test: false,
            is_main: false,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            required_params: 0,
            param_defaults: vec![],
        };

        assert_eq!(sig.effect_class(&interner), EffectClass::ReadsOnly);
    }

    #[test]
    fn effect_class_has_effects() {
        let interner = ori_ir::StringInterner::new();
        let name = Name::from_raw(1);
        let http = interner.intern("Http");

        let sig = FunctionSig {
            name,
            type_params: vec![],
            param_names: vec![],
            param_types: vec![],
            return_type: Idx::STR,
            capabilities: vec![http],
            is_public: false,
            is_test: false,
            is_main: false,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            required_params: 0,
            param_defaults: vec![],
        };

        assert_eq!(sig.effect_class(&interner), EffectClass::HasEffects);
    }

    #[test]
    fn effect_class_mixed_caps_is_has_effects() {
        let interner = ori_ir::StringInterner::new();
        let name = Name::from_raw(1);
        let clock = interner.intern("Clock");
        let http = interner.intern("Http");

        // One read-only + one effectful → HasEffects
        let sig = FunctionSig {
            name,
            type_params: vec![],
            param_names: vec![],
            param_types: vec![],
            return_type: Idx::UNIT,
            capabilities: vec![clock, http],
            is_public: false,
            is_test: false,
            is_main: false,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            required_params: 0,
            param_defaults: vec![],
        };

        assert_eq!(sig.effect_class(&interner), EffectClass::HasEffects);
    }

    #[test]
    fn effect_class_ordering() {
        assert!(EffectClass::Pure < EffectClass::ReadsOnly);
        assert!(EffectClass::ReadsOnly < EffectClass::HasEffects);
    }

    #[test]
    fn function_lookup() {
        let mut module = TypedModule::new();
        let foo = Name::from_raw(1);
        let bar = Name::from_raw(2);

        module
            .functions
            .push(FunctionSig::simple(foo, vec![], Idx::UNIT));
        module
            .functions
            .push(FunctionSig::simple(bar, vec![Idx::INT], Idx::STR));

        assert!(module.function(foo).is_some());
        assert!(module.function(bar).is_some());
        assert!(module.function(Name::from_raw(99)).is_none());

        assert_eq!(module.function(foo).map(FunctionSig::arity), Some(0));
        assert_eq!(module.function(bar).map(FunctionSig::arity), Some(1));
    }

    #[test]
    fn type_def_export() {
        use crate::registry::{FieldDef, StructDef, TypeKind, Visibility};
        use crate::ValueCategory;

        let mut module = TypedModule::new();
        let point_name = Name::from_raw(10);
        let x_name = Name::from_raw(11);
        let y_name = Name::from_raw(12);

        module.types.push(TypeEntry {
            name: point_name,
            idx: Idx::from_raw(100),
            kind: TypeKind::Struct(StructDef {
                fields: vec![
                    FieldDef {
                        name: x_name,
                        ty: Idx::INT,
                        span: Span::DUMMY,
                        visibility: Visibility::Public,
                    },
                    FieldDef {
                        name: y_name,
                        ty: Idx::INT,
                        span: Span::DUMMY,
                        visibility: Visibility::Public,
                    },
                ],
                category: ValueCategory::default(),
            }),
            span: Span::DUMMY,
            type_params: vec![],
            visibility: Visibility::Public,
        });

        assert_eq!(module.type_count(), 1);
        assert!(module.type_def(point_name).is_some());
        assert!(module.type_def(Name::from_raw(99)).is_none());

        let entry = module.type_def(point_name).unwrap();
        assert!(matches!(entry.kind, TypeKind::Struct(_)));

        if let TypeKind::Struct(ref s) = entry.kind {
            assert_eq!(s.fields.len(), 2);
            assert_eq!(s.fields[0].ty, Idx::INT);
        }
    }
}
