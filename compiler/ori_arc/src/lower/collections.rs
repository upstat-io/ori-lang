//! Collection and constructor lowering.
//!
//! Lowers tuple, list, map, struct, Ok/Err/Some/None, field access, index,
//! range, try (`?`), cast, spread variants, and template strings.

use ori_ir::{
    ExprId, ExprRange, FieldInitRange, ListElementRange, MapElementRange, MapEntryRange, Name,
    Span, StructLitFieldRange, TemplatePartRange,
};
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId, CtorKind, LitValue, PrimOp};

use super::expr::ArcLowerer;

impl ArcLowerer<'_> {
    // ── Tuple ──────────────────────────────────────────────────

    pub(crate) fn lower_tuple(&mut self, exprs: ExprRange, ty: Idx, span: Span) -> ArcVarId {
        let elem_ids: Vec<_> = self.arena.get_expr_list(exprs).to_vec();
        let args: Vec<_> = elem_ids.iter().map(|&id| self.lower_expr(id)).collect();
        self.builder
            .emit_construct(ty, CtorKind::Tuple, args, Some(span))
    }

    // ── List ───────────────────────────────────────────────────

    pub(crate) fn lower_list(&mut self, exprs: ExprRange, ty: Idx, span: Span) -> ArcVarId {
        let elem_ids: Vec<_> = self.arena.get_expr_list(exprs).to_vec();
        let args: Vec<_> = elem_ids.iter().map(|&id| self.lower_expr(id)).collect();
        self.builder
            .emit_construct(ty, CtorKind::ListLiteral, args, Some(span))
    }

    // ── Map ────────────────────────────────────────────────────

    pub(crate) fn lower_map(&mut self, entries: MapEntryRange, ty: Idx, span: Span) -> ArcVarId {
        let entry_slice: Vec<_> = self.arena.get_map_entries(entries).to_vec();
        let mut args = Vec::with_capacity(entry_slice.len() * 2);
        for entry in &entry_slice {
            args.push(self.lower_expr(entry.key));
            args.push(self.lower_expr(entry.value));
        }
        self.builder
            .emit_construct(ty, CtorKind::MapLiteral, args, Some(span))
    }

    // ── Struct ─────────────────────────────────────────────────

    pub(crate) fn lower_struct(
        &mut self,
        name: Name,
        fields: FieldInitRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let field_inits: Vec<_> = self.arena.get_field_inits(fields).to_vec();
        let args: Vec<_> = field_inits
            .iter()
            .map(|init| {
                if let Some(value_id) = init.value {
                    self.lower_expr(value_id)
                } else {
                    // Shorthand: `Point { x }` — look up `x` in scope.
                    self.lower_ident_by_name(init.name, ty, span)
                }
            })
            .collect();
        self.builder
            .emit_construct(ty, CtorKind::Struct(name), args, Some(span))
    }

    // ── Ok / Err / Some / None ─────────────────────────────────

    pub(crate) fn lower_ok(&mut self, inner: ExprId, ty: Idx, span: Span) -> ArcVarId {
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

    pub(crate) fn lower_err(&mut self, inner: ExprId, ty: Idx, span: Span) -> ArcVarId {
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

    pub(crate) fn lower_some(&mut self, inner: ExprId, ty: Idx, span: Span) -> ArcVarId {
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

    // ── Field / Index ──────────────────────────────────────────

    pub(crate) fn lower_field(
        &mut self,
        receiver: ExprId,
        field: Name,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let recv = self.lower_expr(receiver);
        let recv_ty = self.expr_type(receiver);
        let field_idx = self.resolve_field_index(recv_ty, field);
        self.builder.emit_project(ty, recv, field_idx, Some(span))
    }

    pub(crate) fn lower_index(
        &mut self,
        receiver: ExprId,
        index: ExprId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let recv = self.lower_expr(receiver);
        let idx_var = self.lower_expr(index);
        let index_fn = self.interner.intern("__index");
        self.builder
            .emit_apply(ty, index_fn, vec![recv, idx_var], Some(span))
    }

    // ── Range ──────────────────────────────────────────────────

    pub(crate) fn lower_range(
        &mut self,
        start: ExprId,
        end: ExprId,
        step: ExprId,
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

    // ── Try (?) ────────────────────────────────────────────────

    /// Lower `expr?` — desugar to match on Ok/Err variant tag.
    pub(crate) fn lower_try(&mut self, inner: ExprId, ty: Idx, span: Span) -> ArcVarId {
        let scrut = self.lower_expr(inner);
        let inner_ty = self.expr_type(inner);

        // Project tag field (field 0 = discriminant for result type).
        let tag_var = self.builder.emit_project(Idx::INT, scrut, 0, Some(span));

        // ok_val = tag == 0 (Ok variant).
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

        // Ok path: extract payload and jump to merge.
        self.builder.position_at(ok_block);
        let ok_payload = self.builder.emit_project(ty, scrut, 1, Some(span));
        self.builder.terminate_jump(merge_block, vec![ok_payload]);

        // Err path: extract error, wrap in Err, and return early.
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

        // Continue in merge block.
        self.builder.position_at(merge_block);
        self.builder.add_block_param(merge_block, ty)
    }

    // ── Cast ───────────────────────────────────────────────────

    pub(crate) fn lower_cast(
        &mut self,
        expr: ExprId,
        _fallible: bool,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let val = self.lower_expr(expr);
        let cast_fn = self.interner.intern("__cast");
        self.builder.emit_apply(ty, cast_fn, vec![val], Some(span))
    }

    // ── Spread variants ────────────────────────────────────────

    pub(crate) fn lower_list_with_spread(
        &mut self,
        elements: ListElementRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let elem_slice: Vec<_> = self.arena.get_list_elements(elements).to_vec();
        let mut args = Vec::new();
        for elem in &elem_slice {
            match elem {
                ori_ir::ListElement::Expr { expr, .. }
                | ori_ir::ListElement::Spread { expr, .. } => {
                    args.push(self.lower_expr(*expr));
                }
            }
        }
        let spread_fn = self.interner.intern("__list_spread");
        self.builder.emit_apply(ty, spread_fn, args, Some(span))
    }

    pub(crate) fn lower_map_with_spread(
        &mut self,
        elements: MapElementRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let elem_slice: Vec<_> = self.arena.get_map_elements(elements).to_vec();
        let mut args = Vec::new();
        for elem in &elem_slice {
            match elem {
                ori_ir::MapElement::Entry(entry) => {
                    args.push(self.lower_expr(entry.key));
                    args.push(self.lower_expr(entry.value));
                }
                ori_ir::MapElement::Spread { expr, .. } => {
                    args.push(self.lower_expr(*expr));
                }
            }
        }
        let spread_fn = self.interner.intern("__map_spread");
        self.builder.emit_apply(ty, spread_fn, args, Some(span))
    }

    pub(crate) fn lower_struct_with_spread(
        &mut self,
        _name: Name,
        fields: StructLitFieldRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let field_slice: Vec<_> = self.arena.get_struct_lit_fields(fields).to_vec();
        let mut args = Vec::new();
        for field in &field_slice {
            match field {
                ori_ir::StructLitField::Field(init) => {
                    if let Some(value_id) = init.value {
                        args.push(self.lower_expr(value_id));
                    } else {
                        args.push(self.lower_ident_by_name(init.name, ty, span));
                    }
                }
                ori_ir::StructLitField::Spread { expr, .. } => {
                    args.push(self.lower_expr(*expr));
                }
            }
        }
        let spread_fn = self.interner.intern("__struct_spread");
        self.builder.emit_apply(ty, spread_fn, args, Some(span))
    }

    // ── Template strings ───────────────────────────────────────

    pub(crate) fn lower_template_full(&mut self, name: Name, ty: Idx, span: Span) -> ArcVarId {
        self.builder
            .emit_let(ty, ArcValue::Literal(LitValue::String(name)), Some(span))
    }

    pub(crate) fn lower_template_literal(
        &mut self,
        head: Name,
        parts: TemplatePartRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let part_slice: Vec<_> = self.arena.get_template_parts(parts).to_vec();
        let mut args = Vec::new();
        args.push(
            self.builder
                .emit_let(Idx::STR, ArcValue::Literal(LitValue::String(head)), None),
        );
        for part in &part_slice {
            args.push(self.lower_expr(part.expr));
            if part.text_after != Name::EMPTY {
                args.push(self.builder.emit_let(
                    Idx::STR,
                    ArcValue::Literal(LitValue::String(part.text_after)),
                    None,
                ));
            }
        }
        let format_fn = self.interner.intern("__format");
        self.builder.emit_apply(ty, format_fn, args, Some(span))
    }

    // ── Helpers ────────────────────────────────────────────────

    /// Helper for struct shorthand — look up a name in scope and emit a Var ref.
    pub(crate) fn lower_ident_by_name(&mut self, name: Name, ty: Idx, span: Span) -> ArcVarId {
        if let Some(var) = self.scope.lookup(name) {
            self.builder.emit_let(ty, ArcValue::Var(var), Some(span))
        } else {
            self.builder
                .emit_let(ty, ArcValue::Literal(LitValue::Unit), Some(span))
        }
    }

    /// Resolve a field name to its index in the struct type.
    // Field indices never exceed u32.
    #[allow(clippy::cast_possible_truncation)]
    fn resolve_field_index(&self, recv_ty: Idx, field: Name) -> u32 {
        let tag = self.pool.tag(recv_ty);

        // For resolved struct types, look up the field index.
        if tag == Tag::Struct {
            let count = self.pool.struct_field_count(recv_ty);
            for i in 0..count {
                let (fname, _) = self.pool.struct_field(recv_ty, i);
                if fname == field {
                    return i as u32;
                }
            }
        }

        // Try resolving named types.
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

        // For tuples, the field name is a numeric index like "0", "1", etc.
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

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use ori_ir::ast::{Expr, ExprKind};
    use ori_ir::{ExprArena, Name, Span, StringInterner};
    use ori_types::{Idx, Pool};

    use crate::ir::{ArcInstr, CtorKind};

    #[test]
    fn lower_tuple() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        let a = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(1, 2)));
        let b = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(4, 5)));
        let exprs = arena.alloc_expr_list_inline(&[a, b]);
        let tup = arena.alloc_expr(Expr::new(ExprKind::Tuple(exprs), Span::new(0, 6)));

        let mut expr_types = vec![Idx::ERROR; tup.index() + 1];
        expr_types[a.index()] = Idx::INT;
        expr_types[b.index()] = Idx::INT;
        expr_types[tup.index()] = Idx::UNIT;

        let mut problems = Vec::new();
        let (func, _) = super::super::super::lower_function(
            Name::from_raw(1),
            &[],
            Idx::UNIT,
            tup,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        assert!(problems.is_empty());
        let last = &func.blocks[0].body[2];
        assert!(matches!(
            last,
            ArcInstr::Construct {
                ctor: CtorKind::Tuple,
                ..
            }
        ));
    }

    #[test]
    fn lower_none() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        let none_id = arena.alloc_expr(Expr::new(ExprKind::None, Span::new(0, 4)));
        let mut expr_types = vec![Idx::ERROR; none_id.index() + 1];
        expr_types[none_id.index()] = Idx::UNIT;

        let mut problems = Vec::new();
        let (func, _) = super::super::super::lower_function(
            Name::from_raw(1),
            &[],
            Idx::UNIT,
            none_id,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        assert!(problems.is_empty());
        let last = &func.blocks[0].body[0];
        assert!(matches!(
            last,
            ArcInstr::Construct {
                ctor: CtorKind::EnumVariant { variant: 1, .. },
                ..
            }
        ));
    }
}
