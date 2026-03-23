//! Iterator consumer method emission (collect, count, any, all, find, `for_each`, fold).
//!
//! These methods consume the iterator to produce a final value, unlike adapters
//! which return new iterators. All use the same pattern: pass the opaque
//! `iter_ptr` + closure trampoline (for predicate/fold consumers) to a runtime
//! function.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;
use super::trampolines::TrampolineKind;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    pub(in crate::codegen) fn emit_iter_collect(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_collect");

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let elem_inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        // sret pattern: allocate output list struct {i64 len, i64 cap, ptr data}
        let i64_llvm = self.builder.scx().type_i64().into();
        let ptr_llvm = self.builder.scx().type_ptr().into();
        let list_struct = self
            .builder
            .scx()
            .type_struct(&[i64_llvm, i64_llvm, ptr_llvm], false);
        let list_struct_ty = self.builder.register_type(list_struct.into());

        let out_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "collect.out", list_struct_ty);

        self.builder.call(
            func_id,
            &[iter_ptr, elem_size_val, elem_inc_fn, out_ptr],
            "",
        );

        // Load the result list from the sret alloca
        let result = self.builder.load(list_struct_ty, out_ptr, "collect.list");

        // Store elem_dec_fn and elem_count in the new buffer's RC header.
        // ori_iter_collect stores elem_count internally, but elem_dec_fn is
        // an LLVM-generated thunk — must be stored by codegen after collect.
        let result_data = self
            .builder
            .extract_value(result, 2, "collect.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let result_len = self
            .builder
            .extract_value(result, 0, "collect.len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let store_dec = self.builder.runtime_fn("ori_buffer_store_elem_dec");
        self.builder
            .call(store_dec, &[result_data, elem_dec_fn], "");
        let store_count = self.builder.runtime_fn("ori_buffer_store_elem_count");
        self.builder
            .call(store_count, &[result_data, result_len], "");

        Some(result)
    }

    /// Emit `__collect_set(iter)` — collect iterator elements into a hash table set.
    ///
    /// Same sret pattern as `emit_iter_collect` but calls `ori_iter_collect_set`
    /// which deduplicates elements via hash probing + eq callbacks.
    pub(in crate::codegen) fn emit_iter_collect_set(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_collect_set");

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // Get eq and hash thunks for the element type
        let eq_thunk = self
            .get_or_create_eq_thunk(elem_ty)
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let hash_thunk = self
            .get_or_create_hash_thunk(elem_ty)
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let elem_inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        // sret pattern: allocate output set struct {i64 len, i64 cap, ptr data}
        let i64_llvm = self.builder.scx().type_i64().into();
        let ptr_llvm = self.builder.scx().type_ptr().into();
        let set_struct = self
            .builder
            .scx()
            .type_struct(&[i64_llvm, i64_llvm, ptr_llvm], false);
        let set_struct_ty = self.builder.register_type(set_struct.into());

        let out_ptr = self.builder.create_entry_alloca(
            self.current_function,
            "collect_set.out",
            set_struct_ty,
        );

        self.emit_rt_call(
            func_id,
            &[
                iter_ptr,
                elem_size_val,
                eq_thunk,
                hash_thunk,
                elem_inc_fn,
                out_ptr,
            ],
            "",
        );

        let result = self
            .builder
            .load(set_struct_ty, out_ptr, "collect_set.result");

        // Store elem_dec_fn in the set buffer's RC header for defense-in-depth.
        // Sets use metadata scanning for cleanup, not elem_count, so only
        // elem_dec_fn is needed. The LLVM-generated thunk must be stored by codegen.
        let result_data = self
            .builder
            .extract_value(result, 2, "collect_set.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let store_dec = self.builder.runtime_fn("ori_buffer_store_elem_dec");
        self.builder
            .call(store_dec, &[result_data, elem_dec_fn], "");

        Some(result)
    }

    pub(in crate::codegen) fn emit_iter_count(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_count");

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        self.builder
            .call(func_id, &[iter_ptr, elem_size_val], "iter.count")
    }

    pub(in crate::codegen) fn emit_iter_any(
        &mut self,
        iter_ptr: ValueId,
        arg_vals: &[ValueId],
        _args: &[ArcVarId],
        _arc_func: &ArcFunction,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];

        let (tramp_fn, closure_env) =
            self.build_trampoline(closure, elem_ty, TrampolineKind::Predicate, None);

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func_id = self.builder.runtime_fn("ori_iter_any");
        let result = self.emit_rt_call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val],
            "iter.any",
        )?;

        // Convert i8 -> i1
        let zero = self.builder.const_i64(0);
        let i8_ty = self.builder.i8_type();
        let zero_i8 = self.builder.trunc(zero, i8_ty, "zero");
        Some(self.builder.icmp_ne(result, zero_i8, "iter.any.bool"))
    }

    pub(in crate::codegen) fn emit_iter_all(
        &mut self,
        iter_ptr: ValueId,
        arg_vals: &[ValueId],
        _args: &[ArcVarId],
        _arc_func: &ArcFunction,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];

        let (tramp_fn, closure_env) =
            self.build_trampoline(closure, elem_ty, TrampolineKind::Predicate, None);

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func_id = self.builder.runtime_fn("ori_iter_all");
        let result = self.emit_rt_call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val],
            "iter.all",
        )?;

        // Convert i8 -> i1
        let zero = self.builder.const_i64(0);
        let i8_ty = self.builder.i8_type();
        let zero_i8 = self.builder.trunc(zero, i8_ty, "zero");
        Some(self.builder.icmp_ne(result, zero_i8, "iter.all.bool"))
    }

    pub(in crate::codegen) fn emit_iter_find(
        &mut self,
        iter_ptr: ValueId,
        arg_vals: &[ValueId],
        _args: &[ArcVarId],
        _arc_func: &ArcFunction,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];

        let (tramp_fn, closure_env) =
            self.build_trampoline(closure, elem_ty, TrampolineKind::Predicate, None);

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func_id = self.builder.runtime_fn("ori_iter_find");

        // sret pattern for Option<T> result
        // Option layout: {i64 tag, T payload} — matches TypeLayoutResolver
        let i64_llvm = self.builder.scx().type_i64().into();
        let payload_llvm = self.type_resolver.resolve(elem_ty);
        let opt_struct = self
            .builder
            .scx()
            .type_struct(&[i64_llvm, payload_llvm], false);
        let opt_struct_ty = self.builder.register_type(opt_struct.into());

        let out_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "find.out", opt_struct_ty);

        self.emit_rt_call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val, out_ptr],
            "",
        );

        Some(self.builder.load(opt_struct_ty, out_ptr, "find.result"))
    }

    pub(in crate::codegen) fn emit_iter_for_each(
        &mut self,
        iter_ptr: ValueId,
        arg_vals: &[ValueId],
        _args: &[ArcVarId],
        _arc_func: &ArcFunction,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }
        let closure = arg_vals[1];

        let (tramp_fn, closure_env) =
            self.build_trampoline(closure, elem_ty, TrampolineKind::ForEach, None);

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func_id = self.builder.runtime_fn("ori_iter_for_each");
        self.emit_rt_call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val],
            "",
        );

        // for_each returns unit
        Some(self.builder.const_i64(0))
    }

    pub(in crate::codegen) fn emit_iter_fold(
        &mut self,
        iter_ptr: ValueId,
        arg_vals: &[ValueId],
        args: &[ArcVarId],
        arc_func: &ArcFunction,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        if arg_vals.len() < 3 {
            return None;
        }
        let init_val = arg_vals[1];
        let closure = arg_vals[2];

        // Determine accumulator type from init value
        let acc_ty = arc_func.var_type(args[1]);
        let acc_llvm_ty = self.resolve_type(acc_ty);

        let (tramp_fn, closure_env) =
            self.build_trampoline(closure, elem_ty, TrampolineKind::Fold, Some(acc_ty));

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let acc_size = self.element_store_size(acc_ty);
        let acc_size_val = self.builder.const_i64(acc_size as i64);

        // Store init value to alloca for passing as ptr
        let init_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "fold.init", acc_llvm_ty);
        self.builder.store(init_val, init_alloca);

        // Output alloca for sret
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "fold.out", acc_llvm_ty);

        let func_id = self.builder.runtime_fn("ori_iter_fold");
        self.emit_rt_call(
            func_id,
            &[
                iter_ptr,
                init_alloca,
                tramp_fn,
                closure_env,
                elem_size_val,
                acc_size_val,
                out_alloca,
            ],
            "",
        );

        Some(self.builder.load(acc_llvm_ty, out_alloca, "fold.result"))
    }
}
