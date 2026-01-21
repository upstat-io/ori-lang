// Pattern definition trait for the Sigil compiler
//
// The unified trait for pattern implementations. Each pattern implements
// this trait to provide its type checking, evaluation, and lowering behavior
// in a single, self-contained module.

use crate::ast::{PatternExpr, TypeExpr};
use crate::errors::DiagnosticResult;
use crate::eval::value::{Environment, Value};
use crate::ir::{TExpr, Type};
use crate::types::context::TypeContext;

use super::param::ParamSpec;

/// The unified trait for pattern definitions.
///
/// Each pattern (fold, map, filter, recurse, etc.) implements this trait
/// to provide all its behavior in one place:
/// - Type checking: Validates types and infers result type
/// - Evaluation: Executes the pattern at runtime
/// - Lowering: Transforms to TIR for code generation
///
/// This is the "single source of truth" for each pattern's semantics.
///
/// # Example
///
/// ```ignore
/// pub struct FoldPattern;
///
/// impl PatternDefinition for FoldPattern {
///     fn keyword(&self) -> &'static str { "fold" }
///
///     fn params(&self) -> &'static [ParamSpec] {
///         &[
///             ParamSpec::required_with(".over", "collection to fold", TypeConstraint::Iterable),
///             ParamSpec::required(".init", "initial accumulator value"),
///             ParamSpec::required(".with", "combining function"),
///         ]
///     }
///
///     fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
///         // Type inference logic
///     }
///
///     fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
///         // Evaluation logic
///     }
///
///     fn lower_to_tir(&self, pattern: &PatternExpr, ctx: &LowerContext) -> TExpr {
///         // Lowering logic
///     }
/// }
/// ```
pub trait PatternDefinition: Send + Sync + 'static {
    /// Returns the keyword that identifies this pattern (e.g., "fold", "map").
    fn keyword(&self) -> &'static str;

    /// Returns the parameter specifications for this pattern.
    fn params(&self) -> &'static [ParamSpec];

    /// Infer the result type of a pattern expression.
    ///
    /// This performs type checking and returns the inferred result type.
    /// Uses the new DiagnosticResult type for structured error reporting.
    ///
    /// # Arguments
    /// * `pattern` - The pattern expression to type check
    /// * `ctx` - The type checking context
    ///
    /// # Returns
    /// The inferred result type wrapped in DiagnosticResult
    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr>;

    /// Evaluate a pattern expression at runtime.
    ///
    /// # Arguments
    /// * `pattern` - The pattern expression to evaluate
    /// * `env` - The runtime environment
    ///
    /// # Returns
    /// The computed value, or an error string
    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String>;

    /// Lower a pattern expression to typed intermediate representation.
    ///
    /// This transforms the high-level pattern into lower-level constructs
    /// suitable for code generation.
    ///
    /// # Arguments
    /// * `pattern` - The pattern expression to lower
    /// * `result_type` - The expected result type
    ///
    /// # Returns
    /// The lowered TIR expression, or None to use default lowering
    fn lower_to_tir(&self, _pattern: &PatternExpr, _result_type: &Type) -> Option<TExpr> {
        None // Default: use standard lowering
    }

    /// Returns a human-readable description of this pattern.
    fn description(&self) -> &'static str {
        "pattern"
    }

    /// Returns an extended help message for documentation.
    fn help(&self) -> &'static str {
        ""
    }

    /// Returns example usage strings.
    fn examples(&self) -> &'static [&'static str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test that a basic implementation compiles
    struct TestPattern;

    impl PatternDefinition for TestPattern {
        fn keyword(&self) -> &'static str {
            "test"
        }

        fn params(&self) -> &'static [ParamSpec] {
            &[]
        }

        fn infer_type(
            &self,
            _pattern: &PatternExpr,
            _ctx: &TypeContext,
        ) -> DiagnosticResult<TypeExpr> {
            Ok(TypeExpr::Named("void".to_string()))
        }

        fn evaluate(&self, _pattern: &PatternExpr, _env: &Environment) -> Result<Value, String> {
            Ok(Value::Nil)
        }

        fn description(&self) -> &'static str {
            "A test pattern"
        }
    }

    #[test]
    fn test_pattern_definition_trait() {
        let pattern = TestPattern;
        assert_eq!(pattern.keyword(), "test");
        assert_eq!(pattern.description(), "A test pattern");
        assert!(pattern.params().is_empty());
    }
}
