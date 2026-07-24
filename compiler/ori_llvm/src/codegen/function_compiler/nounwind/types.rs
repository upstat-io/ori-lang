//! Prepared function and lambda types for two-pass nounwind analysis.

use ori_ir::Name;

use crate::codegen::abi::FunctionAbi;
use crate::codegen::value_id::FunctionId;

/// ARC function buffered until whole-module nounwind analysis completes.
///
/// This ordering exposes monomorphized callee status before LLVM emission,
/// avoiding unnecessary `invoke` instructions and landing pads.
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
/// Construction and consumption stay inside nounwind analysis, making LLVM
/// emission before the fixed point unrepresentable.
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

/// Prepared lambda whose LLVM declaration is registered before body emission.
#[derive(Debug)]
pub(in crate::codegen::function_compiler) struct PreparedLambda {
    pub(in crate::codegen::function_compiler) name: Name,
    pub(in crate::codegen::function_compiler) func_id: FunctionId,
    pub(in crate::codegen::function_compiler) abi: FunctionAbi,
    pub(in crate::codegen::function_compiler) arc_func: ori_arc::ArcFunction,
}
