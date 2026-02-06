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
//! - `run`, `try` (sequential blocks)
//! - `match` arms
//! - `recurse`, `parallel`, `spawn`, `nursery`

mod calls;
mod collections;
mod compounds;
mod control;
mod helpers;
mod literals;
mod operators;
mod patterns;
mod wrappers;

#[cfg(test)]
mod tests;

use calls::{call_named_width, call_width, method_call_named_width, method_call_width};
use collections::{
    list_width, list_with_spread_width, map_width, map_with_spread_width, range_width,
    struct_width, struct_with_spread_width, tuple_width,
};
use compounds::{duration_width, size_width};
use control::{
    assign_width, block_width, break_width, continue_width, field_width, for_width, if_width,
    index_width, with_capability_width,
};
use helpers::{accumulate_widths, COMMA_SEPARATOR_WIDTH};
use literals::{bool_width, char_width, float_width, int_width, string_width};
use operators::{binary_op_width, unary_op_width};
use ori_ir::{ExprArena, ExprId, ExprKind, FunctionExpKind, FunctionSeq, StringLookup};
use patterns::binding_pattern_width;
use rustc_hash::{FxBuildHasher, FxHashMap};
use wrappers::{await_width, cast_width, err_width, loop_width, ok_width, some_width, try_width};

/// Sentinel value indicating a construct that always uses stacked format.
///
/// When width calculation returns this value, the formatter should skip
/// the inline attempt and go directly to broken/stacked rendering.
pub const ALWAYS_STACKED: usize = usize::MAX;

/// Placeholder width estimate for type annotations.
///
/// Used when the actual type width is not yet computable.
const TYPE_ANNOTATION_WIDTH_ESTIMATE: usize = 10;

