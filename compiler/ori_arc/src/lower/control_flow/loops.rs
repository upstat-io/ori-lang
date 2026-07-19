//! Loop lowering — `loop`, `for` (range, option, iterator).
//!
//! Each loop variant produces a distinct block structure with SSA block
//! parameters for mutable variable flow-through. The `LoopContext` tracks
//! exit/continue targets so `break` and `continue` (in `mod.rs`) can
//! emit jumps to the correct blocks.
//!
//! [`for_loops`](super::for_loops) implements iterator, option, and range
//! variants.

use ori_ir::canon::CanId;
use ori_types::Idx;

use crate::ir::{ArcVarId, MethodCallForm};

use super::super::expr::{ArcLowerer, ForLoop, ForYieldShape, LoopContext};

impl ArcLowerer<'_> {
    // Loop

    /// Lower `Loop { body }` — infinite loop with break/continue.
    pub(crate) fn lower_loop(&mut self, body: CanId, ty: Idx, label: ori_ir::Name) -> ArcVarId {
        let header_block = self.builder.new_block();
        let exit_block = self.builder.new_block();

        let pre_scope = self.scope.clone();
        let mut header_params = Vec::new();

        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = self.builder.var_type(var);
            let param_var = self.builder.add_block_param(header_block, var_ty);
            header_params.push((name, var, param_var));
        }

        tracing::debug!(
            header_bb = header_block.index(),
            exit_bb = exit_block.index(),
            mutable_vars = header_params.len(),
            "loop: enter"
        );
        for &(name, pre_var, param_var) in &header_params {
            tracing::trace!(
                name = self.name_str(name),
                entry_var = pre_var.raw(),
                header_param = param_var.raw(),
                "loop: header param"
            );
        }

        let entry_args: Vec<_> = header_params.iter().map(|(_, var, _)| *var).collect();
        self.builder.terminate_jump(header_block, entry_args);

        self.builder.position_at(header_block);
        self.scope = pre_scope.clone();
        for (name, _, param_var) in &header_params {
            self.scope.bind_mutable(*name, *param_var);
        }

        let mutable_var_entries: Vec<_> = header_params
            .iter()
            .map(|&(name, _, param)| (name, param))
            .collect();
        self.loop_ctx_stack.push(LoopContext {
            label,
            exit_block,
            continue_block: header_block,
            mutable_vars: mutable_var_entries,
            abandon_iter: None,
            yield_ctx: None,
        });

        self.lower_expr(body);

        if self.builder.is_terminated() {
            tracing::debug!("loop: body terminated (all paths break/continue)");
        } else {
            let continue_args: Vec<_> = header_params
                .iter()
                .map(|&(name, _, param)| self.scope.lookup(name).unwrap_or(param))
                .collect();
            for (i, &(name, _, param)) in header_params.iter().enumerate() {
                tracing::trace!(
                    name = self.name_str(name),
                    header_param = param.raw(),
                    updated_var = continue_args[i].raw(),
                    changed = (param.raw() != continue_args[i].raw()),
                    "loop: fall-through arg"
                );
            }
            self.builder.terminate_jump(header_block, continue_args);
        }

        self.loop_ctx_stack.pop();

        // Exit block: result value + mutable var params so post-loop code
        // sees the final values of mutable variables (not pre-loop values).
        self.builder.position_at(exit_block);
        let result_param = self.builder.add_block_param(exit_block, ty);
        self.scope = pre_scope;
        for &(name, pre_var, _) in &header_params {
            let var_ty = self.builder.var_type(pre_var);
            let exit_param = self.builder.add_block_param(exit_block, var_ty);
            tracing::trace!(
                name = self.name_str(name),
                exit_param = exit_param.raw(),
                "loop: exit param"
            );
            self.scope.bind_mutable(name, exit_param);
        }
        tracing::debug!(
            result = result_param.raw(),
            exit_bb = exit_block.index(),
            "loop: exit"
        );
        result_param
    }

    // For

    /// Lower `For { binding, iter, guard, body }` — range iteration.
    ///
    /// Produces 4+ blocks: header, body, latch, exit (plus guard blocks).
    /// Mutable variables from the enclosing scope flow through the loop
    /// as block parameters on header and latch blocks, ensuring SSA
    /// dominance at the exit point.
    ///
    /// Block param layout:
    /// - header: `[i_var, mut0, mut1, ...]`
    /// - latch:  `[mut0, mut1, ...]` (`i_var` from header dominates latch)
    pub(crate) fn lower_for(&mut self, for_loop: ForLoop) -> ArcVarId {
        let ForLoop {
            pattern,
            iter,
            guard,
            body,
            ty,
            is_yield,
            label,
        } = for_loop;
        let iter_val = self.lower_expr(iter);
        let iter_ty = self.expr_type(iter);
        let tag = self.pool.tag(iter_ty);
        tracing::debug!(
            pattern = ?pattern,
            ?tag,
            is_yield,
            has_guard = guard.is_valid(),
            "for: enter"
        );

        if is_yield {
            let shape = ForYieldShape {
                pattern,
                guard,
                body,
                result_ty: ty,
            };
            return self.lower_for_yield(shape, iter_val, iter_ty, tag, label);
        }

        if tag == ori_types::Tag::Range {
            self.lower_for_range(pattern, iter_val, iter_ty, guard, body, label)
        } else if tag == ori_types::Tag::Option {
            let elem_ty = self.pool.option_inner(iter_ty);
            self.lower_for_option(pattern, iter_val, elem_ty, guard, body, label)
        } else if tag.is_iterator() {
            let elem_ty = self.pool.iterator_elem(iter_ty);
            self.lower_for_iterator(pattern, iter_val, elem_ty, guard, body, label)
        } else {
            let elem_ty = self.extract_iterable_elem_type(tag, iter_ty);

            let iter_name = self
                .interner
                .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
            // INVARIANT: Iterator handles are opaque pointers released by `ori_iter_drop`.
            let iter_result =
                self.builder
                    .emit_apply(Idx::INT, iter_name, vec![iter_val], None, None);
            self.builder
                .note_method_call(iter_result, iter_ty, MethodCallForm::Instance);
            self.lower_for_iterator(pattern, iter_result, elem_ty, guard, body, label)
        }
    }

    /// Extract the element type from an iterable's type tag.
    ///
    /// Used for types that go through `.iter()` → `__iter_next`.
    /// Option and Range are handled by dedicated lowering paths.
    fn extract_iterable_elem_type(&self, tag: ori_types::Tag, iter_ty: Idx) -> Idx {
        match tag {
            ori_types::Tag::List => self.pool.list_elem(iter_ty),
            ori_types::Tag::Set => self.pool.set_elem(iter_ty),
            ori_types::Tag::Str => Idx::CHAR,
            ori_types::Tag::Map => {
                // Map iteration yields (key, value) tuples.
                // The type checker already created this tuple type during
                // `infer_for` — look it up in the pool without mutating.
                let key_ty = self.pool.map_key(iter_ty);
                let val_ty = self.pool.map_value(iter_ty);
                self.pool.find_tuple(&[key_ty, val_ty]).unwrap_or(Idx::INT)
            }
            _ => Idx::INT,
        }
    }
}
