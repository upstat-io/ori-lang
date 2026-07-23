//! Function-level emission for the ARC IR emitter.
//!
//! Contains [`ArcIrEmitter::emit_function`], the main entry point that
//! orchestrates block pre-creation, parameter binding, EH setup, and
//! per-block instruction/terminator emission in reverse post-order.

use super::block_label::BlockLabel;
use super::context::EmittedValue;
use super::dead_unwind::assert_dead_unwind_unreachable;
use super::field_scan::{compute_pointer_only_params, scan_used_fields};
use super::yield_type_index::index_yield_types_by_elem_size_var;
use super::ArcIrEmitter;
use crate::codegen::abi::FunctionAbi;
use crate::codegen::eh_model::EhModel;
use crate::codegen::value_id::{FunctionId, ValueId};
use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Reports whether builtin handling converts an `Invoke` callee to `call`.
    /// Intercepted callees leave their unwind blocks without predecessors, so
    /// dead-unwind classification omits those blocks.
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
    pub fn emit_function(&mut self, func: &ArcFunction, abi: &FunctionAbi) {
        let (dead_unwind, unwind_blocks) = self.classify_unwind_blocks(func);
        self.create_function_blocks(func, &dead_unwind);
        self.prepare_function_inputs(func, abi);
        let (eh_model, personality) = self.install_function_personality(func, &unwind_blocks);

        let entry = self.block(func.entry);
        self.builder.position_at_end(entry);
        self.compute_narrowed_vars(func);
        let phi_nodes = self.create_function_phi_nodes(func, &dead_unwind);
        self.emit_function_blocks(
            func,
            abi,
            &dead_unwind,
            &unwind_blocks,
            eh_model,
            personality,
            &phi_nodes,
        );
        self.patch_function_phi_nodes(&phi_nodes);
    }

    fn classify_unwind_blocks(&self, func: &ArcFunction) -> (FxHashSet<usize>, FxHashSet<usize>) {
        let result = self.detect_dead_unwind_blocks(func);
        let mut dead = result.dead;
        let mut live = result.live;
        for &(_, handler) in &func.catch_scoped_checked_ops {
            let index = handler.index();
            if index < func.blocks.len() {
                live.insert(index);
                dead.remove(&index);
            }
        }
        assert_dead_unwind_unreachable(func, &dead);
        (dead, live)
    }

    fn create_function_blocks(&mut self, func: &ArcFunction, dead_unwind: &FxHashSet<usize>) {
        let entry_index = func.entry.index();
        let mut block_map = vec![None; func.blocks.len()];
        let entry_label = BlockLabel::new(entry_index);
        block_map[entry_index] = Some(
            self.builder
                .append_block(self.current_function, entry_label.as_str()),
        );

        for (index, _) in func.blocks.iter().enumerate() {
            if index == entry_index || dead_unwind.contains(&index) {
                continue;
            }
            let label = BlockLabel::new(index);
            block_map[index] = Some(
                self.builder
                    .append_block(self.current_function, label.as_str()),
            );
        }
        self.block_map = block_map;
    }

    fn prepare_function_inputs(&mut self, func: &ArcFunction, abi: &FunctionAbi) {
        self.same_frame_catch_landing_pads.clear();
        for &(checked_var, handler) in &func.catch_scoped_checked_ops {
            self.same_frame_catch_landing_pads
                .insert(checked_var, self.block(handler));
        }
        self.var_map.resize(func.var_types.len(), None);
        self.yield_lineages = ori_arc::YieldLineageIndex::for_function(func);
        self.yield_types_by_elem_size_var = index_yield_types_by_elem_size_var(func);

        let used_fields = scan_used_fields(func);
        let pointer_only = self.find_pointer_only_params(func);
        assert_pointer_only_params_have_no_rc(func, &pointer_only);
        self.bind_function_params(func, abi, &pointer_only, &used_fields);
        self.pointer_only_params = pointer_only;
        self.compute_borrowed_rooted_vars(func);
    }

    fn find_pointer_only_params(&mut self, func: &ArcFunction) -> FxHashSet<ArcVarId> {
        let length_name = self.interner.intern("length");
        let len_name = self.interner.intern("len");
        compute_pointer_only_params(func, |dst, callee, args| {
            let callee_name = self.interner.lookup(callee);
            let is_string_length_call = || {
                let Some(&receiver) = args.first() else {
                    return false;
                };
                (callee == length_name || callee == len_name)
                    && self.pool.tag(func.var_type(receiver)) == ori_types::Tag::Str
            };
            if self.ctx.executable_facts_bound {
                match self.ctx.executable_call_targets.get(&(func.name, dst)) {
                    Some(
                        ori_repr::executable::CallableTarget::Function(_)
                        | ori_repr::executable::CallableTarget::External(_),
                    ) => return true,

                    Some(ori_repr::executable::CallableTarget::Runtime(_)) | None => {
                        return is_string_length_call();
                    }
                }
            }
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
            is_string_length_call()
        })
    }

    fn install_function_personality(
        &mut self,
        func: &ArcFunction,
        unwind_blocks: &FxHashSet<usize>,
    ) -> (EhModel, Option<FunctionId>) {
        let eh_model = self.builder.eh_model();
        let needs_personality = if eh_model == EhModel::Seh {
            unwind_blocks
                .iter()
                .any(|&index| matches!(func.blocks[index].terminator, ArcTerminator::Resume))
        } else {
            !unwind_blocks.is_empty()
        };

        let personality = needs_personality.then(|| {
            let id = self.builder.runtime_fn(eh_model.personality_name());
            self.builder.set_personality(self.current_function, id);
            id
        });
        (eh_model, personality)
    }

    fn create_function_phi_nodes(
        &mut self,
        func: &ArcFunction,
        dead_unwind: &FxHashSet<usize>,
    ) -> Vec<Vec<(ArcVarId, ValueId)>> {
        let mut phi_nodes = Vec::with_capacity(func.blocks.len());
        let mut narrowed = Vec::new();
        for block in &func.blocks {
            let mut block_phis = Vec::new();
            if !block.params.is_empty() && !dead_unwind.contains(&block.id.index()) {
                self.builder.position_at_end(self.block(block.id));
                for &(var, ty) in &block.params {
                    if let Some(&width) = self.narrowed_vars.get(&var) {
                        let narrow_ty = self.llvm_type_for_int_width(width);
                        let phi = self.builder.phi(narrow_ty, "phi.narrow");
                        narrowed.push((var, phi, block.id));
                        block_phis.push((var, phi));
                    } else {
                        let llvm_ty = self.resolve_type(ty);
                        let phi = self.builder.phi(llvm_ty, "phi");
                        self.def_var_repr(var, phi, func);
                        block_phis.push((var, phi));
                    }
                }
            }
            phi_nodes.push(block_phis);
        }

        for (var, phi, block_id) in narrowed {
            self.builder.position_at_end(self.block(block_id));
            let i64_ty = self.builder.i64_type();
            let widened = self.builder.sext(phi, i64_ty, "phi.wide");
            self.def_var(var, EmittedValue::Immediate(widened));
        }
        phi_nodes
    }

    fn emit_function_blocks(
        &mut self,
        func: &ArcFunction,
        abi: &FunctionAbi,
        dead_unwind: &FxHashSet<usize>,
        unwind_blocks: &FxHashSet<usize>,
        eh_model: EhModel,
        personality: Option<FunctionId>,
        phi_nodes: &[Vec<(ArcVarId, ValueId)>],
    ) {
        let catch_roots = func
            .catch_scoped_checked_ops
            .iter()
            .map(|&(_, handler)| handler.index())
            .collect::<Vec<_>>();
        let rpo = super::rpo::compute_block_rpo(func, dead_unwind, &catch_roots);
        let mut landingpad_values = FxHashMap::default();
        let errors_at_start = self.builder.codegen_error_count();

        for &block_index in &rpo {
            let block = &func.blocks[block_index];
            self.builder.position_at_end(self.block(block.id));
            if self.builder.codegen_error_count() > errors_at_start {
                self.builder.unreachable();
                continue;
            }
            self.builder.clear_cse_cache();
            self.emit_unwind_prelude(
                func,
                block_index,
                unwind_blocks,
                eh_model,
                personality,
                &mut landingpad_values,
            );

            if self.emit_block_instructions(func, block_index) {
                self.current_cleanup_pad = None;
                continue;
            }

            self.current_instr_idx = block.body.len();
            self.emit_terminator(
                &block.terminator,
                block.id,
                phi_nodes,
                abi,
                &landingpad_values,
                func,
            );
            self.current_cleanup_pad = None;
        }
        self.terminate_unvisited_blocks(&rpo);
    }

    fn emit_unwind_prelude(
        &mut self,
        func: &ArcFunction,
        block_index: usize,
        unwind_blocks: &FxHashSet<usize>,
        eh_model: EhModel,
        personality: Option<FunctionId>,
        landingpad_values: &mut FxHashMap<usize, ValueId>,
    ) {
        let block = &func.blocks[block_index];
        if !unwind_blocks.contains(&block.id.index()) {
            return;
        }

        let is_catch = !matches!(block.terminator, ArcTerminator::Resume);
        if eh_model == EhModel::Seh && is_catch {
            let cleanup = self.builder.runtime_fn("ori_catch_cleanup");
            let null_exception = self.builder.const_null_ptr();
            self.builder.call(cleanup, &[null_exception], "");
            return;
        }
        let Some(personality) = personality else {
            return;
        };
        match eh_model {
            EhModel::Itanium if is_catch => {
                let pad = self.builder.landingpad_catch_all(personality, "lp.catch");
                landingpad_values.insert(block.id.index(), pad);
                if let Some(exception) = self.builder.extract_value(pad, 0, "exc.ptr") {
                    self.emit_catch_cleanup(exception);
                }
            }

            EhModel::Itanium => {
                let pad = self.builder.landingpad(personality, true, "lp");
                landingpad_values.insert(block.id.index(), pad);
            }

            EhModel::Seh => {
                let pad = self.builder.cleanuppad(None, &[]);
                self.current_cleanup_pad = Some(pad);
            }
        }
    }

    fn emit_block_instructions(&mut self, func: &ArcFunction, block_index: usize) -> bool {
        let block = &func.blocks[block_index];
        let errors_at_start = self.builder.codegen_error_count();
        for (instr_index, instr) in block.body.iter().enumerate() {
            self.current_block_idx = block_index;
            self.current_instr_idx = instr_index;
            self.emit_instr(instr, func);

            if self.builder.codegen_error_count() > errors_at_start {
                self.builder.unreachable();
                return true;
            }
            if let ArcInstr::Apply { func: callee, .. } = instr {
                let callee_name = self.interner.lookup(*callee);
                if crate::codegen::runtime_decl::runtime_functions::is_rt_fn_noreturn(callee_name)
                    == Some(true)
                {
                    self.builder.unreachable();
                    return true;
                }
            }
        }
        false
    }

    fn terminate_unvisited_blocks(&mut self, rpo: &[usize]) {
        let visited = rpo.iter().copied().collect::<FxHashSet<_>>();
        for (index, llvm_block) in self.block_map.iter().enumerate() {
            if let Some(block_id) = llvm_block {
                if !visited.contains(&index) {
                    self.builder.position_at_end(*block_id);
                    self.builder.unreachable();
                }
            }
        }
    }

    fn patch_function_phi_nodes(&mut self, phi_nodes: &[Vec<(ArcVarId, ValueId)>]) {
        for &(block_index, param_index, value, source_block) in &self.phi_incoming {
            let (_, phi) = phi_nodes[block_index][param_index];
            self.builder.add_phi_incoming(phi, &[(value, source_block)]);
        }
    }
}

/// Panics if load elision would hide a direct ARC instruction on a parameter.
pub(super) fn assert_pointer_only_params_have_no_rc(
    func: &ArcFunction,
    pointer_only: &FxHashSet<ArcVarId>,
) {
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::RcInc { var, .. } | ArcInstr::RcDec { var, .. } = instr {
                assert!(
                    !pointer_only.contains(var),
                    "pointer-only param v{} has RC operation — cannot skip load",
                    var.raw(),
                );
            }
        }
    }
}
