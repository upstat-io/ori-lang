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

use super::ArcIrEmitter;
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};
use ori_arc::ir::ArcVarId;
use ori_ir::Name;
use ori_types::{Idx, Tag};

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
    /// Uses the exact narrowed element size for for-yield integer lists.
    /// Other element-size arguments remain canonical.
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

        let elem_size_var = if is_list_new && arc_args.len() == 2 {
            Some(arc_args[1])
        } else if is_list_push && arc_args.len() == 3 {
            Some(arc_args[2])
        } else {
            None
        };

        if let Some((elem_size_var, (collection_ty, elem_ty))) = elem_size_var.and_then(|var| {
            self.for_yield_elem_size_types
                .get(&var)
                .copied()
                .map(|types| (var, types))
        }) {
            let narrowed = self.pool.tag(self.pool.resolve_fully(elem_ty)) == Tag::Int;
            let Some(width) = narrowed
                .then(|| self.narrowed_collection_element_width(collection_ty))
                .flatten()
            else {
                return coerced_args;
            };
            let narrowed_size = self.builder.const_i64(i64::from(width.size_bytes()));
            if is_list_new && arc_args[1] == elem_size_var {
                coerced_args[1] = narrowed_size;
            } else if is_list_push && arc_args[2] == elem_size_var {
                coerced_args[2] = narrowed_size;
            }
        }

        coerced_args
    }

    /// Apply parameter passing modes to argument values.
    ///
    /// Apply param passing: `Indirect`/`Reference` (alloca+store+pass ptr),
    /// `Direct` (pass through), `Void` (skip).
    ///
    /// When `arc_vars` is `Some`, borrowed pointer FORWARDING is active: a
    /// `Reference`/`Indirect` callee parameter whose argument was itself
    /// received as a borrowed parameter pointer forwards the original
    /// pointer directly — eliminating the `ptr → load → alloca → store →
    /// ptr` round-trip. Callers without ARC-variable context pass `None`
    /// (every Indirect/Reference arg is spilled to a fresh alloca).
    pub(super) fn apply_param_passing(
        &mut self,
        args: &[ValueId],
        arc_vars: Option<&[ArcVarId]>,
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
                    // Forwarding: this argument has a known source pointer
                    // from a borrowed parameter — forward it directly.
                    let forwarded = arc_vars
                        .and_then(|vars| vars.get(arg_idx))
                        .and_then(|var| self.borrowed_param_ptrs.get(var))
                        .copied();
                    if let Some(src_ptr) = forwarded {
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
                    // A Direct (by-value) parameter needs the materialized
                    // aggregate. When the argument is a borrowed `Reference`/
                    // `Indirect` parameter whose entry load was elided
                    // (pointer-only), its value slot is a zero placeholder —
                    // load the aggregate from the source pointer (Direct
                    // passing is <=16 bytes, so a single load is FastISel-safe).
                    let elided_ptr = arc_vars
                        .and_then(|vars| vars.get(arg_idx))
                        .filter(|var| self.pointer_only_params.contains(var))
                        .and_then(|var| self.borrowed_param_ptrs.get(var))
                        .copied();
                    if let Some(src_ptr) = elided_ptr {
                        let param_ty = self.resolve_type(param_abi.ty);
                        let loaded = self.builder.load(param_ty, src_ptr, "borrow.byval.load");
                        result.push(loaded);
                    } else {
                        result.push(args[arg_idx]);
                    }
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
    /// When the current function itself returns the same LLVM type via sret
    /// and no compatible prior call has consumed its destination, forwards
    /// that destination directly (avoiding intermediate alloca+load+store).
    /// A differently typed nested return always gets a fresh entry alloca.
    pub(super) fn call_with_sret(
        &mut self,
        func_id: FunctionId,
        args: &[ValueId],
        ret_ty: LLVMTypeId,
        name: &str,
    ) -> Option<ValueId> {
        // LLVM pointers are opaque, so forwarding without comparing the
        // pointee types can let a larger nested return overwrite the caller's
        // smaller result slot. Compare the registered LLVM types rather than
        // arena IDs because resolving the same type may register it twice.
        let forward_ptr = self.current_sret.and_then(|(ptr, current_ty)| {
            self.builder
                .same_llvm_type(current_ty, ret_ty)
                .then_some(ptr)
        });

        let sret_alloca = if let Some(sret_ptr) = forward_ptr {
            self.current_sret = None;
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
        if forward_ptr.is_some() {
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
