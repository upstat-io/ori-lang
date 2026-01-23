// Exhaustive ARC Classifier Traits
//
// These traits define exhaustive classification methods for ARC analysis.
// Each IR variant (TExprKind, Type, TPattern, TMatchPattern) has a dedicated
// method that MUST be implemented.
//
// The key enforcement mechanism is the dispatch function at the bottom of
// this file: `classify_expr`, `classify_type`, `classify_pattern`, and
// `classify_match_pattern`. These use exhaustive pattern matching with NO
// wildcard (`_ =>`), so adding a new variant to any IR enum will cause a
// Rust compiler error until the corresponding method is added to the trait
// and implemented.
//
// This makes it IMPOSSIBLE to forget ARC handling when adding new IR variants.

use crate::ast::{BinaryOp, UnaryOp};
use crate::ir::{
    FuncRef, LocalId, TExpr, TExprKind, TMatch, TMatchPattern, TPattern, TStmt, Type,
};

use super::traits::StorageClass;

// =============================================================================
// Expression ARC Info
// =============================================================================

/// Result of classifying an expression for ARC
#[derive(Debug, Clone)]
pub struct ExprArcInfo {
    /// Whether this expression needs a retain after evaluation
    pub needs_retain: bool,

    /// Whether this expression needs a release after use
    pub needs_release: bool,

    /// Storage class of the expression's result
    pub storage_class: StorageClass,

    /// Child expressions that should be visited for ARC analysis
    pub children_to_visit: Vec<ChildVisit>,
}

impl ExprArcInfo {
    /// Create info for a value expression (no ARC needed)
    pub fn value() -> Self {
        ExprArcInfo {
            needs_retain: false,
            needs_release: false,
            storage_class: StorageClass::Value,
            children_to_visit: Vec::new(),
        }
    }

    /// Create info for a reference expression (ARC needed)
    pub fn reference() -> Self {
        ExprArcInfo {
            needs_retain: true,
            needs_release: true,
            storage_class: StorageClass::Reference,
            children_to_visit: Vec::new(),
        }
    }

    /// Add children to visit
    pub fn with_children(mut self, children: Vec<ChildVisit>) -> Self {
        self.children_to_visit = children;
        self
    }
}

/// A child expression that should be visited during ARC analysis
#[derive(Debug, Clone)]
pub struct ChildVisit {
    /// Description of what this child is (for debugging)
    pub description: &'static str,

    /// Whether this child is an owned value (needs cleanup)
    pub is_owned: bool,
}

impl ChildVisit {
    pub fn owned(description: &'static str) -> Self {
        ChildVisit {
            description,
            is_owned: true,
        }
    }

    pub fn borrowed(description: &'static str) -> Self {
        ChildVisit {
            description,
            is_owned: false,
        }
    }
}

// =============================================================================
// Type ARC Info
// =============================================================================

/// Result of classifying a type for ARC
#[derive(Debug, Clone)]
pub struct TypeArcInfo {
    /// Storage class for this type
    pub storage_class: StorageClass,

    /// Whether values of this type need ARC management
    pub needs_arc: bool,

    /// Whether values of this type need destruction at scope exit
    pub needs_destruction: bool,

    /// Size in bytes (for value types)
    pub size_bytes: usize,
}

impl TypeArcInfo {
    /// Create info for a value type
    pub fn value(size_bytes: usize) -> Self {
        TypeArcInfo {
            storage_class: StorageClass::Value,
            needs_arc: false,
            needs_destruction: false,
            size_bytes,
        }
    }

    /// Create info for a reference type
    pub fn reference(size_bytes: usize) -> Self {
        TypeArcInfo {
            storage_class: StorageClass::Reference,
            needs_arc: true,
            needs_destruction: true,
            size_bytes,
        }
    }

    /// Create info for a hybrid type (value with reference fields)
    pub fn hybrid(size_bytes: usize) -> Self {
        TypeArcInfo {
            storage_class: StorageClass::Hybrid,
            needs_arc: true,
            needs_destruction: true,
            size_bytes,
        }
    }
}

