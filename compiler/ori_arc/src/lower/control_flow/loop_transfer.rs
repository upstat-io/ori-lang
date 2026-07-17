//! Labeled break/continue lowering and abandoned-loop cleanup.

use super::{ArcLowerer, ArcVarId, CanId, Idx};

impl ArcLowerer<'_> {
    // Break / Continue

    /// Lower a `break` expression to ARC IR.
    ///
    /// For-do: exit block expects `[break_value, mut_var_0, mut_var_1, ...]`.
    /// Resolve the loop-context-stack index a break/continue with `label`
    /// targets. `Name::EMPTY` (unlabeled) → innermost (top of stack); a
    /// non-empty label → the nearest enclosing loop whose label matches,
    /// searched top-down. `None` when no enclosing loop matches (the
    /// label-resolution error path — typeck already rejected it as E0871).
    /// Spec: Clause 16.3.3.
    fn resolve_loop_ctx_index(&self, label: ori_ir::Name) -> Option<usize> {
        if label == ori_ir::Name::EMPTY {
            self.loop_ctx_stack.len().checked_sub(1)
        } else {
            self.loop_ctx_stack
                .iter()
                .rposition(|ctx| ctx.label == label)
        }
    }

    /// Drop iterator handles owned by loops skipped by a labeled transfer.
    ///
    /// The target loop remains live for `continue`, and its own exit block
    /// performs the drop for `break`, so only contexts above `target_idx` are
    /// abandoned here. Cleanups run innermost-first.
    fn emit_abandoned_loop_cleanups(&mut self, target_idx: usize) {
        let iter_handles: Vec<_> = self
            .loop_ctx_stack
            .iter()
            .skip(target_idx + 1)
            .rev()
            .filter_map(|ctx| ctx.abandon_iter)
            .collect();
        if iter_handles.is_empty() {
            return;
        }

        let iter_drop_name = self.interner.intern("ori_iter_drop");
        for iter_handle in iter_handles {
            self.builder
                .emit_apply(Idx::UNIT, iter_drop_name, vec![iter_handle], None, None);
        }
    }

    /// For-yield: optionally pushes break value to list, then jumps to exit
    /// with `[mut_var_0, mut_var_1, ...]`.
    pub(crate) fn lower_break(&mut self, value: CanId, label: ori_ir::Name) -> ArcVarId {
        // Resolve which enclosing loop this (possibly labeled) break targets.
        let idx = self.resolve_loop_ctx_index(label);
        // Extract for-yield info before mutable borrows (lower_expr needs &mut self).
        let yield_info = idx
            .and_then(|i| self.loop_ctx_stack.get(i))
            .and_then(|ctx| ctx.yield_ctx.as_ref())
            .map(|yc| (yc.list_ptr, yc.elem_size, yc.list_push_name));

        if let Some((list_ptr, elem_size, push_name)) = yield_info {
            // For-yield break: optionally push value, then jump to exit.
            if value.is_valid() {
                let val = self.lower_expr(value);
                self.builder.emit_apply(
                    Idx::UNIT,
                    push_name,
                    vec![list_ptr, val, elem_size],
                    None,
                    None,
                );
            }
            if let Some(target_idx) = idx {
                self.emit_abandoned_loop_cleanups(target_idx);
            }
            // Re-borrow the matched loop context for jump args (mutable borrows done).
            if let Some(ctx) = idx.and_then(|i| self.loop_ctx_stack.get(i)) {
                let exit_block = ctx.exit_block;
                let mutable_vars = ctx.mutable_vars.clone();
                let mut args: Vec<ArcVarId> = Vec::new();
                for &(name, fallback) in &mutable_vars {
                    args.push(self.scope.lookup(name).unwrap_or(fallback));
                }
                tracing::debug!(
                    exit_bb = exit_block.index(),
                    has_value = value.is_valid(),
                    mutable_args = mutable_vars.len(),
                    "for-yield break: jump to exit"
                );
                self.builder.terminate_jump(exit_block, args);
            }
        } else {
            // For-do break: send break value + mutable vars to exit.
            let break_val = if value.is_valid() {
                self.lower_expr(value)
            } else {
                self.emit_unit()
            };

            if let Some(target_idx) = idx {
                self.emit_abandoned_loop_cleanups(target_idx);
            }

            if let Some(ctx) = idx.and_then(|i| self.loop_ctx_stack.get(i)) {
                let exit_block = ctx.exit_block;
                let mutable_vars = ctx.mutable_vars.clone();
                let mut args = vec![break_val];
                for &(name, fallback) in &mutable_vars {
                    args.push(self.scope.lookup(name).unwrap_or(fallback));
                }
                tracing::debug!(
                    exit_bb = exit_block.index(),
                    break_val = break_val.raw(),
                    mutable_args = args.len() - 1,
                    "break: jump to exit"
                );
                self.builder.terminate_jump(exit_block, args);
            } else {
                tracing::warn!("break outside of loop in ARC IR lowering");
            }
        }

        self.emit_unit()
    }

    /// Lower a `continue` expression to ARC IR.
    ///
    /// For-do: jumps to header with `[mut_var_0, mut_var_1, ...]`.
    /// For-yield: optionally pushes value to list, then jumps to header
    /// with `[mut_var_0, mut_var_1, ...]`.
    pub(crate) fn lower_continue(&mut self, value: CanId, label: ori_ir::Name) -> ArcVarId {
        // Resolve which enclosing loop this (possibly labeled) continue targets.
        let idx = self.resolve_loop_ctx_index(label);
        // Extract for-yield info before mutable borrows (lower_expr needs &mut self).
        let yield_info = idx
            .and_then(|i| self.loop_ctx_stack.get(i))
            .and_then(|ctx| ctx.yield_ctx.as_ref())
            .map(|yc| (yc.list_ptr, yc.elem_size, yc.list_push_name));

        if let Some((list_ptr, elem_size, push_name)) = yield_info {
            // For-yield continue: optionally push value, then jump to header.
            if value.is_valid() {
                let val = self.lower_expr(value);
                self.builder.emit_apply(
                    Idx::UNIT,
                    push_name,
                    vec![list_ptr, val, elem_size],
                    None,
                    None,
                );
            }
            if let Some(target_idx) = idx {
                self.emit_abandoned_loop_cleanups(target_idx);
            }
            // Re-borrow the matched loop context for jump args (mutable borrows done).
            if let Some(ctx) = idx.and_then(|i| self.loop_ctx_stack.get(i)) {
                let continue_block = ctx.continue_block;
                let mutable_vars = ctx.mutable_vars.clone();
                let mut args: Vec<ArcVarId> = Vec::new();
                for &(name, fallback) in &mutable_vars {
                    args.push(self.scope.lookup(name).unwrap_or(fallback));
                }
                tracing::debug!(
                    continue_bb = continue_block.index(),
                    has_value = value.is_valid(),
                    mutable_args = mutable_vars.len(),
                    "for-yield continue: jump to header"
                );
                self.builder.terminate_jump(continue_block, args);
            }
        } else if let Some(target_idx) = idx {
            self.emit_abandoned_loop_cleanups(target_idx);
            let Some(ctx) = self.loop_ctx_stack.get(target_idx) else {
                return self.emit_unit();
            };
            // For-do continue: jump to header with mutable vars only.
            let continue_block = ctx.continue_block;
            let mutable_vars = ctx.mutable_vars.clone();
            let args: Vec<_> = mutable_vars
                .iter()
                .map(|&(name, fallback)| self.scope.lookup(name).unwrap_or(fallback))
                .collect();
            tracing::debug!(
                continue_bb = continue_block.index(),
                mutable_args = args.len(),
                "continue: jump to header"
            );
            self.builder.terminate_jump(continue_block, args);
        } else {
            tracing::warn!("continue outside of loop in ARC IR lowering");
        }

        self.emit_unit()
    }
}
