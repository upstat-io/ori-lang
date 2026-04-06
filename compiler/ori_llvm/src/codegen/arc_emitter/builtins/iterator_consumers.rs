//! Iterator consumer method emission (collect, count, any, all, find, `for_each`, fold).
//!
//! These methods consume the iterator to produce a final value, unlike adapters
//! which return new iterators. All use the same pattern: pass the opaque
//! `iter_ptr` + closure trampoline (for predicate/fold consumers) to a runtime
//! function.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::{FIELD_CAP, FIELD_DATA, FIELD_LEN};
use ori_types::{Idx, Tag};

use crate::codegen::value_id::{FunctionId, ValueId};

use super::super::ArcIrEmitter;
use super::trampolines::TrampolineKind;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    pub(in crate::codegen) fn emit_iter_collect(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_iter_collect");

        // Use narrowed element size for int elements.
        let elem_size = self.int_element_store_size(elem_ty);
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
            .extract_value(result, FIELD_DATA, "collect.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let result_len = self
            .builder
            .extract_value(result, FIELD_LEN, "collect.len")
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

        // Sets use canonical element sizes — set hash tables always store
        // full-width elements regardless of list narrowing. The iterator
        // yields narrowed bytes, but ori_iter_collect_set's elem_buf is
        // zeroed, so on little-endian the zero-padded bytes form the
        // correct canonical value for small non-negative integers.
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
            .extract_value(result, FIELD_DATA, "collect_set.data")
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

        // Use narrowed element size for int elements.
        let elem_size = self.int_element_store_size(elem_ty);
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

        // Use narrowed element size for int elements.
        let elem_size = self.int_element_store_size(elem_ty);
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

        // Use narrowed element size for int elements.
        let elem_size = self.int_element_store_size(elem_ty);
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

        // Use narrowed element size for int elements.
        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func_id = self.builder.runtime_fn("ori_iter_find");

        // sret pattern for Option<T> result
        // Option layout: {i64 tag, T payload} — runtime (ori_rt) writes i64 tags
        let tag_llvm = self.builder.scx().type_i64().into();
        let payload_llvm = self.type_resolver.resolve(elem_ty);
        let opt_struct = self
            .builder
            .scx()
            .type_struct(&[tag_llvm, payload_llvm], false);
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

        // Use narrowed element size for int elements.
        let elem_size = self.int_element_store_size(elem_ty);
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

        // Use narrowed element size for int elements.
        let elem_size = self.int_element_store_size(elem_ty);
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

    // New consumers (runtime-backed)

    /// Emit `last()` — iterate forward keeping the last element.
    ///
    /// Returns `Option<T>`: `{ i64 tag, T payload }` via sret.
    pub(in crate::codegen) fn emit_iter_last(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // Option layout: {i64 tag, T payload}
        let tag_llvm = self.builder.scx().type_i64().into();
        let payload_llvm = self.type_resolver.resolve(elem_ty);
        let opt_struct = self
            .builder
            .scx()
            .type_struct(&[tag_llvm, payload_llvm], false);
        let opt_struct_ty = self.builder.register_type(opt_struct.into());

        let out_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "last.out", opt_struct_ty);

        let func_id = self.builder.runtime_fn("ori_iter_last");
        self.emit_rt_call(func_id, &[iter_ptr, elem_size_val, out_ptr], "");

        Some(self.builder.load(opt_struct_ty, out_ptr, "last.result"))
    }

    /// Emit `rfind(predicate)` — find last matching element (collect + search backward).
    pub(in crate::codegen) fn emit_iter_rfind(
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

        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // Option layout: {i64 tag, T payload}
        let tag_llvm = self.builder.scx().type_i64().into();
        let payload_llvm = self.type_resolver.resolve(elem_ty);
        let opt_struct = self
            .builder
            .scx()
            .type_struct(&[tag_llvm, payload_llvm], false);
        let opt_struct_ty = self.builder.register_type(opt_struct.into());

        let out_ptr =
            self.builder
                .create_entry_alloca(self.current_function, "rfind.out", opt_struct_ty);

        let func_id = self.builder.runtime_fn("ori_iter_rfind");
        self.emit_rt_call(
            func_id,
            &[iter_ptr, tramp_fn, closure_env, elem_size_val, out_ptr],
            "",
        );

        Some(self.builder.load(opt_struct_ty, out_ptr, "rfind.result"))
    }

    /// Emit `rfold(initial, op)` — fold right-to-left (collect + fold backward).
    ///
    /// Follows the same pattern as `emit_iter_fold`, delegating to `ori_iter_rfold`.
    pub(in crate::codegen) fn emit_iter_rfold(
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

        let acc_ty = arc_func.var_type(args[1]);
        let acc_llvm_ty = self.resolve_type(acc_ty);

        let (tramp_fn, closure_env) =
            self.build_trampoline(closure, elem_ty, TrampolineKind::Fold, Some(acc_ty));

        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let acc_size = self.element_store_size(acc_ty);
        let acc_size_val = self.builder.const_i64(acc_size as i64);

        let init_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "rfold.init", acc_llvm_ty);
        self.builder.store(init_val, init_alloca);

        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "rfold.result", acc_llvm_ty);

        let func_id = self.builder.runtime_fn("ori_iter_rfold");
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

        Some(self.builder.load(acc_llvm_ty, out_alloca, "rfold.result"))
    }

    /// Emit `join(separator)` — join iterator elements into a string.
    ///
    /// For string-element iterators, passes `null` for `to_str_fn` (elements
    /// are already strings). For primitive types (int, float, bool, char, byte),
    /// generates a `to_str` trampoline that calls the appropriate
    /// `ori_str_from_*` runtime function. Unsupported types (Duration, Size,
    /// Ordering, structs, closures, etc.) produce a codegen error.
    pub(in crate::codegen) fn emit_iter_join(
        &mut self,
        iter_ptr: ValueId,
        arg_vals: &[ValueId],
        elem_ty: Idx,
    ) -> Option<ValueId> {
        if arg_vals.len() < 2 {
            return None;
        }

        let resolved_elem = self.pool.resolve_fully(elem_ty);
        let tag = self.pool.tag(resolved_elem);

        // Determine to_str_fn: null for strings, trampoline for primitives
        let (to_str_fn, to_str_env) = if tag == Tag::Str {
            // Elements are already strings — no conversion needed.
            (self.builder.const_null_ptr(), self.builder.const_null_ptr())
        } else if let Some(tramp_fn_id) = self.generate_join_to_str_trampoline(elem_ty) {
            let tramp_fn_ptr = self.builder.get_function_ptr(tramp_fn_id);
            // No closure environment needed — conversion logic is baked in.
            (tramp_fn_ptr, self.builder.const_null_ptr())
        } else {
            self.builder.record_codegen_error_with_msg(format!(
                "iter_join on {tag:?} elements not yet supported in LLVM backend"
            ));
            return Some(self.builder.poison_value);
        };

        let separator = arg_vals[1];

        // Separator is an OriStr (24-byte union: heap or SSO).
        // Pass all 3 struct fields to the runtime, which reconstructs
        // the OriStr and handles SSO vs heap internally. Direct field
        // extraction is safe here because we pass the RAW bits — the
        // runtime reinterprets them correctly regardless of SSO state.
        let sep_field0 = self
            .builder
            .extract_value(separator, FIELD_LEN, "join.sep.len")?;
        let sep_field1 = self
            .builder
            .extract_value(separator, FIELD_CAP, "join.sep.cap")?;
        let sep_field2 = self
            .builder
            .extract_value(separator, FIELD_DATA, "join.sep.data")?;

        let elem_size = self.int_element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        // OriStr result type (always str, regardless of element type)
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let out_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "join.out", str_ty);

        let func_id = self.builder.runtime_fn("ori_iter_join");
        self.emit_rt_call(
            func_id,
            &[
                iter_ptr,
                sep_field0,
                sep_field1,
                sep_field2,
                to_str_fn,
                to_str_env,
                elem_size_val,
                out_alloca,
            ],
            "",
        );

        Some(self.builder.load(str_ty, out_alloca, "join.str"))
    }

    /// Generate a `to_str` trampoline for `join` on non-string element types.
    ///
    /// The trampoline has C ABI signature `(env: ptr, elem_ptr: ptr, out_ptr: ptr) -> void`.
    /// It reads the element from `elem_ptr`, calls the appropriate `ori_str_from_*`
    /// runtime function, and writes the resulting `OriStr` to `out_ptr` (sret pattern).
    ///
    /// Returns `None` for unsupported types (structs, closures, etc.).
    fn generate_join_to_str_trampoline(&mut self, elem_ty: Idx) -> Option<FunctionId> {
        let resolved = self.pool.resolve_fully(elem_ty);
        let tag = self.pool.tag(resolved);

        // Determine the runtime conversion function name and the element
        // load type. All conversion functions write an OriStr to the sret
        // pointer, so the trampoline uses out_ptr directly as the sret arg.
        //
        // Duration, Size, and Ordering are excluded: their Printable semantics
        // format with units (e.g. "1s", "1kb", "Equal") but ori_str_from_int
        // would produce raw storage values (e.g. "1000000000", "1000", "1").
        // These need proper Printable method dispatch — future work.
        let (rt_func_name, needs_sext_to_i64) = match tag {
            Tag::Int | Tag::Byte => ("ori_str_from_int", true),
            Tag::Float => ("ori_str_from_float", false),
            Tag::Bool => ("ori_str_from_bool", false),
            Tag::Char => ("ori_str_from_char", false),
            _ => return None,
        };

        let tramp_id = self.partial_apply_counter;
        self.partial_apply_counter += 1;
        let tramp_name = format!("_ori_join_to_str_{tramp_id}");

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        let ptr_ty = self.builder.ptr_type();

        // Declare: (env: ptr, elem_ptr: ptr, out_ptr: ptr) -> void
        let func_id = self
            .builder
            .declare_void_function(&tramp_name, &[ptr_ty, ptr_ty, ptr_ty]);
        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);
        self.builder.add_uwtable_attribute(func_id);
        for i in 0..3 {
            self.builder.add_noundef_param_attribute(func_id, i);
        }

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);

        // Parameters: env (ignored), elem_ptr, out_ptr
        let _env_ptr = self.builder.get_param(func_id, 0);
        let elem_ptr = self.builder.get_param(func_id, 1);
        let out_ptr = self.builder.get_param(func_id, 2);

        // Load element from elem_ptr. Use narrowed type for int elements
        // (iterator buffers store narrowed ints).
        let buf_elem_llvm_ty = self.int_element_llvm_type(elem_ty);
        let raw = self.builder.load(buf_elem_llvm_ty, elem_ptr, "elem");

        // Widen to the canonical type expected by the runtime function.
        let elem_val = if needs_sext_to_i64 {
            let i64_ty = self.builder.i64_type();
            self.builder.sext(raw, i64_ty, "elem.sext")
        } else {
            raw
        };

        // Call the runtime conversion function with out_ptr as sret.
        // Runtime functions returning OriStr (24 bytes) use sret pattern:
        // void @ori_str_from_*(ptr sret(%OriStr) out_ptr, <param_ty> value)
        let rt_func = self.builder.runtime_fn(rt_func_name);
        self.builder.call(rt_func, &[out_ptr, elem_val], "");

        self.builder.ret_void();

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        Some(func_id)
    }
}
