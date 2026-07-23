//! Two-pass function compilation for V2 codegen.

mod accessors;
mod artifact_projection;
mod compiler;
mod declarations;
mod define_phase;
mod derive_methods;
mod effect_projection;
mod entry_point;
mod error_ctor;
mod impls;
mod lambda_rewrite;
mod length_projection;
mod nounwind;
mod panic_trampoline;
mod return_projection;
mod rl31_projection;
mod seh_main_thunk;
mod shared_seam;
mod test_wrappers;

use ori_arc::{AnnotatedSig, ArcClassifier, MemoryContract};
use ori_ir::{Name, Span, StringInterner};
use ori_types::{FunctionSig, Idx, Pool};
use rustc_hash::FxHashMap;
use tracing::warn;

#[cfg(test)]
use ori_ir::Function;

use crate::aot::debug::DebugContext;
use crate::aot::mangle::Mangler;

#[cfg(test)]
use super::abi::CallConv;
use super::abi::{compute_function_abi, FunctionAbi, ParamPassing, ReturnPassing};
use super::arc_emitter::CodegenContext;
use super::ir_builder::IrBuilder;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{FunctionId, LLVMTypeId, ValueId};

pub(super) use compiler::rl31_noalias_disabled;
pub use compiler::FunctionCompiler;
pub use nounwind::{NounwindAnalyzedFunctions, PreparedFunction};

#[cfg(test)]
mod tests;
