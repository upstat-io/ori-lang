//! LLVM projection of frozen backend-neutral RL-31 facts.

use super::FunctionCompiler;
use crate::codegen::abi::{FunctionAbi, ParamPassing, ReturnPassing};
use crate::codegen::ir_builder::IrBuilder;
use crate::codegen::value_id::FunctionId;
use ori_ir::Name;

/// Project one already-proven neutral fact into LLVM. Keeping the ablation at
/// this last physical seam ensures it can only remove an attribute; it cannot
/// manufacture or reinterpret a shared RL-31 proof.
fn project_proven_param_noalias(
    builder: &mut IrBuilder<'_, '_>,
    function: FunctionId,
    parameter: u32,
) {
    if !super::rl31_noalias_disabled() {
        builder.add_noalias_attribute(function, parameter);
    }
}

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
        if !nounwind {
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
                        project_proven_param_noalias(self.builder, func_id, llvm_index);
                    }
                    llvm_index += 1;
                }
                ParamPassing::Void => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem::ManuallyDrop;

    use inkwell::context::Context;

    use super::project_proven_param_noalias;
    use crate::codegen::ir_builder::IrBuilder;
    use crate::context::SimpleCx;

    fn emit_proven_noalias_projection_ir() -> String {
        let context = Context::create();
        let simple = ManuallyDrop::new(SimpleCx::new(&context, "rl31_projection"));
        let mut builder = IrBuilder::new(&simple);
        let pointer = builder.ptr_type();
        let return_type = builder.i64_type();
        let function = builder.declare_function("rl31_target", &[pointer], return_type);

        project_proven_param_noalias(&mut builder, function, 0);

        drop(builder);
        simple.llmod.print_to_string().to_string()
    }

    fn target_declaration(ir: &str) -> &str {
        ir.lines()
            .find(|line| line.contains("@rl31_target("))
            .unwrap_or_else(|| panic!("missing rl31_target declaration:\n{ir}"))
    }

    #[test]
    fn proven_rl31_parameter_projects_noalias_by_default() {
        let ir = emit_proven_noalias_projection_ir();

        assert!(
            target_declaration(&ir).contains("noalias"),
            "a proven RL-31 pointer parameter must project noalias:\n{ir}"
        );
    }

    #[test]
    fn rl31_noalias_toggle_reproduces_attribute_omission() {
        const CHILD: &str = "ORI_RL31_BEHAVIOR_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(
                std::env::current_exe().expect("test executable path must be available"),
            )
            .arg("rl31_noalias_toggle_reproduces_attribute_omission")
            .arg("--nocapture")
            .env(CHILD, "1")
            .env("ORI_DISABLE_RL31_NOALIAS", "1")
            .output()
            .expect("RL-31 behavior child must start");
            assert!(
                output.status.success(),
                "RL-31 behavior child failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            return;
        }

        let ir = emit_proven_noalias_projection_ir();

        assert!(
            !target_declaration(&ir).contains("noalias"),
            "the ablation must omit noalias even for a proven RL-31 parameter:\n{ir}"
        );
    }
}
