//! Function parameter binding and borrowed-rooted variable computation.

use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use ori_arc::{CalleeOwnerDemand, Ownership};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::codegen::abi::{FunctionAbi, ParamPassing, ReturnPassing};

use super::context::EmittedValue;
use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Bind function parameters to LLVM values under their ABI passing modes.
    ///
    /// `Reference` and `Indirect` parameters arrive as pointers; binding loads
    /// their values so ARC IR sees aggregates rather than pointers. The setup
    /// also handles sret slots, non-capturing closure environment offsets, and
    /// selective field loads.
    pub(super) fn bind_function_params(
        &mut self,
        func: &ArcFunction,
        abi: &FunctionAbi,
        pointer_only: &FxHashSet<ArcVarId>,
        used_fields: &FxHashMap<ArcVarId, Option<FxHashSet<u32>>>,
    ) {
        // INVARIANT: Non-capturing closure ABIs reserve a leading environment pointer.
        let has_sret = match abi.return_abi.passing {
            ReturnPassing::Sret { .. } => true,
            ReturnPassing::Direct | ReturnPassing::Void => false,
        };
        let sret_offset = u32::from(has_sret);
        // INVARIANT: Sret forwarding requires identical physical return types.
        if has_sret {
            let return_ty = self.resolve_boundary_type(abi.return_abi.ty);
            self.current_sret = Some((self.builder.get_param(self.current_function, 0), return_ty));
        }
        let phantom_env_offset = u32::from(self.ctx.non_capturing_lambdas.contains(&func.name));
        // A Direct narrowed aggregate emits its conversion here, so it needs
        // the entry block just as a pointer load does.
        let needs_loads = abi.params.iter().any(|p| {
            matches!(
                p.passing,
                ParamPassing::Indirect { .. } | ParamPassing::Reference
            ) || (matches!(p.passing, ParamPassing::Direct)
                && self.type_resolver.is_narrowed_aggregate(p.ty))
        });
        if needs_loads {
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
                    let stored = self.narrow_to_storage(llvm_param, param.ty);
                    self.def_var_repr(param.var, stored, func);
                    llvm_param_idx += 1;
                }
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    let ptr_param = self
                        .builder
                        .get_param(self.current_function, llvm_param_idx);
                    let ty = self.resolve_type(param.ty);

                    if pointer_only.contains(&param.var) {
                        // INVARIANT: Pointer-only parameters never expose their placeholder values.
                        let zero = self.builder.const_zero_ty(ty);
                        self.def_var_repr(param.var, zero, func);
                    } else {
                        let field_set = used_fields.get(&param.var);
                        let remapped_set = field_set.and_then(|opt| {
                            opt.as_ref().map(|set| {
                                set.iter()
                                    .map(|&f| self.remap_struct_field(param.ty, f))
                                    .collect::<FxHashSet<u32>>()
                            })
                        });
                        let loaded = if let Some(selective) = field_set {
                            self.builder.load_struct_selective(
                                ty,
                                ptr_param,
                                remapped_set.as_ref().or(selective.as_ref()),
                                "param.load",
                            )
                        } else {
                            self.builder.load_struct_selective(
                                ty,
                                ptr_param,
                                Some(&FxHashSet::default()),
                                "param.load",
                            )
                        };
                        self.def_var_repr(param.var, loaded, func);
                    }
                    self.borrowed_param_ptrs.insert(param.var, ptr_param);
                    llvm_param_idx += 1;
                }
                ParamPassing::Void => {
                    let zero = self.builder.const_i64(0);
                    self.def_var(param.var, EmittedValue::Immediate(zero));
                }
            }
        }
    }

    /// Compute variables transitively rooted at borrowed parameters.
    ///
    /// Borrowed-rooted inline enums need a sub-pointer increment when stored in
    /// a boxed field because the caller retains its reference. The analysis
    /// follows `Let` aliases and `Jump` block parameters.
    pub(super) fn compute_borrowed_rooted_vars(&mut self, func: &ArcFunction) {
        self.borrowed_rooted_vars.clear();
        self.iter_owns_rooted_vars.clear();
        for param in &func.params {
            if param.ownership == Ownership::Borrowed {
                self.borrowed_rooted_vars.insert(param.var);
            }
        }
        // INVARIANT: Function and contract parameter tables share positional identity.
        if let Some(contract) = self.func_contract {
            for (i, param) in func.params.iter().enumerate() {
                if let Some(pc) = contract.params.get(i) {
                    match pc.callee_owner_demand() {
                        CalleeOwnerDemand::WholeValue => {
                            self.iter_owns_rooted_vars.insert(param.var);
                        }
                        CalleeOwnerDemand::Borrow => {}
                    }
                }
            }
        }
        let mut changed = true;
        while changed {
            changed = false;
            for block in &func.blocks {
                for instr in &block.body {
                    if let ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } = instr
                    {
                        if self.borrowed_rooted_vars.contains(src)
                            && self.borrowed_rooted_vars.insert(*dst)
                        {
                            changed = true;
                        }
                        if self.iter_owns_rooted_vars.contains(src)
                            && self.iter_owns_rooted_vars.insert(*dst)
                        {
                            changed = true;
                        }
                    }
                }
                if let ArcTerminator::Jump { target, args } = &block.terminator {
                    let target_params = &func.blocks[target.index()].params;
                    for (arg, &(param_var, _)) in args.iter().zip(target_params.iter()) {
                        if self.borrowed_rooted_vars.contains(arg)
                            && self.borrowed_rooted_vars.insert(param_var)
                        {
                            changed = true;
                        }
                        if self.iter_owns_rooted_vars.contains(arg)
                            && self.iter_owns_rooted_vars.insert(param_var)
                        {
                            changed = true;
                        }
                    }
                }
            }
        }
    }
}
