//! Function-level emission for the ARC IR emitter.
//!
//! Contains [`ArcIrEmitter::emit_function`], the main entry point that
//! orchestrates block pre-creation, parameter binding, EH setup, and
//! per-block instruction/terminator emission in reverse post-order.

use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use super::context::EmittedValue;
use super::dead_unwind::debug_assert_dead_unwind_unreachable;
use super::field_scan::{compute_pointer_only_params, scan_used_fields};
use super::ArcIrEmitter;
use super::FuncletPadKind;
use crate::codegen::abi::{FunctionAbi, ParamPassing, ReturnPassing};
use crate::codegen::eh_model::EhModel;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Check if an Invoke callee will be intercepted by a builtin handler.
    ///
    /// Several handler paths in [`Self::emit_invoke`] always emit `call`
    /// regardless of the invoke mode: format calls, prelude builtins, and
    /// builtin type methods. When a callee is intercepted, its unwind block
    /// will never have a predecessor — it's dead code.
    ///
    /// Used by dead unwind detection (to skip creating LLVM blocks) and by
    /// [`Self::emit_invoke`] (to use `Call` mode instead of `Invoke`).
    ///
    /// Delegates to the shared [`super::context::is_callee_intercepted`] free
    /// function for the actual 6-condition check.
    pub(super) fn callee_will_be_intercepted(
        &self,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> bool {
        let callee_name = self.interner.lookup(callee);
        super::context::is_callee_intercepted(
            callee_name,
            callee,
            args,
            func,
            self.ctx,
            self.type_info,
        )
    }

    /// Emit an entire `ArcFunction` as LLVM IR.
    ///
    /// Pre-creates all LLVM blocks, binds function parameters, emits each
    /// block's instructions and terminator, then patches phi nodes.
    #[expect(
        clippy::too_many_lines,
        reason = "function emission orchestrates blocks, params, phis, and terminators"
    )]
    pub fn emit_function(&mut self, func: &ArcFunction, abi: &FunctionAbi) {
        // Pre-scan: find dead unwind blocks. With nounwind analysis,
        // Invoke terminators calling known-nounwind functions are downgraded
        // to `call` + `br`, so their unwind blocks become dead code.
        let unwind_result = self.detect_dead_unwind_blocks(func);
        let dead_unwind = unwind_result.dead;
        let unwind_blocks = unwind_result.live;

        debug_assert_dead_unwind_unreachable(func, &dead_unwind);

        // Pre-create LLVM blocks, skipping only dead unwind blocks.
        //
        // NOTE: Block merging (aliasing single-predecessor normal
        // continuations to their predecessor's LLVM block) was attempted
        // here but is fundamentally incompatible with instructions that
        // create internal LLVM basic blocks (RcInc/RcDec on fat pointers
        // emit inline SSO/null checks that move the builder to internal
        // blocks like `rc_inc.sso_skip`). The self-loop detection in
        // `br_exiting_catchpad` fails when the builder is at an internal
        // block, causing entry blocks to gain predecessors and terminators
        // to appear mid-block. Block merging should instead be done as a
        // pre-emission ARC IR pass.
        // LLVM requires the first appended block to be the function entry.
        // Create the entry block first, then the rest in order.
        let entry_idx = func.entry.index();
        let mut block_map: Vec<Option<_>> = vec![None; func.blocks.len()];
        block_map[entry_idx] = Some(
            self.builder
                .append_block(self.current_function, &format!("bb{entry_idx}")),
        );
        for (i, _) in func.blocks.iter().enumerate() {
            if i == entry_idx || dead_unwind.contains(&i) {
                continue;
            }
            block_map[i] = Some(
                self.builder
                    .append_block(self.current_function, &format!("bb{i}")),
            );
        }
        self.block_map = block_map;

        // Resize var_map to hold all variables
        self.var_map.resize(func.var_types.len(), None);

        // Pre-scan: determine which struct fields are actually used per variable.
        // This enables surgical struct loading — only accessed fields are loaded.
        let used_fields = scan_used_fields(func);

        // Identify parameters whose loaded aggregate value is never needed.
        // These params are only used as Apply/Invoke args where pointer
        // forwarding (via borrowed_param_ptrs) handles everything. Skipping
        // the load eliminates dead `%param.load` instructions in the IR.
        let pointer_only = compute_pointer_only_params(func, |callee, args| {
            let callee_name = self.interner.lookup(callee);
            // Not intercepted → ABI path → pointer forwarding handles args
            if !super::context::is_callee_intercepted(
                callee_name,
                callee,
                args,
                func,
                self.ctx,
                self.type_info,
            ) {
                return true;
            }
            // Intercepted, but str.length/str.len use str_to_ptr_forwarded
            // which checks borrowed_param_ptrs — loaded value not needed.
            if (callee_name == "length" || callee_name == "len") && !args.is_empty() {
                let receiver_ty = func.var_type(args[0]);
                if self.pool.tag(receiver_ty) == ori_types::Tag::Str {
                    return true;
                }
            }
            false
        });

        // Invariant: pointer-only params must not have RcInc/RcDec in the ARC IR.
        // Borrowed params shouldn't get RC ops from the AIMS pipeline. If this
        // fires, the param was incorrectly classified as pointer-only.
        #[cfg(debug_assertions)]
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::RcInc { var, .. } | ArcInstr::RcDec { var, .. } => {
                        debug_assert!(
                            !pointer_only.contains(var),
                            "pointer-only param v{} has RC operation — cannot skip load",
                            var.raw(),
                        );
                    }
                    _ => {}
                }
            }
        }

        // Bind function parameters (respecting ABI passing modes).
        // Reference and Indirect params arrive as pointers — load the actual
        // value so ARC IR sees the struct, not the pointer.
        //
        // Non-capturing lambdas have a phantom `ptr %_env` prepended to their
        // LLVM param list (so they're directly callable as closures). Skip it
        // by adding 1 to the starting index.
        let has_sret = matches!(abi.return_abi.passing, ReturnPassing::Sret { .. });
        let sret_offset = u32::from(has_sret);
        // Register sret pointer for sret forwarding optimization.
        // When the function returns a large struct via sret, the first parameter
        // is the caller-allocated return slot. We can forward this directly to
        // inner call_with_sret calls to avoid intermediate alloca+load+store.
        if has_sret {
            self.current_sret_ptr = Some(self.builder.get_param(self.current_function, 0));
        }
        let phantom_env_offset = u32::from(self.ctx.non_capturing_lambdas.contains(&func.name));
        let needs_loads = abi.params.iter().any(|p| {
            matches!(
                p.passing,
                ParamPassing::Indirect { .. } | ParamPassing::Reference
            )
        });
        if needs_loads {
            // Position at entry block for load instructions
            self.builder.position_at_end(self.block(func.entry));
        }
        let mut llvm_param_idx = sret_offset + phantom_env_offset;
        for (i, param) in func.params.iter().enumerate() {
            let passing = &abi.params[i].passing;
            match passing {
                ParamPassing::Direct => {
                    let llvm_param = self
                        .builder
                        .get_param(self.current_function, llvm_param_idx);
                    self.def_var_repr(param.var, llvm_param, func);
                    llvm_param_idx += 1;
                }
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    let ptr_param = self
                        .builder
                        .get_param(self.current_function, llvm_param_idx);
                    let ty = self.resolve_type(param.ty);

                    if pointer_only.contains(&param.var) {
                        // Parameter's loaded value is never used — all Apply/Invoke
                        // callees forward the pointer via borrowed_param_ptrs.
                        // Bind a zero-init value (no load instruction emitted).
                        let zero = self.builder.const_zero_ty(ty);
                        self.def_var_repr(param.var, zero, func);
                    } else {
                        // Surgical loading: only load fields actually used.
                        // `None` in the map = all fields needed, `Some(set)` = selective.
                        let field_set = used_fields.get(&param.var);
                        let loaded = if let Some(selective) = field_set {
                            self.builder.load_struct_selective(
                                ty,
                                ptr_param,
                                selective.as_ref(),
                                "param.load",
                            )
                        } else {
                            // Variable not in usage map at all — unused param.
                            // Load nothing (zero-init). The aggregate is never read.
                            self.builder.load_struct_selective(
                                ty,
                                ptr_param,
                                Some(&FxHashSet::default()),
                                "param.load",
                            )
                        };
                        self.def_var_repr(param.var, loaded, func);
                    }
                    // Register source pointer for borrowed parameter forwarding.
                    // When this variable is passed to another function that also
                    // expects a pointer, we forward ptr_param directly instead
                    // of alloca+store of the loaded value.
                    self.borrowed_param_ptrs.insert(param.var, ptr_param);
                    llvm_param_idx += 1;
                }
                ParamPassing::Void => {
                    // No physical LLVM param — bind to a zero/unit constant
                    let zero = self.builder.const_i64(0);
                    self.def_var(param.var, EmittedValue::Immediate(zero));
                }
            }
        }

        // Pre-compute set of variables rooted at borrowed parameters.
        // When storing inline enums to boxed fields, borrowed-rooted vars
        // need sub-pointer inc (the caller retains a reference). Consumed
        // (owned) vars don't need it (move semantics).
        self.borrowed_rooted_vars.clear();
        {
            use ori_arc::Ownership;
            for param in &func.params {
                if param.ownership == Ownership::Borrowed {
                    self.borrowed_rooted_vars.insert(param.var);
                }
            }
            // Trace alias chains: Let{Var} + Jump block-param passing.
            let mut changed = true;
            while changed {
                changed = false;
                for block in &func.blocks {
                    // Let { dst, Var(src) } — direct alias
                    for instr in &block.body {
                        if let ArcInstr::Let {
                            dst,
                            value: ori_arc::ir::ArcValue::Var(src),
                            ..
                        } = instr
                        {
                            if self.borrowed_rooted_vars.contains(src)
                                && self.borrowed_rooted_vars.insert(*dst)
                            {
                                changed = true;
                            }
                        }
                    }
                    // Jump { target, args } — args[i] flows to target.params[i]
                    if let ori_arc::ir::ArcTerminator::Jump { target, args } = &block.terminator {
                        let target_params = &func.blocks[target.index()].params;
                        for (arg, &(param_var, _)) in args.iter().zip(target_params.iter()) {
                            if self.borrowed_rooted_vars.contains(arg)
                                && self.borrowed_rooted_vars.insert(param_var)
                            {
                                changed = true;
                            }
                        }
                    }
                }
            }
        }

        // Set personality function on the LLVM function if any real invokes exist.
        // Required for any function containing `invoke`/`landingpad` (Itanium) or
        // `catchswitch`/`catchpad`/`cleanuppad` (SEH).
        //
        // On SEH, catch-type unwind blocks use the `ori_try_call` trampoline
        // (catch_unwind at the runtime level) instead of LLVM `catchpad`.
        // This avoids the "Rust panics must be rethrown" abort on Windows MSVC
        // where Rust detects foreign (non-catch_unwind) exception handlers.
        // Only cleanup-type blocks (Resume terminator) require the personality
        // function on SEH — catch blocks become regular blocks.
        let eh_model = self.builder.eh_model();
        let needs_personality = if eh_model == EhModel::Seh {
            // On SEH, only cleanup blocks (Resume terminator) need personality
            unwind_blocks
                .iter()
                .any(|&idx| matches!(func.blocks[idx].terminator, ArcTerminator::Resume))
        } else {
            !unwind_blocks.is_empty()
        };
        let personality_id = if needs_personality {
            let personality_name = eh_model.personality_name();
            let pid = self.builder.runtime_fn(personality_name);
            self.builder.set_personality(self.current_function, pid);
            Some(pid)
        } else {
            None
        };

        // Position at entry block
        let entry = self.block(func.entry);
        self.builder.position_at_end(entry);

        // Create phi nodes for blocks with parameters (skip dead unwind blocks)
        let mut phi_nodes: Vec<Vec<(ArcVarId, ValueId)>> = Vec::new();
        for block in &func.blocks {
            let mut block_phis = Vec::new();
            if !block.params.is_empty() && !dead_unwind.contains(&block.id.index()) {
                self.builder.position_at_end(self.block(block.id));
                for &(var, ty) in &block.params {
                    let llvm_ty = self.resolve_type(ty);
                    let phi_val = self.builder.phi(llvm_ty, &format!("v{}", var.raw()));
                    self.def_var_repr(var, phi_val, func);
                    block_phis.push((var, phi_val));
                }
            }
            phi_nodes.push(block_phis);
        }

        // Emit each block's body and terminator in Reverse Post-Order (RPO).
        //
        // RPO ensures that a block's dominator (and thus the variable definitions
        // from preceding blocks) is visited first. This is critical after
        // `expand_reuse`, which appends fast/slow/merge blocks at the end of the
        // block array — their Invoke terminators may target existing blocks with
        // lower array indices, creating forward references if emitted in array order.
        //
        // Dead unwind blocks are skipped entirely — no LLVM block was created.
        // Live unwind blocks start with a landing pad (Itanium) or SEH pad (MSVC).
        //
        // **Itanium**:
        // - Cleanup (Resume): `landingpad cleanup` → RC decs → `resume`
        // - Catch (Jump): `landingpad catch null` → `ori_catch_cleanup` → RC decs → br
        //
        // **SEH**:
        // - Cleanup (Resume): `cleanuppad` → RC decs → `cleanupret`
        // - Catch (Jump): NO EH prelude — catch blocks are regular blocks
        //   reached via `ori_try_call` trampoline (see `catch_thunk.rs`)
        let rpo = super::rpo::compute_block_rpo(func, &dead_unwind);
        let mut landingpad_values: FxHashMap<usize, ValueId> = FxHashMap::default();
        for &block_idx in &rpo {
            let block = &func.blocks[block_idx];

            self.builder.position_at_end(self.block(block.id));

            // Clear CSE cache at ARC block boundary. Each ARC block gets
            // an independent cache — the LLVM phi node for loop variables
            // produces new SSA values each iteration, naturally preventing
            // cross-iteration staleness.
            self.builder.clear_cse_cache();

            // Live unwind blocks: emit EH prelude (landingpad or SEH pad).
            // On SEH, catch-type blocks are skipped (handled by ori_try_call).
            if unwind_blocks.contains(&block.id.index()) {
                let is_catch = !matches!(block.terminator, ArcTerminator::Resume);
                let is_seh_catch = eh_model == EhModel::Seh && is_catch;

                if is_seh_catch {
                    // SEH catch blocks use the ori_try_call trampoline.
                    // No catchpad prelude — this is a regular block reached
                    // via conditional branch from the ori_try_call call site.
                    // Call ori_catch_cleanup for consistency (it's a no-op).
                    let func_id = self.builder.runtime_fn("ori_catch_cleanup");
                    let null_exc = self.builder.const_null_ptr();
                    self.builder.call(func_id, &[null_exc], "");
                } else if let Some(pid) = personality_id {
                    match eh_model {
                        EhModel::Itanium => {
                            if is_catch {
                                let lp = self.builder.landingpad_catch_all(pid, "lp.catch");
                                landingpad_values.insert(block.id.index(), lp);
                                let exc_ptr = self.builder.extract_value(lp, 0, "exc.ptr");
                                if let Some(exc_ptr) = exc_ptr {
                                    self.emit_catch_cleanup(exc_ptr);
                                }
                            } else {
                                let lp = self.builder.landingpad(pid, true, "lp");
                                landingpad_values.insert(block.id.index(), lp);
                            }
                        }
                        EhModel::Seh => {
                            // Only cleanup blocks reach here on SEH
                            debug_assert!(!is_catch, "SEH catch blocks handled above");
                            let pad = self.builder.cleanuppad(None, &[]);
                            self.current_funclet_pad = Some((pad, FuncletPadKind::Cleanup));
                        }
                    }
                }
            }

            let mut block_terminated_by_noreturn = false;
            for (instr_idx, instr) in block.body.iter().enumerate() {
                self.current_block_idx = block_idx;
                self.current_instr_idx = instr_idx;
                self.emit_instr(instr, func);

                // After emitting a call to a known-noreturn function,
                // emit `unreachable` and skip remaining instructions +
                // terminator. The callee never returns, so all subsequent
                // code in this block is dead.
                if let ArcInstr::Apply { func: callee, .. } = instr {
                    let callee_str = self.interner.lookup(*callee);
                    if crate::codegen::runtime_decl::runtime_functions::is_rt_fn_noreturn(
                        callee_str,
                    ) == Some(true)
                    {
                        self.builder.unreachable();
                        block_terminated_by_noreturn = true;
                        break;
                    }
                }
            }

            if block_terminated_by_noreturn {
                self.current_funclet_pad = None;
                continue;
            }

            // Set instruction index for terminator: one past the last body
            // instruction, matching the convention in compute_cow_annotations.
            self.current_instr_idx = block.body.len();
            self.emit_terminator(
                &block.terminator,
                block.id,
                &phi_nodes,
                abi,
                &landingpad_values,
                func,
            );

            // Clear funclet pad after terminator emission — next block starts fresh
            self.current_funclet_pad = None;
        }

        // Terminate blocks that RPO didn't visit (unreachable from entry).
        // These blocks were pre-created as LLVM blocks but never filled with
        // instructions. LLVM requires every block to have a terminator.
        {
            let visited: rustc_hash::FxHashSet<usize> = rpo.iter().copied().collect();
            for (i, llvm_block) in self.block_map.iter().enumerate() {
                if let Some(block_id) = llvm_block {
                    if !visited.contains(&i) {
                        self.builder.position_at_end(*block_id);
                        self.builder.unreachable();
                    }
                }
            }
        }

        // Patch phi incoming values
        for &(block_idx, param_idx, value, source_block) in &self.phi_incoming {
            let (_, phi_val) = phi_nodes[block_idx][param_idx];
            self.builder
                .add_phi_incoming(phi_val, &[(value, source_block)]);
        }
    }
}
