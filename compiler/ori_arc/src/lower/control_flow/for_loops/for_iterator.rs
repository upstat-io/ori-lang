//! Iterator-based for-loop lowering.

use ori_ir::canon::CanBindingPatternId;
use ori_ir::canon::CanId;
use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};
use crate::lower::expr::{ArcLowerer, LoopContext};

impl ArcLowerer<'_> {
    /// Lower `for x in <iterator> do body` using `__iter_next`.
    ///
    /// Loop structure:
    /// ```text
    /// entry → header
    /// header: next = __iter_next(iter); tag = project(next, 0);
    ///         has_more = (tag != 0); branch(has_more, body, exit)
    /// body: elem = project(next, 1); bind(elem); ... → header
    /// exit: ...
    /// ```
    #[expect(
        clippy::too_many_lines,
        reason = "iterator loop lowering with guard/mutable-var SSA merge is inherently sequential"
    )]
    pub(in crate::lower) fn lower_for_iterator(
        &mut self,
        pattern: CanBindingPatternId,
        iter_val: ArcVarId,
        elem_ty: Idx,
        guard: CanId,
        body: CanId,
    ) -> ArcVarId {
        let header_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let exit_block = self.builder.new_block();
        // Normal exit prep block: Branch can't carry args, so the normal
        // exit path (iterator exhausted) goes header → exit_prep → exit.
        let exit_prep_block = self.builder.new_block();

        // Collect mutable bindings for SSA merge.
        let pre_scope = self.scope.clone();
        let mut mutable_var_names = Vec::new();
        let mut mut_info: Vec<(Name, ArcVarId, Idx)> = Vec::new();
        for (name, var) in pre_scope.mutable_bindings() {
            let var_ty = self.builder.var_type_or_unit(var);
            mutable_var_names.push(name);
            mut_info.push((name, var, var_ty));
        }

        tracing::debug!(
            pattern = ?pattern,
            header_bb = header_block.index(),
            body_bb = body_block.index(),
            exit_bb = exit_block.index(),
            mutable_vars = mut_info.len(),
            has_guard = guard.is_valid(),
            "for_iterator: enter"
        );

        // Header params: mutable vars only (no counter variable).
        let mut header_mut_params = Vec::new();
        for &(name, pre_var, var_ty) in &mut_info {
            let param = self.builder.add_block_param(header_block, var_ty);
            header_mut_params.push((name, pre_var, param));
        }

        // Exit block params: result value (from break) + mutable vars.
        // Matches what lower_break() sends: [break_val, mut0, mut1, ...]
        let result_param = self.builder.add_block_param(exit_block, Idx::UNIT);
        let mut exit_mut_params = Vec::new();
        for &(name, _, var_ty) in &mut_info {
            let param = self.builder.add_block_param(exit_block, var_ty);
            exit_mut_params.push((name, param));
        }

        // Entry jump: pass current mutable var values to header.
        let entry_args: Vec<_> = header_mut_params
            .iter()
            .map(|(_, pre_var, _)| *pre_var)
            .collect();
        self.builder.terminate_jump(header_block, entry_args);

        // Header: call __iter_next(iter) → {i8 has_more, T element}
        self.builder.position_at(header_block);
        self.scope = pre_scope.clone();
        for &(name, _, param_var) in &header_mut_params {
            self.scope.bind_mutable(name, param_var);
        }

        let iter_next_name = self
            .interner
            .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::IterNext.name());
        // Use INT for the result type to suppress ARC RC management on the
        // `{tag, elem}` wrapper struct.  The actual element (accessed via
        // Project at index 1) carries elem_ty and gets correct RC.
        // Pass a zero marker of elem_ty as args[1] so the LLVM emitter can
        // recover the element type for scratch buffer sizing.
        let elem_ty_marker =
            self.builder
                .emit_let(elem_ty, ArcValue::Literal(LitValue::Int(0)), None);
        let next_result = self.builder.emit_apply(
            Idx::INT,
            iter_next_name,
            vec![iter_val, elem_ty_marker],
            None,
        );

        // Tag is field 0: 0 = done, 1 = has element
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
            let skip_args: Vec<_> = header_mut_params
                .iter()
                .map(|&(_, _, param_var)| param_var)
                .collect();
            self.builder.terminate_jump(header_block, skip_args);
        } else {
            self.builder
                .terminate_branch(has_more, body_block, exit_prep_block);
        }

        // Body: extract element and bind.
        self.builder.position_at(body_block);
        let elem = self.builder.emit_project(elem_ty, next_result, 1, None);
        self.bind_for_pattern(pattern, elem, elem_ty);

        let prev_loop = self.loop_ctx.take();
        self.loop_ctx = Some(LoopContext {
            exit_block,
            continue_block: header_block,
            mutable_vars: mutable_var_names,
        });

        self.lower_expr(body);

        if !self.builder.is_terminated() {
            // Jump back to header with updated mutable var values.
            let body_args: Vec<_> = header_mut_params
                .iter()
                .map(|(name, _, _)| self.scope.lookup(*name).unwrap_or_else(|| ArcVarId::new(0)))
                .collect();
            self.builder.terminate_jump(header_block, body_args);
        }

        self.loop_ctx = prev_loop;

        // Exit prep: normal loop exhaustion path. Passes unit (no break
        // value) + current mutable var values to the exit block.
        self.builder.position_at(exit_prep_block);
        let unit_val = self.emit_unit();
        let mut prep_args = vec![unit_val];
        prep_args.extend(header_mut_params.iter().map(|&(_, _, param_var)| param_var));
        self.builder.terminate_jump(exit_block, prep_args);

        // Exit: drop the iterator handle, then restore scope.
        // Both normal exhaustion and break paths converge here.
        self.builder.position_at(exit_block);
        let iter_drop_name = self.interner.intern("ori_iter_drop");
        self.builder
            .emit_apply(Idx::UNIT, iter_drop_name, vec![iter_val], None);

        // Emit a dummy reference to the __for_coll phantom binding (if present)
        // AFTER ori_iter_drop. This keeps the collection variable alive past the
        // iterator drop, so the AIMS pass places the collection's RcDec AFTER
        // ori_iter_drop. This ordering ensures the explicit RcDec (with the real
        // elem_dec_fn) is the one that reaches zero and performs element cleanup,
        // not the iterator's drop (which uses NULL elem_dec_fn).
        // Match any __for_coll_N phantom (unique per nesting level).
        if let Some(&(_, exit_param)) = exit_mut_params
            .iter()
            .find(|(name, _)| self.interner.lookup(*name).starts_with("__for_coll_"))
        {
            // A Let that reads the exit param — keeps it alive for AIMS analysis.
            let coll_ty = self.builder.var_type_or_unit(exit_param);
            self.builder
                .emit_let(coll_ty, ArcValue::Var(exit_param), None);
        }

        self.scope = pre_scope;
        for &(name, param) in &exit_mut_params {
            self.scope.bind_mutable(name, param);
        }
        result_param
    }
}
