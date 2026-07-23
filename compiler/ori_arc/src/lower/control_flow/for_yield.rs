//! For-yield list comprehension lowering.
//!
//! `for x in iter yield body` produces a `List` by evaluating `body` for each
//! element and collecting results. Three strategies based on iterable type:
//!
//! - **Option**: 0-or-1 element list via `Construct(ListLiteral, [])` or `[body_val]`.
//! - **Range**: Direct counter loop with `ori_list_new` + `ori_list_push`.
//! - **Iterator/Collection**: Convert to an iterator, loop, and push.

use ori_ir::Name;
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId, LitValue, MethodCallForm, YieldExtent};

use super::super::expr::{ArcLowerer, ForYieldContext, ForYieldShape};
use super::iterator_flow::IteratorFlowSetup;

struct YieldIteratorSetup {
    flow: IteratorFlowSetup,
    list_ptr: ArcVarId,
    elem_size_var: ArcVarId,
    elem_ty: Idx,
    elem_size: u64,
    extent: YieldExtent,
}

#[derive(Clone, Copy)]
enum RangeEnd {
    Exclusive,
    Inclusive,
}

/// Carries allocation identity and normalized extent evidence between yield strategies.
pub(super) struct YieldListAllocation {
    /// Builder identity used by list mutation and realization.
    pub(super) list_ptr: ArcVarId,
    /// Lowered element-size operand for runtime list operations.
    pub(super) elem_size_var: ArcVarId,
    /// Extent normalized to the runtime capacity ABI.
    pub(super) extent: YieldExtent,
}

