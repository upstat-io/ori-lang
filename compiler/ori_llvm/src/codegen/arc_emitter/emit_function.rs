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
use crate::codegen::abi::FunctionAbi;
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

        // §04.4 Phase C: pre-scan for-yield loops to find which elem_size
        // ArcVarIds belong to int-element accumulators. Only those are safe
        // to override with narrowed sizes.
        if self.narrowed_int_collection_element_width().is_some() {
            self.for_yield_int_elem_sizes =
                scan_for_yield_int_elem_sizes(func, self.pool, self.interner);
        }

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

        // Bind function parameters and compute borrowed-rooted variables.
        self.bind_function_params(func, abi, &pointer_only, &used_fields);
        self.compute_borrowed_rooted_vars(func);

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

        // §04.4 Phase B: compute which local int variables can be narrowed.
        self.compute_narrowed_vars(func);

        // Create phi nodes for blocks with parameters (skip dead unwind blocks).
        // Two-pass approach: (1) create all phis first to satisfy LLVM's invariant
        // that ALL phi nodes must be grouped at the top of a block, then (2) emit
        // sext instructions for narrowed phis.
        let mut phi_nodes: Vec<Vec<(ArcVarId, ValueId)>> = Vec::new();
        // Track narrowed phis for deferred sext emission: (var, phi_val, block_id)
        let mut narrowed_phis: Vec<(ArcVarId, ValueId, ori_arc::ir::ArcBlockId)> = Vec::new();
        for block in &func.blocks {
            let mut block_phis = Vec::new();
            if !block.params.is_empty() && !dead_unwind.contains(&block.id.index()) {
                self.builder.position_at_end(self.block(block.id));
                for &(var, ty) in &block.params {
                    // §04.4 Phase B: use narrow type for narrowed int phis
                    if let Some(&width) = self.narrowed_vars.get(&var) {
                        let narrow_ty = self.llvm_type_for_int_width(width);
                        let phi_val = self.builder.phi(narrow_ty, &format!("v{}.n", var.raw()));
                        // Defer sext to after ALL phis are created (LLVM phi grouping)
                        narrowed_phis.push((var, phi_val, block.id));
                        block_phis.push((var, phi_val));
                    } else {
                        let llvm_ty = self.resolve_type(ty);
                        let phi_val = self.builder.phi(llvm_ty, &format!("v{}", var.raw()));
                        self.def_var_repr(var, phi_val, func);
                        block_phis.push((var, phi_val));
                    }
                }
            }
            phi_nodes.push(block_phis);
        }

        // §04.4 Phase B: emit sext instructions AFTER all phis are created.
        // Position after the last phi in each block, then emit sext.
        for (var, phi_val, block_id) in narrowed_phis {
            self.builder.position_at_end(self.block(block_id));
            let i64_ty = self.builder.i64_type();
            let sext_val = self
                .builder
                .sext(phi_val, i64_ty, &format!("v{}", var.raw()));
            self.def_var(var, EmittedValue::Immediate(sext_val));
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
            let visited: FxHashSet<usize> = rpo.iter().copied().collect();
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

/// Pre-scan: find `elem_size` `ArcVarId`s used in `ori_list_push` calls where
/// the pushed element type is `Tag::Int`. These are the for-yield accumulators
/// whose `elem_size` is safe to override with narrowed widths.
///
/// The for-yield lowerer shares the same `elem_size_var` between `ori_list_new`
/// and `ori_list_push`, so finding it in push identifies the corresponding new.
fn scan_for_yield_int_elem_sizes(
    func: &ArcFunction,
    pool: &ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    use ori_types::Tag;

    let mut result = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                let name = interner.lookup(*callee);
                // ori_list_push(list_ptr, elem_val, elem_size_var)
                if name == "ori_list_push" && args.len() == 3 {
                    let elem_ty = func.var_type(args[1]);
                    let resolved = pool.resolve_fully(elem_ty);
                    if pool.tag(resolved) == Tag::Int {
                        result.insert(args[2]);
                    }
                }
            }
        }
    }
    result
}