/// Calculator for inline widths of AST nodes.
///
/// Performs bottom-up traversal to compute how wide each expression
/// would be if rendered on a single line. Results are cached for efficiency.
pub struct WidthCalculator<'a, I: StringLookup> {
    arena: &'a ExprArena,
    interner: &'a I,
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
    #[expect(
        clippy::match_same_arms,
        reason = "Separate arms document each variant's width calculation for maintainability"
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
                unary_op_width(*op) + operand_w
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
            } => if_width(self, *cond, *then_branch, *else_branch),
            ExprKind::Match { .. } => ALWAYS_STACKED,
            ExprKind::For {
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => for_width(self, *binding, *iter, *guard, *body, *is_yield),
            ExprKind::Loop { body } => loop_width(self, *body),
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

                // "let " or "let mut "
                let mut total = if *mutable { 8 } else { 4 };
                let pat = self.arena.get_binding_pattern(*pattern);
                total += binding_pattern_width(pat, self.interner);
                if ty.is_valid() {
                    total += TYPE_ANNOTATION_WIDTH_ESTIMATE;
                }
                total += 3 + init_w; // " = " + init

                total
            }

            // Lambda - complex, kept inline
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
                let params_w = self.width_of_params(params_list);

                let mut total = if params_list.len() == 1 && !ret_ty.is_valid() {
                    params_w // Single param without parens
                } else {
                    1 + params_w + 1 // (params)
                };

                total += 4 + body_w; // " -> " + body
                if ret_ty.is_valid() {
                    total += TYPE_ANNOTATION_WIDTH_ESTIMATE;
                }

                total
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
            ExprKind::Break(val) => break_width(self, *val),
            ExprKind::Continue(val) => continue_width(self, *val),

            // Postfix operators - delegated to wrappers module
            ExprKind::Await(inner) => await_width(self, *inner),
            ExprKind::Try(inner) => try_width(self, *inner),
            ExprKind::Cast { expr, ty, fallible } => {
                cast_width(self, *expr, self.arena.get_parsed_type(*ty), *fallible)
            }

            // Assignment and capability - delegated to control module
            ExprKind::Assign { target, value } => assign_width(self, *target, *value),
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => with_capability_width(self, *capability, *provider, *body),

            // Sequential patterns - always stacked
            ExprKind::FunctionSeq(seq_id) => {
                let seq = self.arena.get_function_seq(*seq_id);
                match seq {
                    FunctionSeq::Run { .. }
                    | FunctionSeq::Try { .. }
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
                    | FunctionExpKind::Unreachable => {
                        let props = self.arena.get_named_exprs(exp.props);
                        let props_w = self.width_of_named_exprs(props);
                        if props_w == ALWAYS_STACKED {
                            return ALWAYS_STACKED;
                        }
                        exp.kind.name().len() + 1 + props_w + 1
                    }
                }
            }

            // Parse error - always stack
            ExprKind::Error => ALWAYS_STACKED,
        }
    }

    /// Calculate width of an expression list (comma-separated).
    fn width_of_expr_list(&mut self, exprs: &[ExprId]) -> usize {
        accumulate_widths(exprs, |id| self.width(*id), COMMA_SEPARATOR_WIDTH)
    }

    /// Calculate width of call arguments (name: value, ...).
    fn width_of_call_args(&mut self, args: &[ori_ir::CallArg]) -> usize {
        if args.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, arg) in args.iter().enumerate() {
            let value_w = self.width(arg.value);
            if value_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;
            }

            if let Some(name) = arg.name {
                total += self.interner.lookup(name).len() + 2 + value_w;
            } else {
                total += value_w;
            }

            if i < args.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of map entries (key: value, ...).
    fn width_of_map_entries(&mut self, entries: &[ori_ir::MapEntry]) -> usize {
        if entries.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, entry) in entries.iter().enumerate() {
            let key_w = self.width(entry.key);
            let value_w = self.width(entry.value);
            if key_w == ALWAYS_STACKED || value_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;
            }
            total += key_w + 2 + value_w;

            if i < entries.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of field initializers (name: value, ...).
    fn width_of_field_inits(&mut self, fields: &[ori_ir::FieldInit]) -> usize {
        if fields.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, field) in fields.iter().enumerate() {
            let name_w = self.interner.lookup(field.name).len();

            if let Some(value) = field.value {
                let value_w = self.width(value);
                if value_w == ALWAYS_STACKED {
                    return ALWAYS_STACKED;
                }
                total += name_w + 2 + value_w;
            } else {
                total += name_w;
            }

            if i < fields.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of struct literal fields (including spreads).
    fn width_of_struct_lit_fields(&mut self, fields: &[ori_ir::StructLitField]) -> usize {
        if fields.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, field) in fields.iter().enumerate() {
            match field {
                ori_ir::StructLitField::Field(init) => {
                    let name_w = self.interner.lookup(init.name).len();
                    if let Some(value) = init.value {
                        let value_w = self.width(value);
                        if value_w == ALWAYS_STACKED {
                            return ALWAYS_STACKED;
                        }
                        total += name_w + 2 + value_w;
                    } else {
                        total += name_w;
                    }
                }
                ori_ir::StructLitField::Spread { expr, .. } => {
                    let expr_w = self.width(*expr);
                    if expr_w == ALWAYS_STACKED {
                        return ALWAYS_STACKED;
                    }
                    // "..." + expr
                    total += 3 + expr_w;
                }
            }

            if i < fields.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of list elements (including spreads).
    fn width_of_list_elements(&mut self, elements: &[ori_ir::ListElement]) -> usize {
        if elements.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, element) in elements.iter().enumerate() {
            match element {
                ori_ir::ListElement::Expr { expr, .. } => {
                    let expr_w = self.width(*expr);
                    if expr_w == ALWAYS_STACKED {
                        return ALWAYS_STACKED;
                    }
                    total += expr_w;
                }
                ori_ir::ListElement::Spread { expr, .. } => {
                    let expr_w = self.width(*expr);
                    if expr_w == ALWAYS_STACKED {
                        return ALWAYS_STACKED;
                    }
                    // "..." + expr
                    total += 3 + expr_w;
                }
            }

            if i < elements.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of map elements (including spreads).
    fn width_of_map_elements(&mut self, elements: &[ori_ir::MapElement]) -> usize {
        if elements.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, element) in elements.iter().enumerate() {
            match element {
                ori_ir::MapElement::Entry(entry) => {
                    let key_w = self.width(entry.key);
                    let value_w = self.width(entry.value);
                    if key_w == ALWAYS_STACKED || value_w == ALWAYS_STACKED {
                        return ALWAYS_STACKED;
                    }
                    total += key_w + 2 + value_w; // key: value
                }
                ori_ir::MapElement::Spread { expr, .. } => {
                    let expr_w = self.width(*expr);
                    if expr_w == ALWAYS_STACKED {
                        return ALWAYS_STACKED;
                    }
                    // "..." + expr
                    total += 3 + expr_w;
                }
            }

            if i < elements.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of named expressions (name: value, ...).
    fn width_of_named_exprs(&mut self, exprs: &[ori_ir::NamedExpr]) -> usize {
        if exprs.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, expr) in exprs.iter().enumerate() {
            let name_w = self.interner.lookup(expr.name).len();
            let value_w = self.width(expr.value);
            if value_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;
            }
            total += name_w + 2 + value_w;

            if i < exprs.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }

    /// Calculate width of function parameters.
    fn width_of_params(&self, params: &[ori_ir::Param]) -> usize {
        if params.is_empty() {
            return 0;
        }

        let mut total = 0;
        for (i, param) in params.iter().enumerate() {
            let name_w = self.interner.lookup(param.name).len();
            total += name_w + 2 + 5; // "name: Type" estimate

            if i < params.len() - 1 {
                total += COMMA_SEPARATOR_WIDTH;
            }
        }
        total
    }
}
