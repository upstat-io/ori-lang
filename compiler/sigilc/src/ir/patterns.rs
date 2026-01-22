// Typed pattern expressions for Sigil TIR
// High-level patterns kept for optimization opportunities before lowering

use super::expr::TExpr;
use super::types::Type;

/// Error handling strategy for parallel execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnError {
    /// Cancel sibling branches on first error (default)
    FailFast,
    /// Wait for all branches, collect errors
    CollectAll,
}

/// Iteration direction for iterate pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterDirection {
    Forward,
    Backward,
}

/// Typed pattern expressions
/// These are kept high-level for pattern-specific optimizations
/// They get lowered to loops/calls by the PatternLoweringPass
#[derive(Debug, Clone)]
pub enum TPattern {
    /// fold(collection, init, op) -> accumulator
    /// Element-wise reduction with accumulator
    Fold {
        collection: TExpr,
        elem_ty: Type,
        init: TExpr,
        op: TExpr,       // Lambda: (acc, elem) -> acc
        result_ty: Type, // Same as init type
    },

    /// map(collection, transform) -> [result]
    /// Transform each element
    Map {
        collection: TExpr,
        elem_ty: Type,
        transform: TExpr, // Lambda: elem -> result
        result_elem_ty: Type,
    },

    /// filter(collection, predicate) -> [elem]
    /// Select elements matching predicate
    Filter {
        collection: TExpr,
        elem_ty: Type,
        predicate: TExpr, // Lambda: elem -> bool
    },

    /// collect(range, transform) -> [result]
    /// Build list from range
    Collect {
        range: TExpr,
        transform: TExpr, // Lambda: int -> result
        result_elem_ty: Type,
    },

    /// recurse(cond, base, step) -> result
    /// Recursive function with optional memoization
    Recurse {
        cond: TExpr,             // Base case condition
        base: TExpr,             // Base case value
        step: TExpr,             // Recursive step using self()
        result_ty: Type,         // Return type
        memo: bool,              // Enable memoization
        parallel_threshold: i64, // Parallelize when n > threshold (0 = no parallelism)
    },

    /// iterate(.over: x, .direction: dir, .into: init, .with: op)
    /// Directional iteration with accumulator
    Iterate {
        over: TExpr,
        elem_ty: Type,
        direction: IterDirection,
        into: TExpr,
        with: TExpr,
        result_ty: Type,
    },

    /// transform(input, step1, step2, ...) -> result
    /// Pipeline of transformations
    Transform {
        input: TExpr,
        steps: Vec<TExpr>,
        result_ty: Type,
    },

    /// count(collection, predicate) -> int
    /// Count elements matching predicate
    Count {
        collection: TExpr,
        elem_ty: Type,
        predicate: TExpr, // Lambda: elem -> bool
    },

    /// parallel(.name: expr, ...) -> { name: result, ... }
    /// Concurrent execution of branches
    Parallel {
        branches: Vec<(String, TExpr, Type)>, // Named branches with their types
        timeout: Option<TExpr>,               // Optional timeout duration
        on_error: OnError,                    // Error handling strategy
        result_ty: Type,                      // Record type of results
    },

    /// find(.in: collection, .where: predicate) -> Option<elem> or elem
    /// Find first element matching predicate
    Find {
        collection: TExpr,
        elem_ty: Type,
        predicate: TExpr,       // Lambda: elem -> bool
        default: Option<TExpr>, // If provided, returns elem_ty instead of Option
        result_ty: Type,        // Option<elem_ty> or elem_ty
    },

    /// try(.body: expr) -> Result<T, Error> or T
    /// Wrap expression in error handling
    Try {
        body: TExpr,
        catch: Option<TExpr>, // Optional error handler: (err) -> T
        result_ty: Type,      // Result<T, Error> or T (with catch)
    },

    /// retry(.op: expr, .times: N, .backoff: strategy, .delay: ms)
    /// Retry operation with backoff
    Retry {
        operation: TExpr,
        max_attempts: TExpr,
        backoff: RetryBackoff,
        delay_ms: Option<TExpr>,
        result_ty: Type, // Result<T, Error>
    },

    /// validate(.rules: [...], .then: value)
    /// Validate with error accumulation
    Validate {
        rules: Vec<(TExpr, TExpr)>, // List of (condition, error_message) pairs
        then_value: TExpr,          // Value to return if all pass
        result_ty: Type,            // Result<T, [str]>
    },
}

/// Backoff strategy for retry pattern (mirrors AST)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryBackoff {
    None,
    Constant,
    Linear,
    Exponential,
}

impl TPattern {
    /// Get the result type of this pattern
    pub fn result_type(&self) -> &Type {
        match self {
            TPattern::Fold { result_ty, .. } => result_ty,
            TPattern::Map { result_elem_ty, .. } => result_elem_ty,
            TPattern::Filter { elem_ty, .. } => elem_ty,
            TPattern::Collect { result_elem_ty, .. } => result_elem_ty,
            TPattern::Recurse { result_ty, .. } => result_ty,
            TPattern::Iterate { result_ty, .. } => result_ty,
            TPattern::Transform { result_ty, .. } => result_ty,
            TPattern::Count { .. } => &Type::Int,
            TPattern::Parallel { result_ty, .. } => result_ty,
            TPattern::Find { result_ty, .. } => result_ty,
            TPattern::Try { result_ty, .. } => result_ty,
            TPattern::Retry { result_ty, .. } => result_ty,
            TPattern::Validate { result_ty, .. } => result_ty,
        }
    }

    /// Get the collection being iterated (if any)
    pub fn collection(&self) -> Option<&TExpr> {
        match self {
            TPattern::Fold { collection, .. } => Some(collection),
            TPattern::Map { collection, .. } => Some(collection),
            TPattern::Filter { collection, .. } => Some(collection),
            TPattern::Count { collection, .. } => Some(collection),
            TPattern::Iterate { over, .. } => Some(over),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_on_error_default() {
        let err = OnError::FailFast;
        assert_eq!(err, OnError::FailFast);
    }

    #[test]
    fn test_iter_direction() {
        let dir = IterDirection::Forward;
        assert_eq!(dir, IterDirection::Forward);
    }
}
