//! Prepared function and lambda types for two-pass nounwind analysis.

use ori_ir::Name;

use crate::codegen::abi::FunctionAbi;
use crate::codegen::value_id::FunctionId;

/// A function fully processed through the ARC pipeline, ready for nounwind
/// analysis and LLVM emission.
///
/// Created by
/// [`FunctionCompiler::prepare_all_from_artifact`](crate::codegen::function_compiler::FunctionCompiler::prepare_all_from_artifact)
/// or
/// [`FunctionCompiler::prepare_mono_from_artifact`](crate::codegen::function_compiler::FunctionCompiler::prepare_mono_from_artifact).
/// Enables two-pass compilation:
/// 1. Lower all functions to ARC IR (populate this buffer)
/// 2. Analyze nounwind on the complete set
///    ([`FunctionCompiler::compute_nounwind_set`](crate::codegen::function_compiler::FunctionCompiler::compute_nounwind_set))
/// 3. Emit LLVM IR using the complete nounwind set
///    ([`FunctionCompiler::emit_prepared_functions`](crate::codegen::function_compiler::FunctionCompiler::emit_prepared_functions))
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

/// Prepared functions whose complete nounwind fixed point has been computed.
///
/// Construction is confined to the nounwind implementation. Public callers
/// obtain values from
/// [`FunctionCompiler::compute_nounwind_set`](crate::codegen::function_compiler::FunctionCompiler::compute_nounwind_set)
/// and are consumed by
/// [`FunctionCompiler::emit_prepared_functions`](crate::codegen::function_compiler::FunctionCompiler::emit_prepared_functions),
/// making emission before nounwind analysis unrepresentable.
#[derive(Debug)]
#[must_use = "nounwind analysis must be consumed by LLVM emission"]
pub struct NounwindAnalyzedFunctions(Vec<PreparedFunction>);

impl NounwindAnalyzedFunctions {
    pub(in crate::codegen::function_compiler::nounwind) fn new(
        prepared: Vec<PreparedFunction>,
    ) -> Self {
        Self(prepared)
    }

    pub(in crate::codegen::function_compiler::nounwind) fn into_prepared(
        self,
    ) -> Vec<PreparedFunction> {
        self.0
    }
}

/// A lambda processed through the ARC pipeline, ready for LLVM emission.
///
/// The lambda's LLVM function is already declared and registered in
/// `CodegenContext::functions` during preparation — only the body emission
/// is deferred to
/// [`FunctionCompiler::emit_prepared_functions`](crate::codegen::function_compiler::FunctionCompiler::emit_prepared_functions).
#[derive(Debug)]
pub(in crate::codegen::function_compiler) struct PreparedLambda {
    pub(in crate::codegen::function_compiler) name: Name,
    pub(in crate::codegen::function_compiler) func_id: FunctionId,
    pub(in crate::codegen::function_compiler) abi: FunctionAbi,
    pub(in crate::codegen::function_compiler) arc_func: ori_arc::ArcFunction,
}
