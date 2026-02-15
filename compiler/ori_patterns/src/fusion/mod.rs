//! Pattern fusion detection and optimization.
//!
//! Pattern fusion combines multiple sequential patterns into a single pass,
//! avoiding intermediate allocations and improving performance.
//!
//! ## Fusible Combinations
//!
//! | Pattern 1 | Pattern 2 | Pattern 3 | Fused Form      |
//! |-----------|-----------|-----------|-----------------|
//! | map       | filter    | -         | MapFilter       |
//! | map       | fold      | -         | MapFold         |
//! | filter    | fold      | -         | FilterFold      |
//! | map       | filter    | fold      | MapFilterFold   |
//! | filter    | map       | -         | FilterMap       |
//! | map       | find      | -         | MapFind         |
//! | filter    | find      | -         | FilterFind      |

use ori_ir::{ExprId, FunctionExpKind, Name, Span};

use crate::{EvalError, EvalResult, PatternExecutor, Value};

/// Fused pattern representation.
///
/// Each variant contains the expressions needed to evaluate the fused pattern
/// in a single pass over the input collection.
#[derive(Clone, Debug)]
pub enum FusedPattern {
    /// map followed by filter: `filter(over: map(over: xs, transform: f), predicate: p)`
    /// Evaluates as: for x in xs, let y = f(x), if p(y) then yield y
    MapFilter {
        input: ExprId,
        map_fn: ExprId,
        filter_fn: ExprId,
    },

    /// filter followed by map: `map(over: filter(over: xs, predicate: p), transform: f)`
    /// Evaluates as: for x in xs, if p(x) then yield f(x)
    FilterMap {
        input: ExprId,
        filter_fn: ExprId,
        map_fn: ExprId,
    },

    /// map followed by fold: `fold(over: map(over: xs, transform: f), init: i, op: g)`
    /// Evaluates as: acc = i; for x in xs, acc = g(acc, f(x)); return acc
    MapFold {
        input: ExprId,
        map_fn: ExprId,
        init: ExprId,
        fold_fn: ExprId,
    },

    /// filter followed by fold: `fold(over: filter(over: xs, predicate: p), init: i, op: g)`
    /// Evaluates as: acc = i; for x in xs, if p(x) then acc = g(acc, x); return acc
    FilterFold {
        input: ExprId,
        filter_fn: ExprId,
        init: ExprId,
        fold_fn: ExprId,
    },

    /// map, filter, then fold: full pipeline fusion
    /// Evaluates as: acc = i; for x in xs, let y = f(x), if p(y) then acc = g(acc, y); return acc
    MapFilterFold {
        input: ExprId,
        map_fn: ExprId,
        filter_fn: ExprId,
        init: ExprId,
        fold_fn: ExprId,
    },

    /// map followed by find: `find(over: map(over: xs, transform: f), where: p)`
    /// Evaluates as: for x in xs, let y = f(x), if p(y) then return Some(y); return None
    MapFind {
        input: ExprId,
        map_fn: ExprId,
        find_fn: ExprId,
    },

    /// filter followed by find: `find(over: filter(over: xs, predicate: p1), where: p2)`
    /// Evaluates as: for x in xs, if p1(x) && p2(x) then return Some(x); return None
    FilterFind {
        input: ExprId,
        filter_fn: ExprId,
        find_fn: ExprId,
    },
}

