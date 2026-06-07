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

use ori_arc::ir::ArcVarId;
use ori_ir::Name;
use ori_types::{Idx, Tag};

use super::ArcIrEmitter;
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Coerce runtime-function (`ori_*`) arguments to pointers and apply the
    /// for-yield narrowed elem-size override. Shared by `Apply` emission and
    /// the `Invoke` terminator's `emit_runtime_fn_call` so the two call
    /// shapes cannot drift.
    ///
    /// Runtime functions take `ptr` params, but ARC IR passes aggregate
    /// structs (Str, List, etc.) by value:
    ///
    /// - `ori_list_push` arg 1 is the element value — coerced to a pointer
    ///   regardless of its type (even scalars).
    /// - Borrowed parameters with a known source pointer forward that
    ///   pointer directly (no alloca+store copy) on call AND invoke paths —
    ///   the source pointer is a function parameter that outlives the call,
    ///   and the runtime must observe the original buffer (COW uniqueness),
    ///   not a fresh copy.
    /// - Other aggregates are spilled to an alloca and passed by pointer.
    ///
    /// The elem-size override replaces canonical `elem_size=8` baked at ARC
    /// lowering time (before the `ReprPlan` exists) with the narrowed size for
    /// int-element for-yield lists. Only `elem_size` vars identified by the
    /// pre-scan are overridden (prevents corrupting non-int accumulators
    /// like `[str]` when a narrowed `[int]` exists in the program).
    pub(super) fn coerce_runtime_fn_args(
        &mut self,
        callee: Name,
        arc_args: &[ArcVarId],
        arg_vals: &[ValueId],
        arc_func: &ori_arc::ir::ArcFunction,
    ) -> Vec<ValueId> {
        let is_list_push = callee == self.list_rt_names.push;
        let is_list_new = callee == self.list_rt_names.new;
        let mut coerced_args: Vec<ValueId> = arc_args
            .iter()
            .zip(arg_vals.iter())
            .enumerate()
            .map(|(i, (arc_var, &val))| {
                let arg_ty = arc_func.var_type(*arc_var);
                if is_list_push && i == 1 {
                    self.coerce_any_to_ptr(val, arg_ty)
                } else if let Some(&src_ptr) = self.borrowed_param_ptrs.get(arc_var) {
                    let tag = self.pool.tag(arg_ty);
                    if matches!(tag, Tag::Str | Tag::List | Tag::Set | Tag::Map) {
                        src_ptr
                    } else {
                        self.coerce_aggregate_to_ptr(val, arg_ty)
                    }
                } else {
                    self.coerce_aggregate_to_ptr(val, arg_ty)
                }
            })
            .collect();

        if let Some(width) = self.narrowed_int_collection_element_width() {
            let narrowed_size = self.builder.const_i64(i64::from(width.size_bytes()));
            if is_list_new
                && arc_args.len() == 2
                && self.for_yield_int_elem_sizes.contains(&arc_args[1])
            {
                coerced_args[1] = narrowed_size;
            } else if is_list_push
                && arc_args.len() == 3
                && self.for_yield_int_elem_sizes.contains(&arc_args[2])
            {
                coerced_args[2] = narrowed_size;
            }
        }

        coerced_args
    }

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

    /// Apply parameter passing with borrowed pointer forwarding.
    ///
    /// Like [`apply_param_passing`], but when a `Reference`/`Indirect` callee
    /// parameter matches an argument that was itself received as a borrowed
    /// parameter pointer, forwards the original pointer directly instead of
    /// creating an alloca+store round-trip.
    ///
    /// This eliminates the pattern: `ptr → load → alloca → store → ptr`.
    pub(super) fn apply_param_passing_with_forwarding(
        &mut self,
        args: &[ValueId],
        arc_vars: &[ArcVarId],
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
                    // Check if this argument has a known source pointer from
                    // a borrowed parameter — forward it directly.
                    if let Some(&src_ptr) = arc_vars
                        .get(arg_idx)
                        .and_then(|var| self.borrowed_param_ptrs.get(var))
                    {
                        result.push(src_ptr);
                    } else {
                        let param_ty = self.resolve_type(param_abi.ty);
                        let alloca = self.builder.create_entry_alloca(
                            self.current_function,
                            "ref_arg",
                            param_ty,
                        );
                        self.builder.store(args[arg_idx], alloca);
                        result.push(alloca);
                    }
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
    /// When the current function itself returns via sret and no prior
    /// `call_with_sret` has consumed the sret pointer, forwards the
    /// function's sret destination directly (avoiding intermediate
    /// alloca+load+store). Otherwise creates a fresh entry-block alloca.
    pub(super) fn call_with_sret(
        &mut self,
        func_id: FunctionId,
        args: &[ValueId],
        ret_ty: LLVMTypeId,
        name: &str,
    ) -> Option<ValueId> {
        // Sret forwarding: reuse the function's sret pointer if available.
        // `take()` ensures only the first call_with_sret gets forwarded —
        // subsequent calls create their own allocas to avoid clobbering.
        let forwarded = self.current_sret_ptr.is_some();
        let sret_alloca = if let Some(sret_ptr) = self.current_sret_ptr.take() {
            sret_ptr
        } else {
            self.builder
                .create_entry_alloca(self.current_function, "sret.tmp", ret_ty)
        };
        let mut full_args = Vec::with_capacity(1 + args.len());
        full_args.push(sret_alloca);
        full_args.extend_from_slice(args);
        self.emit_rt_call(func_id, &full_args, name);
        let result = self.builder.load(ret_ty, sret_alloca, "sret.load");

        // Track the forwarded result: if this value is returned directly,
        // the Return terminator can skip the identity store (value is
        // already at the sret destination).
        if forwarded {
            self.sret_forwarded_result = Some(result);
        }

        Some(result)
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
