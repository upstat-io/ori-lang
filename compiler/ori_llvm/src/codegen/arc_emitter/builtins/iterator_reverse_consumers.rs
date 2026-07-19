//! Reverse iterator consumers and string joining.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::{FIELD_CAP, FIELD_DATA, FIELD_LEN};
use ori_types::{Idx, Tag};

use crate::codegen::value_id::{FunctionId, ValueId};

use super::super::ArcIrEmitter;
use super::trampolines::TrampolineKind;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit `last()` — advance the double-ended iterator from the back once.
    ///
    /// Returns `Option<T>`: `{ i64 tag, T payload }` via sret.
    pub(in crate::codegen) fn emit_iter_last(
        &mut self,
        iter_ptr: ValueId,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let elem_inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

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
        self.emit_rt_call(
            func_id,
            &[iter_ptr, elem_size_val, elem_inc_fn, out_ptr],
            "",
        );

        Some(self.builder.load(opt_struct_ty, out_ptr, "last.result"))
    }

    /// Emit `rfind(predicate)` — search directly from the iterator's back.
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

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let elem_inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

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
            &[
                iter_ptr,
                tramp_fn,
                closure_env,
                elem_size_val,
                elem_inc_fn,
                out_ptr,
            ],
            "",
        );

        Some(self.builder.load(opt_struct_ty, out_ptr, "rfind.result"))
    }

    /// Emit `rfold(initial, op)` — fold by advancing directly from the back.
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

        let elem_size = self.element_store_size(elem_ty);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let acc_size = self.element_store_size(acc_ty);
        let acc_size_val = self.builder.const_i64(acc_size as i64);
        let acc_dec_fn = self.get_or_generate_elem_dec_fn(acc_ty);

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
                acc_dec_fn,
                out_alloca,
            ],
            "",
        );

        Some(self.builder.load(acc_llvm_ty, out_alloca, "rfold.result"))
    }

    /// Emit `join(separator)` — join iterator elements into a string.
    ///
    /// For string-element iterators, passes `null` for `to_str_fn` (elements
    /// are already strings). For primitive types (int, float, bool, char),
    /// generates a `to_str` trampoline that calls the appropriate
    /// `ori_str_from_*` runtime function. Unsupported types (byte, Duration,
    /// Size, Ordering, structs, closures, etc.) produce a codegen error.
    ///
    /// The runtime iterator state owns yield provenance and releases each input
    /// after join copies it. LLVM therefore passes no ownership verdict.
    pub(in crate::codegen) fn emit_iter_join(
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

        let elem_size = self.element_store_size(elem_ty);
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
        // Only types whose ori_str_from_* produces the same output as
        // the interpreter's Printable::to_str() are supported. Duration/Size
        // route to their unit-aware formatters (matching the interpreter's
        // "1s"/"1kb" output). Still excluded:
        // - Ordering: formatted as "Less"/"Equal"/"Greater" not ints
        // - Byte: formatted as hex ("0xff") not decimal
        // These need proper Printable method dispatch — future work.
        let rt_func_name = match tag {
            Tag::Int => "ori_str_from_int",
            Tag::Duration => "ori_str_from_duration",
            Tag::Size => "ori_str_from_size",
            Tag::Float => "ori_str_from_float",
            Tag::Bool => "ori_str_from_bool",
            Tag::Char => "ori_str_from_char",
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
        self.builder.set_module_local(func_id);
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

        // Load element from elem_ptr using canonical type.
        // Narrowing is confined to the list storage boundary.
        let buf_elem_llvm_ty = self.resolve_type(elem_ty);
        let raw = self.builder.load(buf_elem_llvm_ty, elem_ptr, "elem");

        // With canonical types, buf_elem_llvm_ty is already the
        // canonical type (i64 for int). No sext needed — the load produces
        // the correct canonical value directly.
        let elem_val = raw;

        // Call the runtime conversion function with out_ptr as sret.
        // Runtime functions returning OriStr (24 bytes) use sret pattern:
        // void @ori_str_from_*(ptr sret(%OriStr) out_ptr, <param_ty> value)
        let rt_func = self.builder.runtime_fn(rt_func_name);
        self.builder.call(rt_func, &[out_ptr, elem_val], "");

        self.builder.ret_void();

        // Function-level LLVM IR verification.
        if self.verify_arc {
            let fn_val = self.builder.get_function_value(func_id);
            if !fn_val.verify(true) {
                tracing::error!(
                    name = tramp_name,
                    "LLVM IR verification failed (generate_join_to_str_trampoline)"
                );
                self.builder.record_codegen_error();
            }
        }

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        Some(func_id)
    }
}
