// Pattern expression type checking for Sigil
// Uses the PatternDefinition trait for unified pattern handling

use super::context::TypeContext;
use crate::ast::{PatternExpr, TypeExpr};
use crate::patterns::builtins::{
    CollectPattern, CountPattern, FilterPattern, FoldPattern, IteratePattern, MapPattern,
    ParallelPattern, RecursePattern, TransformPattern,
};
use crate::patterns::core::PatternDefinition;

/// Type check a pattern expression using the PatternDefinition trait
pub fn check_pattern_expr(p: &PatternExpr, ctx: &TypeContext) -> Result<TypeExpr, String> {
    let result = match p {
        PatternExpr::Fold { .. } => FoldPattern.infer_type(p, ctx),
        PatternExpr::Map { .. } => MapPattern.infer_type(p, ctx),
        PatternExpr::Filter { .. } => FilterPattern.infer_type(p, ctx),
        PatternExpr::Collect { .. } => CollectPattern.infer_type(p, ctx),
        PatternExpr::Count { .. } => CountPattern.infer_type(p, ctx),
        PatternExpr::Recurse { .. } => RecursePattern.infer_type(p, ctx),
        PatternExpr::Iterate { .. } => IteratePattern.infer_type(p, ctx),
        PatternExpr::Transform { .. } => TransformPattern.infer_type(p, ctx),
        PatternExpr::Parallel { .. } => ParallelPattern.infer_type(p, ctx),
    };

    // Convert DiagnosticResult to Result<TypeExpr, String>
    result.map_err(|d| d.message)
}