impl ArcLowerer<'_> {
    /// Dispatch for-yield to the appropriate strategy based on iterable type.
    pub(crate) fn lower_for_yield(
        &mut self,
        shape: ForYieldShape,
        iter_val: ArcVarId,
        iter_ty: Idx,
        tag: Tag,
        label: ori_ir::Name,
    ) -> ArcVarId {
        tracing::debug!(
            pattern = ?shape.pattern,
            ?tag,
            has_guard = shape.guard.is_valid(),
            "for_yield: enter"
        );
        if tag == Tag::Option {
            let elem_ty = self.pool.option_inner(iter_ty);
            self.lower_for_yield_option(shape, iter_val, elem_ty, label)
        } else if tag == Tag::Range {
            let extent = self.yield_extent(shape.iter, iter_val, iter_ty, tag);
            self.lower_for_yield_range(shape, iter_val, extent, label)
        } else {
            let extent = self.yield_extent(shape.iter, iter_val, iter_ty, tag);
            let (iter_handle, elem_ty) = self.prepare_iterator(iter_val, iter_ty, tag);
            self.lower_for_yield_iterator(shape, iter_handle, elem_ty, extent, label)
        }
    }

    /// Prepare an iterator handle from any iterable type.
    ///
    /// Returns `(iterator_ptr_var, element_type)`.
    fn prepare_iterator(&mut self, iter_val: ArcVarId, iter_ty: Idx, tag: Tag) -> (ArcVarId, Idx) {
        if tag == Tag::Range {
            let iter_name = self
                .interner
                .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
            let iter_handle =
                self.builder
                    .emit_apply(Idx::INT, iter_name, vec![iter_val], None, None);
            self.builder
                .note_method_call(iter_handle, iter_ty, MethodCallForm::Instance);
            (iter_handle, Idx::INT)
        } else if tag.is_iterator() {
            let elem_ty = self.pool.iterator_elem(iter_ty);
            (iter_val, elem_ty)
        } else {
            let elem_ty = self.extract_yield_elem_type(tag, iter_ty);

            let iter_name = self
                .interner
                .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
            let iter_handle =
                self.builder
                    .emit_apply(Idx::INT, iter_name, vec![iter_val], None, None);
            self.builder
                .note_method_call(iter_handle, iter_ty, MethodCallForm::Instance);

            (iter_handle, elem_ty)
        }
    }

    /// Extract element type for yield expressions.
    fn extract_yield_elem_type(&self, tag: Tag, iter_ty: Idx) -> Idx {
        match tag {
            Tag::List => self.pool.list_elem(iter_ty),
            Tag::Set => self.pool.set_elem(iter_ty),
            Tag::Str => Idx::CHAR,
            Tag::Map => {
                let key_ty = self.pool.map_key(iter_ty);
                let val_ty = self.pool.map_value(iter_ty);
                self.pool.find_tuple(&[key_ty, val_ty]).unwrap_or(Idx::INT)
            }
            _ => Idx::INT,
        }
    }

    /// Lowers an iterator-backed comprehension into a growable list.
    ///
    /// Mutable bindings flow through header and exit parameters. Exhaustion and
    /// break paths converge before the iterator is dropped and the list is taken.
    fn lower_for_yield_iterator(
        &mut self,
        shape: ForYieldShape,
        iter_val: ArcVarId,
        elem_ty: Idx,
        extent: YieldExtent,
        label: ori_ir::Name,
    ) -> ArcVarId {
        let setup = self.prepare_yield_iterator_loop(shape.result_ty, elem_ty, extent);

        tracing::debug!(
            pattern = ?shape.pattern,
            header_bb = setup.flow.header_block.index(),
            body_bb = setup.flow.body_block.index(),
            exit_bb = setup.flow.exit_block.index(),
            mutable_vars = setup.flow.header_mut_params.len(),
            has_guard = shape.guard.is_valid(),
            "for_yield_iterator: enter"
        );

        let (next_result, has_more) = self.emit_iterator_next(iter_val, elem_ty);
        let list_push = self.interner.intern("ori_list_push");
        self.push_iterator_loop_context(
            label,
            iter_val,
            &setup.flow,
            Some(ForYieldContext {
                list_ptr: setup.list_ptr,
                elem_size: setup.elem_size_var,
                list_push_name: list_push,
            }),
        );
        self.lower_iterator_guard(
            shape.pattern,
            elem_ty,
            shape.guard,
            next_result,
            has_more,
            &setup.flow,
        );
        self.lower_yield_iterator_body(shape, elem_ty, next_result, list_push, &setup);
        self.loop_ctx_stack.pop();
        self.finish_yield_iterator(iter_val, shape.result_ty, setup)
    }

    fn prepare_yield_iterator_loop(
        &mut self,
        result_ty: Idx,
        fallback_elem_ty: Idx,
        extent: YieldExtent,
    ) -> YieldIteratorSetup {
        let body_elem_ty = if self.pool.tag(result_ty) == Tag::List {
            self.pool.list_elem(result_ty)
        } else {
            fallback_elem_ty
        };
        let elem_size = self.compute_elem_size(body_elem_ty).cast_unsigned();
        let allocation = self.allocate_yield_list(body_elem_ty, extent);
        let flow = self.prepare_iterator_flow(None);
        YieldIteratorSetup {
            flow,
            list_ptr: allocation.list_ptr,
            elem_size_var: allocation.elem_size_var,
            elem_ty: body_elem_ty,
            elem_size,
            extent: allocation.extent,
        }
    }

    /// Allocate a list builder while preserving normalized extent evidence.
    pub(super) fn allocate_yield_list(
        &mut self,
        elem_ty: Idx,
        extent: YieldExtent,
    ) -> YieldListAllocation {
        let list_new = self.interner.intern("ori_list_new");
        let extent = representable_yield_extent(extent);
        let capacity = match extent {
            YieldExtent::StaticExact(exact) => {
                let exact = i64::try_from(exact)
                    .unwrap_or_else(|_| unreachable!("yield extent was normalized above"));
                self.builder
                    .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(exact)), None)
            }
            YieldExtent::RuntimeExact(var) => var,
            YieldExtent::Unknown => {
                self.builder
                    .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(8)), None)
            }
        };
        let elem_size = self.compute_elem_size(elem_ty);
        let elem_size_var =
            self.builder
                .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(elem_size)), None);
        let list_ptr = self.builder.emit_apply(
            Idx::INT,
            list_new,
            vec![capacity, elem_size_var],
            None,
            None,
        );
        YieldListAllocation {
            list_ptr,
            elem_size_var,
            extent,
        }
    }

    fn yield_extent(
        &mut self,
        iter_expr: ori_ir::canon::CanId,
        iter_val: ArcVarId,
        iter_ty: Idx,
        tag: Tag,
    ) -> YieldExtent {
        if tag != Tag::Range {
            return YieldExtent::Unknown;
        }
        let ori_ir::canon::CanExpr::Range { end, .. } = *self.arena.kind(iter_expr) else {
            return YieldExtent::Unknown;
        };
        if !end.is_valid() {
            return YieldExtent::Unknown;
        }

        let start = self.builder.get_field_literal_int(iter_val, 0);
        let end = self.builder.get_field_literal_int(iter_val, 1);
        let step = self.builder.get_field_literal_int(iter_val, 2);
        let inclusive = self.builder.get_field_literal_int(iter_val, 3);
        if let (Some(start), Some(end), Some(step), Some(inclusive)) = (start, end, step, inclusive)
        {
            let end_kind = if inclusive == 0 {
                RangeEnd::Exclusive
            } else {
                RangeEnd::Inclusive
            };
            return range_cardinality(start, end, step, end_kind)
                .map_or(YieldExtent::Unknown, YieldExtent::StaticExact);
        }

        let len_name = self.interner.intern("len");
        let len = self
            .builder
            .emit_apply(Idx::INT, len_name, vec![iter_val], None, None);
        self.builder
            .note_method_call(len, iter_ty, MethodCallForm::Instance);
        YieldExtent::RuntimeExact(len)
    }

    fn lower_yield_iterator_body(
        &mut self,
        shape: ForYieldShape,
        elem_ty: Idx,
        next_result: ArcVarId,
        list_push: Name,
        setup: &YieldIteratorSetup,
    ) {
        self.builder.position_at(setup.flow.body_block);
        let elem = self.builder.emit_project(elem_ty, next_result, 1, None);
        self.bind_for_pattern(shape.pattern, elem, elem_ty);
        let body_val = self.lower_expr(shape.body);
        if self.builder.is_terminated() {
            return;
        }
        self.builder.emit_apply(
            Idx::UNIT,
            list_push,
            vec![setup.list_ptr, body_val, setup.elem_size_var],
            None,
            None,
        );
        let back_args = setup
            .flow
            .header_mut_params
            .iter()
            .map(|&(name, _, param)| self.scope.lookup(name).unwrap_or(param))
            .collect();
        self.builder
            .terminate_jump(setup.flow.header_block, back_args);
    }

    fn finish_yield_iterator(
        &mut self,
        iter_val: ArcVarId,
        result_ty: Idx,
        setup: YieldIteratorSetup,
    ) -> ArcVarId {
        self.builder.position_at(setup.flow.exit_prep_block);
        let prep_args = setup
            .flow
            .header_mut_params
            .iter()
            .map(|&(_, _, param)| param)
            .collect();
        self.builder
            .terminate_jump(setup.flow.exit_block, prep_args);

        self.builder.position_at(setup.flow.exit_block);
        let iter_drop = self.interner.intern("ori_iter_drop");
        self.builder
            .emit_apply(Idx::UNIT, iter_drop, vec![iter_val], None, None);
        self.scope = setup.flow.pre_scope;
        for &(name, param) in &setup.flow.exit_mut_params {
            self.scope.bind_mutable(name, param);
        }
        let list_take = self.interner.intern("ori_list_take");
        let result =
            self.builder
                .emit_apply(result_ty, list_take, vec![setup.list_ptr], None, None);
        self.builder.note_yield_allocation(
            setup.list_ptr,
            result,
            setup.elem_ty,
            setup.elem_size_var,
            setup.elem_size,
            setup.extent,
        );
        result
    }

    /// Computes the LLVM/runtime ABI element size for list allocation and push.
    ///
    /// This physical compatibility value is not backend-neutral layout authority.
    pub(super) fn compute_elem_size(&self, elem_ty: Idx) -> i64 {
        Self::type_store_size(elem_ty, self.pool, 0)
    }

    fn type_store_size(ty: Idx, pool: &ori_types::Pool, depth: u32) -> i64 {
        super::type_layout::pool_type_store_size(ty, pool, depth)
    }
}

