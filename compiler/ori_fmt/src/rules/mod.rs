//! Breaking Rules (Layer 4)
//!
//! Ori-specific breaking rules for special constructs that don't fit
//! into simple packing strategies.
//!
//! # Architecture
//!
//! This module implements the fourth layer of the 5-layer formatter architecture.
//! Each rule is a named struct with:
//! - Documentation of the rule's semantics
//! - Decision logic (when to apply)
//! - Associated constants (thresholds, etc.)
//!
//! # Rules
//!
//! 1. **`MethodChainRule`**: All chain elements break together
//! 2. **`ShortBodyRule`**: ~20 char threshold for yield/do bodies
//! 3. **`BooleanBreakRule`**: 3+ `||` clauses break with leading `||`
//! 4. **`ChainedElseIfRule`**: Kotlin style (first `if` with assignment)
//! 5. **`NestedForRule`**: Rust-style indentation for nested `for`
//! 6. **`ParenthesesRule`**: Preserve user parens, add when needed
//! 7. **`FunctionSeq helpers`**: Query functions for try, match, generic `FunctionSeq`
//! 8. **`LoopRule`**: Complex body (try/match/for) breaks
//!
//! # Spec Reference
//!
//! - Various sections in 16-formatting.md
//! - See individual rule modules for specific line references

mod boolean_break;
mod chained_else_if;
mod loop_rule;
mod method_chain;
mod nested_for;
mod parentheses;
mod seq_helpers;
mod short_body;

pub use boolean_break::{collect_or_clauses, is_or_expression, BooleanBreakRule};
pub use chained_else_if::{collect_if_chain, ChainedElseIfRule, ElseIfBranch, IfChain};
pub use loop_rule::{get_loop_body, is_loop, is_simple_conditional_body, LoopRule};
pub use method_chain::{
    collect_method_chain, is_method_chain, ChainedCall, MethodChain, MethodChainRule,
};
pub use nested_for::{collect_for_chain, is_for_expression, ForChain, ForLevel, NestedForRule};
pub use parentheses::{is_simple_expr, needs_parens, ParenPosition, ParenthesesRule};
pub use seq_helpers::{get_function_seq, is_function_seq, is_match_seq, is_try};
pub use short_body::{
    is_always_short, is_short_body, suggest_break_point, BreakPoint, ShortBodyRule,
};

#[cfg(test)]
mod tests;
