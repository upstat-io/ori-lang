#![deny(clippy::arithmetic_side_effects)]
#![allow(
    clippy::result_large_err,
    reason = "EvalError is fundamental — boxing would add complexity across the crate"
)]
//! Ori Patterns - Pattern system for the Ori compiler.
//!
//! This crate provides:
//! - Runtime value types (`Value`, `Heap`, `FunctionValue`, `RangeValue`, etc.)
//! - Evaluation error types (`EvalError`, `EvalResult`)
//! - Pattern registry and trait definitions
//! - Built-in pattern implementations (recurse, parallel, spawn, timeout, cache, with)
//!
//! # Architecture
//!
//! The pattern system follows the Open/Closed principle:
//! - New patterns can be added by implementing `PatternDefinition`
//! - No modifications to existing code required
//! - Patterns are registered in `PatternRegistry`
//!
//! # Value Types
//!
//! The value module provides runtime values with enforced Arc usage:
//! - All heap allocations go through `Value::` factory methods
//! - `Heap<T>` wrapper enforces this invariant
//! - Thread-safe reference counting via `Arc`

mod errors;
mod fusion;
pub mod method_key;
mod registry;
mod signature;
pub mod user_methods;
mod value;

// Pattern implementations
mod builtins;
mod cache;
mod channel;
mod parallel;
mod recurse;
mod spawn;
mod timeout;
mod with_pattern;

#[cfg(test)]
mod parallel_tests;

#[cfg(test)]
mod test_helpers;

use ori_ir::{ExprArena, ExprId, Name, NamedExpr, StringInterner};

pub use errors::{
    BacktraceFrame, ControlAction, EvalBacktrace, EvalError, EvalErrorKind, EvalNote, EvalResult,
};
pub use fusion::{ChainLink, FusedPattern, FusionHints, PatternChain};
pub use method_key::{MethodKey, MethodKeyDisplay};
pub use registry::{Pattern, PatternRegistry};
pub use signature::{DefaultValue, FunctionSignature, OptionalArg, PatternSignature};
pub use user_methods::{MethodEntry, UserMethod, UserMethodRegistry};
pub use value::{
    FunctionValFn, FunctionValue, Heap, IteratorValue, MemoizedFunctionValue, OrderingValue,
    RangeValue, ScalarInt, StringLookup, StructLayout, StructValue, Value,
};

// Re-export error constructors for use by other crates
pub use errors::{
    // Collection method errors
    all_requires_list,
    any_requires_list,
    // Miscellaneous errors
    await_not_supported,
    // Binary operation errors
    binary_type_mismatch,
    // Index and field access errors
    cannot_access_field,
    // Control flow errors
    cannot_assign_immutable,
    cannot_get_length,
    cannot_index,
    collect_requires_range,
    // Index context errors
    collection_too_large,
    // Not implemented errors
    default_requires_type_context,
    division_by_zero,
    // Pattern binding errors
    expected_list,
    expected_struct,
    expected_tuple,
    field_assignment_not_implemented,
    filter_entries_not_implemented,
    filter_entries_requires_map,
    filter_requires_collection,
    find_requires_list,
    fold_requires_collection,
    // Pattern errors
    for_pattern_requires_list,
    for_requires_iterable,
    hash_outside_index,
    index_assignment_not_implemented,
    index_out_of_bounds,
    integer_overflow,
    invalid_assignment_target,
    invalid_binary_op_for,
    invalid_literal_pattern,
    invalid_tuple_field,
    key_not_found,
    list_pattern_too_long,
    map_entries_not_implemented,
    map_entries_requires_map,
    // Type conversion errors
    map_key_not_hashable,
    map_requires_collection,
    missing_struct_field,
    modulo_by_zero,
    no_field_on_struct,
    no_member_in_module,
    // Method call errors
    no_such_method,
    non_exhaustive_match,
    non_integer_in_index,
    // Variable and function errors
    not_callable,
    operator_not_supported_in_index,
    parse_error,
    propagated_error_message,
    range_bound_not_int,
    recursion_limit_exceeded,
    self_outside_method,
    size_negative_divide,
    size_negative_multiply,
    size_would_be_negative,
    spread_requires_list,
    spread_requires_map,
    spread_requires_struct,
    tuple_index_out_of_bounds,
    tuple_pattern_mismatch,
    unbounded_range_end,
    undefined_const,
    undefined_function,
    undefined_variable,
    unknown_pattern,
    wrong_arg_count,
    wrong_arg_type,
    wrong_function_args,
};

