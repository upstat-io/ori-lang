//! Collection and constructor lowering.
//!
//! Lowers tuple, list, map, struct, Ok/Err/Some/None, field access, index,
//! range, try (`?`), and cast.
//!
//! Spread variants (`ListWithSpread`, `MapWithSpread`, `StructWithSpread`)
//! and template strings are eliminated during canonicalization.

use ori_ir::canon::{CanFieldRange, CanId, CanMapEntryRange, CanRange};
use ori_ir::{Name, Span, StringInterner};
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId, CtorKind, LitValue, PrimOp};

use super::expr::ArcLowerer;
use super::ArcIrBuilder;

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
                variant: ori_ir::RESULT_VARIANT_OK,
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
                variant: ori_ir::RESULT_VARIANT_ERR,
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
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let recv = self.lower_expr(receiver);

        // For list/string receivers, extract length for `#` resolution.
        // Set hash_length so `#` resolves to collection length in the index expression.
        let old_hash = self.hash_length.take();
        let recv_ty = self.expr_type(receiver);
        if recv_ty == Idx::STR || self.pool.tag(recv_ty) == Tag::List {
            // Emit a Project to extract the length field (field 0 for both
            // str {len, data} and list {len, cap, data})
            let len_var = self.builder.emit_project(Idx::INT, recv, 0, Some(span));
            self.hash_length = Some(len_var);
        }

        let idx_var = self.lower_expr(index);
        self.hash_length = old_hash;

        let index_fn = self
            .interner
            .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Index.name());
        // List indexing panics on OOB (`__index` lowers to may-unwind
        // `ori_list_get`), so an in-frame `catch(expr:)` needs an Invoke
        // carrier here for the panic to land in the handler. BUG-04-153:
        // the conversion is blocked on the Phase-5 burden walk's
        // terminator-position accounting for contract-less borrowed Invoke
        // args (the walk in-lines an alias release BEFORE the terminator
        // that reads it — use-after-free). Until that lands, `__index`
        // keeps the body-Apply carrier; the codegen-side Invoke plumbing
        // (protocol dispatch + armed unwind route) is already in place.
        self.builder
            .emit_apply(ty, index_fn, vec![recv, idx_var], Some(span), None)
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
        let resolved = self.pool.resolve_fully(inner_ty);
        let tag = self.pool.tag(resolved);

        match tag {
            Tag::Result => {
                // Result<T, E>: extract Err payload, re-wrap, early return.
                let err_ty = self.pool.resolve_fully(self.pool.result_err(resolved));
                let err_payload = self.builder.emit_project(err_ty, scrut, 1, Some(span));
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
            }
            Tag::Option => {
                // Option<T>: construct None (variant 1, no payload), early return.
                // Must use the function's return type — `inner_ty` is the scrutinee
                // type which may differ from the return type (e.g., `Option<str>?`
                // in a function returning `Option<int>`).
                let ret_ty = self.return_type;
                let option_name = self.interner.intern("Option");
                let none = self.builder.emit_construct(
                    ret_ty,
                    CtorKind::EnumVariant {
                        enum_name: option_name,
                        variant: 1,
                    },
                    vec![],
                    Some(span),
                );
                self.builder.terminate_return(none);
            }
            // PANIC: typeck rejects `?` on non-Option/Result scrutinees
            // (PC-2), so reaching this arm is a compiler bug.
            other => {
                unreachable!("PC-2 violation: `?` operator on non-Option/Result tag {other:?}")
            }
        }

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
        let cast_fn = self
            .interner
            .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Cast.name());
        self.builder
            .emit_apply(ty, cast_fn, vec![val], Some(span), None)
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

// Shared helpers — list element access and rest-slicing
//
// Consumed by `lower::patterns::bind_pattern_inner` (let-binding destructuring)
// and `decision_tree::emit::resolve_path` (match-pattern decision-tree
// lowering). Both paths must emit the same primitives so `let` and `match`
// produce identical observable behavior on list patterns. Re BUG-04-086.

pub(crate) fn emit_list_element(
    builder: &mut ArcIrBuilder,
    interner: &StringInterner,
    list_value: ArcVarId,
    idx: u32,
    elem_ty: Idx,
    span: Option<Span>,
) -> ArcVarId {
    let idx_var = builder.emit_let(
        Idx::INT,
        ArcValue::Literal(LitValue::Int(i64::from(idx))),
        span,
    );
    let index_fn =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Index.name());
    builder.emit_apply(elem_ty, index_fn, vec![list_value, idx_var], span, None)
}

pub(crate) fn emit_list_rest_slice(
    builder: &mut ArcIrBuilder,
    interner: &StringInterner,
    list_value: ArcVarId,
    start_idx: u32,
    list_ty: Idx,
    span: Option<Span>,
) -> ArcVarId {
    let start_var = builder.emit_let(
        Idx::INT,
        ArcValue::Literal(LitValue::Int(i64::from(start_idx))),
        span,
    );
    let slice_fn = interner.intern("ori_list_slice_drop");
    builder.emit_apply(list_ty, slice_fn, vec![list_value, start_var], span, None)
}

// Tests

#[cfg(test)]
mod tests;
