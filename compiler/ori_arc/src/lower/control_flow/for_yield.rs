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

use crate::ir::{ArcBlockId, ArcValue, ArcVarId, LitValue, MethodCallForm, PrimOp};
use crate::lower::scope::ArcScope;

use super::super::expr::{ArcLowerer, ForYieldContext, ForYieldShape, LoopContext};

type MutableBinding = (Name, ArcVarId, Idx);
type HeaderMutableParam = (Name, ArcVarId, ArcVarId);

struct YieldIteratorSetup {
    header_block: ArcBlockId,
    body_block: ArcBlockId,
    exit_block: ArcBlockId,
    exit_prep_block: ArcBlockId,
    pre_scope: ArcScope,
    header_mut_params: Vec<HeaderMutableParam>,
    exit_mut_params: Vec<(Name, ArcVarId)>,
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
            header_bb = setup.header_block.index(),
            body_bb = setup.body_block.index(),
            exit_bb = setup.exit_block.index(),
            mutable_vars = setup.header_mut_params.len(),
            has_guard = shape.guard.is_valid(),
            "for_yield_iterator: enter"
        );

        let (next_result, has_more) = self.emit_yield_iterator_next(iter_val, elem_ty);
        let list_push = self.interner.intern("ori_list_push");
        self.push_yield_iterator_context(label, iter_val, list_push, &setup);
        self.lower_yield_iterator_guard(shape, elem_ty, next_result, has_more, &setup);
        self.lower_yield_iterator_body(shape, elem_ty, next_result, list_push, &setup);
        self.loop_ctx_stack.pop();
        self.finish_yield_iterator(iter_val, shape.result_ty, setup)
    }

    fn prepare_yield_iterator_loop(
        &mut self,
        result_ty: Idx,
        fallback_elem_ty: Idx,
    ) -> YieldIteratorSetup {
        let header_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let exit_block = self.builder.new_block();
        let exit_prep_block = self.builder.new_block();
        let pre_scope = self.scope.clone();
        let mutable_bindings: Vec<MutableBinding> = pre_scope
            .mutable_bindings()
            .map(|(name, var)| (name, var, self.builder.var_type_or_unit(var)))
            .collect();
        let body_elem_ty = if self.pool.tag(result_ty) == Tag::List {
            self.pool.list_elem(result_ty)
        } else {
            fallback_elem_ty
        };
        let (list_ptr, elem_size_var) = self.allocate_yield_list(body_elem_ty);
        let header_mut_params = mutable_bindings
            .iter()
            .map(|&(name, pre_var, ty)| {
                (
                    name,
                    pre_var,
                    self.builder.add_block_param(header_block, ty),
                )
            })
            .collect::<Vec<_>>();
        let exit_mut_params = mutable_bindings
            .iter()
            .map(|&(name, _, ty)| (name, self.builder.add_block_param(exit_block, ty)))
            .collect();

        let entry_args = header_mut_params
            .iter()
            .map(|&(_, pre_var, _)| pre_var)
            .collect();
        self.builder.terminate_jump(header_block, entry_args);
        self.builder.position_at(header_block);
        self.scope = pre_scope.clone();
        for &(name, _, param) in &header_mut_params {
            self.scope.bind_mutable(name, param);
        }
        YieldIteratorSetup {
            header_block,
            body_block,
            exit_block,
            exit_prep_block,
            pre_scope,
            header_mut_params,
            exit_mut_params,
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

    fn emit_yield_iterator_next(
        &mut self,
        iter_val: ArcVarId,
        elem_ty: Idx,
    ) -> (ArcVarId, ArcVarId) {
        let iter_next = self
            .interner
            .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::IterNext.name());
        // INVARIANT: The scalar wrapper carries a typed marker for physical scratch sizing.
        let elem_ty_marker =
            self.builder
                .emit_let(elem_ty, ArcValue::Literal(LitValue::Int(0)), None);
        let next_result = self.builder.emit_apply(
            Idx::INT,
            iter_next,
            vec![iter_val, elem_ty_marker],
            None,
            None,
        );
        let tag = self.builder.emit_project(Idx::INT, next_result, 0, None);
        let zero = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
        let has_more = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::NotEq),
                args: vec![tag, zero],
            },
            None,
        );
        (next_result, has_more)
    }

    fn push_yield_iterator_context(
        &mut self,
        label: Name,
        iter_val: ArcVarId,
        list_push: Name,
        setup: &YieldIteratorSetup,
    ) {
        let mutable_vars = setup
            .header_mut_params
            .iter()
            .map(|&(name, _, param)| (name, param))
            .collect();
        self.loop_ctx_stack.push(LoopContext {
            label,
            exit_block: setup.exit_block,
            continue_block: setup.header_block,
            mutable_vars,
            abandon_iter: Some(iter_val),
            yield_ctx: Some(ForYieldContext {
                list_ptr: setup.list_ptr,
                elem_size: setup.elem_size_var,
                list_push_name: list_push,
            }),
        });
    }

    fn lower_yield_iterator_guard(
        &mut self,
        shape: ForYieldShape,
        elem_ty: Idx,
        next_result: ArcVarId,
        has_more: ArcVarId,
        setup: &YieldIteratorSetup,
    ) {
        if !shape.guard.is_valid() {
            self.builder
                .terminate_branch(has_more, setup.body_block, setup.exit_prep_block);
            return;
        }
        let guarded_block = self.builder.new_block();
        self.builder
            .terminate_branch(has_more, guarded_block, setup.exit_prep_block);
        self.builder.position_at(guarded_block);
        let elem = self.builder.emit_project(elem_ty, next_result, 1, None);
        self.bind_for_pattern(shape.pattern, elem, elem_ty);
        let guard_val = self.lower_expr(shape.guard);
        if self.builder.is_terminated() {
            return;
        }
        let guard_skip = self.builder.new_block();
        self.builder
            .terminate_branch(guard_val, setup.body_block, guard_skip);
        self.builder.position_at(guard_skip);
        let skip_args = setup
            .header_mut_params
            .iter()
            .map(|&(_, _, param)| param)
            .collect();
        self.builder.terminate_jump(setup.header_block, skip_args);
    }

    fn lower_yield_iterator_body(
        &mut self,
        shape: ForYieldShape,
        elem_ty: Idx,
        next_result: ArcVarId,
        list_push: Name,
        setup: &YieldIteratorSetup,
    ) {
        self.builder.position_at(setup.body_block);
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
            .header_mut_params
            .iter()
            .map(|&(name, _, param)| self.scope.lookup(name).unwrap_or(param))
            .collect();
        self.builder.terminate_jump(setup.header_block, back_args);
    }

    fn finish_yield_iterator(
        &mut self,
        iter_val: ArcVarId,
        result_ty: Idx,
        setup: YieldIteratorSetup,
    ) -> ArcVarId {
        self.builder.position_at(setup.exit_prep_block);
        let prep_args = setup
            .header_mut_params
            .iter()
            .map(|&(_, _, param)| param)
            .collect();
        self.builder.terminate_jump(setup.exit_block, prep_args);

        self.builder.position_at(setup.exit_block);
        let iter_drop = self.interner.intern("ori_iter_drop");
        self.builder
            .emit_apply(Idx::UNIT, iter_drop, vec![iter_val], None, None);
        self.scope = setup.pre_scope;
        for &(name, param) in &setup.exit_mut_params {
            self.scope.bind_mutable(name, param);
        }
        let list_take = self.interner.intern("ori_list_take");
        self.builder
            .emit_apply(result_ty, list_take, vec![setup.list_ptr], None, None)
    }

    /// Compute the transitional LLVM/runtime ABI element size in bytes.
    ///
    /// Used to pass `elem_size` to `ori_list_new` and `ori_list_push`. This is
    /// physical compatibility plumbing, not an AIMS policy or a
    /// backend-neutral layout authority. It must match
    /// `TypeLayoutResolver::type_store_size()` in the current LLVM projection.
    pub(super) fn compute_elem_size(&self, elem_ty: Idx) -> i64 {
        Self::type_store_size(elem_ty, self.pool, 0)
    }

    fn type_store_size(ty: Idx, pool: &ori_types::Pool, depth: u32) -> i64 {
        super::type_layout::pool_type_store_size(ty, pool, depth)
    }
}
