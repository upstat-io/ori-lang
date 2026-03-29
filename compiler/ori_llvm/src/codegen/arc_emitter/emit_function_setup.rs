//! Function parameter binding and borrowed-rooted variable computation.
//!
//! Extracted from [`super::emit_function`] to keep file sizes under the
//! 500-line limit. Called during the function emission prologue.

use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use ori_arc::Ownership;
use rustc_hash::{FxHashMap, FxHashSet};

use super::context::EmittedValue;
use super::ArcIrEmitter;
use crate::codegen::abi::{FunctionAbi, ParamPassing, ReturnPassing};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Bind function parameters to LLVM values, respecting ABI passing modes.
    ///
    /// Reference and Indirect params arrive as pointers — loads the actual
    /// value so ARC IR sees the struct, not the pointer. Handles sret return
    /// slots, phantom env offsets for non-capturing lambdas, and surgical
    /// struct loading for selective field access.
    pub(super) fn bind_function_params(
        &mut self,
        func: &ArcFunction,
        abi: &FunctionAbi,
        pointer_only: &FxHashSet<ArcVarId>,
        used_fields: &FxHashMap<ArcVarId, Option<FxHashSet<u32>>>,
    ) {
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
    }

    /// Compute set of variables rooted at borrowed parameters.
    ///
    /// When storing inline enums to boxed fields, borrowed-rooted vars need
    /// sub-pointer inc (the caller retains a reference). Consumed (owned) vars
    /// don't need it (move semantics). Traces alias chains through `Let{Var}`
    /// and `Jump` block-parameter passing.
    pub(super) fn compute_borrowed_rooted_vars(&mut self, func: &ArcFunction) {
        self.borrowed_rooted_vars.clear();
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
                        value: ArcValue::Var(src),
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
                if let ArcTerminator::Jump { target, args } = &block.terminator {
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
}
