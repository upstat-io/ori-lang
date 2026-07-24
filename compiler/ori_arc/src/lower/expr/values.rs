//! Identifier, constant, and primitive-operator lowering.

use ori_ir::canon::{CanId, GenericConstValue};
use ori_ir::{Name, Span};
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId, CtorKind, LitValue, PrimOp};
use crate::operator_calls::operator_call_plan;

use super::super::ArcProblem;
use super::ArcLowerer;

impl ArcLowerer<'_> {
    pub(super) fn lower_ident(&mut self, name: Name, ty: Idx, span: Span) -> ArcVarId {
        if let Some(var) = self.scope.lookup(name) {
            self.builder.emit_let(ty, ArcValue::Var(var), Some(span))
        } else if let Some(literal) = self.const_binding_literal(name) {
            self.builder
                .emit_let(ty, ArcValue::Literal(literal), Some(span))
        } else if let Some(&(enum_name, variant_idx, field_count)) = self.variant_ctors.get(&name) {
            if field_count == 0 {
                self.builder.emit_construct(
                    ty,
                    CtorKind::EnumVariant {
                        enum_name,
                        variant: variant_idx,
                    },
                    vec![],
                    Some(span),
                )
            } else {
                tracing::warn!(
                    variant = self.name_str(name),
                    "tuple variant used as first-class value (not yet supported)"
                );
                self.emit_unit()
            }
        } else if self.pool.tag(self.pool.resolve_fully(ty)) == Tag::Function {
            self.builder
                .emit_partial_apply(ty, name, vec![], Some(span))
        } else {
            tracing::debug!(name = ?name, "unbound identifier in ARC IR lowering");
            self.builder
                .emit_let(ty, ArcValue::Literal(LitValue::Unit), Some(span))
        }
    }

    fn const_binding_literal(&self, name: Name) -> Option<LitValue> {
        let binding = self
            .const_bindings?
            .iter()
            .find(|binding| binding.name == name)?;
        Some(match &binding.value {
            GenericConstValue::Int(value) => LitValue::Int(*value),
            GenericConstValue::Bool(value) => LitValue::Bool(*value),
        })
    }

    /// Lower a named generic-const reference for a concrete mono instance.
    pub(super) fn lower_const_reference(&mut self, name: Name, ty: Idx, span: Span) -> ArcVarId {
        if let Some(literal) = self.const_binding_literal(name) {
            return self
                .builder
                .emit_let(ty, ArcValue::Literal(literal), Some(span));
        }

        self.problems.push(ArcProblem::InternalError {
            message: format!(
                "named constant `{}` survived canonicalization without an exact monomorphization binding",
                self.name_str(name)
            ),
            span,
        });
        self.emit_unit()
    }

    pub(super) fn lower_constant(
        &mut self,
        const_id: ori_ir::canon::ConstantId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        use ori_ir::canon::ConstValue;
        let literal = match self.canon.constants.get(const_id) {
            ConstValue::Int(value) => LitValue::Int(*value),
            ConstValue::Float(bits) => LitValue::Float(*bits),
            ConstValue::Bool(value) => LitValue::Bool(*value),
            ConstValue::Str(name) => LitValue::String(*name),
            ConstValue::Char(value) => LitValue::Char(*value),
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
            .emit_let(ty, ArcValue::Literal(literal), Some(span))
    }

    pub(super) fn lower_binary(
        &mut self,
        op: ori_ir::BinaryOp,
        left: CanId,
        right: CanId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        if op == ori_ir::BinaryOp::Coalesce {
            return self.lower_coalesce(left, right, ty, span);
        }
        if op == ori_ir::BinaryOp::And {
            return self.lower_short_circuit_and(left, right, ty, span);
        }
        if op == ori_ir::BinaryOp::Or {
            return self.lower_short_circuit_or(left, right, ty, span);
        }

        let lhs = self.lower_expr(left);
        let rhs = self.lower_expr(right);
        let operation = PrimOp::Binary(op);
        if let Some(destination) = self.lower_unresolved_operator_call(
            operation,
            self.expr_type(left),
            lhs,
            vec![lhs, rhs],
            ty,
            span,
        ) {
            return destination;
        }
        let destination = self.builder.emit_let(
            ty,
            ArcValue::PrimOp {
                op: operation,
                args: vec![lhs, rhs],
            },
            Some(span),
        );
        if op.may_panic_on_int()
            && self
                .pool
                .tag(self.pool.resolve_fully(ty))
                .is_checked_int_arithmetic()
        {
            self.builder.note_checked_op(destination);
        }
        destination
    }

    pub(super) fn lower_unary(
        &mut self,
        op: ori_ir::UnaryOp,
        operand: CanId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let argument = self.lower_expr(operand);
        if self.builder.is_terminated() {
            return argument;
        }

        let operation = PrimOp::Unary(op);
        if let Some(destination) = self.lower_unresolved_operator_call(
            operation,
            self.expr_type(operand),
            argument,
            vec![argument],
            ty,
            span,
        ) {
            return destination;
        }
        let destination = self.builder.emit_let(
            ty,
            ArcValue::PrimOp {
                op: operation,
                args: vec![argument],
            },
            Some(span),
        );
        if op.may_panic_on_int()
            && self
                .pool
                .tag(self.pool.resolve_fully(ty))
                .is_checked_int_arithmetic()
        {
            self.builder.note_checked_op(destination);
        }
        destination
    }

    fn lower_unresolved_operator_call(
        &mut self,
        operation: PrimOp,
        receiver_type: Idx,
        receiver: ArcVarId,
        arguments: Vec<ArcVarId>,
        result_type: Idx,
        span: Span,
    ) -> Option<ArcVarId> {
        let plan = operator_call_plan(operation)?;
        if self.pool.builtin_method_type_tag(receiver_type).is_some() {
            return None;
        }
        let destination = self.builder.emit_invoke(
            result_type,
            self.interner.intern(plan.method),
            arguments,
            Some(span),
            None,
        );
        self.builder
            .note_operator_call(destination, receiver, operation, Some(span));
        Some(destination)
    }
}
