//! Inline width calculation for AST expressions.
//!
//! [`WidthCalculator`] caches bottom-up measurements. Expressions that require
//! stacked output return [`ALWAYS_STACKED`]. Width helpers mirror formatter
//! token sequences so measurement and emission stay aligned.

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
use ori_ir::{
    BinaryOp, BindingPatternId, ExprArena, ExprId, ExprKind, FunctionExpId, FunctionExpKind,
    FunctionSeq, FunctionSeqId, Mutability, Name, ParamRange, ParsedTypeId, StringLookup, UnaryOp,
};
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
#[derive(Debug)]
pub struct WidthCalculator<'a, I: StringLookup> {
    pub(super) arena: &'a ExprArena,
    pub(super) interner: &'a I,
    cache: FxHashMap<ExprId, usize>,
}

impl<'a, I: StringLookup> WidthCalculator<'a, I> {
    /// Creates a calculator with an empty cache.
    pub fn new(arena: &'a ExprArena, interner: &'a I) -> Self {
        Self {
            arena,
            interner,
            cache: FxHashMap::default(),
        }
    }

    /// Creates a calculator with space for `capacity` cached widths.
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

    /// Returns whether `expr_id` already has a cached width.
    pub fn is_cached(&self, expr_id: ExprId) -> bool {
        self.cache.contains_key(&expr_id)
    }

