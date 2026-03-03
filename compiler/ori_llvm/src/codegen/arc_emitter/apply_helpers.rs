//! ABI parameter passing and type coercion helpers for function call emission.
//!
//! These helpers bridge the gap between ARC IR's uniform value representation
//! and LLVM's ABI requirements:
//!
//! - **Parameter passing**: `Indirect`/`Reference` params are spilled to alloca
//!   and passed by pointer; `Direct` params pass through; `Void` params are skipped.
//! - **Sret**: Functions returning large structs receive a hidden first pointer
//!   argument; the caller allocates, the callee stores, then the caller loads.
//! - **Aggregate coercion**: Runtime functions expect `ptr` but ARC IR passes
//!   aggregates (Str, List, Map, Set) by value — alloca + store + pass pointer.

use ori_types::{Idx, Tag};

use super::ArcIrEmitter;
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Apply parameter passing modes to argument values.
    ///
    /// Apply param passing: `Indirect`/`Reference` (alloca+store+pass ptr),
    /// `Direct` (pass through), `Void` (skip).
    pub(super) fn apply_param_passing(
        &mut self,
        args: &[ValueId],
        params: &[crate::codegen::abi::ParamAbi],
    ) -> Vec<ValueId> {
        let mut result = Vec::with_capacity(args.len());
        let mut arg_idx = 0;

        for param_abi in params {
            if arg_idx >= args.len() {
                break;
            }

            match &param_abi.passing {
                crate::codegen::abi::ParamPassing::Indirect { .. }
                | crate::codegen::abi::ParamPassing::Reference => {
                    let param_ty = self.resolve_type(param_abi.ty);
                    let alloca = self.builder.create_entry_alloca(
                        self.current_function,
                        "ref_arg",
                        param_ty,
                    );
                    self.builder.store(args[arg_idx], alloca);
                    result.push(alloca);
                    arg_idx += 1;
                }
                crate::codegen::abi::ParamPassing::Direct => {
                    result.push(args[arg_idx]);
                    arg_idx += 1;
                }
                crate::codegen::abi::ParamPassing::Void => {
                    // Void params are not physically passed — skip
                }
            }
        }

        // Pass remaining args directly (shouldn't happen in well-typed code)
        while arg_idx < args.len() {
            result.push(args[arg_idx]);
            arg_idx += 1;
        }

        result
    }

    /// Call a function with sret (struct return via hidden pointer).
    ///
    /// The sret alloca is placed in the entry block (via `create_entry_alloca`)
    /// to ensure it dominates all uses, even in loop bodies or branch targets.
    pub(super) fn call_with_sret(
        &mut self,
        func_id: FunctionId,
        args: &[ValueId],
        ret_ty: LLVMTypeId,
        name: &str,
    ) -> Option<ValueId> {
        let sret_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "sret.tmp", ret_ty);
        let mut full_args = Vec::with_capacity(1 + args.len());
        full_args.push(sret_alloca);
        full_args.extend_from_slice(args);
        self.emit_rt_call(func_id, &full_args, name);
        Some(self.builder.load(ret_ty, sret_alloca, "sret.load"))
    }

    /// Coerce an aggregate value to a pointer for runtime function calls.
    ///
    /// Runtime functions like `ori_print` expect `ptr` arguments (pointers to
    /// structs), but ARC IR passes aggregate values directly. When we detect
    /// that a call arg is an aggregate but the callee expects `ptr`, we
    /// alloca+store+pass the pointer.
    pub(super) fn coerce_aggregate_to_ptr(&mut self, val: ValueId, ty: Idx) -> ValueId {
        let tag = self.pool.tag(ty);
        match tag {
            Tag::Str | Tag::List | Tag::Set | Tag::Map => {
                let llvm_ty = self.resolve_type(ty);
                let alloca =
                    self.builder
                        .create_entry_alloca(self.current_function, "rt_arg", llvm_ty);
                self.builder.store(val, alloca);
                alloca
            }
            _ => val,
        }
    }

    /// Coerce any value (including scalars) to a pointer via alloca+store.
    ///
    /// Unlike `coerce_aggregate_to_ptr` which only handles struct types,
    /// this works for ALL types. Used by `ori_list_push` which needs a
    /// `*const u8` pointer to any element's bytes.
    pub(super) fn coerce_any_to_ptr(&mut self, val: ValueId, ty: Idx) -> ValueId {
        let llvm_ty = self.resolve_type(ty);
        let alloca = self
            .builder
            .create_entry_alloca(self.current_function, "elem_arg", llvm_ty);
        self.builder.store(val, alloca);
        alloca
    }
}
