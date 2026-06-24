//! Pattern trait hierarchy (ISP-compliant): [`PatternCore`], [`PatternFusable`],
//! [`PatternVariadic`], [`PatternDefinition`], and the scoped-binding descriptors.

use crate::context::EvalContext;
use crate::errors::EvalResult;
use crate::executor::PatternExecutor;
use crate::fusion::FusedPattern;
use crate::signature::OptionalArg;

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