// =============================================================================
// Pattern ARC Info
// =============================================================================

/// Result of classifying a TPattern for ARC
#[derive(Debug, Clone)]
pub struct PatternArcInfo {
    /// Whether this pattern creates reference-typed temporaries
    pub creates_temporaries: bool,

    /// Whether the pattern result needs ARC management
    pub result_needs_arc: bool,

    /// Child expressions in the pattern that need ARC analysis
    pub children_to_visit: Vec<ChildVisit>,
}

impl PatternArcInfo {
    /// Create info for a pattern with no ARC requirements
    pub fn none() -> Self {
        PatternArcInfo {
            creates_temporaries: false,
            result_needs_arc: false,
            children_to_visit: Vec::new(),
        }
    }

    /// Create info for a pattern that creates temporaries
    pub fn with_temporaries() -> Self {
        PatternArcInfo {
            creates_temporaries: true,
            result_needs_arc: true,
            children_to_visit: Vec::new(),
        }
    }
}

// =============================================================================
// Match Pattern ARC Info
// =============================================================================

/// Result of classifying a TMatchPattern for ARC
#[derive(Debug, Clone)]
pub struct MatchPatternArcInfo {
    /// Whether this pattern binds reference types
    pub binds_references: bool,

    /// Local IDs that are bound by this pattern (if any)
    pub bound_locals: Vec<LocalId>,
}

impl MatchPatternArcInfo {
    /// Create info for a pattern with no bindings
    pub fn none() -> Self {
        MatchPatternArcInfo {
            binds_references: false,
            bound_locals: Vec::new(),
        }
    }

    /// Create info for a pattern with bindings
    pub fn with_bindings(locals: Vec<LocalId>, binds_refs: bool) -> Self {
        MatchPatternArcInfo {
            binds_references: binds_refs,
            bound_locals: locals,
        }
    }
}

// =============================================================================
// Exhaustive Expression Classifier Trait
// =============================================================================

/// Trait that MUST handle every TExprKind variant explicitly.
///
/// IMPORTANT: This trait has one method per TExprKind variant. The dispatch
/// function `classify_expr` uses exhaustive matching, so adding a new variant
/// to TExprKind will cause a Rust compile error until:
/// 1. A new method is added to this trait
/// 2. The method is implemented in ExhaustiveArcAnalyzer
///
/// This makes it IMPOSSIBLE to forget ARC handling for new expression kinds.
pub trait ArcExprClassifier {
    // =========================================================================
    // Literals (7 variants)
    // =========================================================================

    fn classify_int_literal(&self, value: i64, ty: &Type) -> ExprArcInfo;
    fn classify_float_literal(&self, value: f64, ty: &Type) -> ExprArcInfo;
    fn classify_string_literal(&self, value: &str, ty: &Type) -> ExprArcInfo;
    fn classify_bool_literal(&self, value: bool, ty: &Type) -> ExprArcInfo;
    fn classify_nil(&self, ty: &Type) -> ExprArcInfo;

    // =========================================================================
    // Variables (3 variants)
    // =========================================================================

    fn classify_local(&self, id: LocalId, ty: &Type) -> ExprArcInfo;
    fn classify_param(&self, index: usize, ty: &Type) -> ExprArcInfo;
    fn classify_config(&self, name: &str, ty: &Type) -> ExprArcInfo;

    // =========================================================================
    // Collections (4 variants)
    // =========================================================================

    fn classify_list(&self, elements: &[TExpr], ty: &Type) -> ExprArcInfo;
    fn classify_map_literal(&self, entries: &[(TExpr, TExpr)], ty: &Type) -> ExprArcInfo;
    fn classify_tuple(&self, elements: &[TExpr], ty: &Type) -> ExprArcInfo;
    fn classify_struct(&self, name: &str, fields: &[(String, TExpr)], ty: &Type) -> ExprArcInfo;

    // =========================================================================
    // Operations (2 variants)
    // =========================================================================

