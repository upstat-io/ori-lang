//! Bounded local yield storage emission.

use ori_ir::{FIELD_DATA, FIELD_LEN};
use ori_types::Idx;

use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;
use super::planned_runtime_i64;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    pub(super) fn emit_local_yield_push(
        &mut self,
        builder: ValueId,
        elem: ValueId,
        collection_ty: Idx,
        elem_ty: Idx,
        elem_size: u64,
        requires_runtime_header: bool,
    ) {
        let list_ty = self.fat_ptr_llvm_type();
        let len_ptr =
            self.builder
                .struct_gep(list_ty, builder, FIELD_LEN, "yield.local.push.len_ptr");

        let cap_ptr = self.builder.struct_gep(
            list_ty,
            builder,
            ori_ir::FIELD_CAP,
            "yield.local.push.cap_ptr",
        );

        let data_ptr_ptr = self.builder.struct_gep(
            list_ty,
            builder,
            FIELD_DATA,
            "yield.local.push.data_ptr_ptr",
        );
        let i64_ty = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();
        let len = self.builder.load(i64_ty, len_ptr, "yield.local.push.len");
        let cap = self.builder.load(i64_ty, cap_ptr, "yield.local.push.cap");
        let data = self
            .builder
            .load(ptr_ty, data_ptr_ptr, "yield.local.push.data");

        let has_capacity = self
            .builder
            .icmp_ult(len, cap, "yield.local.push.has_capacity");

        let Some(()) = self.emit_unwrap_branch(
            has_capacity,
            "compiler's bounded-yield capacity proof was violated; report this compiler bug",
            "yield.local.push.capacity",
        ) else {
            // Why: `emit_unwrap_branch` always positions and returns its continuation block.
            unreachable!("local yield capacity guard must emit its continuation");
        };
        let elem_llvm_ty = self.int_element_llvm_type(collection_ty, elem_ty);
        let stored = if self.pool.tag(self.pool.resolve_fully(elem_ty)) == ori_types::Tag::Int
            && elem_size < 8
        {
            self.builder
                .trunc(elem, elem_llvm_ty, "yield.local.push.elem.trunc")
        } else {
            elem
        };

        let dst = self
            .builder
            .gep(elem_llvm_ty, data, &[len], "yield.local.push.elem_ptr");
        self.builder.store(stored, dst);
        let one = self.builder.const_i64(1);
        let next_len = self.builder.add(len, one, "yield.local.push.next_len");
        self.builder.store(next_len, len_ptr);
        if requires_runtime_header {
            let i8_ty = self.builder.i8_type();
            let elem_dec_offset = self.builder.const_i64(-24);
            let elem_dec_ptr = self.builder.gep(
                i8_ty,
                data,
                &[elem_dec_offset],
                "yield.local.push.elem_dec_ptr",
            );
            let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);
            self.builder.store(elem_dec_fn, elem_dec_ptr);
            let elem_count_offset = self.builder.const_i64(-16);
            let elem_count_ptr = self.builder.gep(
                i8_ty,
                data,
                &[elem_count_offset],
                "yield.local.push.elem_count_ptr",
            );
            self.builder.store(next_len, elem_count_ptr);
        }
    }

    pub(super) fn emit_local_yield_builder(
        &mut self,
        capacity: u64,
        elem_size: u64,
        requires_runtime_header: bool,
    ) -> ValueId {
        const RC_HEADER_SIZE: u32 = 32;
        const LOCAL_DATA_SIZE: i64 = -1;

        let Some(bytes) = capacity.checked_mul(elem_size.max(1)).and_then(|size| {
            size.checked_add(if requires_runtime_header {
                u64::from(RC_HEADER_SIZE)
            } else {
                0
            })
        }) else {
            // Why: Representation planning admits only checked local-yield allocation sizes.
            unreachable!("local yield byte size must fit u64");
        };
        let Ok(bytes) = u32::try_from(bytes) else {
            // Why: The local-yield threshold bounds every admitted LLVM stack-array length.
            unreachable!("local yield byte size must fit the LLVM u32 array length");
        };
        let byte_array_ty = self.builder.byte_array_type(bytes);
        let storage = self.builder.create_entry_alloca_aligned(
            self.current_function,
            "yield.local.data",
            byte_array_ty,
            8,
        );
        let i8_ty = self.builder.i8_type();
        let zero = self.builder.const_i64(0);
        let data = if requires_runtime_header {
            let offset = self.builder.const_i64(i64::from(RC_HEADER_SIZE));
            let data = self
                .builder
                .gep(i8_ty, storage, &[offset], "yield.local.elements");

            let data_size = self.builder.const_i64(LOCAL_DATA_SIZE);
            self.builder.store(data_size, storage);
            let elem_dec_offset = self.builder.const_i64(8);
            let elem_dec_ptr =
                self.builder
                    .gep(i8_ty, storage, &[elem_dec_offset], "yield.local.elem_dec");
            let null = self.builder.const_null_ptr();
            self.builder.store(null, elem_dec_ptr);
            let elem_count_offset = self.builder.const_i64(16);
            let elem_count_ptr = self.builder.gep(
                i8_ty,
                storage,
                &[elem_count_offset],
                "yield.local.elem_count",
            );
            self.builder.store(zero, elem_count_ptr);
            let strong_count_offset = self.builder.const_i64(24);
            let strong_count_ptr = self.builder.gep(
                i8_ty,
                storage,
                &[strong_count_offset],
                "yield.local.strong_count",
            );
            let one = self.builder.const_i64(1);
            self.builder.store(one, strong_count_ptr);
            data
        } else {
            storage
        };

        let list_ty = self.fat_ptr_llvm_type();
        let builder = self.builder.create_entry_alloca_aligned(
            self.current_function,
            "yield.local.builder",
            list_ty,
            8,
        );

        let len_ptr =
            self.builder
                .struct_gep(list_ty, builder, ori_ir::FIELD_LEN, "yield.local.len");

        let cap_ptr =
            self.builder
                .struct_gep(list_ty, builder, ori_ir::FIELD_CAP, "yield.local.cap");

        let data_ptr =
            self.builder
                .struct_gep(list_ty, builder, ori_ir::FIELD_DATA, "yield.local.data_ptr");
        self.builder.store(zero, len_ptr);
        let capacity = self
            .builder
            .const_i64(planned_runtime_i64(capacity, "yield capacity"));
        self.builder.store(capacity, cap_ptr);
        self.builder.store(data, data_ptr);
        builder
    }

    pub(super) fn emit_local_yield_take(&mut self, builder: ValueId) -> ValueId {
        let list_ty = self.fat_ptr_llvm_type();
        self.builder.load(list_ty, builder, "yield.local.list")
    }

    pub(super) fn emit_local_yield_free(&mut self, builder: ValueId, elem_size: u64) {
        let list_ty = self.fat_ptr_llvm_type();
        let list = self
            .builder
            .load(list_ty, builder, "yield.local.cleanup.list");

        let (Some(data), Some(len), Some(cap)) = (
            self.builder
                .extract_value(list, ori_ir::FIELD_DATA, "yield.local.cleanup.data"),
            self.builder
                .extract_value(list, ori_ir::FIELD_LEN, "yield.local.cleanup.len"),
            self.builder
                .extract_value(list, ori_ir::FIELD_CAP, "yield.local.cleanup.cap"),
        ) else {
            // Why: The fat-list LLVM type always defines all three runtime list fields.
            unreachable!("fat-list cleanup value must contain data, length, and capacity fields");
        };
        let elem_size = self
            .builder
            .const_i64(planned_runtime_i64(elem_size, "yield element size"));
        let null = self.builder.const_null_ptr();
        let free = self.builder.runtime_fn("ori_buffer_rc_dec");
        self.builder
            .call(free, &[data, len, cap, elem_size, null], "");
    }
}