/// Context for evaluating a pattern.
///
/// Provides access to the evaluator's components without exposing the full evaluator.
pub struct EvalContext<'a> {
    pub interner: &'a StringInterner,
    pub arena: &'a ExprArena,
    /// Named expressions (properties) for this pattern.
    pub props: &'a [NamedExpr],
}

impl<'a> EvalContext<'a> {
    /// Create a new evaluation context.
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena, props: &'a [NamedExpr]) -> Self {
        EvalContext {
            interner,
            arena,
            props,
        }
    }

    /// Get the span of the first property, if any.
    ///
    /// Used as a fallback span for errors when no specific property is available.
    pub fn first_prop_span(&self) -> Option<ori_ir::Span> {
        self.props
            .first()
            .map(|p| self.arena.get_expr(p.value).span)
    }

    /// Get the span of a named property, if present.
    pub fn prop_span(&self, name: &str) -> Option<ori_ir::Span> {
        let target = self.interner.intern(name);
        for prop in self.props {
            if prop.name == target {
                return Some(self.arena.get_expr(prop.value).span);
            }
        }
        None
    }

    /// Get a required property's `ExprId` by name.
    #[allow(
        clippy::result_large_err,
        reason = "EvalError is fundamental — boxing would add complexity across the crate"
    )]
    pub fn get_prop(&self, name: &str) -> Result<ExprId, EvalError> {
        let target = self.interner.intern(name);
        for prop in self.props {
            if prop.name == target {
                return Ok(prop.value);
            }
        }
        let mut err = EvalError::new(format!("missing required property: .{name}"));
        if let Some(span) = self.first_prop_span() {
            err = err.with_span(span);
        }
        Err(err)
    }

    /// Get an optional property's `ExprId` by name.
    pub fn get_prop_opt(&self, name: &str) -> Option<ExprId> {
        let target = self.interner.intern(name);
        for prop in self.props {
            if prop.name == target {
                return Some(prop.value);
            }
        }
        None
    }

    /// Get a required property and evaluate it.
    ///
    /// This is a convenience method that combines `get_prop` and `exec.eval`.
    pub fn eval_prop(&self, name: &str, exec: &mut dyn PatternExecutor) -> EvalResult {
        let expr_id = self.get_prop(name)?;
        exec.eval(expr_id)
    }

    /// Get a required property and evaluate it, attaching span on error.
    ///
    /// This is like `eval_prop` but attaches the property's span to any evaluation error,
    /// providing better error messages with location information.
    pub fn eval_prop_spanned(&self, name: &str, exec: &mut dyn PatternExecutor) -> EvalResult {
        let expr_id = self.get_prop(name)?;
        let span = self.arena.get_expr(expr_id).span;
        exec.eval(expr_id)
            .map_err(|action| action.with_span_if_error(span))
    }

    /// Get an optional property and evaluate it if present.
    ///
    /// Returns `Ok(None)` if the property is not present, `Ok(Some(value))` if present
    /// and evaluation succeeds, or `Err` if evaluation fails.
    pub fn eval_prop_opt(
        &self,
        name: &str,
        exec: &mut dyn PatternExecutor,
    ) -> Result<Option<Value>, ControlAction> {
        match self.get_prop_opt(name) {
            Some(expr_id) => Ok(Some(exec.eval(expr_id)?)),
            None => Ok(None),
        }
    }

    /// Get an optional property and evaluate it if present, attaching span on error.
    ///
    /// This is like `eval_prop_opt` but attaches the property's span to any evaluation error.
    pub fn eval_prop_opt_spanned(
        &self,
        name: &str,
        exec: &mut dyn PatternExecutor,
    ) -> Result<Option<Value>, ControlAction> {
        match self.get_prop_opt(name) {
            Some(expr_id) => {
                let span = self.arena.get_expr(expr_id).span;
                let value = exec
                    .eval(expr_id)
                    .map_err(|action| action.with_span_if_error(span))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Create an error with span attached from a named property.
    ///
    /// If the property exists, attaches its span to the error.
    /// Otherwise, uses the first property's span as a fallback.
    #[cold]
    pub fn error_with_prop_span(&self, message: impl Into<String>, prop_name: &str) -> EvalError {
        let err = EvalError::new(message);
        if let Some(span) = self.prop_span(prop_name) {
            err.with_span(span)
        } else if let Some(span) = self.first_prop_span() {
            err.with_span(span)
        } else {
            err
        }
    }
}

// Iterable Abstraction

/// Represents something that can be iterated over in pattern evaluation.
///
/// This abstraction unifies the handling of lists and ranges across patterns
/// like `map`, `filter`, `fold`, `find`, and `collect`.
#[derive(Clone)]
pub enum Iterable {
    /// A list of values.
    List(Heap<Vec<Value>>),
    /// A range of integers.
    Range(RangeValue),
}

/// Iterator over Iterable values.
///
/// Uses enum dispatch instead of `Box<dyn Iterator>` for better performance
/// (no heap allocation, no vtable indirection).
pub enum IterableIter<'a> {
    /// Iterator over list elements (cloned).
    List(std::iter::Cloned<std::slice::Iter<'a, Value>>),
    /// Iterator over range integers converted to Values.
    Range(std::iter::Map<std::ops::Range<i64>, fn(i64) -> Value>),
}

impl Iterator for IterableIter<'_> {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::List(iter) => iter.next(),
            Self::Range(iter) => iter.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::List(iter) => iter.size_hint(),
            Self::Range(iter) => iter.size_hint(),
        }
    }
}

