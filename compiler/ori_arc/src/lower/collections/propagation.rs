//! Lowering for the result/option propagation operator.

use ori_ir::canon::CanId;
use ori_ir::Span;
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId, CtorKind, LitValue, PrimOp};

use super::super::expr::ArcLowerer;

impl ArcLowerer<'_> {
    /// Lower `expr?` to tag dispatch with an early error return.
    pub(crate) fn lower_try(&mut self, inner: CanId, ty: Idx, span: Span) -> ArcVarId {
        let scrutinee = self.lower_expr(inner);
        let inner_ty = self.expr_type(inner);
        let tag = self
            .builder
            .emit_project(Idx::INT, scrutinee, 0, Some(span));
        let zero = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
        let is_ok = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                args: vec![tag, zero],
            },
            Some(span),
        );

        let ok_block = self.builder.new_block();
        let err_block = self.builder.new_block();
        let merge_block = self.builder.new_block();
        self.builder.terminate_branch(is_ok, ok_block, err_block);

        self.builder.position_at(ok_block);
        let ok_payload = self.builder.emit_project(ty, scrutinee, 1, Some(span));
        self.builder.terminate_jump(merge_block, vec![ok_payload]);

        self.builder.position_at(err_block);
        let resolved = self.pool.resolve_fully(inner_ty);
        match self.pool.tag(resolved) {
            Tag::Result => {
                let raw_error_ty = self.pool.result_err(resolved);
                let error_ty = self.pool.resolve_fully(raw_error_ty);
                let mut error = self
                    .builder
                    .emit_project(error_ty, scrutinee, 1, Some(span));
                if self.pool.is_error_struct_receiver(raw_error_ty) {
                    let inject_trace = self.interner.intern("__ori_inject_trace");
                    error = self.builder.emit_apply(
                        error_ty,
                        inject_trace,
                        vec![error],
                        Some(span),
                        None,
                    );
                }
                let result_name = self.interner.intern("Result");
                let wrapped_error = self.builder.emit_construct(
                    inner_ty,
                    CtorKind::EnumVariant {
                        enum_name: result_name,
                        variant: 1,
                    },
                    vec![error],
                    Some(span),
                );
                self.builder.terminate_return(wrapped_error);
            }
            Tag::Option => {
                let option_name = self.interner.intern("Option");
                let none = self.builder.emit_construct(
                    self.return_type,
                    CtorKind::EnumVariant {
                        enum_name: option_name,
                        variant: 1,
                    },
                    vec![],
                    Some(span),
                );
                self.builder.terminate_return(none);
            }
            other => {
                unreachable!("PC-2 violation: `?` operator on non-Option/Result tag {other:?}")
            }
        }

        self.builder.position_at(merge_block);
        self.builder.add_block_param(merge_block, ty)
    }
}