    fn classify_binary_op(
        &self,
        op: &BinaryOp,
        left: &TExpr,
        right: &TExpr,
        ty: &Type,
    ) -> ExprArcInfo;
    fn classify_unary_op(&self, op: &UnaryOp, operand: &TExpr, ty: &Type) -> ExprArcInfo;

    // =========================================================================
    // Access (3 variants)
    // =========================================================================

    fn classify_field(&self, expr: &TExpr, field: &str, ty: &Type) -> ExprArcInfo;
    fn classify_index(&self, expr: &TExpr, index: &TExpr, ty: &Type) -> ExprArcInfo;
    fn classify_length_of(&self, expr: &TExpr, ty: &Type) -> ExprArcInfo;

    // =========================================================================
    // Calls (2 variants)
    // =========================================================================

    fn classify_call(&self, func: &FuncRef, args: &[TExpr], ty: &Type) -> ExprArcInfo;
    fn classify_method_call(
        &self,
        receiver: &TExpr,
        method: &str,
        args: &[TExpr],
        ty: &Type,
    ) -> ExprArcInfo;

    // =========================================================================
    // Lambda (1 variant)
    // =========================================================================

    fn classify_lambda(
        &self,
        params: &[(String, Type)],
        captures: &[LocalId],
        body: &TExpr,
        ty: &Type,
    ) -> ExprArcInfo;

    // =========================================================================
    // Control Flow (4 variants)
    // =========================================================================

    fn classify_if(
        &self,
        cond: &TExpr,
        then_branch: &TExpr,
        else_branch: &TExpr,
        ty: &Type,
    ) -> ExprArcInfo;
    fn classify_match(&self, match_expr: &TMatch, ty: &Type) -> ExprArcInfo;
    fn classify_block(&self, stmts: &[TStmt], result: &TExpr, ty: &Type) -> ExprArcInfo;
    fn classify_for(
        &self,
        binding: LocalId,
        iter: &TExpr,
        body: &TExpr,
        ty: &Type,
    ) -> ExprArcInfo;

    // =========================================================================
    // Assignment (1 variant)
    // =========================================================================

    fn classify_assign(&self, target: LocalId, value: &TExpr, ty: &Type) -> ExprArcInfo;

    // =========================================================================
    // Range (1 variant)
    // =========================================================================

    fn classify_range(&self, start: &TExpr, end: &TExpr, ty: &Type) -> ExprArcInfo;

    // =========================================================================
    // Patterns (1 variant - delegates to pattern classifier)
    // =========================================================================

    fn classify_pattern_expr(&self, pattern: &TPattern, ty: &Type) -> ExprArcInfo;

    // =========================================================================
    // Result/Option Constructors (6 variants)
    // =========================================================================

    fn classify_ok(&self, value: &TExpr, ty: &Type) -> ExprArcInfo;
    fn classify_err(&self, value: &TExpr, ty: &Type) -> ExprArcInfo;
    fn classify_some(&self, value: &TExpr, ty: &Type) -> ExprArcInfo;
    fn classify_none(&self, ty: &Type) -> ExprArcInfo;
    fn classify_coalesce(&self, value: &TExpr, default: &TExpr, ty: &Type) -> ExprArcInfo;
    fn classify_unwrap(&self, value: &TExpr, ty: &Type) -> ExprArcInfo;

    // =========================================================================
    // Capability (1 variant)
    // =========================================================================

    fn classify_with(
        &self,
        capability: &str,
        implementation: &TExpr,
        body: &TExpr,
        ty: &Type,
    ) -> ExprArcInfo;
}

// =============================================================================
// Exhaustive Type Classifier Trait
// =============================================================================

/// Trait that MUST handle every Type variant explicitly.
///
/// Similar to ArcExprClassifier, this enforces exhaustive handling of all
/// type variants for ARC classification.
pub trait ArcTypeClassifier {
    // =========================================================================
    // Primitives (5 variants)
    // =========================================================================