impl Iterable {
    /// Try to convert a Value into an Iterable.
    ///
    /// Returns an error if the value is neither a list nor a range.
    pub fn try_from_value(value: Value) -> Result<Self, EvalError> {
        match value {
            Value::List(list) => Ok(Iterable::List(list)),
            Value::Range(range) => Ok(Iterable::Range(range)),
            _ => Err(EvalError::new(format!(
                "expected a list or range, got {}",
                value.type_name()
            ))),
        }
    }

    /// Iterate over the values in this iterable.
    ///
    /// Returns owned `Value`s: cloned from List elements or created from Range integers.
    ///
    /// # Performance
    /// Uses `IterableIter` enum dispatch instead of `Box<dyn Iterator>` for
    /// zero-allocation iteration (no heap allocation, no vtable indirection).
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "range bound arithmetic on user-provided i64 values"
    )]
    fn iter_values(&self) -> IterableIter<'_> {
        match self {
            Iterable::List(list) => IterableIter::List(list.iter().cloned()),
            Iterable::Range(range) => {
                let end = if range.inclusive {
                    range.end + 1
                } else {
                    range.end
                };
                IterableIter::Range((range.start..end).map(Value::int as fn(i64) -> Value))
            }
        }
    }

    /// Apply a mapping function to each element and collect results into a list.
    pub fn map_values(&self, func: &Value, exec: &mut dyn PatternExecutor) -> EvalResult {
        let mut results = Vec::new();
        for item in self.iter_values() {
            let result = exec.call(func, vec![item])?;
            results.push(result);
        }
        Ok(Value::list(results))
    }

    /// Filter elements using a predicate function and collect results into a list.
    pub fn filter_values(&self, func: &Value, exec: &mut dyn PatternExecutor) -> EvalResult {
        let mut results = Vec::new();
        for item in self.iter_values() {
            let keep = exec.call(func, vec![item.clone()])?;
            if keep.is_truthy() {
                results.push(item);
            }
        }
        Ok(Value::list(results))
    }

    /// Reduce the iterable to a single value using an accumulator function.
    pub fn fold_values(
        &self,
        mut acc: Value,
        func: &Value,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        for item in self.iter_values() {
            acc = exec.call(func, vec![acc, item])?;
        }
        Ok(acc)
    }

    /// Find the first element matching a predicate.
    ///
    /// Returns `Some(value)` if found, `None` if not found.
    pub fn find_value(
        &self,
        func: &Value,
        exec: &mut dyn PatternExecutor,
    ) -> Result<Option<Value>, ControlAction> {
        for item in self.iter_values() {
            let matches = exec.call(func, vec![item.clone()])?;
            if matches.is_truthy() {
                return Ok(Some(item));
            }
        }
        Ok(None)
    }
}

