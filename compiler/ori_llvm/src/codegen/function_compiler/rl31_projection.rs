//! LLVM projection of frozen backend-neutral RL-31 facts.

use super::FunctionCompiler;
use crate::codegen::abi::{FunctionAbi, ParamPassing, ReturnPassing};
use crate::codegen::value_id::FunctionId;
use ori_ir::Name;

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Attach LLVM parameter attributes only from the closed shared artifact.
    pub(super) fn apply_rl31_param_attributes(
        &mut self,
        name: Name,
        func_id: FunctionId,
        abi: &FunctionAbi,
        nounwind: bool,
        extra_leading_params: u32,
    ) {
        if *super::RL31_NOALIAS_DISABLED || !nounwind {
            return;
        }
        let Some(function) = self.bound_executable_function_id(name, abi) else {
            return;
        };
        let program = self
            .executable_program
            .expect("bound executable identity requires an executable program");
        let facts = program.param_disjointness(function);
        if facts.type_disjointness().len() != abi.params.len() {
            return;
        }
        let mut llvm_index =
            u32::from(matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }))
                + extra_leading_params;
        for (param_index, param) in abi.params.iter().enumerate() {
            match param.passing {
                ParamPassing::Direct => llvm_index += 1,
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    if facts.proves_disjoint(param_index) {
                        self.builder.add_noalias_attribute(func_id, llvm_index);
                    }
                    llvm_index += 1;
                }
                ParamPassing::Void => {}
            }
        }
    }
}
