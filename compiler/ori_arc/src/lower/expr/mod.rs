//! Expression lowering — the core dispatch for canonical IR → ARC IR.
//!
//! [`ArcLowerer`] walks the canonical expression tree and emits ARC IR
//! instructions via [`ArcIrBuilder`]. Each expression lowers to an
//! [`ArcVarId`] (the SSA variable holding the result).

use ori_ir::canon::{CanArena, CanExpr, CanId, CanonResult};
use ori_ir::{Name, Span, StringInterner};
use ori_types::Idx;
use ori_types::Pool;

use crate::ir::{ArcFunction, ArcValue, ArcVarId, LitValue, PrimOp};

use super::scope::ArcScope;
use super::{ArcIrBuilder, ArcProblem};

// Loop context

/// Context for the enclosing loop (used by `break`/`continue`).
pub(crate) struct LoopContext {
    /// Block to jump to on `break`.
    pub exit_block: crate::ir::ArcBlockId,
    /// Block to jump to on `continue`.
    pub continue_block: crate::ir::ArcBlockId,
    /// Mutable variable types for SSA merge at loop header.
    pub mutable_var_types: rustc_hash::FxHashMap<Name, Idx>,
}

// ArcLowerer

/// Expression lowerer that walks the canonical IR and emits ARC IR.
///
/// Borrows the `ArcIrBuilder` and contextual data (arena, canon result,
/// interner, pool) needed to lower each expression variant.
pub struct ArcLowerer<'a> {
    pub(crate) builder: &'a mut ArcIrBuilder,
    pub(crate) arena: &'a CanArena,
    pub(crate) canon: &'a CanonResult,
    pub(crate) interner: &'a StringInterner,
    pub(crate) pool: &'a Pool,
    pub(crate) scope: ArcScope,
    pub(crate) loop_ctx: Option<LoopContext>,
    pub(crate) problems: &'a mut Vec<ArcProblem>,
    pub(crate) lambdas: &'a mut Vec<ArcFunction>,
}