/// Actions that patterns can request the evaluator to perform.
#[derive(Debug)]
pub enum EvalAction {
    /// Evaluate an expression.
    Eval(ExprId),
    /// Call a function value with arguments.
    Call(Value, Vec<Value>),
}

/// Executor trait for pattern evaluation.
///
/// This trait abstracts over the evaluator, allowing patterns to request
/// expression evaluation and function calls without directly accessing the evaluator.
pub trait PatternExecutor {
    /// Evaluate an expression by its ID.
    fn eval(&mut self, expr_id: ExprId) -> EvalResult;

    /// Call a function value with the given arguments.
    fn call(&mut self, func: &Value, args: Vec<Value>) -> EvalResult;

    /// Look up a capability from the environment.
    ///
    /// Used by patterns that need to access capabilities like Print.
    fn lookup_capability(&self, name: Name) -> Option<Value>;

    /// Call a method on a value.
    ///
    /// Used by patterns to invoke capability methods.
    fn call_method(&mut self, receiver: Value, method: Name, args: Vec<Value>) -> EvalResult;

    /// Look up a variable in the current scope.
    ///
    /// Returns `None` if the variable is not defined.
    fn lookup_var(&self, name: Name) -> Option<Value>;

    /// Bind a variable in the current scope.
    ///
    /// Used by patterns to introduce scoped bindings during evaluation.
    fn bind_var(&mut self, name: Name, value: Value);
}

// Focused Pattern Traits (ISP Compliance)

/// Core pattern behavior - required by all patterns.
///
/// This is the minimal interface that every pattern must implement.
/// More specialized behaviors are defined in separate traits.
pub trait PatternCore: Send + Sync {
    /// The pattern's name (e.g., "map", "filter").
    fn name(&self) -> &'static str;

    /// Required property names for this pattern.
    fn required_props(&self) -> &'static [&'static str];

    /// Evaluate this pattern.
    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult;
}

/// Patterns that support fusion (map, filter only).
///
/// Pattern fusion combines multiple patterns into a single pass,
/// improving performance by avoiding intermediate allocations.
pub trait PatternFusable: PatternCore {
    /// Check if this pattern can be fused with the given next pattern.
    fn can_fuse_with(&self, next_name: &str) -> bool;

    /// Create a fused pattern combining this pattern with the next one.
    fn fuse_with(
        &self,
        next: &dyn PatternDefinition,
        self_ctx: &EvalContext,
        next_ctx: &EvalContext,
    ) -> Option<FusedPattern>;
}

/// Patterns that accept arbitrary properties (parallel only).
///
/// Most patterns have a fixed set of required and optional properties.
/// This trait marks patterns that can accept any property names.
pub trait PatternVariadic: PatternCore {
    /// Always returns true - variadic patterns accept any properties.
    fn allows_arbitrary_props(&self) -> bool {
        true
    }
}

// Main PatternDefinition Trait (Backward Compatible)

/// Describes a binding that should be in scope when type-checking certain properties.
///
/// This allows patterns to introduce identifiers (like `self` for recursion) that
/// are available during type checking of specific properties.
#[derive(Clone, Debug)]
pub struct ScopedBinding {
    /// The identifier name to bind (e.g., "self").
    pub name: &'static str,
    /// Properties that require this binding to be in scope.
    pub for_props: &'static [&'static str],
    /// How to compute the binding's type from other properties.
    pub type_from: ScopedBindingType,
}

