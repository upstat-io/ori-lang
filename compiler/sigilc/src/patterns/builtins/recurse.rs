// Recurse pattern handler - Self-contained implementation
//
// This pattern is the single source of truth for recurse semantics:
// - Type checking (infer_type)
// - Evaluation (evaluate)
// - Documentation

use crate::ast::{PatternExpr, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};
use crate::eval::{eval_expr, is_truthy, Environment, Value};
use crate::patterns::core::{ParamSpec, PatternDefinition, TypeConstraint};
use crate::types::{check_expr, types_compatible, TypeContext};

/// Handler for the recurse pattern.
///
/// recurse(.cond: condition, .base: base_value, .step: recursive_step, .memo: bool)
///
/// Implements recursive computation with optional memoization.
pub struct RecursePattern;

static RECURSE_PARAMS: &[ParamSpec] = &[
    ParamSpec::required_with(".cond", "base case condition", TypeConstraint::Boolean),
    ParamSpec::required(".base", "base case value"),
    ParamSpec::required(".step", "recursive step (can use self())"),
    ParamSpec::flag(".memo", "enable memoization"),
    ParamSpec::optional_default(".parallel", "parallel threshold", "0"),
];

impl PatternDefinition for RecursePattern {
    fn keyword(&self) -> &'static str {
        "recurse"
    }

    fn params(&self) -> &'static [ParamSpec] {
        RECURSE_PARAMS
    }

    fn infer_type(&self, pattern: &PatternExpr, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
        let PatternExpr::Recurse {
            condition,
            base_value,
            step,
            ..
        } = pattern
        else {
            return Err(Diagnostic::error(
                ErrorCode::E3009,
                "expected recurse pattern",
            ));
        };

        // Check condition type (should be bool)
        check_expr(condition, ctx)?;

        // Check base value type
        let base_type = check_expr(base_value, ctx)?;

        // Check step type
        let step_type = check_expr(step, ctx)?;

        // Base and step should have compatible types
        if !types_compatible(&base_type, &step_type, ctx) {
            return Err(Diagnostic::error(
                ErrorCode::E3001,
                format!(
                    "recurse base type {:?} doesn't match step type {:?}",
                    base_type, step_type
                ),
            ));
        }

        Ok(base_type)
    }

    fn evaluate(&self, pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
        let PatternExpr::Recurse {
            condition,
            base_value,
            step,
            memo,
            parallel_threshold,
        } = pattern
        else {
            return Err("expected recurse pattern".to_string());
        };

        // For recurse, we need to be inside a function context
        // The recurse pattern creates a recursive function that:
        // 1. Checks condition - if true, returns base_value
        // 2. Otherwise evaluates step with self() for recursive calls

        let param_names = &env.current_params;

        // First evaluate condition
        let cond_result = eval_expr(condition, env)?;

        if is_truthy(&cond_result) {
            // Base case: return base_value
            eval_expr(base_value, env)
        } else {
            // Recursive case: evaluate step
            // Get current n value to check against parallel threshold
            let current_n = param_names
                .iter()
                .find_map(|name| {
                    if let Some(Value::Int(n)) = env.get(name) {
                        Some(n)
                    } else {
                        None
                    }
                })
                .unwrap_or(0);

            crate::eval::patterns::recurse::eval_recurse_step(
                step,
                condition,
                base_value,
                env,
                *memo,
                *parallel_threshold,
                current_n,
                param_names,
            )
        }
    }

    fn description(&self) -> &'static str {
        "Recursive computation with optional memoization"
    }

    fn help(&self) -> &'static str {
        r#"The recurse pattern implements recursion with optional memoization:
  recurse(.cond: n <= 1, .base: n, .step: self(n-1) + self(n-2), .memo: true)

- .cond: When true, return .base value (base case)
- .base: Value to return in base case
- .step: Expression for recursive case (use self() for recursive calls)
- .memo: Enable memoization for performance (optional, default: false)
- .parallel: Threshold for parallel execution (optional, default: 0)"#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "recurse(.cond: n <= 1, .base: n, .step: self(n-1) + self(n-2), .memo: true)",
            "recurse(.cond: n == 0, .base: 1, .step: n * self(n-1))",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recurse_pattern_keyword() {
        let recurse = RecursePattern;
        assert_eq!(recurse.keyword(), "recurse");
    }

    #[test]
    fn test_recurse_pattern_params() {
        let recurse = RecursePattern;
        let params = recurse.params();
        assert_eq!(params.len(), 5);
        assert_eq!(params[0].name, ".cond");
        assert_eq!(params[1].name, ".base");
        assert_eq!(params[2].name, ".step");
        assert_eq!(params[3].name, ".memo");
        assert_eq!(params[4].name, ".parallel");
    }

    #[test]
    fn test_recurse_pattern_description() {
        let recurse = RecursePattern;
        assert!(recurse.description().contains("memoization"));
    }
}
