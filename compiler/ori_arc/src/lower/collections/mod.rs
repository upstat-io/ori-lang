//! Collection and constructor lowering.
//!
//! Lowers tuple, list, map, struct, Ok/Err/Some/None, field access, index,
//! range, try (`?`), and cast.
//!
//! Spread variants (`ListWithSpread`, `MapWithSpread`, `StructWithSpread`)
//! and template strings are eliminated during canonicalization.

use ori_ir::canon::{CanFieldRange, CanId, CanMapEntryRange, CanRange};
use ori_ir::{Name, Span};
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId, CtorKind, LitValue, PrimOp};

use super::expr::ArcLowerer;

impl ArcLowerer<'_> {
    // Tuple

    /// Lower a tuple expression to ARC IR.
    pub(crate) fn lower_tuple(&mut self, exprs: CanRange, ty: Idx, span: Span) -> ArcVarId {
        let elem_ids: Vec<_> = self.arena.get_expr_list(exprs).to_vec();
        let args: Vec<_> = elem_ids.iter().map(|&id| self.lower_expr(id)).collect();
        self.builder
            .emit_construct(ty, CtorKind::Tuple, args, Some(span))
    }

    // List

    /// Lower a list expression to ARC IR.
    pub(crate) fn lower_list(&mut self, exprs: CanRange, ty: Idx, span: Span) -> ArcVarId {
        let elem_ids: Vec<_> = self.arena.get_expr_list(exprs).to_vec();
        let args: Vec<_> = elem_ids.iter().map(|&id| self.lower_expr(id)).collect();
        self.builder
            .emit_construct(ty, CtorKind::ListLiteral, args, Some(span))
    }

    // Map

    /// Lower a map expression to ARC IR.
    pub(crate) fn lower_map(&mut self, entries: CanMapEntryRange, ty: Idx, span: Span) -> ArcVarId {
        let entry_slice: Vec<_> = self.arena.get_map_entries(entries).to_vec();
        let mut args = Vec::with_capacity(entry_slice.len() * 2);
        for entry in &entry_slice {
            args.push(self.lower_expr(entry.key));
            args.push(self.lower_expr(entry.value));
        }
        self.builder
            .emit_construct(ty, CtorKind::MapLiteral, args, Some(span))
    }

    // Struct

    /// Lower a struct expression to ARC IR.
    pub(crate) fn lower_struct(
        &mut self,
        name: Name,
        fields: CanFieldRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let field_slice: Vec<_> = self.arena.get_fields(fields).to_vec();
        let args: Vec<_> = field_slice
            .iter()
            .map(|field| self.lower_expr(field.value))
            .collect();
        self.builder
            .emit_construct(ty, CtorKind::Struct(name), args, Some(span))
    }

    // Ok / Err / Some / None

    /// Lower an `Ok` constructor to ARC IR.
    pub(crate) fn lower_ok(&mut self, inner: CanId, ty: Idx, span: Span) -> ArcVarId {
        let arg = if inner.is_valid() {
            self.lower_expr(inner)
        } else {
            self.emit_unit()
        };
        let result_name = self.interner.intern("Result");
        self.builder.emit_construct(
            ty,
            CtorKind::EnumVariant {
                enum_name: result_name,
                variant: 0,
            },
            vec![arg],
            Some(span),
        )
    }

    /// Lower an `Err` constructor to ARC IR.
    pub(crate) fn lower_err(&mut self, inner: CanId, ty: Idx, span: Span) -> ArcVarId {
        let arg = if inner.is_valid() {
            self.lower_expr(inner)
        } else {
            self.emit_unit()
        };
        let result_name = self.interner.intern("Result");
        self.builder.emit_construct(
            ty,
            CtorKind::EnumVariant {
                enum_name: result_name,
                variant: 1,
            },
            vec![arg],
            Some(span),
        )
    }

    /// Lower a `Some` constructor to ARC IR.
    pub(crate) fn lower_some(&mut self, inner: CanId, ty: Idx, span: Span) -> ArcVarId {
        let arg = self.lower_expr(inner);
        let option_name = self.interner.intern("Option");
        self.builder.emit_construct(
            ty,
            CtorKind::EnumVariant {
                enum_name: option_name,
                variant: 0,
            },
            vec![arg],
            Some(span),
        )
    }

    /// Lower a `None` constructor to ARC IR.
    pub(crate) fn lower_none(&mut self, ty: Idx, span: Span) -> ArcVarId {
        let option_name = self.interner.intern("Option");
        self.builder.emit_construct(
            ty,
            CtorKind::EnumVariant {
                enum_name: option_name,
                variant: 1,
            },
            vec![],
            Some(span),
        )
    }

    // Field / Index

    /// Lower a field access expression to ARC IR.
    pub(crate) fn lower_field(
        &mut self,
        receiver: CanId,
        field: Name,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let recv = self.lower_expr(receiver);
        let recv_ty = self.expr_type(receiver);
        let field_idx = self.resolve_field_index(recv_ty, field);
        self.builder.emit_project(ty, recv, field_idx, Some(span))
    }

    /// Lower an index expression to ARC IR.
    pub(crate) fn lower_index(
        &mut self,
        receiver: CanId,
        index: CanId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let recv = self.lower_expr(receiver);
        let idx_var = self.lower_expr(index);
        let index_fn = self.interner.intern("__index");
        self.builder
            .emit_apply(ty, index_fn, vec![recv, idx_var], Some(span))
    }

    // Range

    /// Lower a range expression to ARC IR.
    pub(crate) fn lower_range(
        &mut self,
        start: CanId,
        end: CanId,
        step: CanId,
        _inclusive: bool,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let mut args = Vec::with_capacity(3);
        args.push(if start.is_valid() {
            self.lower_expr(start)
        } else {
            self.builder
                .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None)
        });
        args.push(if end.is_valid() {
            self.lower_expr(end)
        } else {
            self.builder
                .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(i64::MAX)), None)
        });
        args.push(if step.is_valid() {
            self.lower_expr(step)
        } else {
            self.builder
                .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(1)), None)
        });
        self.builder
            .emit_construct(ty, CtorKind::Tuple, args, Some(span))
    }

    // Try (?)

    /// Lower `expr?` — desugar to match on Ok/Err variant tag.
    pub(crate) fn lower_try(&mut self, inner: CanId, ty: Idx, span: Span) -> ArcVarId {
        let scrut = self.lower_expr(inner);
        let inner_ty = self.expr_type(inner);

        let tag_var = self.builder.emit_project(Idx::INT, scrut, 0, Some(span));

        let zero = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
        let is_ok = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                args: vec![tag_var, zero],
            },
            Some(span),
        );

        let ok_block = self.builder.new_block();
        let err_block = self.builder.new_block();
        let merge_block = self.builder.new_block();

        self.builder.terminate_branch(is_ok, ok_block, err_block);

        self.builder.position_at(ok_block);
        let ok_payload = self.builder.emit_project(ty, scrut, 1, Some(span));
        self.builder.terminate_jump(merge_block, vec![ok_payload]);

        self.builder.position_at(err_block);
        let err_payload = self.builder.emit_project(Idx::ERROR, scrut, 1, Some(span));
        let result_name = self.interner.intern("Result");
        let wrapped_err = self.builder.emit_construct(
            inner_ty,
            CtorKind::EnumVariant {
                enum_name: result_name,
                variant: 1,
            },
            vec![err_payload],
            Some(span),
        );
        self.builder.terminate_return(wrapped_err);

        self.builder.position_at(merge_block);
        self.builder.add_block_param(merge_block, ty)
    }

    // Cast

    /// Lower a type cast expression to ARC IR.
    pub(crate) fn lower_cast(
        &mut self,
        expr: CanId,
        _fallible: bool,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let val = self.lower_expr(expr);
        let cast_fn = self.interner.intern("__cast");
        self.builder.emit_apply(ty, cast_fn, vec![val], Some(span))
    }

    // Helpers

    /// Resolve a field name to its index in the struct type.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "field indices never exceed u32"
    )]
    fn resolve_field_index(&self, recv_ty: Idx, field: Name) -> u32 {
        let tag = self.pool.tag(recv_ty);

        if tag == Tag::Struct {
            let count = self.pool.struct_field_count(recv_ty);
            for i in 0..count {
                let (fname, _) = self.pool.struct_field(recv_ty, i);
                if fname == field {
                    return i as u32;
                }
            }
        }

        if let Some(resolved) = self.pool.resolve(recv_ty) {
            if self.pool.tag(resolved) == Tag::Struct {
                let count = self.pool.struct_field_count(resolved);
                for i in 0..count {
                    let (fname, _) = self.pool.struct_field(resolved, i);
                    if fname == field {
                        return i as u32;
                    }
                }
            }
        }

        if tag == Tag::Tuple {
            let field_str = self.interner.lookup(field);
            if let Ok(idx) = field_str.parse::<u32>() {
                return idx;
            }
        }

        tracing::debug!(
            ?recv_ty,
            ?field,
            "could not resolve field index — defaulting to 0"
        );
        0
    }
}

// Tests

#[cfg(test)]
mod tests;
