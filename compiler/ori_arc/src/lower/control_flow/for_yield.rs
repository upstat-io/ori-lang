//! For-yield list comprehension lowering.
//!
//! `for x in iter yield body` produces a `List` by evaluating `body` for each
//! element and collecting results. Three strategies based on iterable type:
//!
//! - **Option**: 0-or-1 element list via `Construct(ListLiteral, [])` or `[body_val]`.
//! - **Range**: Iterator-based loop with `ori_list_new` + `ori_list_push`.
//! - **Iterator/Collection**: Same as range — convert to iterator, loop, push.

use ori_ir::canon::{CanBindingPatternId, CanId};
use ori_ir::Name;
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};

use super::super::expr::{ArcLowerer, ForYieldContext, LoopContext};

impl ArcLowerer<'_> {
    /// Dispatch for-yield to the appropriate strategy based on iterable type.
    #[expect(
        clippy::too_many_arguments,
        reason = "lowering params are all semantically distinct"
    )]
    pub(crate) fn lower_for_yield(
        &mut self,
        pattern: CanBindingPatternId,
        iter_val: ArcVarId,
        iter_ty: Idx,
        tag: Tag,
        guard: CanId,
        body: CanId,
        result_ty: Idx,
    ) -> ArcVarId {
        tracing::debug!(
            pattern = ?pattern,
            ?tag,
            has_guard = guard.is_valid(),
            "for_yield: enter"
        );
        if tag == Tag::Option {
            let elem_ty = self.pool.option_inner(iter_ty);
            self.lower_for_yield_option(pattern, iter_val, elem_ty, guard, body, result_ty)
        } else {
            // Range, Iterator, List, Set, Str, Map — all go through iterator loop.
            let (iter_handle, elem_ty, coll_var) = self.prepare_iterator(iter_val, iter_ty, tag);
            self.lower_for_yield_iterator(
                pattern,
                iter_handle,
                elem_ty,
                guard,
                body,
                result_ty,
                coll_var,
            )
        }
    }

    /// Prepare an iterator handle from any iterable type.
    ///
    /// Returns `(iterator_ptr_var, element_type, optional_collection_var)`.
    /// The optional collection variable is `Some` for List/Set collections
    /// that need a `__for_coll` phantom to ensure correct cleanup ordering.
    fn prepare_iterator(
        &mut self,
        iter_val: ArcVarId,
        iter_ty: Idx,
        tag: Tag,
    ) -> (ArcVarId, Idx, Option<ArcVarId>) {
        if tag == Tag::Range {
            let iter_name = self.interner.intern("iter");
            let iter_handle = self
                .builder
                .emit_apply(Idx::INT, iter_name, vec![iter_val], None);
            (iter_handle, Idx::INT, None)
        } else if tag.is_iterator() {
            let elem_ty = self.pool.iterator_elem(iter_ty);
            (iter_val, elem_ty, None)
        } else {
            let elem_ty = self.extract_yield_elem_type(tag, iter_ty);

            let iter_name = self.interner.intern("iter");
            let iter_handle = self
                .builder
                .emit_apply(Idx::INT, iter_name, vec![iter_val], None);

            // For List/Set: return the collection variable so the yield
            // loop can thread it as a block param. This makes the original
            // variable die at the Jump to header (its last use), preventing
            // the AIMS analysis from emitting a spurious extra RcDec in
            // post-loop code. The header param takes over and its final
            // use (dummy let in exit block) ensures cleanup ordering.
            let coll = if matches!(tag, Tag::List | Tag::Set) {
                Some(iter_val)
            } else {
                None
            };
            (iter_handle, elem_ty, coll)
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

    // lower_for_yield_option moved to for_yield_option.rs

    /// Lower `for x in <iterator> yield body` — dynamic list building.
    ///
    /// Uses `ori_list_new` to create a heap-allocated growable list,
    /// `ori_list_push` to append each body result, and `ori_list_take`
    /// to extract the final list struct and free the wrapper.
    ///
    /// Mutable variables from the enclosing scope are threaded through
    /// the loop as header/exit block parameters (SSA phi nodes), matching
    /// the for-do pattern in `for_iterator.rs`. This ensures assignments
    /// to outer mutable variables inside the body are correctly propagated.
    ///
    /// ```text
    /// entry:     list_ptr = ori_list_new(8, elem_size)
    ///            jump → header(coll_var, mut0, mut1, ...)
    /// header:    next = __iter_next(iter)
    ///            has_more = (tag != 0)
    ///            branch(has_more, body, exit_prep)
    /// body:      elem = project(next, 1)
    ///            body_val = lower(body)
    ///            ori_list_push(list_ptr, body_val, elem_size)
    ///            jump → header(coll_param, mut0', mut1', ...)
    /// exit_prep: jump → exit(coll_param, mut0, mut1, ...)
    /// exit:      ori_iter_drop(iter)
    ///            result = ori_list_take(list_ptr)
    /// ```
    #[expect(
        clippy::too_many_arguments,
        reason = "coll_var is an optional phantom for RC cleanup ordering — extracting a config struct would obscure the control flow"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "iterator loop lowering with mutable-var SSA merge is inherently sequential"
    )]
    fn lower_for_yield_iterator(
        &mut self,
        pattern: CanBindingPatternId,
        iter_val: ArcVarId,
        elem_ty: Idx,
        guard: CanId,
        body: CanId,
        result_ty: Idx,
        coll_var: Option<ArcVarId>,
    ) -> ArcVarId {
        let header_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let exit_block = self.builder.new_block();
        // Branch can't carry args, so the normal exit path (iterator
        // exhausted) goes header → exit_prep → exit with mutable params.
        let exit_prep_block = self.builder.new_block();

        // Collect outer mutable bindings for SSA merge through the loop,
        // matching the for-do pattern in for_iterator.rs.
        let pre_scope = self.scope.clone();
        let mut mut_info: Vec<(Name, ArcVarId, Idx)> = Vec::new();
        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = self.builder.var_type_or_unit(var);
            mut_info.push((name, var, var_ty));
        }

        // Determine the body result element type from the list result type.
        // result_ty is `List<T>` — extract T for the element size.
        let body_elem_ty = if self.pool.tag(result_ty) == Tag::List {
            self.pool.list_elem(result_ty)
        } else {
            elem_ty // fallback: use iterator element type
        };

        tracing::debug!(
            pattern = ?pattern,
            header_bb = header_block.index(),
            body_bb = body_block.index(),
            exit_bb = exit_block.index(),
            mutable_vars = mut_info.len(),
            has_guard = guard.is_valid(),
            "for_yield_iterator: enter"
        );

        // Allocate growable list: ori_list_new(initial_cap=8, elem_size)
        let list_new = self.interner.intern("ori_list_new");
        let eight = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(8)), None);
        let body_elem_size = self.compute_elem_size(body_elem_ty);
        let elem_size_var = self.builder.emit_let(
            Idx::INT,
            ArcValue::Literal(LitValue::Int(body_elem_size)),
            None,
        );
        // ori_list_new returns ptr (opaque); use INT type in ARC IR (scalar, no RC).
        let list_ptr =
            self.builder
                .emit_apply(Idx::INT, list_new, vec![eight, elem_size_var], None);

        // Header params: optional collection phantom + mutable vars.
        let coll_param = coll_var.map(|cv| {
            let coll_ty = self.builder.var_type_or_unit(cv);
            self.builder.add_block_param(header_block, coll_ty)
        });
        let mut header_mut_params = Vec::new();
        for &(name, pre_var, var_ty) in &mut_info {
            let param = self.builder.add_block_param(header_block, var_ty);
            header_mut_params.push((name, pre_var, param));
        }

        // Exit block params: optional collection phantom + mutable vars.
        let exit_coll_param = coll_var.map(|_| {
            let coll_ty = coll_param.map_or(Idx::UNIT, |cp| self.builder.var_type_or_unit(cp));
            self.builder.add_block_param(exit_block, coll_ty)
        });
        let mut exit_mut_params = Vec::new();
        for &(name, _, var_ty) in &mut_info {
            let param = self.builder.add_block_param(exit_block, var_ty);
            exit_mut_params.push((name, param));
        }

        // Entry jump: pass coll_var + current mutable var values to header.
        let mut entry_args: Vec<_> = coll_var.into_iter().collect();
        entry_args.extend(header_mut_params.iter().map(|(_, pre_var, _)| *pre_var));
        self.builder.terminate_jump(header_block, entry_args);

        // Header: bind mutable params, call __iter_next.
        self.builder.position_at(header_block);
        self.scope = pre_scope.clone();
        for &(name, _, param_var) in &header_mut_params {
            self.scope.bind_mutable(name, param_var);
        }

        let iter_next = self
            .interner
            .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::IterNext.name());
        // Use INT result type to suppress ARC RC on the wrapper struct.
        // Pass elem_ty marker so the LLVM emitter can size the scratch buffer.
        let elem_ty_marker =
            self.builder
                .emit_let(elem_ty, ArcValue::Literal(LitValue::Int(0)), None);
        let next_result =
            self.builder
                .emit_apply(Idx::INT, iter_next, vec![iter_val, elem_ty_marker], None);

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

        if guard.is_valid() {
            let guarded_block = self.builder.new_block();
            self.builder
                .terminate_branch(has_more, guarded_block, exit_prep_block);

            self.builder.position_at(guarded_block);
            let elem = self.builder.emit_project(elem_ty, next_result, 1, None);
            self.bind_for_pattern(pattern, elem, elem_ty);
            let guard_val = self.lower_expr(guard);

            let guard_skip = self.builder.new_block();
            self.builder
                .terminate_branch(guard_val, body_block, guard_skip);

            // Guard skip: jump back to header with unmodified mutable vars.
            self.builder.position_at(guard_skip);
            let mut skip_args: Vec<_> = coll_param.into_iter().collect();
            skip_args.extend(header_mut_params.iter().map(|&(_, _, param)| param));
            self.builder.terminate_jump(header_block, skip_args);
        } else {
            self.builder
                .terminate_branch(has_more, body_block, exit_prep_block);
        }

        // Body: extract element, evaluate body, push to list.
        self.builder.position_at(body_block);
        let elem = self.builder.emit_project(elem_ty, next_result, 1, None);
        self.bind_for_pattern(pattern, elem, elem_ty);

        // Intern list_push before setting up LoopContext (break/continue need it).
        let list_push = self.interner.intern("ori_list_push");

        // Set up LoopContext so break/continue work inside the yield body.
        // Matches the for-do pattern in for_iterator.rs.
        let mutable_var_names: Vec<_> = mut_info.iter().map(|(name, _, _)| *name).collect();
        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block,
            continue_block: header_block,
            mutable_vars: mutable_var_names,
            yield_ctx: Some(ForYieldContext {
                list_ptr,
                elem_size: elem_size_var,
                list_push_name: list_push,
                coll_param,
            }),
        });

        let body_val = self.lower_expr(body);

        if !self.builder.is_terminated() {
            // Normal body completion: push result and jump to header.
            // (If break/continue terminated, the handlers already handled
            // the push and jump — skip this to avoid dead code.)
            self.builder.emit_apply(
                Idx::UNIT,
                list_push,
                vec![list_ptr, body_val, elem_size_var],
                None,
            );

            // Jump back to header with coll_param + updated mutable var values.
            let mut back_args: Vec<_> = coll_param.into_iter().collect();
            back_args.extend(
                header_mut_params.iter().map(|(name, _, _)| {
                    self.scope.lookup(*name).unwrap_or_else(|| ArcVarId::new(0))
                }),
            );
            self.builder.terminate_jump(header_block, back_args);
        }

        self.loop_ctx = prev_loop;

        // Exit prep: normal loop exhaustion path. Passes coll_param +
        // current mutable var values to the exit block.
        self.builder.position_at(exit_prep_block);
        let mut prep_args: Vec<_> = coll_param.into_iter().collect();
        prep_args.extend(header_mut_params.iter().map(|&(_, _, param)| param));
        self.builder.terminate_jump(exit_block, prep_args);

        // Exit: drop the iterator handle, then extract the final list.
        self.builder.position_at(exit_block);
        let iter_drop = self.interner.intern("ori_iter_drop");
        self.builder
            .emit_apply(Idx::UNIT, iter_drop, vec![iter_val], None);

        // Dummy reference to collection param AFTER ori_iter_drop.
        // Keeps the collection alive past the iterator drop, so the
        // AIMS pipeline's RcDec (with real elem_dec_fn) reaches zero.
        if let Some(param) = exit_coll_param {
            let coll_ty = self.builder.var_type_or_unit(param);
            self.builder.emit_let(coll_ty, ArcValue::Var(param), None);
        }

        // Restore scope with final mutable var values from the exit block.
        self.scope = pre_scope;
        for &(name, param) in &exit_mut_params {
            self.scope.bind_mutable(name, param);
        }

        let list_take = self.interner.intern("ori_list_take");
        self.builder
            .emit_apply(result_ty, list_take, vec![list_ptr], None)
    }

    /// Compute element size in bytes for a given type.
    ///
    /// Used to pass `elem_size` to `ori_list_new` and `ori_list_push`.
    /// Must match `TypeLayoutResolver::type_store_size()` in `ori_llvm`
    /// (sum of field sizes, no alignment padding).
    pub(super) fn compute_elem_size(&self, elem_ty: Idx) -> i64 {
        Self::type_store_size(elem_ty, self.pool, 0)
    }

    fn type_store_size(ty: Idx, pool: &ori_types::Pool, depth: u32) -> i64 {
        super::type_layout::pool_type_store_size(ty, pool, depth)
    }
}