    fn classify_int(&self) -> TypeArcInfo;
    fn classify_float(&self) -> TypeArcInfo;
    fn classify_bool(&self) -> TypeArcInfo;
    fn classify_str(&self) -> TypeArcInfo;
    fn classify_void(&self) -> TypeArcInfo;

    // =========================================================================
    // Collections (3 variants)
    // =========================================================================

    fn classify_list(&self, elem_ty: &Type) -> TypeArcInfo;
    fn classify_map(&self, key_ty: &Type, val_ty: &Type) -> TypeArcInfo;
    fn classify_tuple(&self, elem_tys: &[Type]) -> TypeArcInfo;

    // =========================================================================
    // User-Defined (3 variants)
    // =========================================================================

    fn classify_struct_type(&self, name: &str, fields: &[(String, Type)]) -> TypeArcInfo;
    fn classify_enum_type(
        &self,
        name: &str,
        variants: &[(String, Vec<(String, Type)>)],
    ) -> TypeArcInfo;
    fn classify_named(&self, name: &str) -> TypeArcInfo;

    // =========================================================================
    // Function (1 variant)
    // =========================================================================

    fn classify_function(&self, params: &[Type], ret: &Type) -> TypeArcInfo;

    // =========================================================================
    // Result/Option (2 variants)
    // =========================================================================

    fn classify_result(&self, ok_ty: &Type, err_ty: &Type) -> TypeArcInfo;
    fn classify_option(&self, inner_ty: &Type) -> TypeArcInfo;

    // =========================================================================
    // Other (4 variants)
    // =========================================================================

    fn classify_record(&self, fields: &[(String, Type)]) -> TypeArcInfo;
    fn classify_range(&self) -> TypeArcInfo;
    fn classify_any(&self) -> TypeArcInfo;
    fn classify_dyn_trait(&self, trait_name: &str) -> TypeArcInfo;
}

// =============================================================================
// Exhaustive Pattern Classifier Trait
// =============================================================================

/// Trait that MUST handle every TPattern variant explicitly.
pub trait ArcPatternClassifier {
    fn classify_fold(
        &self,
        collection: &TExpr,
        elem_ty: &Type,
        init: &TExpr,
        op: &TExpr,
        result_ty: &Type,
    ) -> PatternArcInfo;

    fn classify_map(
        &self,
        collection: &TExpr,
        elem_ty: &Type,
        transform: &TExpr,
        result_elem_ty: &Type,
    ) -> PatternArcInfo;

    fn classify_filter(
        &self,
        collection: &TExpr,
        elem_ty: &Type,
        predicate: &TExpr,
    ) -> PatternArcInfo;

    fn classify_collect(
        &self,
        range: &TExpr,
        transform: &TExpr,
        result_elem_ty: &Type,
    ) -> PatternArcInfo;

    fn classify_recurse(
        &self,
        cond: &TExpr,
        base: &TExpr,
        step: &TExpr,
        result_ty: &Type,
        memo: bool,
        parallel_threshold: i64,
    ) -> PatternArcInfo;

    fn classify_iterate(
        &self,
        over: &TExpr,
        elem_ty: &Type,
        direction: crate::ir::IterDirection,
        into: &TExpr,
        with: &TExpr,
        result_ty: &Type,
    ) -> PatternArcInfo;

    fn classify_transform(
        &self,
        input: &TExpr,
        steps: &[TExpr],
        result_ty: &Type,
    ) -> PatternArcInfo;

    fn classify_count(
        &self,
        collection: &TExpr,
        elem_ty: &Type,
        predicate: &TExpr,
    ) -> PatternArcInfo;

    fn classify_parallel(
        &self,
        branches: &[(String, TExpr, Type)],
        timeout: Option<&TExpr>,
        on_error: crate::ir::OnError,
        result_ty: &Type,
    ) -> PatternArcInfo;

    fn classify_find(
        &self,
        collection: &TExpr,
        elem_ty: &Type,
        predicate: &TExpr,
        default: Option<&TExpr>,
        result_ty: &Type,
    ) -> PatternArcInfo;