impl FusedPattern {
    /// Evaluate the fused pattern in a single pass.
    #[allow(
        clippy::result_large_err,
        reason = "EvalError is fundamental — boxing would add complexity across the crate"
    )]
    pub fn evaluate(&self, exec: &mut dyn PatternExecutor) -> EvalResult {
        match self {
            FusedPattern::MapFilter {
                input,
                map_fn,
                filter_fn,
            } => {
                let items = exec.eval(*input)?;
                let map_f = exec.eval(*map_fn)?;
                let filter_f = exec.eval(*filter_fn)?;

                match items {
                    Value::List(list) => {
                        let mut results = Vec::new();
                        for item in list.iter() {
                            // Apply map
                            let mapped = exec.call(&map_f, vec![item.clone()])?;
                            // Apply filter
                            if exec.call(&filter_f, vec![mapped.clone()])?.is_truthy() {
                                results.push(mapped);
                            }
                        }
                        Ok(Value::list(results))
                    }
                    _ => Err(EvalError::new("fused map-filter requires a list").into()),
                }
            }

            FusedPattern::FilterMap {
                input,
                filter_fn,
                map_fn,
            } => {
                let items = exec.eval(*input)?;
                let filter_f = exec.eval(*filter_fn)?;
                let map_f = exec.eval(*map_fn)?;

                match items {
                    Value::List(list) => {
                        let mut results = Vec::new();
                        for item in list.iter() {
                            // Apply filter first
                            if exec.call(&filter_f, vec![item.clone()])?.is_truthy() {
                                // Then map
                                let mapped = exec.call(&map_f, vec![item.clone()])?;
                                results.push(mapped);
                            }
                        }
                        Ok(Value::list(results))
                    }
                    _ => Err(EvalError::new("fused filter-map requires a list").into()),
                }
            }

            FusedPattern::MapFold {
                input,
                map_fn,
                init,
                fold_fn,
            } => {
                let items = exec.eval(*input)?;
                let map_f = exec.eval(*map_fn)?;
                let mut acc = exec.eval(*init)?;
                let fold_f = exec.eval(*fold_fn)?;

                match items {
                    Value::List(list) => {
                        for item in list.iter() {
                            // Apply map then fold in single pass
                            let mapped = exec.call(&map_f, vec![item.clone()])?;
                            acc = exec.call(&fold_f, vec![acc, mapped])?;
                        }
                        Ok(acc)
                    }
                    _ => Err(EvalError::new("fused map-fold requires a list").into()),
                }
            }

            FusedPattern::FilterFold {
                input,
                filter_fn,
                init,
                fold_fn,
            } => {
                let items = exec.eval(*input)?;
                let filter_f = exec.eval(*filter_fn)?;
                let mut acc = exec.eval(*init)?;
                let fold_f = exec.eval(*fold_fn)?;

                match items {
                    Value::List(list) => {
                        for item in list.iter() {
                            // Only fold items that pass filter
                            if exec.call(&filter_f, vec![item.clone()])?.is_truthy() {
                                acc = exec.call(&fold_f, vec![acc, item.clone()])?;
                            }
                        }
                        Ok(acc)
                    }
                    _ => Err(EvalError::new("fused filter-fold requires a list").into()),
                }
            }

            FusedPattern::MapFilterFold {
                input,
                map_fn,
                filter_fn,
                init,
                fold_fn,
            } => {
                let items = exec.eval(*input)?;
                let map_f = exec.eval(*map_fn)?;
                let filter_f = exec.eval(*filter_fn)?;
                let mut acc = exec.eval(*init)?;
                let fold_f = exec.eval(*fold_fn)?;

                match items {
                    Value::List(list) => {
                        for item in list.iter() {
                            // Map -> Filter -> Fold in single pass
                            let mapped = exec.call(&map_f, vec![item.clone()])?;
                            if exec.call(&filter_f, vec![mapped.clone()])?.is_truthy() {
                                acc = exec.call(&fold_f, vec![acc, mapped])?;
                            }
                        }
                        Ok(acc)
                    }
                    _ => Err(EvalError::new("fused map-filter-fold requires a list").into()),
                }
            }

            FusedPattern::MapFind {
                input,
                map_fn,
                find_fn,
            } => {
                let items = exec.eval(*input)?;
                let map_f = exec.eval(*map_fn)?;
                let find_f = exec.eval(*find_fn)?;

                match items {
                    Value::List(list) => {
                        for item in list.iter() {
                            let mapped = exec.call(&map_f, vec![item.clone()])?;
                            if exec.call(&find_f, vec![mapped.clone()])?.is_truthy() {
                                return Ok(Value::some(mapped));
                            }
                        }
                        Ok(Value::None)
                    }
                    _ => Err(EvalError::new("fused map-find requires a list").into()),
                }
            }

            FusedPattern::FilterFind {
                input,
                filter_fn,
                find_fn,
            } => {
                let items = exec.eval(*input)?;
                let filter_f = exec.eval(*filter_fn)?;
                let find_f = exec.eval(*find_fn)?;

                match items {
                    Value::List(list) => {
                        for item in list.iter() {
                            // Both predicates must pass
                            if exec.call(&filter_f, vec![item.clone()])?.is_truthy()
                                && exec.call(&find_f, vec![item.clone()])?.is_truthy()
                            {
                                return Ok(Value::some(item.clone()));
                            }
                        }
                        Ok(Value::None)
                    }
                    _ => Err(EvalError::new("fused filter-find requires a list").into()),
                }
            }
        }
    }

    /// Get the name of this fused pattern for debugging.
    pub fn name(&self) -> &'static str {
        match self {
            FusedPattern::MapFilter { .. } => "map-filter",
            FusedPattern::FilterMap { .. } => "filter-map",
            FusedPattern::MapFold { .. } => "map-fold",
            FusedPattern::FilterFold { .. } => "filter-fold",
            FusedPattern::MapFilterFold { .. } => "map-filter-fold",
            FusedPattern::MapFind { .. } => "map-find",
            FusedPattern::FilterFind { .. } => "filter-find",
        }
    }
}

/// A link in a pattern chain.
#[derive(Clone, Debug)]
pub struct ChainLink {
    /// The pattern kind.
    pub kind: FunctionExpKind,
    /// Named expression IDs for this pattern's arguments.
    pub props: Vec<(Name, ExprId)>,
    /// The expression ID of this pattern call.
    pub expr_id: ExprId,
    /// Span of this pattern.
    pub span: Span,
}

/// A chain of patterns that can potentially be fused.
#[derive(Clone, Debug)]
pub struct PatternChain {
    /// Patterns from innermost to outermost.
    pub links: Vec<ChainLink>,
    /// The original input expression (innermost .over).
    pub input: ExprId,
    /// Span covering the entire chain.
    pub span: Span,
}

/// Hints about what optimizations fusion provides.
#[derive(Clone, Debug, Default)]
pub struct FusionHints {
    /// Estimated number of intermediate allocations avoided.
    pub allocations_avoided: usize,
    /// Estimated number of iterations saved.
    pub iterations_saved: usize,
    /// Whether this fusion eliminates intermediate lists.
    pub eliminates_intermediate_lists: bool,
}

impl FusionHints {
    /// Create hints for a two-pattern fusion.
    pub fn two_pattern() -> Self {
        FusionHints {
            allocations_avoided: 1,
            iterations_saved: 1,
            eliminates_intermediate_lists: true,
        }
    }

    /// Create hints for a three-pattern fusion.
    pub fn three_pattern() -> Self {
        FusionHints {
            allocations_avoided: 2,
            iterations_saved: 2,
            eliminates_intermediate_lists: true,
        }
    }
}

#[cfg(test)]
mod tests;
