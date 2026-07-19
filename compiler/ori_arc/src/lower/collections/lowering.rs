//! Tuple, collection, constructor, access, range, propagation, and cast lowering.

use ori_ir::canon::{CanFieldRange, CanId, CanMapEntryRange, CanRange, IndexDispatch};
use ori_ir::{Name, Span};
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId, CtorKind, LitValue, MethodCallForm};

use super::super::expr::ArcLowerer;

impl ArcLowerer<'_> {
    // Tuple

    /// Lower a tuple expression to ARC IR.
    pub(crate) fn lower_tuple(&mut self, exprs: CanRange, ty: Idx, span: Span) -> ArcVarId {
        let elem_ids: Vec<_> = self.arena.get_expr_list(exprs).to_vec();
        if elem_ids.is_empty() {
            return self
                .builder
                .emit_let(ty, ArcValue::Literal(LitValue::Unit), Some(span));
        }
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

    /// Lower a `Result` variant (`Ok`/`Err`) to ARC IR. The payload is the
    /// lowered `inner` expression, or unit when `inner` is invalid (unit-payload
    /// variant). Shared body for [`Self::lower_ok`] / [`Self::lower_err`].
    fn lower_result_variant(
        &mut self,
        inner: CanId,
        ty: Idx,
        span: Span,
        variant: u32,
    ) -> ArcVarId {
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
                variant,
            },
            vec![arg],
            Some(span),
        )
    }

    /// Lower an `Ok` constructor to ARC IR.
    pub(crate) fn lower_ok(&mut self, inner: CanId, ty: Idx, span: Span) -> ArcVarId {
        self.lower_result_variant(inner, ty, span, ori_ir::RESULT_VARIANT_OK)
    }

    /// Lower an `Err` constructor to ARC IR.
    pub(crate) fn lower_err(&mut self, inner: CanId, ty: Idx, span: Span) -> ArcVarId {
        self.lower_result_variant(inner, ty, span, ori_ir::RESULT_VARIANT_ERR)
    }

    /// Lower a `Some` constructor to ARC IR.
    pub(crate) fn lower_some(&mut self, inner: CanId, ty: Idx, span: Span) -> ArcVarId {
        let arg = self.lower_expr(inner);
        let option_name = self.interner.intern("Option");
        self.builder.emit_construct(
            ty,
            CtorKind::EnumVariant {
                enum_name: option_name,
                variant: ori_ir::OPTION_VARIANT_SOME,
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
                variant: ori_ir::OPTION_VARIANT_NONE,
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
    ///
    /// Sets `hash_length` before lowering the index sub-expression so that
    /// `CanExpr::HashLength` (`#`) resolves to the collection's length.
    pub(crate) fn lower_index(
        &mut self,
        receiver: CanId,
        index: CanId,
        dispatch: IndexDispatch,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let recv = self.lower_expr(receiver);

        // For list/string receivers, extract length for `#` resolution.
        // Set hash_length so `#` resolves to collection length in the index expression.
        let old_hash = self.hash_length.take();
        let recv_ty = self.pool.resolve_fully(self.expr_type(receiver));
        // List / str index panics on OOB; map / set index returns `Option<V>`
        // and never panics. Only the panic-carrier receivers route through an
        // Invoke carrier (Spec: Clause 17.2.3); a map index stays an `Apply`.
        let panics_on_oob = recv_ty == Idx::STR || self.pool.tag(recv_ty) == Tag::List;
        if panics_on_oob {
            let len_var = if recv_ty == Idx::STR {
                // str's length is NOT a raw field-0 read: OriStr is an SSO/heap
                // union where field 0 means "len" only for the heap variant —
                // for SSO strings it is the first 8 inline bytes reinterpreted
                // as an i64. Route through the SSO-aware `len` builtin method
                // (dispatches to `ori_str_len` at codegen time) instead.
                let len_fn = self.interner.intern("len");
                let len = self
                    .builder
                    .emit_apply(Idx::INT, len_fn, vec![recv], Some(span), None);
                self.builder
                    .note_method_call(len, recv_ty, MethodCallForm::Instance);
                len
            } else {
                // Emit a Project to extract the length field (field 0 for
                // list {len, cap, data})
                self.builder.emit_project(Idx::INT, recv, 0, Some(span))
            };
            self.hash_length = Some(len_var);
        }

        let idx_var = self.lower_expr(index);
        self.hash_length = old_hash;

        match dispatch {
            IndexDispatch::Selected(producer) => {
                let index_fn = self.interner.intern("index");
                let result =
                    self.builder
                        .emit_invoke(ty, index_fn, vec![recv, idx_var], Some(span), None);
                self.builder.note_selected_method_call(
                    result,
                    recv_ty,
                    MethodCallForm::Instance,
                    producer,
                );
                return result;
            }
            IndexDispatch::Error => {
                self.problems.push(super::super::ArcProblem::InternalError {
                    message: "invalid index dispatch reached ARC lowering".into(),
                    span,
                });
                return self
                    .builder
                    .emit_let(ty, ArcValue::Literal(LitValue::Unit), Some(span));
            }
            IndexDispatch::Builtin | IndexDispatch::Deferred => {}
        }

        let deferred_builtin_index = matches!(dispatch, IndexDispatch::Deferred);
        let index_fn = self
            .interner
            .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Index.name());
        // Spec: Clause 17.2.3 — list / str indexing panics on OOB. Always retain
        // an Invoke carrier: even when the catch lives in a caller, this frame
        // must run cleanup for values live across `__index` before resuming the
        // unwind. A non-panicking map / set index keeps the Apply carrier.
        if panics_on_oob || deferred_builtin_index {
            self.builder
                .emit_invoke(ty, index_fn, vec![recv, idx_var], Some(span), None)
        } else {
            self.builder
                .emit_apply(ty, index_fn, vec![recv, idx_var], Some(span), None)
        }
    }

    // Range

    /// Lower a range expression to ARC IR.
    ///
    /// Produces a 4-element tuple: `{start, end, step, inclusive}`.
    /// The inclusive flag is stored as an i64 (0 or 1) to keep the
    /// Range representation uniform. The emitter truncates to i1 for
    /// the runtime call.
    pub(crate) fn lower_range(
        &mut self,
        start: CanId,
        end: CanId,
        step: CanId,
        inclusive: bool,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let mut args = Vec::with_capacity(4);
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
        // Inclusive flag as i64 (0=exclusive, 1=inclusive)
        args.push(self.builder.emit_let(
            Idx::INT,
            ArcValue::Literal(LitValue::Int(i64::from(inclusive))),
            None,
        ));
        self.builder
            .emit_construct(ty, CtorKind::Tuple, args, Some(span))
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
        let source_ty = self.pool.resolve_fully(self.builder.var_type(val));
        let target_ty = self.pool.resolve_fully(ty);
        let may_unwind = self.pool.tag(source_ty) == Tag::Int
            && matches!(self.pool.tag(target_ty), Tag::Byte | Tag::Char);
        let cast_fn = self
            .interner
            .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Cast.name());
        if may_unwind {
            self.builder
                .emit_invoke(ty, cast_fn, vec![val], Some(span), None)
        } else {
            self.builder
                .emit_apply(ty, cast_fn, vec![val], Some(span), None)
        }
    }

    // Helpers

    /// Resolve a field name to its index in the struct type.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "field indices never exceed u32"
    )]
    fn resolve_field_index(&self, recv_ty: Idx, field: Name) -> u32 {
        // resolve_fully follows VarState::Link chains (from unification),
        // resolutions hashmap, and Applied→Named fallback. This is needed
        // when recv_ty is a type variable unified with a struct type (e.g.,
        // closure params inferred from iterator adapters).
        let resolved = self.pool.resolve_fully(recv_ty);
        let tag = self.pool.tag(resolved);

        if tag == Tag::Struct {
            let count = self.pool.struct_field_count(resolved);
            for i in 0..count {
                let (fname, _) = self.pool.struct_field(resolved, i);
                if fname == field {
                    return i as u32;
                }
            }
        }

        if tag == Tag::Tuple {
            // PANIC: tuple field access is always a numeric ordinal (`.0`,
            // `.1`) after typeck (PC-2); a non-numeric name here is a
            // compiler bug, not a fallthrough case.
            let field_str = self.interner.lookup(field);
            return field_str.parse::<u32>().unwrap_or_else(|_| {
                unreachable!(
                    "PC-2 violation: tuple field `{field_str}` is not a numeric ordinal \
                     on receiver type {recv_ty:?}"
                )
            });
        }

        // PANIC: typeck resolves every field access (PC-2), so a field name
        // resolving in neither the struct fields nor as a tuple ordinal is a
        // compiler bug. Defaulting would silently project the wrong field.
        unreachable!(
            "PC-2 violation: field `{}` unresolvable on receiver type {recv_ty:?} (tag {tag:?})",
            self.interner.lookup(field)
        );
    }
}
