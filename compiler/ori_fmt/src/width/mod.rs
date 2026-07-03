//! Width Calculation for AST Nodes
//!
//! Bottom-up traversal calculating inline width of each AST node.
//! Widths are cached for performance.
//!
//! # Width Formulas
//!
//! | Construct | Width Formula |
//! |-----------|---------------|
//! | Identifier | `name.len()` |
//! | Integer literal | `text.len()` |
//! | String literal | `text.len() + 2` (quotes) |
//! | Binary expr | `left + 3 + right` (space-op-space) |
//! | Function call | `name + 1 + args_width + separators + 1` |
//! | Named argument | `name + 2 + value` (`: `) |
//! | Struct literal | `name + 3 + fields_width + separators + 2` (` { ` + ` }`) |
//! | List | `2 + items_width + separators` (`[` + `]`) |
//! | Map | `2 + entries_width + separators` (`{` + `}`) |
//!
//! # Always-Stacked Constructs
//!
//! Some constructs always use stacked format regardless of width:
//! - `try` (sequential blocks)
//! - `match` arms
//! - `recurse`, `parallel`, `spawn`, `nursery`

mod calls;
mod collections;
mod compounds;
mod control;
mod element_widths;
pub(crate) mod lambda;
mod literals;
mod metrics;
mod operators;
mod patterns;
mod wrappers;

#[cfg(test)]
mod tests;

use crate::rules::{needs_parens, ParenPosition};
use calls::{call_named_width, call_width, method_call_named_width, method_call_width};
use collections::{
    list_width, list_with_spread_width, map_width, map_with_spread_width, range_width,
    struct_width, struct_with_spread_width, tuple_width,
};
use compounds::{duration_width, size_width};
use control::{
    assign_target_width, assign_width, block_width, break_width, continue_width, field_width,
    for_width, if_width, index_width, while_width, with_capability_width,
};
use literals::{bool_width, char_width, float_width, int_width, string_width};
use operators::{binary_op_width, unary_op_width};
use ori_ir::{ExprArena, ExprId, ExprKind, FunctionExpKind, FunctionSeq, StringLookup, UnaryOp};
use patterns::{binding_pattern_width, for_binding_pattern_width};
use rustc_hash::{FxBuildHasher, FxHashMap};
use wrappers::{
    await_width, cast_width, err_width, loop_width, ok_width, some_width, try_width, unsafe_width,
};

/// Sentinel value indicating a construct that always uses stacked format.
///
/// When width calculation returns this value, the formatter should skip
/// the inline attempt and go directly to broken/stacked rendering.
pub const ALWAYS_STACKED: usize = usize::MAX;

/// Calculator for inline widths of AST nodes.
///
/// Performs bottom-up traversal to compute how wide each expression
/// would be if rendered on a single line. Results are cached for efficiency.
pub struct WidthCalculator<'a, I: StringLookup> {
    pub(super) arena: &'a ExprArena,
    pub(super) interner: &'a I,
    cache: FxHashMap<ExprId, usize>,
}

impl<'a, I: StringLookup> WidthCalculator<'a, I> {
    /// Create a new width calculator.
    pub fn new(arena: &'a ExprArena, interner: &'a I) -> Self {
        Self {
            arena,
            interner,
            cache: FxHashMap::default(),
        }
    }

