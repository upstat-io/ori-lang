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
use ori_ir::{Name, Span};

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
    /// Indexed by the function's `Name` for O(1) lookup when resolving
    /// function calls from other modules.
    pub functions: Vec<FunctionSig>,

    /// Type errors accumulated during type checking.
    pub errors: Vec<TypeCheckError>,
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
            errors: Vec::new(),
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

    /// Get the number of typed expressions.
    pub fn expr_count(&self) -> usize {
        self.expr_types.len()
    }

    /// Get the number of functions.
    pub fn function_count(&self) -> usize {
        self.functions.len()
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
}

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
}
