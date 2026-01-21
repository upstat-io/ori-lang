// Pattern argument extraction for the Sigil compiler
//
// Provides helpers for extracting and validating pattern arguments
// from PatternExpr variants.

use crate::ast::{Expr, PatternExpr};
use crate::errors::{codes::ErrorCode, Diagnostic};

use super::param::ParamSpec;

/// Extracted arguments from a pattern expression.
///
/// Provides type-safe access to pattern arguments with validation.
pub struct PatternArgs<'a> {
    /// The pattern keyword (e.g., "fold", "map")
    pub keyword: &'static str,
    /// Extracted named arguments
    args: Vec<(&'static str, Option<&'a Expr>)>,
}

impl<'a> PatternArgs<'a> {
    /// Create a new PatternArgs from specs and extracted values.
    pub fn new(keyword: &'static str, specs: &'static [ParamSpec]) -> Self {
        let args = specs.iter().map(|spec| (spec.name, None)).collect();
        PatternArgs { keyword, args }
    }

    /// Set an argument value.
    pub fn set(&mut self, name: &'static str, value: &'a Expr) {
        if let Some(arg) = self.args.iter_mut().find(|(n, _)| *n == name) {
            arg.1 = Some(value);
        }
    }

    /// Get a required argument, returning an error if missing.
    pub fn get_required(&self, name: &str) -> Result<&'a Expr, Diagnostic> {
        self.get(name).ok_or_else(|| {
            Diagnostic::error(
                ErrorCode::E2005,
                format!(
                    "pattern '{}' requires argument '{}'",
                    self.keyword, name
                ),
            )
        })
    }

    /// Get an optional argument.
    pub fn get(&self, name: &str) -> Option<&'a Expr> {
        self.args
            .iter()
            .find(|(n, _)| *n == name)
            .and_then(|(_, v)| *v)
    }

    /// Check if an argument is present.
    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Validate that all required arguments are present.
    pub fn validate(&self, specs: &[ParamSpec]) -> Result<(), Diagnostic> {
        for spec in specs {
            if spec.required && self.get(spec.name).is_none() {
                return Err(Diagnostic::error(
                    ErrorCode::E2005,
                    format!(
                        "pattern '{}' requires argument '{}': {}",
                        self.keyword, spec.name, spec.description
                    ),
                ));
            }
        }
        Ok(())
    }
}

/// Extract arguments from a Fold pattern.
pub fn extract_fold_args(pattern: &PatternExpr) -> Option<FoldArgs<'_>> {
    match pattern {
        PatternExpr::Fold {
            collection,
            init,
            op,
        } => Some(FoldArgs {
            collection,
            init,
            op,
        }),
        _ => None,
    }
}

/// Type-safe fold pattern arguments.
pub struct FoldArgs<'a> {
    pub collection: &'a Expr,
    pub init: &'a Expr,
    pub op: &'a Expr,
}

/// Extract arguments from a Map pattern.
pub fn extract_map_args(pattern: &PatternExpr) -> Option<MapArgs<'_>> {
    match pattern {
        PatternExpr::Map {
            collection,
            transform,
        } => Some(MapArgs {
            collection,
            transform,
        }),
        _ => None,
    }
}

/// Type-safe map pattern arguments.
pub struct MapArgs<'a> {
    pub collection: &'a Expr,
    pub transform: &'a Expr,
}

/// Extract arguments from a Filter pattern.
pub fn extract_filter_args(pattern: &PatternExpr) -> Option<FilterArgs<'_>> {
    match pattern {
        PatternExpr::Filter {
            collection,
            predicate,
        } => Some(FilterArgs {
            collection,
            predicate,
        }),
        _ => None,
    }
}

/// Type-safe filter pattern arguments.
pub struct FilterArgs<'a> {
    pub collection: &'a Expr,
    pub predicate: &'a Expr,
}

/// Extract arguments from a Collect pattern.
pub fn extract_collect_args(pattern: &PatternExpr) -> Option<CollectArgs<'_>> {
    match pattern {
        PatternExpr::Collect { range, transform } => Some(CollectArgs { range, transform }),
        _ => None,
    }
}

/// Type-safe collect pattern arguments.
pub struct CollectArgs<'a> {
    pub range: &'a Expr,
    pub transform: &'a Expr,
}

