//! COW (Copy-on-Write) mutation codegen for lists.
//!
//! All list mutation methods use COW semantics: when the list is uniquely
//! owned (RC == 1), mutation happens in-place; when shared, a copy is made
//! first. Each method returns a `{i64 len, i64 cap, ptr data}` struct.

use ori_arc::CowMode;
use ori_ir::{FIELD_DATA, FIELD_LEN};
use ori_types::{Idx, Tag};

use crate::codegen::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

use super::super::super::ArcIrEmitter;

struct ScalarUpdatedArgs {
    receiver: ValueId,
    key: ValueId,
    elem: ValueId,
    data_ptr: ValueId,
    len: ValueId,
    cap: ValueId,
    elem_ty: Idx,
    cow_mode: CowMode,
    list_ty: Idx,
    stack_slot_receiver: bool,
    compact_stack_receiver: bool,
    func_id: FunctionId,
}

/// Read and report the result-buffer metadata-store ablation toggle.
fn push_result_elem_header_store_disabled() -> bool {
    let disabled = std::env::var_os("ORI_DISABLE_PUSH_RESULT_ELEM_HEADER_STORE").is_some();
    if disabled {
        tracing::info!(
            toggle = "ORI_DISABLE_PUSH_RESULT_ELEM_HEADER_STORE",
            effect = "skip result-buffer element destructor metadata stores",
            "ablation toggle fired"
        );
    }
    disabled
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Shared out-param `sret`-style runtime-call tail for list COW mutation
    /// methods: allocates the `{i64,i64,ptr}` out slot, calls `func_id` with
    /// `args` followed by the out pointer, and loads the resulting list
    /// value. Every COW list method in this file shares this exact tail —
    /// only the runtime function and its argument list differ per method.
    fn emit_list_cow_call(
        &mut self,
        func_id: FunctionId,
        label: &str,
        mut args: Vec<ValueId>,
    ) -> Option<ValueId> {
        let list_struct_ty = self.list_struct_type();
        let out =
            self.builder
                .create_entry_alloca(self.current_function, "list.cow.out", list_struct_ty);
        args.push(out);
        self.emit_rt_call(func_id, &args, label);
        Some(self.builder.load(list_struct_ty, out, "list.cow.val"))
    }

    /// Emit `list.push(x)` — COW push returning the (possibly mutated) list.
    ///
    /// Fast path (unique + capacity): appends in place, O(1).
    /// Slow path (shared): copies to new buffer, O(n).
    pub(crate) fn emit_list_push_cow(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
        receiver_returned: bool,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_push_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "push.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        let result = self.emit_list_cow_call(
            func_id,
            "push",
            vec![
                data_ptr,
                len,
                cap,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )?;
        // INVARIANT: only returned receivers own the element keep-alive credit.
        let header_store_disabled = push_result_elem_header_store_disabled();
        if !receiver_returned || header_store_disabled {
            return Some(result);
        }
        let result_data = self
            .builder
            .extract_value(result, FIELD_DATA, "push.data")?;
        let result_len = self.builder.extract_value(result, FIELD_LEN, "push.len")?;
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let store_dec = self.builder.runtime_fn("ori_buffer_store_elem_dec");
        self.builder
            .call(store_dec, &[result_data, elem_dec_fn], "");
        let store_count = self.builder.runtime_fn("ori_buffer_store_elem_count");
        self.builder
            .call(store_count, &[result_data, result_len], "");
        Some(result)
    }

    /// Emit `list.set(index, value)` — COW index assignment returning modified list.
    ///
    /// Fast path (unique): overwrites element at index in place.
    /// Slow path (shared): copies buffer, overwrites target index.
    pub(crate) fn emit_list_set_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_set_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "set.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "set",
            vec![
                data_ptr,
                len,
                cap,
                index,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )
    }

    /// Emit `list.updated(key, value)` — COW replacement returning modified list
    /// (`IndexSet.updated`).
    ///
    /// Fast path (unique): releases the replaced element, overwrites in place.
    /// Slow path (shared): copies buffer, overwrites target index.
    /// Out-of-bounds keys PANIC in the runtime (matching `list[key]`).
    ///
    /// The value is MOVED into the list (`arg_ownership` marks it `Owned`, the
    /// runtime takes the caller's reference) — no caller-side `RcDec` follows.
    pub(crate) fn emit_list_updated_cow(
        &mut self,
        receiver: ValueId,
        key: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: CowMode,
        list_ty: Idx,
        stack_slot_receiver: bool,
        compact_stack_receiver: bool,
        negated_same_index: bool,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_updated_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let bool_elem = self.pool.tag(self.pool.resolve_fully(elem_ty)) == Tag::Bool;
        if bool_elem
            && stack_slot_receiver
            && cow_mode == CowMode::StaticUnique
            && negated_same_index
        {
            let elem_llvm_ty = self
                .builder
                .register_type(self.builder.scx().type_i8().into());
            let elem_ptr =
                self.builder
                    .gep(elem_llvm_ty, data_ptr, &[key], "updated.toggle.elem_ptr");
            let old = self
                .builder
                .load(elem_llvm_ty, elem_ptr, "updated.toggle.old");
            let one = self.builder.const_i8(1);
            let toggled = self.builder.xor(old, one, "updated.toggle.value");
            self.builder.store(toggled, elem_ptr);
            return Some(receiver);
        }
        // Scalar replacement needs no element retain/release callbacks. Emit
        // the dynamic uniqueness check and the unique overwrite in LLVM so a
        // loop does not cross the runtime ABI for every element. Slices,
        // shared buffers, and out-of-bounds indices retain the exact runtime
        // path (including its panic and unwind behavior).
        if self.classifier.is_scalar(elem_ty) {
            return self.emit_scalar_list_updated_cow(&ScalarUpdatedArgs {
                receiver,
                key,
                elem,
                data_ptr,
                len,
                cap,
                elem_ty,
                cow_mode,
                list_ty,
                stack_slot_receiver,
                compact_stack_receiver,
                func_id,
            });
        }

        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "updated.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);
        let dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
        let cow_mode = self.builder.const_i32(cow_mode as i32);

        self.emit_list_cow_call(
            func_id,
            "updated",
            vec![
                data_ptr,
                len,
                cap,
                key,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                dec_fn,
                cow_mode,
            ],
        )
    }

    fn emit_scalar_list_updated_cow(&mut self, args: &ScalarUpdatedArgs) -> Option<ValueId> {
        let list_struct_ty = self.list_struct_type();
        let out = self.builder.create_entry_alloca(
            self.current_function,
            "list.updated.fallback",
            list_struct_ty,
        );
        let check_unique = self
            .builder
            .append_block(self.current_function, "updated.check_unique");
        let direct = self
            .builder
            .append_block(self.current_function, "updated.direct");
        let fallback = self
            .builder
            .append_block(self.current_function, "updated.fallback");
        let merge = self
            .builder
            .append_block(self.current_function, "updated.merge");

        let zero = self.builder.const_i64(0);
        let in_bounds = self
            .builder
            .icmp_ult(args.key, args.len, "updated.in_bounds");
        let may_update = if args.stack_slot_receiver && args.cow_mode == CowMode::StaticUnique {
            in_bounds
        } else {
            let regular = self.builder.icmp_sge(args.cap, zero, "updated.regular");
            self.builder.and(regular, in_bounds, "updated.may_update")
        };
        self.builder.cond_br(may_update, check_unique, fallback);

        self.emit_scalar_updated_uniqueness_branch(args, check_unique, direct, fallback);

        self.builder.position_at_end(direct);
        let collection_idx = self.pool.resolve_fully(args.list_ty);
        let bool_elem = self.pool.tag(self.pool.resolve_fully(args.elem_ty)) == Tag::Bool;
        let elem_llvm_ty = if bool_elem {
            self.builder
                .register_type(self.builder.scx().type_i8().into())
        } else {
            self.collection_elem_llvm_type(collection_idx, args.elem_ty)
        };
        let stored = if bool_elem {
            self.builder
                .zext(args.elem, elem_llvm_ty, "updated.elem.bool")
        } else {
            self.trunc_for_narrowed_collection_element(
                args.elem,
                collection_idx,
                "updated.elem.trunc",
            )
        };
        let dst = self
            .builder
            .gep(elem_llvm_ty, args.data_ptr, &[args.key], "updated.elem_ptr");
        self.builder.store(stored, dst);
        let direct_end = self.builder.current_block().expect("direct update block");
        self.builder.br(merge);

        self.builder.position_at_end(fallback);
        let fallback_value = self.emit_scalar_updated_fallback(args, out, list_struct_ty);
        let fallback_end = self.builder.current_block().expect("fallback update block");
        self.builder.br(merge);

        self.builder.position_at_end(merge);
        let result = self.builder.phi(list_struct_ty, "updated.value");
        self.builder.add_phi_incoming(
            result,
            &[(args.receiver, direct_end), (fallback_value, fallback_end)],
        );
        Some(result)
    }

    fn emit_scalar_updated_uniqueness_branch(
        &mut self,
        args: &ScalarUpdatedArgs,
        check_unique: BlockId,
        direct: BlockId,
        fallback: BlockId,
    ) {
        self.builder.position_at_end(check_unique);
        match args.cow_mode {
            CowMode::StaticUnique => self.builder.br(direct),
            CowMode::StaticShared => self.builder.br(fallback),
            CowMode::Dynamic => {
                let i8_ty = self.builder.i8_type();
                let rc_offset = self.builder.const_i64(-8);
                let rc_ptr = self
                    .builder
                    .gep(i8_ty, args.data_ptr, &[rc_offset], "updated.rc_ptr");
                let i64_ty = self.builder.i64_type();
                let rc = self.builder.load(i64_ty, rc_ptr, "updated.rc");
                let one = self.builder.const_i64(1);
                let unique = self.builder.icmp_eq(rc, one, "updated.unique");
                self.builder.cond_br(unique, direct, fallback);
            }
        }
    }

    fn emit_scalar_updated_fallback(
        &mut self,
        args: &ScalarUpdatedArgs,
        out: ValueId,
        list_struct_ty: LLVMTypeId,
    ) -> ValueId {
        if args.compact_stack_receiver {
            let panic_fn = self.builder.runtime_fn("ori_panic_index_out_of_bounds");
            self.emit_rt_call(panic_fn, &[args.key, args.len], "updated.oob");
        } else {
            let elem_ptr = self.elem_to_ptr(args.elem, args.elem_ty, "updated.elem");
            let (elem_size_val, elem_align_val) =
                self.elem_size_and_align(args.elem_ty, Some(args.list_ty));
            let inc_fn = self.get_or_generate_elem_inc_fn(args.elem_ty);
            let dec_fn = self.get_or_generate_elem_dec_fn(args.elem_ty);
            let cow_mode = self.builder.const_i32(args.cow_mode as i32);
            self.emit_rt_call(
                args.func_id,
                &[
                    args.data_ptr,
                    args.len,
                    args.cap,
                    args.key,
                    elem_ptr,
                    elem_size_val,
                    elem_align_val,
                    inc_fn,
                    dec_fn,
                    cow_mode,
                    out,
                ],
                "updated",
            );
        }
        if args.stack_slot_receiver && args.cow_mode == CowMode::StaticUnique {
            args.receiver
        } else {
            self.builder
                .load(list_struct_ty, out, "updated.fallback_value")
        }
    }

    /// Emit `list.insert(index, value)` — COW insert returning modified list.
    ///
    /// Fast path (unique + capacity): memmove + write in place.
    /// Slow path (shared): new allocation with element inserted.
    pub(crate) fn emit_list_insert_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_insert_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let elem_ptr = self.elem_to_ptr(elem, elem_ty, "insert.elem");
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "insert",
            vec![
                data_ptr,
                len,
                cap,
                index,
                elem_ptr,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )
    }

    /// Emit `list.prepend(value)` through the canonical index-zero insert path.
    pub(crate) fn emit_list_prepend_cow(
        &mut self,
        receiver: ValueId,
        elem: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let index = self.builder.const_i64(0);
        self.emit_list_insert_cow(receiver, index, elem, elem_ty, cow_mode, list_ty)
    }

    /// Emit `list.remove(index)` — COW remove returning modified list.
    ///
    /// Fast path (unique): memmove shift left in place.
    /// Slow path (shared): new allocation without the removed element.
    pub(crate) fn emit_list_remove_cow(
        &mut self,
        receiver: ValueId,
        index: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_remove_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "remove",
            vec![
                data_ptr,
                len,
                cap,
                index,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )
    }

    /// Emit `list.concat(other)` / `list.add(other)` — COW concatenation.
    ///
    /// Both lists are consumed (ownership transferred to the runtime). The
    /// runtime checks uniqueness at runtime to skip RC increments when the
    /// source list is uniquely owned.
    pub(crate) fn emit_list_concat_cow(
        &mut self,
        receiver: ValueId,
        other: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_concat_cow");

        let (data1, len1, cap1) = self.extract_list_fields(receiver)?;
        let (data2, len2, cap2) = self.extract_list_fields(other)?;
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "concat",
            vec![
                data1,
                len1,
                cap1,
                data2,
                len2,
                cap2,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )
    }

    /// Emit `list.reverse()` — COW reverse returning the reversed list.
    ///
    /// Fast path (unique): swaps pairs from both ends inward, O(n), no allocation.
    /// Slow path (shared): new allocation with elements in reverse order.
    pub(crate) fn emit_list_reverse_cow(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_reverse_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "reverse",
            vec![
                data_ptr,
                len,
                cap,
                elem_size_val,
                elem_align_val,
                inc_fn,
                cow_mode,
            ],
        )
    }

    /// Emit `list.sort()` — COW sort returning the sorted list.
    ///
    /// Generates a comparison thunk function for the element type, then passes
    /// it to `ori_list_sort_cow`. The thunk has signature
    /// `fn(*const u8, *const u8) -> i32` and loads elements by type before
    /// comparing.
    ///
    /// Supports primitive element types (int, float, bool, char, byte, str).
    pub(crate) fn emit_list_sort_cow(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        // Use narrowed compare thunk for narrowed int lists.
        let compare_fn_ptr = self
            .get_or_create_narrowed_compare_thunk(list_ty)
            .or_else(|| self.get_or_create_compare_thunk(elem_ty))?;

        let func_id = self.builder.runtime_fn("ori_list_sort_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "sort",
            vec![
                data_ptr,
                len,
                cap,
                elem_size_val,
                elem_align_val,
                compare_fn_ptr,
                inc_fn,
                cow_mode,
            ],
        )
    }

    /// Emit a stable sort (`TimSort`) — preserves relative order of equal elements.
    /// Identical to `emit_list_sort_cow` but calls `ori_list_sort_stable_cow`.
    pub(crate) fn emit_list_sort_stable_cow(
        &mut self,
        receiver: ValueId,
        elem_ty: Idx,
        cow_mode: ValueId,
        list_ty: Idx,
    ) -> Option<ValueId> {
        // Use narrowed compare thunk for narrowed int lists.
        let compare_fn_ptr = self
            .get_or_create_narrowed_compare_thunk(list_ty)
            .or_else(|| self.get_or_create_compare_thunk(elem_ty))?;

        let func_id = self.builder.runtime_fn("ori_list_sort_stable_cow");

        let (data_ptr, len, cap) = self.extract_list_fields(receiver)?;
        let (elem_size_val, elem_align_val) = self.elem_size_and_align(elem_ty, Some(list_ty));
        let inc_fn = self.get_or_generate_elem_inc_fn(elem_ty);

        self.emit_list_cow_call(
            func_id,
            "sort_stable",
            vec![
                data_ptr,
                len,
                cap,
                elem_size_val,
                elem_align_val,
                compare_fn_ptr,
                inc_fn,
                cow_mode,
            ],
        )
    }
}