    fn classify_try(
        &self,
        body: &TExpr,
        catch: Option<&TExpr>,
        result_ty: &Type,
    ) -> PatternArcInfo;

    fn classify_retry(
        &self,
        operation: &TExpr,
        max_attempts: &TExpr,
        backoff: crate::ir::RetryBackoff,
        delay_ms: Option<&TExpr>,
        result_ty: &Type,
    ) -> PatternArcInfo;

    fn classify_validate(
        &self,
        rules: &[(TExpr, TExpr)],
        then_value: &TExpr,
        result_ty: &Type,
    ) -> PatternArcInfo;
}

// =============================================================================
// Exhaustive Match Pattern Classifier Trait
// =============================================================================

/// Trait that MUST handle every TMatchPattern variant explicitly.
pub trait ArcMatchPatternClassifier {
    fn classify_wildcard(&self) -> MatchPatternArcInfo;
    fn classify_literal(&self, expr: &TExpr) -> MatchPatternArcInfo;
    fn classify_binding(&self, local: LocalId, ty: &Type) -> MatchPatternArcInfo;
    fn classify_variant(
        &self,
        name: &str,
        bindings: &[(String, LocalId, Type)],
    ) -> MatchPatternArcInfo;
    fn classify_condition(&self, expr: &TExpr) -> MatchPatternArcInfo;
}

// =============================================================================
// Dispatch Functions (THE ENFORCEMENT POINT)
// =============================================================================

/// Classify an expression using the exhaustive trait.
///
/// **THIS IS THE KEY ENFORCEMENT POINT.**
///
/// This function uses exhaustive pattern matching with NO wildcard (`_ =>`).
/// Adding a new TExprKind variant will cause a Rust compile error here until:
/// 1. A new case is added to this match
/// 2. A new method is added to ArcExprClassifier
/// 3. The method is implemented in all implementors
pub fn classify_expr<C: ArcExprClassifier>(classifier: &C, expr: &TExpr) -> ExprArcInfo {
    let ty = &expr.ty;
    match &expr.kind {
        // Literals
        TExprKind::Int(v) => classifier.classify_int_literal(*v, ty),
        TExprKind::Float(v) => classifier.classify_float_literal(*v, ty),
        TExprKind::String(v) => classifier.classify_string_literal(v, ty),
        TExprKind::Bool(v) => classifier.classify_bool_literal(*v, ty),
        TExprKind::Nil => classifier.classify_nil(ty),

        // Variables
        TExprKind::Local(id) => classifier.classify_local(*id, ty),
        TExprKind::Param(idx) => classifier.classify_param(*idx, ty),
        TExprKind::Config(name) => classifier.classify_config(name, ty),

        // Collections
        TExprKind::List(elems) => classifier.classify_list(elems, ty),
        TExprKind::MapLiteral(entries) => classifier.classify_map_literal(entries, ty),
        TExprKind::Tuple(elems) => classifier.classify_tuple(elems, ty),
        TExprKind::Struct { name, fields } => classifier.classify_struct(name, fields, ty),

        // Operations
        TExprKind::Binary { op, left, right } => classifier.classify_binary_op(op, left, right, ty),
        TExprKind::Unary { op, operand } => classifier.classify_unary_op(op, operand, ty),

        // Access
        TExprKind::Field(expr, field) => classifier.classify_field(expr, field, ty),
        TExprKind::Index(expr, idx) => classifier.classify_index(expr, idx, ty),
        TExprKind::LengthOf(expr) => classifier.classify_length_of(expr, ty),

        // Calls
        TExprKind::Call { func, args } => classifier.classify_call(func, args, ty),
        TExprKind::MethodCall {
            receiver,
            method,
            args,
        } => classifier.classify_method_call(receiver, method, args, ty),

        // Lambda
        TExprKind::Lambda {
            params,
            captures,
            body,
        } => classifier.classify_lambda(params, captures, body, ty),

        // Control flow
        TExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => classifier.classify_if(cond, then_branch, else_branch, ty),
        TExprKind::Match(match_expr) => classifier.classify_match(match_expr, ty),
        TExprKind::Block(stmts, result) => classifier.classify_block(stmts, result, ty),
        TExprKind::For {
            binding,
            iter,
            body,
        } => classifier.classify_for(*binding, iter, body, ty),

        // Assignment
        TExprKind::Assign { target, value } => classifier.classify_assign(*target, value, ty),

        // Range
        TExprKind::Range { start, end } => classifier.classify_range(start, end, ty),

        // Patterns
        TExprKind::Pattern(pattern) => classifier.classify_pattern_expr(pattern, ty),

        // Result/Option constructors
        TExprKind::Ok(v) => classifier.classify_ok(v, ty),
        TExprKind::Err(v) => classifier.classify_err(v, ty),
        TExprKind::Some(v) => classifier.classify_some(v, ty),
        TExprKind::None_ => classifier.classify_none(ty),
        TExprKind::Coalesce { value, default } => classifier.classify_coalesce(value, default, ty),
        TExprKind::Unwrap(v) => classifier.classify_unwrap(v, ty),

        // Capability
        TExprKind::With {
            capability,
            implementation,
            body,
        } => classifier.classify_with(capability, implementation, body, ty),

        // NO WILDCARD HERE - Rust will error if a new variant is added
    }
}

