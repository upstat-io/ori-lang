//! Pattern system module.
//!
//! Implements the Open/Closed principle from `docs/compiler-design/v3/02-design-principles.md`:
//! > New patterns via trait implementation, no changes to existing code required.
//!
//! Each pattern (map, filter, fold, etc.) implements the `PatternDefinition` trait.
//! The `PatternRegistry` allows looking up patterns by their `FunctionExpKind`.

mod registry;
mod signature;
mod fusion;

// Individual pattern implementations.
// Note: map, filter, fold, find, collect, retry, validate are now methods on
// collections per "Lean Core, Rich Libraries". Kept for reference/future use.
#[allow(dead_code)]
mod map;
#[allow(dead_code)]
mod filter;
#[allow(dead_code)]
mod fold;
#[allow(dead_code)]
mod find;
#[allow(dead_code)]
mod collect;
mod recurse;
mod parallel;
mod spawn;
mod timeout;
#[allow(dead_code)]
mod retry;
mod cache;
#[allow(dead_code)]
mod validate;
mod with_pattern;
mod builtins;

pub use registry::{PatternRegistry, SharedPattern};
pub use signature::{PatternSignature, FunctionSignature, DefaultValue, OptionalArg};
pub use fusion::{FusedPattern, FusionAnalyzer, FusionHints, PatternChain, ChainLink};

use crate::ir::{Name, NamedExpr, ExprId, StringInterner, ExprArena};
use crate::types::{Type, InferenceContext};
use crate::eval::{Value, RangeValue, EvalResult, EvalError, Heap};
use std::collections::HashMap;

/// Context for type checking a pattern.
///
/// Wraps the necessary components for type inference during pattern type checking.
pub struct TypeCheckContext<'a> {
    pub interner: &'a StringInterner,
    pub ctx: &'a mut InferenceContext,
    /// Types of evaluated properties, keyed by property name.
    pub prop_types: HashMap<Name, Type>,
}

impl<'a> TypeCheckContext<'a> {
    /// Create a new type check context.
    pub fn new(
        interner: &'a StringInterner,
        ctx: &'a mut InferenceContext,
        prop_types: HashMap<Name, Type>,
    ) -> Self {
        TypeCheckContext {
            interner,
            ctx,
            prop_types,
        }
    }

    /// Get the type of a required property.
    pub fn get_prop_type(&self, name: &str) -> Option<Type> {
        let name_id = self.interner.intern(name);
        self.prop_types.get(&name_id).cloned()
    }

    /// Get the type of a required property, returning Error type if missing.
    pub fn require_prop_type(&self, name: &str) -> Type {
        self.get_prop_type(name).unwrap_or(Type::Error)
    }

    /// Create a fresh type variable.
    pub fn fresh_var(&mut self) -> Type {
        self.ctx.fresh_var()
    }

    /// Get the return type of a function-typed property.
    ///
    /// Returns a fresh type variable if the property is missing or not a function type.
    pub fn get_function_return_type(&mut self, prop_name: &str) -> Type {
        match self.get_prop_type(prop_name) {
            Some(Type::Function { ret, .. }) => *ret,
            _ => self.fresh_var(),
        }
    }

    /// Wrap a type in List.
    pub fn list_of(&self, elem: Type) -> Type {
        Type::List(Box::new(elem))
    }

    /// Wrap a type in Option.
    pub fn option_of(&self, elem: Type) -> Type {
        Type::Option(Box::new(elem))
    }

    /// Create a Result type with the given ok and error types.
    pub fn result_of(&self, ok: Type, err: Type) -> Type {
        Type::Result {
            ok: Box::new(ok),
            err: Box::new(err),
        }
    }
}

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
    pub fn new(
        interner: &'a StringInterner,
        arena: &'a ExprArena,
        props: &'a [NamedExpr],
    ) -> Self {
        EvalContext {
            interner,
            arena,
            props,
        }
    }

    /// Get a required property's ExprId by name.
    pub fn get_prop(&self, name: &str) -> Result<ExprId, EvalError> {
        let target = self.interner.intern(name);
        for prop in self.props {
            if prop.name == target {
                return Ok(prop.value);
            }
        }
        Err(EvalError::new(format!("missing required property: .{}", name)))
    }

    /// Get an optional property's ExprId by name.
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

    /// Get an optional property and evaluate it if present.
    ///
    /// Returns `Ok(None)` if the property is not present, `Ok(Some(value))` if present
    /// and evaluation succeeds, or `Err` if evaluation fails.
    pub fn eval_prop_opt(&self, name: &str, exec: &mut dyn PatternExecutor) -> Result<Option<Value>, EvalError> {
        match self.get_prop_opt(name) {
            Some(expr_id) => Ok(Some(exec.eval(expr_id)?)),
            None => Ok(None),
        }
    }
}

