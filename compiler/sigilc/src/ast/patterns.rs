// AST pattern expression definitions for Sigil
// Contains PatternExpr and supporting enums (OnError, IterDirection)

use super::Expr;

#[derive(Debug, Clone)]
pub enum PatternExpr {
    /// fold(collection, init, op)
    Fold {
        collection: Box<Expr>,
        init: Box<Expr>,
        op: Box<Expr>,
    },

    /// map(collection, transform)
    Map {
        collection: Box<Expr>,
        transform: Box<Expr>,
    },

    /// filter(collection, predicate)
    Filter {
        collection: Box<Expr>,
        predicate: Box<Expr>,
    },

    /// collect(range, transform)
    Collect {
        range: Box<Expr>,
        transform: Box<Expr>,
    },

    /// recurse(condition, base_value, step) with optional memoization and parallelism
    /// When condition is true, returns base_value; otherwise evaluates step
    /// step can use `self(...)` for recursive calls
    Recurse {
        condition: Box<Expr>,    // Base case condition (e.g., n <= 1)
        base_value: Box<Expr>,   // Value to return when condition is true
        step: Box<Expr>,         // Recursive step using self()
        memo: bool,              // Enable memoization when true
        parallel_threshold: i64, // Parallelize when n > threshold (0 = no parallelism)
    },

    /// iterate(.over: x, .direction: dir, .into: init, .with: op)
    Iterate {
        over: Box<Expr>,
        direction: IterDirection,
        into: Box<Expr>,
        with: Box<Expr>,
    },

    /// transform(input, step1, step2, ...)
    Transform { input: Box<Expr>, steps: Vec<Expr> },

    /// count(collection, predicate)
    Count {
        collection: Box<Expr>,
        predicate: Box<Expr>,
    },

    /// parallel(.name: expr, .name2: expr2, ...) - concurrent execution
    /// Returns a struct with named fields containing results
    Parallel {
        branches: Vec<(String, Expr)>, // Named branches to execute concurrently
        timeout: Option<Box<Expr>>,    // Optional timeout duration
        on_error: OnError,             // Error handling strategy
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnError {
    FailFast,   // Cancel siblings on first error (default)
    CollectAll, // Wait for all, collect errors
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterDirection {
    Forward,
    Backward,
}
