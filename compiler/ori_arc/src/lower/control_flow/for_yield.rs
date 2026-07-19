//! For-yield list comprehension lowering.
//!
//! `for x in iter yield body` produces a `List` by evaluating `body` for each
//! element and collecting results. Three strategies based on iterable type:
//!
//! - **Option**: 0-or-1 element list via `Construct(ListLiteral, [])` or `[body_val]`.
//! - **Range**: Iterator-based loop with `ori_list_new` + `ori_list_push`.
//! - **Iterator/Collection**: Same as range — convert to iterator, loop, push.

use ori_ir::Name;
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId, LitValue, MethodCallForm};

use super::super::expr::{ArcLowerer, ForYieldContext, ForYieldShape};
use super::iterator_flow::IteratorFlowSetup;

struct YieldIteratorSetup {
    flow: IteratorFlowSetup,
    list_ptr: ArcVarId,
    elem_size_var: ArcVarId,
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
        } else {
            // Range, Iterator, List, Set, Str, Map — all go through iterator loop.
            let (iter_handle, elem_ty) = self.prepare_iterator(iter_val, iter_ty, tag);
            self.lower_for_yield_iterator(shape, iter_handle, elem_ty, label)
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
        label: ori_ir::Name,
    ) -> ArcVarId {
        let setup = self.prepare_yield_iterator_loop(shape.result_ty, elem_ty);

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
    ) -> YieldIteratorSetup {
        let body_elem_ty = if self.pool.tag(result_ty) == Tag::List {
            self.pool.list_elem(result_ty)
        } else {
            fallback_elem_ty
        };
        let (list_ptr, elem_size_var) = self.allocate_yield_list(body_elem_ty);
        let flow = self.prepare_iterator_flow(None);
        YieldIteratorSetup {
            flow,
            list_ptr,
            elem_size_var,
        }
    }

    fn allocate_yield_list(&mut self, elem_ty: Idx) -> (ArcVarId, ArcVarId) {
        let list_new = self.interner.intern("ori_list_new");
        let eight = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(8)), None);
        let elem_size = self.compute_elem_size(elem_ty);
        let elem_size_var =
            self.builder
                .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(elem_size)), None);
        let list_ptr =
            self.builder
                .emit_apply(Idx::INT, list_new, vec![eight, elem_size_var], None, None);
        (list_ptr, elem_size_var)
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
        self.builder
            .emit_apply(result_ty, list_take, vec![setup.list_ptr], None, None)
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
