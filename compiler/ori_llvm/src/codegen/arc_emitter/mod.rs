//! ARC IR to LLVM IR emission.

mod apply;
mod apply_abi;
mod apply_casts;
mod apply_protocols;
mod apply_resolution;
mod block_label;
pub(crate) mod builtins;
mod catch_thunk;
mod catch_thunk_gen;
mod closure_wrappers;
mod closures;
mod collection_reuse;
mod construction;
pub(crate) mod context;
mod dead_unwind;
mod drop_collections;
mod drop_enum;
mod drop_enum_encodings;
mod drop_gen;
mod element_dec_fn;
mod element_fn_gen;
mod element_inc_fn;
mod emit_function;
mod emit_function_setup;
mod emitted_mappings;
mod emitter;
mod emitter_context;
mod emitter_utils;
mod field_scan;
mod field_walk;
mod instr_dispatch;
mod invoke_terminators;
mod narrowing_codegen;
mod narrowing_local;
mod operators;
mod rc_buffer_ops;
mod rc_data_pointers;
mod rc_enum_payload;
mod rc_enum_values;
mod rc_ops;
mod rc_runtime_calls;
mod rc_value_traversal;
mod representation_access;
mod rpo;
mod runtime_names;
pub(super) mod tag_access;
mod tagless_enum;
mod terminators;
mod value_emission;
mod variant_construction;
mod yield_type_index;

use ori_arc::ir::ArcVarId;
use ori_arc::{ArcClassification, MemoryContract};
use ori_ir::StringInterner;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aot::debug::DebugContext;

use super::ir_builder::IrBuilder;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{BlockId, FunctionId, LLVMTypeId, TokenId, ValueId};
pub use context::CodegenContext;
use context::EmittedValue;
pub use emitter::ArcIrEmitter;
pub(crate) use emitter_context::ArcEmitterFunctionContext;
pub(crate) use narrowing_codegen::narrowed_collection_element_width;
use runtime_names::{FormatRtNames, ListRtNames, StringRuntimeReturnAbi};

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod tests;