/// Classify a type using the exhaustive trait.
///
/// Uses exhaustive matching - adding a new Type variant will cause a compile error.
pub fn classify_type<C: ArcTypeClassifier>(classifier: &C, ty: &Type) -> TypeArcInfo {
    match ty {
        // Primitives
        Type::Int => classifier.classify_int(),
        Type::Float => classifier.classify_float(),
        Type::Bool => classifier.classify_bool(),
        Type::Str => classifier.classify_str(),
        Type::Void => classifier.classify_void(),

        // Collections
        Type::List(elem) => classifier.classify_list(elem),
        Type::Map(k, v) => classifier.classify_map(k, v),
        Type::Tuple(elems) => classifier.classify_tuple(elems),

        // User-defined
        Type::Struct { name, fields } => classifier.classify_struct_type(name, fields),
        Type::Enum { name, variants } => classifier.classify_enum_type(name, variants),
        Type::Named(name) => classifier.classify_named(name),

        // Function
        Type::Function { params, ret } => classifier.classify_function(params, ret),

        // Result/Option
        Type::Result(ok, err) => classifier.classify_result(ok, err),
        Type::Option(inner) => classifier.classify_option(inner),

        // Other
        Type::Record(fields) => classifier.classify_record(fields),
        Type::Range => classifier.classify_range(),
        Type::Any => classifier.classify_any(),
        Type::DynTrait(name) => classifier.classify_dyn_trait(name),

        // NO WILDCARD HERE - Rust will error if a new variant is added
    }
}

