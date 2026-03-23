//! Prepared function and lambda types for two-pass nounwind analysis.

use ori_ir::Name;

use crate::codegen::abi::FunctionAbi;
use crate::codegen::value_id::FunctionId;

/// A function fully processed through the ARC pipeline, ready for nounwind
/// analysis and LLVM emission.
///
/// Created by [`FunctionCompiler::prepare_all_cached`] or
/// [`FunctionCompiler::prepare_mono_cached`]. Enables two-pass compilation:
/// 1. Lower all functions to ARC IR (populate this buffer)
/// 2. Analyze nounwind on the complete set ([`FunctionCompiler::compute_nounwind_set`])
/// 3. Emit LLVM IR using the complete nounwind set ([`FunctionCompiler::emit_prepared_functions`])
///
/// This ensures monomorphized callee nounwind status is available when
/// analyzing callers, preventing unnecessary `invoke` + landing pad overhead.
#[derive(Debug)]
pub struct PreparedFunction {
    pub(in crate::codegen::function_compiler) name: Name,
    pub(in crate::codegen::function_compiler) func_id: FunctionId,
    pub(in crate::codegen::function_compiler) abi: FunctionAbi,
    pub(in crate::codegen::function_compiler) arc_func: ori_arc::ArcFunction,
    pub(in crate::codegen::function_compiler) lambdas: Vec<PreparedLambda>,
}

/// A lambda processed through the ARC pipeline, ready for LLVM emission.
///
/// The lambda's LLVM function is already declared and registered in
/// `CodegenContext::functions` during preparation — only the body emission
/// is deferred to [`FunctionCompiler::emit_prepared_functions`].
#[derive(Debug)]
pub(in crate::codegen::function_compiler) struct PreparedLambda {
    pub(in crate::codegen::function_compiler) name: Name,
    pub(in crate::codegen::function_compiler) func_id: FunctionId,
    pub(in crate::codegen::function_compiler) abi: FunctionAbi,
    pub(in crate::codegen::function_compiler) arc_func: ori_arc::ArcFunction,
}