impl ArcLowerer<'_> {
    /// Get the type of a canonical expression by its ID.
    #[inline]
    pub(crate) fn expr_type(&self, id: CanId) -> Idx {
        if !id.is_valid() {
            return Idx::ERROR;
        }
        let ty = self.arena.ty(id);
        Idx::from_raw(ty.raw())
    }

    /// Emit a unit literal.
    pub(crate) fn emit_unit(&mut self) -> ArcVarId {
        self.builder
            .emit_let(Idx::UNIT, ArcValue::Literal(LitValue::Unit), None)
    }

    // Main dispatch

    /// Lower a single canonical expression, returning the `ArcVarId` of the result.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive CanExpr → ARC lowering router"
    )]
    pub(crate) fn lower_expr(&mut self, id: CanId) -> ArcVarId {
        if !id.is_valid() {
            return self.emit_unit();
        }

        let kind = *self.arena.kind(id);
        let span = self.arena.span(id);
        let ty = self.expr_type(id);

        match kind {
            // Literals
            CanExpr::Int(n) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Int(n)), Some(span))
            }
            CanExpr::Float(bits) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Float(bits)), Some(span))
            }
            CanExpr::Bool(b) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Bool(b)), Some(span))
            }
            CanExpr::Str(name) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::String(name)), Some(span))
            }
            CanExpr::Char(c) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Char(c)), Some(span))
            }
            CanExpr::Duration { value, unit } => self.builder.emit_let(
                ty,
                ArcValue::Literal(LitValue::Duration { value, unit }),
                Some(span),
            ),
            CanExpr::Size { value, unit } => self.builder.emit_let(
                ty,
                ArcValue::Literal(LitValue::Size { value, unit }),
                Some(span),
            ),
            CanExpr::Unit | CanExpr::HashLength | CanExpr::FunctionRef(_) => {
                self.builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Unit), Some(span))
            }

            // Compile-time constants
            CanExpr::Constant(const_id) => self.lower_constant(const_id, ty, span),

            // Identifiers
            CanExpr::Ident(name) | CanExpr::Const(name) | CanExpr::TypeRef(name) => {
                self.lower_ident(name, ty, span)
            }
            CanExpr::SelfRef => {
                let self_name = self.interner.intern("self");
                self.lower_ident(self_name, ty, span)
            }

            // Binary / Unary operators
            CanExpr::Binary { op, left, right } => self.lower_binary(op, left, right, ty, span),
            CanExpr::Unary { op, operand } => self.lower_unary(op, operand, ty, span),

            // Control flow
            CanExpr::Block { stmts, result } => self.lower_block(stmts, result, ty),
            CanExpr::Let {
                pattern,
                init,
                mutable,
            } => self.lower_let(pattern, init, mutable),
            CanExpr::If {
                cond,
                then_branch,
                else_branch,
            } => self.lower_if(cond, then_branch, else_branch, ty, span),
            CanExpr::Match {
                scrutinee,
                decision_tree,
                arms,
            } => self.lower_match(scrutinee, decision_tree, arms, ty, span),
            CanExpr::Loop { body, .. } => self.lower_loop(body, ty),
            CanExpr::For {
                binding,
                iter,
                guard,
                body,
                is_yield: _,
                ..
            } => self.lower_for(binding, iter, guard, body, ty),
            CanExpr::Break { value, .. } => self.lower_break(value),
            CanExpr::Continue { value, .. } => self.lower_continue(value),
            CanExpr::Assign { target, value } => self.lower_assign(target, value, span),

            // Collections & constructors
            CanExpr::Tuple(exprs) => self.lower_tuple(exprs, ty, span),
            CanExpr::List(exprs) => self.lower_list(exprs, ty, span),
            CanExpr::Map(entries) => self.lower_map(entries, ty, span),
            CanExpr::Struct { name, fields } => self.lower_struct(name, fields, ty, span),
            CanExpr::Ok(inner) => self.lower_ok(inner, ty, span),
            CanExpr::Err(inner) => self.lower_err(inner, ty, span),
            CanExpr::Some(inner) => self.lower_some(inner, ty, span),
            CanExpr::None => self.lower_none(ty, span),
            CanExpr::Field { receiver, field } => self.lower_field(receiver, field, ty, span),
            CanExpr::Index { receiver, index } => self.lower_index(receiver, index, ty, span),
            CanExpr::Range {
                start,
                end,
                step,
                inclusive,
            } => self.lower_range(start, end, step, inclusive, ty, span),
            CanExpr::Try(inner) => self.lower_try(inner, ty, span),
            CanExpr::Cast {
                expr,
                target: _,
                fallible,
            } => self.lower_cast(expr, fallible, ty, span),

            // Calls
            CanExpr::Call { func, args } => self.lower_call(func, args, ty, span),
            CanExpr::MethodCall {
                receiver,
                method,
                args,
            } => self.lower_method_call(receiver, method, args, ty, span),
            CanExpr::Lambda { params, body } => self.lower_lambda(params, body, ty, span),

            // Special forms
            CanExpr::FunctionExp { kind: _, props: _ } => {
                self.problems.push(ArcProblem::UnsupportedExpr {
                    kind: "FunctionExp",
                    span,
                });
                self.emit_unit()
            }

            // Unsupported (post-0.1-alpha)
            CanExpr::Await(_) => {
                self.problems.push(ArcProblem::UnsupportedExpr {
                    kind: "Await",
                    span,
                });
                self.emit_unit()
            }
            CanExpr::WithCapability { .. } => {
                self.problems.push(ArcProblem::UnsupportedExpr {
                    kind: "WithCapability",
                    span,
                });
                self.emit_unit()
            }

            // Formatting — FormatWith consumes expr and produces a string.
            // For ARC analysis, treat like a function call that consumes the value.
            CanExpr::FormatWith { expr, .. } => {
                let _inner = self.lower_expr(expr);
                self.emit_unit()
            }

            // Error recovery
            CanExpr::Error => self.emit_unit(),
        }
    }

    // Identifier lowering

    fn lower_ident(&mut self, name: Name, ty: Idx, span: Span) -> ArcVarId {
        if let Some(var) = self.scope.lookup(name) {
            self.builder.emit_let(ty, ArcValue::Var(var), Some(span))
        } else {
            tracing::debug!(
                name = ?name,
                "unbound identifier in ARC IR lowering"
            );
            self.builder
                .emit_let(ty, ArcValue::Literal(LitValue::Unit), Some(span))
        }
    }

    // Constant lowering

    /// Lower a compile-time constant from the `ConstantPool`.
    fn lower_constant(
        &mut self,
        const_id: ori_ir::canon::ConstantId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        use ori_ir::canon::ConstValue;
        let value = self.canon.constants.get(const_id);
        let lit = match value {
            ConstValue::Int(n) => LitValue::Int(*n),
            ConstValue::Float(bits) => LitValue::Float(*bits),
            ConstValue::Bool(b) => LitValue::Bool(*b),
            ConstValue::Str(name) => LitValue::String(*name),
            ConstValue::Char(c) => LitValue::Char(*c),
            ConstValue::Unit => LitValue::Unit,
            ConstValue::Duration { value, unit } => LitValue::Duration {
                value: *value,
                unit: *unit,
            },
            ConstValue::Size { value, unit } => LitValue::Size {
                value: *value,
                unit: *unit,
            },
        };
        self.builder
            .emit_let(ty, ArcValue::Literal(lit), Some(span))
    }

    // Binary / Unary operators

    fn lower_binary(
        &mut self,
        op: ori_ir::BinaryOp,
        left: CanId,
        right: CanId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let lhs = self.lower_expr(left);
        let rhs = self.lower_expr(right);
        self.builder.emit_let(
            ty,
            ArcValue::PrimOp {
                op: PrimOp::Binary(op),
                args: vec![lhs, rhs],
            },
            Some(span),
        )
    }

    fn lower_unary(
        &mut self,
        op: ori_ir::UnaryOp,
        operand: CanId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let arg = self.lower_expr(operand);
        self.builder.emit_let(
            ty,
            ArcValue::PrimOp {
                op: PrimOp::Unary(op),
                args: vec![arg],
            },
            Some(span),
        )
    }
}

// Tests

#[cfg(test)]
mod tests;