// =============================================================================
// Iterable Abstraction
// =============================================================================

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

    /// Apply a mapping function to each element and collect results into a list.
    pub fn map_values(&self, func: &Value, exec: &mut dyn PatternExecutor) -> EvalResult {
        match self {
            Iterable::List(list) => {
                let mut results = Vec::with_capacity(list.len());
                for item in list.iter() {
                    let result = exec.call(func.clone(), vec![item.clone()])?;
                    results.push(result);
                }
                Ok(Value::list(results))
            }
            Iterable::Range(range) => {
                let mut results = Vec::new();
                for i in range.iter() {
                    let result = exec.call(func.clone(), vec![Value::Int(i)])?;
                    results.push(result);
                }
                Ok(Value::list(results))
            }
        }
    }

    /// Filter elements using a predicate function and collect results into a list.
    pub fn filter_values(&self, func: &Value, exec: &mut dyn PatternExecutor) -> EvalResult {
        match self {
            Iterable::List(list) => {
                let mut results = Vec::new();
                for item in list.iter() {
                    let keep = exec.call(func.clone(), vec![item.clone()])?;
                    if keep.is_truthy() {
                        results.push(item.clone());
                    }
                }
                Ok(Value::list(results))
            }
            Iterable::Range(range) => {
                let mut results = Vec::new();
                for i in range.iter() {
                    let val = Value::Int(i);
                    let keep = exec.call(func.clone(), vec![val.clone()])?;
                    if keep.is_truthy() {
                        results.push(val);
                    }
                }
                Ok(Value::list(results))
            }
        }
    }

    /// Reduce the iterable to a single value using an accumulator function.
    pub fn fold_values(&self, mut acc: Value, func: &Value, exec: &mut dyn PatternExecutor) -> EvalResult {
        match self {
            Iterable::List(list) => {
                for item in list.iter() {
                    acc = exec.call(func.clone(), vec![acc, item.clone()])?;
                }
                Ok(acc)
            }
            Iterable::Range(range) => {
                for i in range.iter() {
                    acc = exec.call(func.clone(), vec![acc, Value::Int(i)])?;
                }
                Ok(acc)
            }
        }
    }

    /// Find the first element matching a predicate.
    ///
    /// Returns `Some(value)` if found, `None` if not found.
    pub fn find_value(&self, func: &Value, exec: &mut dyn PatternExecutor) -> Result<Option<Value>, EvalError> {
        match self {
            Iterable::List(list) => {
                for item in list.iter() {
                    let matches = exec.call(func.clone(), vec![item.clone()])?;
                    if matches.is_truthy() {
                        return Ok(Some(item.clone()));
                    }
                }
                Ok(None)
            }
            Iterable::Range(range) => {
                for i in range.iter() {
                    let val = Value::Int(i);
                    let matches = exec.call(func.clone(), vec![val.clone()])?;
                    if matches.is_truthy() {
                        return Ok(Some(val));
                    }
                }
                Ok(None)
            }
        }
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
    fn call(&mut self, func: Value, args: Vec<Value>) -> EvalResult;
}

// =============================================================================
// Focused Pattern Traits (ISP Compliance)
// =============================================================================

/// Core pattern behavior - required by all patterns.
///
/// This is the minimal interface that every pattern must implement.
/// More specialized behaviors are defined in separate traits.
pub trait PatternCore: Send + Sync {
    /// The pattern's name (e.g., "map", "filter").
    fn name(&self) -> &'static str;

    /// Required property names for this pattern.
    fn required_props(&self) -> &'static [&'static str];

    /// Type check this pattern and return its result type.
    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type;

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

// =============================================================================
// Main PatternDefinition Trait (Backward Compatible)
// =============================================================================

/// Trait defining a pattern's behavior across compilation phases.
///
/// Each pattern (map, filter, fold, etc.) implements this trait to define
/// its type checking and evaluation semantics.
///
/// # Open/Closed Principle
/// Adding a new pattern requires:
/// 1. Create a new file in `patterns/`
/// 2. Implement `PatternDefinition`
/// 3. Register in `PatternRegistry::new()`
///
/// No modifications to evaluator.rs or typeck.rs needed.
///
/// # Interface Segregation
/// This trait provides a complete interface for backward compatibility.
/// For cleaner interfaces, consider implementing the focused traits:
/// - `PatternCore`: Required for all patterns
/// - `PatternFusable`: For patterns that support fusion
/// - `PatternVariadic`: For patterns accepting arbitrary properties
///
/// # Compilation Phases
/// Patterns participate in multiple compilation phases:
/// - **Type checking**: `type_check()` infers and validates types
/// - **Evaluation**: `evaluate()` executes in the interpreter
/// - **Code generation**: `signature()` enables template caching (future)
/// - **Optimization**: `can_fuse_with()`/`fuse_with()` enable fusion (future)
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

    /// Whether this pattern allows arbitrary additional properties.
    /// Only `parallel` uses this (for dynamic task properties).
    fn allows_arbitrary_props(&self) -> bool {
        false
    }

    /// Type check this pattern and return its result type.
    ///
    /// Called during type checking with the types of all property expressions.
    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type;

    /// Evaluate this pattern.
    ///
    /// Called during interpretation with the property expressions.
    /// The executor provides methods to evaluate expressions and call functions.
    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult;

    /// Compute the pattern signature for template caching.
    ///
    /// Two patterns with the same signature can share compiled templates.
    /// This enables efficient code generation through template reuse.
    ///
    /// Default implementation returns a basic signature with just the pattern name.
    fn signature(&self, ctx: &TypeCheckContext) -> PatternSignature {
        // Default: minimal signature with pattern name only
        // Patterns should override this for proper template caching
        let kind = ctx.interner.intern(self.name());
        PatternSignature::new(kind, crate::ir::TypeId::INFER)
    }

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

// =============================================================================
// Blanket Implementation
// =============================================================================

/// Blanket implementation: Any type implementing PatternDefinition also implements PatternCore.
impl<T: PatternDefinition> PatternCore for T {
    fn name(&self) -> &'static str {
        PatternDefinition::name(self)
    }

    fn required_props(&self) -> &'static [&'static str] {
        PatternDefinition::required_props(self)
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        PatternDefinition::type_check(self, ctx)
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        PatternDefinition::evaluate(self, ctx, exec)
    }
}