fn range_cardinality(start: i64, end: i64, step: i64, end_kind: RangeEnd) -> Option<u64> {
    if step == 0 {
        return None;
    }
    let start = i128::from(start);
    let end = i128::from(end);
    let step = i128::from(step);
    let inclusive = i128::from(matches!(end_kind, RangeEnd::Inclusive));
    let span = if step > 0 {
        let end = end + inclusive;
        (end - start).max(0)
    } else {
        let end = end - inclusive;
        (start - end).max(0)
    };
    let step_abs = step.abs();
    let count = (span + step_abs - 1) / step_abs;
    u64::try_from(count).ok()
}

fn representable_yield_extent(extent: YieldExtent) -> YieldExtent {
    match extent {
        YieldExtent::StaticExact(exact) if i64::try_from(exact).is_err() => YieldExtent::Unknown,
        representable => representable,
    }
}

#[cfg(test)]
mod tests {
    use super::{range_cardinality, representable_yield_extent, RangeEnd};
    use crate::ir::YieldExtent;

    #[test]
    fn range_cardinality_handles_full_width_signed_boundaries() {
        assert_eq!(
            range_cardinality(i64::MIN, i64::MAX, 1, RangeEnd::Exclusive),
            Some(u64::MAX)
        );
        assert_eq!(
            range_cardinality(i64::MAX, i64::MIN, -1, RangeEnd::Exclusive),
            Some(u64::MAX)
        );
        assert_eq!(
            range_cardinality(i64::MIN, i64::MAX, 1, RangeEnd::Inclusive),
            None
        );
        assert_eq!(
            range_cardinality(i64::MAX, i64::MIN, -1, RangeEnd::Inclusive),
            None
        );
    }

    #[test]
    fn range_cardinality_fails_closed_for_zero_step() {
        assert_eq!(range_cardinality(0, 10, 0, RangeEnd::Exclusive), None);
    }

    #[test]
    fn static_yield_extent_fails_closed_above_runtime_capacity_range() {
        assert_eq!(
            representable_yield_extent(YieldExtent::StaticExact(i64::MAX.cast_unsigned())),
            YieldExtent::StaticExact(i64::MAX.cast_unsigned())
        );
        assert_eq!(
            representable_yield_extent(YieldExtent::StaticExact(i64::MAX.cast_unsigned() + 1)),
            YieldExtent::Unknown
        );
    }
}