    /// Returns the number of cached expression widths.
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Removes all cached expression widths.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Measures one expression without consulting or updating the cache.
    ///
    /// The exhaustive match keeps additions to [`ExprKind`] compiler-checked.
    fn calculate_width(&mut self, expr_id: ExprId) -> usize {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            ExprKind::Int(n) => int_width(*n),
            ExprKind::Float(bits) => float_width(f64::from_bits(*bits)),
            ExprKind::Bool(b) => bool_width(*b),
            ExprKind::String(name) => string_width(self.interner.lookup(*name)),
            ExprKind::Char(c) => char_width(*c),
            ExprKind::Duration { value, unit } => duration_width(*value, *unit),
            ExprKind::Size { value, unit } => size_width(*value, *unit),
            ExprKind::Unit => "()".len(),
            ExprKind::Ident(name) => self.interner.lookup(*name).len(),
            ExprKind::Const(name) | ExprKind::FunctionRef(name) => {
                self.interner.lookup(*name).len() + 1
            }
            ExprKind::SelfRef => "self".len(),
            ExprKind::HashLength => "#".len(),
            ExprKind::Binary { op, left, right } => self.binary_expr_width(*op, *left, *right),
            ExprKind::Unary { op, operand } => self.unary_expr_width(*op, *operand),
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

            ExprKind::Field { receiver, field } => field_width(self, *receiver, *field),
            ExprKind::Index { receiver, index } => index_width(self, *receiver, *index),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.if_expr_width(expr_id, *cond, *then_branch, *else_branch),
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
            ExprKind::Let {
                pattern,
                ty,
                init,
                mutable,
            } => self.let_expr_width(*pattern, *ty, *init, *mutable),
            ExprKind::Lambda {
                params,
                ret_ty,
                body,
            } => self.lambda_expr_width(*params, *ret_ty, *body),
            ExprKind::List(items) => list_width(self, *items),
            ExprKind::ListWithSpread(elements) => list_with_spread_width(self, *elements),
            ExprKind::Map(entries) => map_width(self, *entries),
            ExprKind::MapWithSpread(elements) => map_with_spread_width(self, *elements),
            ExprKind::Struct { type_path, fields } => struct_width(self, *type_path, *fields),
            ExprKind::StructWithSpread { type_path, fields } => {
                struct_with_spread_width(self, *type_path, *fields)
            }
            ExprKind::Tuple(items) => tuple_width(self, *items),
            ExprKind::Range {
                start,
                end,
                step,
                inclusive,
            } => range_width(self, *start, *end, *step, *inclusive),

            ExprKind::Ok(inner) => ok_width(self, *inner),
            ExprKind::Err(inner) => err_width(self, *inner),
            ExprKind::Some(inner) => some_width(self, *inner),
            ExprKind::None => "None".len(),
            ExprKind::Break { label, value } => break_width(self, *label, *value),
            ExprKind::Continue { label, value } => continue_width(self, *label, *value),
            ExprKind::Unsafe(inner) => unsafe_width(self, *inner),
            ExprKind::Await(inner) => await_width(self, *inner),
            ExprKind::Try(inner) => try_width(self, *inner),
            ExprKind::Cast { expr, ty, fallible } => {
                cast_width(self, *expr, self.arena.get_parsed_type(*ty), *fallible)
            }

            ExprKind::Assign { target, value } => assign_width(self, *target, *value),
            ExprKind::AssignTarget { root, steps } => assign_target_width(self, *root, *steps),
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => with_capability_width(self, *capability, *provider, *body),

            ExprKind::FunctionSeq(seq_id) => self.function_seq_width(*seq_id),
            ExprKind::FunctionExp(exp_id) => self.function_exp_width(*exp_id),
            ExprKind::TemplateFull(name) => self.template_full_width(*name),
            ExprKind::Match { .. } | ExprKind::TemplateLiteral { .. } | ExprKind::Error => {
                ALWAYS_STACKED
            }
        }
    }

    fn binary_expr_width(&mut self, op: BinaryOp, left: ExprId, right: ExprId) -> usize {
        let left_width = self.width(left);
        let right_width = self.width(right);
        if left_width == ALWAYS_STACKED || right_width == ALWAYS_STACKED {
            ALWAYS_STACKED
        } else {
            left_width + binary_op_width(op) + right_width
        }
    }

    fn unary_expr_width(&mut self, op: UnaryOp, operand: ExprId) -> usize {
        let operand_width = self.width(operand);
        if operand_width == ALWAYS_STACKED {
            return ALWAYS_STACKED;
        }
        let parens_width = usize::from(needs_parens(
            self.arena,
            operand,
            ParenPosition::UnaryOperand,
        )) * 2;
        unary_op_width(op) + parens_width + operand_width
    }

    fn if_expr_width(
        &mut self,
        expr_id: ExprId,
        cond: ExprId,
        then_branch: ExprId,
        else_branch: ExprId,
    ) -> usize {
        if crate::rules::ChainedElseIfRule::has_else_if_chain(self.arena, expr_id) {
            ALWAYS_STACKED
        } else {
            if_width(self, cond, then_branch, else_branch)
        }
    }

    fn let_expr_width(
        &mut self,
        pattern: BindingPatternId,
        ty: ParsedTypeId,
        init: ExprId,
        mutable: Mutability,
    ) -> usize {
        let init_width = self.width(init);
        if init_width == ALWAYS_STACKED {
            return ALWAYS_STACKED;
        }

        let mut total = if mutable.is_mutable() {
            "let ".len()
        } else {
            "let $".len()
        };
        total += binding_pattern_width(self.arena.get_binding_pattern(pattern), self.interner);
        if ty.is_valid() {
            total += ": ".len() + lambda::type_width(self, self.arena, ty);
        }
        total + " = ".len() + init_width
    }

    fn lambda_expr_width(
        &mut self,
        params: ParamRange,
        ret_ty: ParsedTypeId,
        body: ExprId,
    ) -> usize {
        let body_width = self.width(body);
        if body_width == ALWAYS_STACKED {
            return ALWAYS_STACKED;
        }
        lambda::lambda_emit_width(
            self,
            self.arena,
            self.arena.get_params(params),
            ret_ty,
            body_width,
        )
    }

    fn function_seq_width(&self, seq_id: FunctionSeqId) -> usize {
        match self.arena.get_function_seq(seq_id) {
            FunctionSeq::Try { .. }
            | FunctionSeq::Match { .. }
            | FunctionSeq::ForPattern { .. } => ALWAYS_STACKED,
        }
    }

    fn function_exp_width(&mut self, exp_id: FunctionExpId) -> usize {
        let exp = self.arena.get_function_exp(exp_id);
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
                let props_width = self.width_of_named_exprs(self.arena.get_named_exprs(exp.props));
                if props_width == ALWAYS_STACKED {
                    ALWAYS_STACKED
                } else {
                    exp.kind.name().len() + "(".len() + props_width + ")".len()
                }
            }
        }
    }

    fn template_full_width(&self, name: Name) -> usize {
        crate::escape_template_text(self.interner.lookup(name)).len() + "``".len()
    }
}
