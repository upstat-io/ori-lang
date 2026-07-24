//! Leaf and unary expression lowering.

use ori_ir::canon::{CanExpr, CanId};
use ori_ir::{ExprId, ExprKind, TypeId};

use super::super::Lowerer;

impl Lowerer<'_> {
    pub(super) fn lower_leaf_kind(
        &mut self,
        kind: ExprKind,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
        match kind {
            ExprKind::Int(value) => self.push(CanExpr::Int(value), span, ty),
            ExprKind::Float(value) => self.push(CanExpr::Float(value), span, ty),
            ExprKind::Bool(value) => self.push(CanExpr::Bool(value), span, ty),
            ExprKind::String(value) => self.push(CanExpr::Str(value), span, ty),
            ExprKind::Char(value) => self.push(CanExpr::Char(value), span, ty),
            ExprKind::Duration { value, unit } => {
                self.push(CanExpr::Duration { value, unit }, span, ty)
            }
            ExprKind::Size { value, unit } => self.push(CanExpr::Size { value, unit }, span, ty),
            ExprKind::Unit => self.push(CanExpr::Unit, span, ty),
            ExprKind::None => self.push(CanExpr::None, span, ty),
            ExprKind::Ident(name) => self.lower_ident_kind(name, span, ty),
            ExprKind::Const(name) => self.lower_const_kind(name, span, ty),
            ExprKind::SelfRef => self.push(CanExpr::SelfRef, span, ty),
            ExprKind::FunctionRef(name) => self.push(CanExpr::FunctionRef(name), span, ty),
            ExprKind::HashLength => self.push(CanExpr::HashLength, span, ty),
            ExprKind::Error => self.push(CanExpr::Error, span, ty),
            _ => unreachable!("lower_leaf_kind called with non-leaf expression"),
        }
    }

    fn lower_ident_kind(&mut self, name: ori_ir::Name, span: ori_ir::Span, ty: TypeId) -> CanId {
        let kind = if self.is_type_reference(name, ori_types::Idx::from_raw(ty.raw())) {
            CanExpr::TypeRef(name)
        } else {
            CanExpr::Ident(name)
        };
        self.push(kind, span, ty)
    }

    fn lower_const_kind(&mut self, name: ori_ir::Name, span: ori_ir::Span, ty: TypeId) -> CanId {
        if let Some(value) = self.named_constants.get(&name).cloned() {
            let constant = self.constants.intern(value);
            self.push(CanExpr::Constant(constant), span, ty)
        } else {
            // Unresolved names preserve generic const parameters and diagnostics.
            self.push(CanExpr::Const(name), span, ty)
        }
    }

    pub(super) fn lower_unary_kind(
        &mut self,
        kind: ExprKind,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
        let lowered = match kind {
            ExprKind::Unary { op, operand } => {
                return self.lower_unary_operator(op, operand, span, ty)
            }
            ExprKind::Ok(inner) => CanExpr::Ok(self.lower_optional(inner)),
            ExprKind::Err(inner) => CanExpr::Err(self.lower_optional(inner)),
            ExprKind::Some(inner) => CanExpr::Some(self.lower_expr(inner)),
            ExprKind::Break { label, value } => CanExpr::Break {
                label,
                value: self.lower_optional(value),
            },
            ExprKind::Continue { label, value } => CanExpr::Continue {
                label,
                value: self.lower_optional(value),
            },
            ExprKind::Unsafe(inner) => CanExpr::Unsafe(self.lower_expr(inner)),
            ExprKind::Await(inner) => CanExpr::Await(self.lower_expr(inner)),
            ExprKind::Try(inner) => CanExpr::Try(self.lower_expr(inner)),
            ExprKind::Loop { label, body } => CanExpr::Loop {
                label,
                body: self.lower_expr(body),
            },
            ExprKind::While { label, cond, body } => {
                return self.desugar_while(label, cond, body, span);
            }
            _ => unreachable!("lower_unary_kind called with non-unary expression"),
        };
        self.push(lowered, span, ty)
    }

    fn lower_unary_operator(
        &mut self,
        op: ori_ir::UnaryOp,
        operand: ExprId,
        span: ori_ir::Span,
        ty: TypeId,
    ) -> CanId {
        let operand = self.lower_expr(operand);
        self.push_foldable(CanExpr::Unary { op, operand }, span, ty)
    }
}
