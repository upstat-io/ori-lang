// Pattern evaluation for Sigil
// Dispatcher module - delegates to specialized pattern evaluators

mod collect;
mod count;
mod filter;
mod fold;
mod iterate;
mod map;
mod parallel;
mod recurse;
mod transform;

use crate::ast::*;

use super::value::{Environment, Value};

pub fn eval_pattern(pattern: &PatternExpr, env: &Environment) -> Result<Value, String> {
    match pattern {
        PatternExpr::Fold {
            collection,
            init,
            op,
        } => fold::eval_fold(collection, init, op, env),

        PatternExpr::Map {
            collection,
            transform,
        } => map::eval_map(collection, transform, env),

        PatternExpr::Filter {
            collection,
            predicate,
        } => filter::eval_filter(collection, predicate, env),

        PatternExpr::Collect { range, transform } => collect::eval_collect(range, transform, env),

        PatternExpr::Count {
            collection,
            predicate,
        } => count::eval_count(collection, predicate, env),

        PatternExpr::Recurse {
            condition,
            base_value,
            step,
            memo,
            parallel_threshold,
        } => recurse::eval_recurse(condition, base_value, step, *memo, *parallel_threshold, env),

        PatternExpr::Iterate {
            over,
            direction,
            into,
            with,
        } => iterate::eval_iterate(over, direction, into, with, env),

        PatternExpr::Transform { input, steps } => transform::eval_transform(input, steps, env),

        PatternExpr::Parallel {
            branches,
            timeout,
            on_error,
        } => parallel::eval_parallel(branches, timeout, on_error, env),
    }
}
