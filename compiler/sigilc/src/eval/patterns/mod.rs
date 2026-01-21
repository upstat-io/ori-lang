// Pattern evaluation for Sigil
// Uses the PatternDefinition trait for unified pattern handling

// Keep recurse module public for eval_recurse_step
pub mod recurse;

use crate::ast::PatternExpr;
use crate::patterns::builtins::{
    CollectPattern, CountPattern, FilterPattern, FoldPattern, IteratePattern, MapPattern,
    ParallelPattern, RecursePattern, TransformPattern,
};
use crate::patterns::core::PatternDefinition;

use super::value::{Environment, Value};

/// Evaluate a pattern expression using the PatternDefinition trait
pub fn eval_pattern(pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
    match pattern {
        PatternExpr::Fold { .. } => FoldPattern.evaluate(pattern, env),
        PatternExpr::Map { .. } => MapPattern.evaluate(pattern, env),
        PatternExpr::Filter { .. } => FilterPattern.evaluate(pattern, env),
        PatternExpr::Collect { .. } => CollectPattern.evaluate(pattern, env),
        PatternExpr::Count { .. } => CountPattern.evaluate(pattern, env),
        PatternExpr::Recurse { .. } => RecursePattern.evaluate(pattern, env),
        PatternExpr::Iterate { .. } => IteratePattern.evaluate(pattern, env),
        PatternExpr::Transform { .. } => TransformPattern.evaluate(pattern, env),
        PatternExpr::Parallel { .. } => ParallelPattern.evaluate(pattern, env),
    }
}