/// Extract arguments from a Recurse pattern.
pub fn extract_recurse_args(pattern: &PatternExpr) -> Option<RecurseArgs<'_>> {
    match pattern {
        PatternExpr::Recurse {
            condition,
            base_value,
            step,
            memo,
            parallel_threshold,
        } => Some(RecurseArgs {
            condition,
            base_value,
            step,
            memo: *memo,
            parallel_threshold: *parallel_threshold,
        }),
        _ => None,
    }
}

/// Type-safe recurse pattern arguments.
pub struct RecurseArgs<'a> {
    pub condition: &'a Expr,
    pub base_value: &'a Expr,
    pub step: &'a Expr,
    pub memo: bool,
    pub parallel_threshold: i64,
}

/// Extract arguments from an Iterate pattern.
pub fn extract_iterate_args(pattern: &PatternExpr) -> Option<IterateArgs<'_>> {
    match pattern {
        PatternExpr::Iterate {
            over,
            direction,
            into,
            with,
        } => Some(IterateArgs {
            over,
            direction: *direction,
            into,
            with,
        }),
        _ => None,
    }
}

/// Type-safe iterate pattern arguments.
pub struct IterateArgs<'a> {
    pub over: &'a Expr,
    pub direction: crate::ast::IterDirection,
    pub into: &'a Expr,
    pub with: &'a Expr,
}

/// Extract arguments from a Transform pattern.
pub fn extract_transform_args(pattern: &PatternExpr) -> Option<TransformArgs<'_>> {
    match pattern {
        PatternExpr::Transform { input, steps } => Some(TransformArgs { input, steps }),
        _ => None,
    }
}

/// Type-safe transform pattern arguments.
pub struct TransformArgs<'a> {
    pub input: &'a Expr,
    pub steps: &'a Vec<Expr>,
}

/// Extract arguments from a Count pattern.
pub fn extract_count_args(pattern: &PatternExpr) -> Option<CountArgs<'_>> {
    match pattern {
        PatternExpr::Count {
            collection,
            predicate,
        } => Some(CountArgs {
            collection,
            predicate,
        }),
        _ => None,
    }
}

/// Type-safe count pattern arguments.
pub struct CountArgs<'a> {
    pub collection: &'a Expr,
    pub predicate: &'a Expr,
}

/// Extract arguments from a Parallel pattern.
pub fn extract_parallel_args(pattern: &PatternExpr) -> Option<ParallelArgs<'_>> {
    match pattern {
        PatternExpr::Parallel {
            branches,
            timeout,
            on_error,
        } => Some(ParallelArgs {
            branches,
            timeout: timeout.as_deref(),
            on_error: *on_error,
        }),
        _ => None,
    }
}

/// Type-safe parallel pattern arguments.
pub struct ParallelArgs<'a> {
    pub branches: &'a Vec<(String, Expr)>,
    pub timeout: Option<&'a Expr>,
    pub on_error: crate::ast::OnError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::PatternExpr;

    #[test]
    fn test_extract_fold() {
        let pattern = PatternExpr::Fold {
            collection: Box::new(Expr::List(vec![])),
            init: Box::new(Expr::Int(0)),
            op: Box::new(Expr::Ident("+".to_string())),
        };

        let args = extract_fold_args(&pattern);
        assert!(args.is_some());
    }

    #[test]
    fn test_extract_wrong_pattern() {
        let pattern = PatternExpr::Map {
            collection: Box::new(Expr::List(vec![])),
            transform: Box::new(Expr::Ident("f".to_string())),
        };

        let args = extract_fold_args(&pattern);
        assert!(args.is_none());
    }

    #[test]
    fn test_pattern_args_validation() {
        use super::super::param::ParamSpec;

        static SPECS: &[ParamSpec] = &[
            ParamSpec::required(".over", "collection"),
            ParamSpec::required(".init", "initial value"),
        ];

        let mut args = PatternArgs::new("fold", SPECS);
        // Missing both required args
        assert!(args.validate(SPECS).is_err());

        // Set one arg
        let expr = Expr::Int(0);
        args.set(".over", &expr);
        // Still missing .init
        assert!(args.validate(SPECS).is_err());
    }
}