/// Classify a pattern using the exhaustive trait.
///
/// Uses exhaustive matching - adding a new TPattern variant will cause a compile error.
pub fn classify_pattern<C: ArcPatternClassifier>(classifier: &C, pattern: &TPattern) -> PatternArcInfo {
    match pattern {
        TPattern::Fold {
            collection,
            elem_ty,
            init,
            op,
            result_ty,
        } => classifier.classify_fold(collection, elem_ty, init, op, result_ty),

        TPattern::Map {
            collection,
            elem_ty,
            transform,
            result_elem_ty,
        } => classifier.classify_map(collection, elem_ty, transform, result_elem_ty),

        TPattern::Filter {
            collection,
            elem_ty,
            predicate,
        } => classifier.classify_filter(collection, elem_ty, predicate),

        TPattern::Collect {
            range,
            transform,
            result_elem_ty,
        } => classifier.classify_collect(range, transform, result_elem_ty),

        TPattern::Recurse {
            cond,
            base,
            step,
            result_ty,
            memo,
            parallel_threshold,
        } => classifier.classify_recurse(cond, base, step, result_ty, *memo, *parallel_threshold),

        TPattern::Iterate {
            over,
            elem_ty,
            direction,
            into,
            with,
            result_ty,
        } => classifier.classify_iterate(over, elem_ty, *direction, into, with, result_ty),

        TPattern::Transform {
            input,
            steps,
            result_ty,
        } => classifier.classify_transform(input, steps, result_ty),

        TPattern::Count {
            collection,
            elem_ty,
            predicate,
        } => classifier.classify_count(collection, elem_ty, predicate),

        TPattern::Parallel {
            branches,
            timeout,
            on_error,
            result_ty,
        } => classifier.classify_parallel(branches, timeout.as_ref(), *on_error, result_ty),

        TPattern::Find {
            collection,
            elem_ty,
            predicate,
            default,
            result_ty,
        } => classifier.classify_find(collection, elem_ty, predicate, default.as_ref(), result_ty),

        TPattern::Try {
            body,
            catch,
            result_ty,
        } => classifier.classify_try(body, catch.as_ref(), result_ty),

        TPattern::Retry {
            operation,
            max_attempts,
            backoff,
            delay_ms,
            result_ty,
        } => classifier.classify_retry(operation, max_attempts, *backoff, delay_ms.as_ref(), result_ty),

        TPattern::Validate {
            rules,
            then_value,
            result_ty,
        } => classifier.classify_validate(rules, then_value, result_ty),

        // NO WILDCARD HERE - Rust will error if a new variant is added
    }
}

/// Classify a match pattern using the exhaustive trait.
///
/// Uses exhaustive matching - adding a new TMatchPattern variant will cause a compile error.
pub fn classify_match_pattern<C: ArcMatchPatternClassifier>(
    classifier: &C,
    pattern: &TMatchPattern,
) -> MatchPatternArcInfo {
    match pattern {
        TMatchPattern::Wildcard => classifier.classify_wildcard(),
        TMatchPattern::Literal(expr) => classifier.classify_literal(expr),
        TMatchPattern::Binding(local, ty) => classifier.classify_binding(*local, ty),
        TMatchPattern::Variant { name, bindings } => classifier.classify_variant(name, bindings),
        TMatchPattern::Condition(expr) => classifier.classify_condition(expr),

        // NO WILDCARD HERE - Rust will error if a new variant is added
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expr_arc_info_value() {
        let info = ExprArcInfo::value();
        assert!(!info.needs_retain);
        assert!(!info.needs_release);
        assert_eq!(info.storage_class, StorageClass::Value);
    }

    #[test]
    fn test_expr_arc_info_reference() {
        let info = ExprArcInfo::reference();
        assert!(info.needs_retain);
        assert!(info.needs_release);
        assert_eq!(info.storage_class, StorageClass::Reference);
    }

    #[test]
    fn test_type_arc_info_value() {
        let info = TypeArcInfo::value(8);
        assert!(!info.needs_arc);
        assert!(!info.needs_destruction);
        assert_eq!(info.size_bytes, 8);
    }

    #[test]
    fn test_type_arc_info_reference() {
        let info = TypeArcInfo::reference(24);
        assert!(info.needs_arc);
        assert!(info.needs_destruction);
        assert_eq!(info.size_bytes, 24);
    }

    #[test]
    fn test_child_visit() {
        let owned = ChildVisit::owned("operand");
        assert!(owned.is_owned);

        let borrowed = ChildVisit::borrowed("condition");
        assert!(!borrowed.is_owned);
    }

    #[test]
    fn test_pattern_arc_info() {
        let none = PatternArcInfo::none();
        assert!(!none.creates_temporaries);

        let with_temps = PatternArcInfo::with_temporaries();
        assert!(with_temps.creates_temporaries);
    }

    #[test]
    fn test_match_pattern_arc_info() {
        let none = MatchPatternArcInfo::none();
        assert!(!none.binds_references);
        assert!(none.bound_locals.is_empty());

        let with_bindings = MatchPatternArcInfo::with_bindings(vec![LocalId(0)], true);
        assert!(with_bindings.binds_references);
        assert_eq!(with_bindings.bound_locals.len(), 1);
    }
}