    /// Create with pre-allocated cache capacity.
    pub fn with_capacity(arena: &'a ExprArena, interner: &'a I, capacity: usize) -> Self {
        Self {
            arena,
            interner,
            cache: FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher),
        }
    }

    /// Calculate the inline width of an expression.
    ///
    /// Returns `ALWAYS_STACKED` for constructs that should never be inline.
    pub fn width(&mut self, expr_id: ExprId) -> usize {
        if let Some(&cached) = self.cache.get(&expr_id) {
            return cached;
        }

        let width = self.calculate_width(expr_id);
        self.cache.insert(expr_id, width);
        width
    }

    /// Check if a cached width exists for an expression.
    pub fn is_cached(&self, expr_id: ExprId) -> bool {
        self.cache.contains_key(&expr_id)
    }

    /// Get the number of cached widths.
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Clear the width cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Calculate width without caching (internal).
    ///
    /// **Invariant:** This match is exhaustive with no wildcard `_ =>` arm.
    /// Every `ExprKind` variant is listed explicitly so that adding a new
    /// variant causes a compile error here, in `emit_broken()`, and in
    /// `emit_inline()`. The `calculate_width_dispatch_has_no_wildcard` test
    /// enforces this at the source level.
    #[expect(
        clippy::match_same_arms,
        reason = "Separate arms document each variant's width calculation for maintainability"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive ExprKind width calculation dispatch"
    )]
    fn calculate_width(&mut self, expr_id: ExprId) -> usize {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            // Literals - delegated to literals module
            ExprKind::Int(n) => int_width(*n),
            ExprKind::Float(bits) => float_width(f64::from_bits(*bits)),
            ExprKind::Bool(b) => bool_width(*b),
            ExprKind::String(name) => string_width(self.interner.lookup(*name)),
            ExprKind::Char(c) => char_width(*c),
            ExprKind::Duration { value, unit } => duration_width(*value, *unit),
            ExprKind::Size { value, unit } => size_width(*value, *unit),
            ExprKind::Unit => 2, // "()"

            // Identifiers - simple inline calculations
            ExprKind::Ident(name) => self.interner.lookup(*name).len(),
            ExprKind::Const(name) => self.interner.lookup(*name).len() + 1, // "$name"
            ExprKind::SelfRef => 4,                                         // "self"
            ExprKind::FunctionRef(name) => self.interner.lookup(*name).len() + 1, // "@name"
            ExprKind::HashLength => 1,                                      // "#"

            // Binary/unary operations - delegated to operators module
            ExprKind::Binary { op, left, right } => {
                let left_w = self.width(*left);
                let right_w = self.width(*right);
                if left_w == ALWAYS_STACKED || right_w == ALWAYS_STACKED {
                    return ALWAYS_STACKED;
                }
                left_w + binary_op_width(*op) + right_w
            }
            ExprKind::Unary { op, operand } => {
                let operand_w = self.width(*operand);
                if operand_w == ALWAYS_STACKED {
                    return ALWAYS_STACKED;
                }
                let position = if *op == UnaryOp::Neg {
                    ParenPosition::UnaryNegOperand
                } else {
                    ParenPosition::UnaryOperand
                };
                let parens_w = if needs_parens(self.arena, *operand, position) {
                    2 // "(" + ")"
                } else {
                    0
                };
                unary_op_width(*op) + parens_w + operand_w
            }

            // Calls - delegated to calls module
            ExprKind::Call { func, args } => call_width(self, *func, *args),
            ExprKind::CallNamed { func, args } => call_named_width(self, *func, *args),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => method_call_width(self, *receiver, *method, *args),
            ExprKind::MethodCallNamed {
                receiver,
                method,
                args,
            } => method_call_named_width(self, *receiver, *method, *args),

            // Access - delegated to control module
            ExprKind::Field { receiver, field } => field_width(self, *receiver, *field),
            ExprKind::Index { receiver, index } => index_width(self, *receiver, *index),

            // Control flow - delegated to control module
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                // Spec: Annex D §If-Then-Else — chained `else if` always breaks.
                if crate::rules::ChainedElseIfRule::has_else_if_chain(self.arena, expr_id) {
                    ALWAYS_STACKED
                } else {
                    if_width(self, *cond, *then_branch, *else_branch)
                }
            }
            ExprKind::Match { .. } => ALWAYS_STACKED,
            ExprKind::For {
                label,
                pattern,
                iter,
                guard,
                body,
                is_yield,
            } => for_width(self, *label, *pattern, *iter, *guard, *body, *is_yield),
            ExprKind::Loop { label, body } => loop_width(self, *label, *body),
            ExprKind::While { label, cond, body } => while_width(self, *label, *cond, *body),
            ExprKind::Block { stmts, result } => block_width(self, *stmts, *result),

            // Let binding - complex, kept inline
            ExprKind::Let {
                pattern,
                ty,
                init,
                mutable,
            } => {
                let init_w = self.width(*init);
                if init_w == ALWAYS_STACKED {
                    return ALWAYS_STACKED;
                }

                // "let " (4 chars, mutable default) or "let $" (5 chars, immutable)
                let mut total = if mutable.is_mutable() { 4 } else { 5 };
                let pat = self.arena.get_binding_pattern(*pattern);
                total += binding_pattern_width(pat, self.interner);
                if ty.is_valid() {
                    // ": " + recursive type measurement (NOT magic constant);
                    // mirrors what the inline + broken Let render emits.
                    total += 2 + lambda::type_width(self, self.arena, *ty);
                }
                total += 3 + init_w; // " = " + init

                total
            }

            // Lambda - complex, kept inline. Delegates to the canonical
            // SSOT in `width::lambda` so width measurement and render
            // (`formatter::{inline,broken}`) cannot drift on the parens
            // decision, the param-type measurement, or the ret_ty
            // ceremony width.
            ExprKind::Lambda {
                params,
                ret_ty,
                body,
            } => {
                let body_w = self.width(*body);
                if body_w == ALWAYS_STACKED {
                    return ALWAYS_STACKED;
                }
                let params_list = self.arena.get_params(*params);
                lambda::lambda_emit_width(self, self.arena, params_list, *ret_ty, body_w)
            }

            // Collections - delegated to collections module
            ExprKind::List(items) => list_width(self, *items),
            ExprKind::ListWithSpread(elements) => list_with_spread_width(self, *elements),
            ExprKind::Map(entries) => map_width(self, *entries),
            ExprKind::MapWithSpread(elements) => map_with_spread_width(self, *elements),
            ExprKind::Struct { name, fields } => struct_width(self, *name, *fields),
            ExprKind::StructWithSpread { name, fields } => {
                struct_with_spread_width(self, *name, *fields)
            }
            ExprKind::Tuple(items) => tuple_width(self, *items),
            ExprKind::Range {
                start,
                end,
                step,
                inclusive,
            } => range_width(self, *start, *end, *step, *inclusive),

            // Result/Option wrappers - delegated to wrappers module
            ExprKind::Ok(inner) => ok_width(self, *inner),
            ExprKind::Err(inner) => err_width(self, *inner),
            ExprKind::Some(inner) => some_width(self, *inner),
            ExprKind::None => 4, // "None"

            // Control flow jumps - delegated to control module
            ExprKind::Break { label, value } => break_width(self, *label, *value),
            ExprKind::Continue { label, value } => continue_width(self, *label, *value),

            // Unsafe block and postfix operators
            ExprKind::Unsafe(inner) => unsafe_width(self, *inner),
            ExprKind::Await(inner) => await_width(self, *inner),
            ExprKind::Try(inner) => try_width(self, *inner),
            ExprKind::Cast { expr, ty, fallible } => {
                cast_width(self, *expr, self.arena.get_parsed_type(*ty), *fallible)
            }

            // Assignment and capability - delegated to control module
            ExprKind::Assign { target, value } => assign_width(self, *target, *value),
            ExprKind::AssignTarget { root, steps } => assign_target_width(self, *root, *steps),
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => with_capability_width(self, *capability, *provider, *body),

            // Sequential patterns - always stacked
            ExprKind::FunctionSeq(seq_id) => {
                let seq = self.arena.get_function_seq(*seq_id);
                match seq {
                    FunctionSeq::Try { .. }
                    | FunctionSeq::Match { .. }
                    | FunctionSeq::ForPattern { .. } => ALWAYS_STACKED,
                }
            }

            // Named expression patterns
            ExprKind::FunctionExp(exp_id) => {
                let exp = self.arena.get_function_exp(*exp_id);
                match exp.kind {
                    FunctionExpKind::Recurse
                    | FunctionExpKind::Parallel
                    | FunctionExpKind::Spawn
                    | FunctionExpKind::Catch => ALWAYS_STACKED,

                    FunctionExpKind::Timeout
                    | FunctionExpKind::Cache
                    | FunctionExpKind::With
                    | FunctionExpKind::Print
                    | FunctionExpKind::Panic
                    | FunctionExpKind::Todo
                    | FunctionExpKind::Unreachable
                    | FunctionExpKind::Channel
                    | FunctionExpKind::ChannelIn
                    | FunctionExpKind::ChannelOut
                    | FunctionExpKind::ChannelAll => {
                        let props = self.arena.get_named_exprs(exp.props);
                        let props_w = self.width_of_named_exprs(props);
                        if props_w == ALWAYS_STACKED {
                            return ALWAYS_STACKED;
                        }
                        exp.kind.name().len() + 1 + props_w + 1
                    }
                }
            }

            // Template literals — width is the re-escaped rendered length
            // (matches emit in formatter/inline/mod.rs), plus two backticks.
            ExprKind::TemplateFull(name) => {
                crate::escape_template_text(self.interner.lookup(*name)).len() + 2
            }
            ExprKind::TemplateLiteral { .. } => ALWAYS_STACKED, // conservative: contains expressions

            // Parse error - always stack
            ExprKind::Error => ALWAYS_STACKED,
        }
    }
}
