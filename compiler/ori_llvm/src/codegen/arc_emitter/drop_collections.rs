//! Collection and map drop-function bodies.

use ori_ir::{CLOSURE_FIELD_ENV, FIELD_CAP, FIELD_DATA, FIELD_LEN};
use ori_types::Idx;

use crate::codegen::value_id::{FunctionId, ValueId};

use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit drop body for a collection type ([T], set[T]).
    pub(super) fn emit_drop_collection(
        &mut self,
        func_id: FunctionId,
        data_ptr: ValueId,
        ty: Idx,
        element_type: Idx,
    ) {
        let list_llvm_ty = self.resolve_type(ty);
        let i64_ty = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();

        let len_ptr = self
            .builder
            .struct_gep(list_llvm_ty, data_ptr, FIELD_LEN, "len.ptr");
        let len = self.builder.load(i64_ty, len_ptr, "len");

        let cap_ptr = self
            .builder
            .struct_gep(list_llvm_ty, data_ptr, FIELD_CAP, "cap.ptr");
        let cap = self.builder.load(i64_ty, cap_ptr, "cap");

        let data_field_ptr =
            self.builder
                .struct_gep(list_llvm_ty, data_ptr, FIELD_DATA, "data.field.ptr");
        let elem_data = self.builder.load(ptr_ty, data_field_ptr, "elem_data");

        let elem_drop_fn = self.get_or_generate_drop_fn(element_type);

        self.emit_drop_element_loop(func_id, elem_data, len, element_type, elem_drop_fn, "elem");

        let resolved_ty = self.pool.resolve_fully(ty);
        self.emit_drop_list_free_data(elem_data, cap, element_type, Some(resolved_ty));
        self.emit_drop_rc_free(data_ptr, ty);
        self.builder.ret_void();
    }

    /// Emit drop body for a map type ({K: V}).
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors the key/value layout and callbacks carried by DropKind::Map"
    )]
    pub(super) fn emit_drop_map(
        &mut self,
        func_id: FunctionId,
        data_ptr: ValueId,
        ty: Idx,
        key_type: Idx,
        value_type: Idx,
        dec_keys: bool,
        dec_values: bool,
    ) {
        let map_llvm_ty = self.resolve_type(ty);
        let i64_ty = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();

        let len_ptr = self
            .builder
            .struct_gep(map_llvm_ty, data_ptr, FIELD_LEN, "len.ptr");
        let len = self.builder.load(i64_ty, len_ptr, "len");

        if dec_keys {
            let keys_ptr = self
                .builder
                .struct_gep(map_llvm_ty, data_ptr, 2, "keys.field.ptr");
            let keys_data = self.builder.load(ptr_ty, keys_ptr, "keys_data");
            let key_drop_fn = self.get_or_generate_drop_fn(key_type);
            self.emit_drop_element_loop(func_id, keys_data, len, key_type, key_drop_fn, "key");
        }

        if dec_values {
            let vals_ptr = self
                .builder
                .struct_gep(map_llvm_ty, data_ptr, 3, "vals.field.ptr");
            let vals_data = self.builder.load(ptr_ty, vals_ptr, "vals_data");
            let val_drop_fn = self.get_or_generate_drop_fn(value_type);
            self.emit_drop_element_loop(func_id, vals_data, len, value_type, val_drop_fn, "val");
        }

        self.emit_drop_rc_free(data_ptr, ty);
        self.builder.ret_void();
    }

    /// Emit a loop that decrements RC for each element in an array.
    ///
    /// Shared between collection and map drop.
    fn emit_drop_element_loop(
        &mut self,
        func_id: FunctionId,
        array_ptr: ValueId,
        len: ValueId,
        element_type: Idx,
        elem_drop_fn: ValueId,
        prefix: &str,
    ) {
        let i64_ty = self.builder.i64_type();
        let elem_llvm_ty = self.resolve_type(element_type);

        let Some(entry_block) = self.builder.current_block() else {
            self.builder
                .record_codegen_error_with_msg("drop-element loop requires an insertion block");
            return;
        };
        let loop_header = self
            .builder
            .append_block(func_id, &format!("{prefix}.loop.hdr"));
        let loop_body = self
            .builder
            .append_block(func_id, &format!("{prefix}.loop.body"));
        let loop_done = self
            .builder
            .append_block(func_id, &format!("{prefix}.loop.done"));

        let zero = self.builder.const_i64(0);
        let one = self.builder.const_i64(1);

        self.builder.br(loop_header);

        self.builder.position_at_end(loop_header);
        let i_phi = self.builder.phi(i64_ty, &format!("{prefix}.i"));
        let done = self.builder.icmp_sge(i_phi, len, &format!("{prefix}.done"));
        self.builder.cond_br(done, loop_done, loop_body);

        self.builder.position_at_end(loop_body);
        let elem_ptr =
            self.builder
                .gep(elem_llvm_ty, array_ptr, &[i_phi], &format!("{prefix}.ptr"));
        let elem_val = self
            .builder
            .load(elem_llvm_ty, elem_ptr, &format!("{prefix}.val"));
        let data_ptrs = self.extract_rc_data_ptrs(elem_val, element_type);
        for ptr in data_ptrs {
            self.emit_drop_rc_dec_with_fn(ptr, elem_drop_fn);
        }
        let i_next = self.builder.add(i_phi, one, &format!("{prefix}.i.next"));
        self.builder.br(loop_header);

        self.builder
            .add_phi_incoming(i_phi, &[(zero, entry_block), (i_next, loop_body)]);

        self.builder.position_at_end(loop_done);
    }

    // Runtime calls

    /// Emit `ori_rc_dec` for a child field value.
    ///
    /// Extracts embedded data pointer(s) from aggregate types (Str, List, etc.)
    /// before calling `ori_rc_dec`, since the runtime expects raw pointers.
    ///
    /// Closures (`Tag::Function`) are special: the drop function is stored
    /// dynamically in the env header (field 0 of the heap allocation), not
    /// generated statically. Uses the same pattern as `emit_rc_dec_closure`.
    pub(super) fn emit_drop_rc_dec(&mut self, val: ValueId, field_type: Idx) {
        let resolved = self.pool.resolve_fully(field_type);
        let tag = self.pool.tag(resolved);

        if tag == ori_types::Tag::Function {
            self.emit_closure_field_rc_dec(val);
            return;
        }

        // INVARIANT: Each inline member uses its own typed release; applying an
        // aggregate drop function to an inner allocation corrupts member storage.
        if matches!(
            tag,
            ori_types::Tag::Option
                | ori_types::Tag::Result
                | ori_types::Tag::Enum
                | ori_types::Tag::Struct
                | ori_types::Tag::Tuple
        ) {
            self.dec_value_rc(val, field_type);
            return;
        }

        let data_ptrs = self.extract_rc_data_ptrs(val, field_type);
        if data_ptrs.is_empty() {
            return;
        }
        let drop_fn = self.get_or_generate_drop_fn(field_type);
        for ptr in data_ptrs {
            self.emit_drop_rc_dec_with_fn(ptr, drop_fn);
        }
    }

    /// Emit RC dec for a closure field value `{ fn_ptr, env_ptr }`.
    ///
    /// Extracts `env_ptr` (field 1), null-checks it, loads the dynamic
    /// drop function from `env_ptr[0]`, and calls `ori_rc_dec(env_ptr, drop_fn)`.
    fn emit_closure_field_rc_dec(&mut self, closure_val: ValueId) {
        let Some(env_ptr) = self
            .builder
            .extract_value(closure_val, CLOSURE_FIELD_ENV, "clos.env")
        else {
            self.builder.record_codegen_error_with_msg(
                "closure RC input is missing its canonical environment field",
            );
            return;
        };

        if self.builder.is_const_null_ptr(env_ptr) {
            return;
        }

        let is_null = self.builder.is_null_ptr(env_ptr, "clos.null");
        let do_dec = self.builder.append_block(self.current_function, "clos.dec");
        let skip = self
            .builder
            .append_block(self.current_function, "clos.skip");
        self.builder.cond_br(is_null, skip, do_dec);

        self.builder.position_at_end(do_dec);
        let ptr_ty = self.builder.ptr_type();
        let drop_fn = self.builder.load(ptr_ty, env_ptr, "clos.drop_fn");
        let rc_dec_id = self.builder.runtime_fn("ori_rc_dec");
        self.emit_rt_call(rc_dec_id, &[env_ptr, drop_fn], "");
        self.builder.br(skip);

        self.builder.position_at_end(skip);
    }

    /// Emit `ori_rc_dec(val, drop_fn_ptr)` with a pre-computed drop function.
    pub(super) fn emit_drop_rc_dec_with_fn(&mut self, val: ValueId, drop_fn_ptr: ValueId) {
        let func_id = self.builder.runtime_fn("ori_rc_dec");
        self.builder.call(func_id, &[val, drop_fn_ptr], "");
    }

    /// Emit `ori_rc_free(data_ptr, size, align)` to deallocate an RC object.
    pub(super) fn emit_drop_rc_free(&mut self, data_ptr: ValueId, ty: Idx) {
        let size = self.element_store_size(ty);
        let align = u64::from(self.type_info.get(ty).alignment());

        let size_val = self.builder.const_i64(size as i64);
        let align_val = self.builder.const_i64(align as i64);

        let func_id = self.builder.runtime_fn("ori_rc_free");
        self.builder
            .call(func_id, &[data_ptr, size_val, align_val], "");
    }

    /// Emit `ori_list_free_data(data, cap, elem_size)` to free a collection buffer.
    ///
    /// `collection_ty` is the resolved collection type (e.g., `[int]`), used
    /// for narrowed element size lookup. Falls back to canonical
    /// size when `None`.
    fn emit_drop_list_free_data(
        &mut self,
        data: ValueId,
        cap: ValueId,
        element_type: Idx,
        collection_ty: Option<Idx>,
    ) {
        let elem_size = if let Some(coll_ty) = collection_ty {
            self.collection_elem_size(coll_ty, element_type)
        } else {
            self.element_store_size(element_type)
        };
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        let func_id = self.builder.runtime_fn("ori_list_free_data");
        self.builder.call(func_id, &[data, cap, elem_size_val], "");
    }
}