/// How to derive a scoped binding's type from other property types.
#[derive(Clone, Debug)]
pub enum ScopedBindingType {
    /// The binding has the same type as another property.
    SameAs(&'static str),
    /// The binding is a zero-argument function returning the same type as another property.
    FunctionReturning(&'static str),
    /// The binding is a function with the same signature as the enclosing function.
    /// Used for `self` in `recurse` pattern to enable recursive calls with arguments.
    EnclosingFunction,
}

/// Trait defining a pattern's behavior across compilation phases.
///
/// Each pattern (map, filter, fold, etc.) implements this trait to define
/// its evaluation semantics. Type checking is handled by `ModuleChecker`
/// in `ori_types`.
///
/// # Open/Closed Principle
/// Adding a new pattern requires:
/// 1. Create a new file in `patterns/`
/// 2. Implement `PatternDefinition`
/// 3. Register in `PatternRegistry::new()`
///
/// No modifications to evaluator.rs needed.
///
/// # Interface Segregation
/// This trait provides a complete interface.
/// For cleaner interfaces, consider implementing the focused traits:
/// - `PatternCore`: Required for all patterns
/// - `PatternFusable`: For patterns that support fusion
/// - `PatternVariadic`: For patterns accepting arbitrary properties
///
/// # Compilation Phases
/// Patterns participate in:
/// - **Evaluation**: `evaluate()` executes in the interpreter
/// - **Optimization**: `can_fuse_with()`/`fuse_with()` enable fusion
pub trait PatternDefinition: Send + Sync {
    /// The pattern's name (e.g., "map", "filter").
    fn name(&self) -> &'static str;

    /// Required property names for this pattern.
    fn required_props(&self) -> &'static [&'static str];

    /// Optional property names for this pattern.
    fn optional_props(&self) -> &'static [&'static str] {
        &[]
    }

    /// Optional arguments with their default values.
    ///
    /// Override this to provide default values for optional arguments.
    fn optional_args(&self) -> &'static [OptionalArg] {
        &[]
    }

    /// Scoped bindings to introduce during type checking.
    ///
    /// Some patterns introduce identifiers that are only available within certain
    /// property expressions. For example, `recurse` introduces `self` which is
    /// available in the `step` property.
    ///
    /// Default: no scoped bindings.
    fn scoped_bindings(&self) -> &'static [ScopedBinding] {
        &[]
    }

    /// Whether this pattern allows arbitrary additional properties.
    /// Only `parallel` uses this (for dynamic task properties).
    fn allows_arbitrary_props(&self) -> bool {
        false
    }

    /// Evaluate this pattern.
    ///
    /// Called during interpretation with the property expressions.
    /// The executor provides methods to evaluate expressions and call functions.
    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult;

    /// Check if this pattern can be fused with the given next pattern.
    ///
    /// Pattern fusion combines multiple patterns into a single pass,
    /// improving performance by avoiding intermediate allocations.
    ///
    /// Default: no fusion.
    fn can_fuse_with(&self, _next: &dyn PatternDefinition) -> bool {
        false
    }

    /// Create a fused pattern combining this pattern with the next one.
    ///
    /// Returns `None` if fusion is not possible. Override this method
    /// along with `can_fuse_with` to enable fusion for specific patterns.
    ///
    /// # Arguments
    /// * `next` - The pattern definition to fuse with
    /// * `self_ctx` - Evaluation context for this pattern
    /// * `next_ctx` - Evaluation context for the next pattern
    ///
    /// Default: no fusion.
    fn fuse_with(
        &self,
        _next: &dyn PatternDefinition,
        _self_ctx: &EvalContext,
        _next_ctx: &EvalContext,
    ) -> Option<FusedPattern> {
        None
    }
}

// Blanket Implementation

/// Blanket implementation: Any type implementing `PatternDefinition` also implements `PatternCore`.
impl<T: PatternDefinition> PatternCore for T {
    fn name(&self) -> &'static str {
        PatternDefinition::name(self)
    }

    fn required_props(&self) -> &'static [&'static str] {
        PatternDefinition::required_props(self)
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        PatternDefinition::evaluate(self, ctx, exec)
    }
}
