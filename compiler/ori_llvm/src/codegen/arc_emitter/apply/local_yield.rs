//! Bounded local and length-only yield allocation emission.

use super::super::{ArcIrEmitter, EmittedValue};
use crate::codegen::value_id::ValueId;
use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::{Name, FIELD_DATA, FIELD_LEN};
use ori_types::Idx;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    pub(super) fn try_emit_local_yield_apply(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> bool {
        if callee == self.list_rt_names.new {
            return self.try_emit_local_yield_new(dst, func);
        }

        if callee == self.list_rt_names.push && args.len() == 3 {
            return self.try_emit_local_yield_push(dst, args, func);
        }

        if callee == self.list_rt_names.take && args.len() == 1 {
            return self.try_emit_local_yield_take(dst, args[0], func);
        }

        if callee == self.list_rt_names.free && args.len() == 2 {
            return self.try_emit_local_yield_free(dst, args[0], func);
        }

        false
    }

    fn try_emit_local_yield_new(&mut self, dst: ArcVarId, func: &ArcFunction) -> bool {
        let Some(decision) = self
            .repr_plan
            .and_then(|plan| plan.yield_allocation_for_builder(func.name, dst))
        else {
            return false;
        };

        let ori_arc::ir::YieldExtent::StaticExact(capacity) = decision.extent else {
            return false;
        };

        let builder = if self.length_only_yield_result == Some(decision.result) {
            self.emit_length_only_yield_builder(capacity)
        } else {
            if decision.mechanism != ori_repr::CompiledAllocationMechanism::StackSlot {
                return false;
            }
            self.emit_local_yield_builder(
                capacity,
                decision.elem_size,
                decision.requires_runtime_header,
            )
        };
        self.def_var_repr(dst, builder, func);
        true
    }

    fn try_emit_local_yield_push(
        &mut self,
        dst: ArcVarId,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> bool {
        let Some(decision) = self
            .repr_plan
            .and_then(|plan| plan.yield_allocation_for_builder(func.name, args[0]))
        else {
            return false;
        };
        let exact_heap = decision.mechanism == ori_repr::CompiledAllocationMechanism::RuntimeHeap
            && matches!(decision.extent, ori_arc::ir::YieldExtent::StaticExact(_));
        if self.length_only_yield_result == Some(decision.result) {
            let ori_arc::ir::YieldExtent::StaticExact(capacity) = decision.extent else {
                return false;
            };
            self.emit_length_only_yield_push(self.var(args[0]), capacity);
        } else {
            if decision.mechanism != ori_repr::CompiledAllocationMechanism::StackSlot && !exact_heap
            {
                return false;
            }
            self.emit_local_yield_push(
                self.var(args[0]),
                self.var(args[1]),
                func.var_type(decision.result),
                func.var_type(args[1]),
                decision.elem_size,
                decision.requires_runtime_header,
            );
        }
        let unit = self.builder.const_i64(0);
        self.def_var(dst, EmittedValue::Immediate(unit));
        true
    }

    fn try_emit_local_yield_take(
        &mut self,
        dst: ArcVarId,
        builder_var: ArcVarId,
        func: &ArcFunction,
    ) -> bool {
        let Some(decision) = self
            .repr_plan
            .and_then(|plan| plan.yield_allocation_for_result(func.name, dst))
        else {
            return false;
        };
        if decision.builder != builder_var {
            return false;
        }
        let result = if self.length_only_yield_result == Some(decision.result) {
            let ori_arc::ir::YieldExtent::StaticExact(capacity) = decision.extent else {
                return false;
            };
            self.emit_length_only_yield_take(self.var(builder_var), capacity)
        } else {
            if decision.mechanism != ori_repr::CompiledAllocationMechanism::StackSlot {
                return false;
            }
            self.emit_local_yield_take(self.var(builder_var))
        };
        self.def_var_repr(dst, result, func);
        true
    }

    fn try_emit_local_yield_free(
        &mut self,
        dst: ArcVarId,
        builder_var: ArcVarId,
        func: &ArcFunction,
    ) -> bool {
        let Some(decision) = self
            .repr_plan
            .and_then(|plan| plan.yield_allocation_for_builder(func.name, builder_var))
        else {
            return false;
        };
        if self.length_only_yield_result != Some(decision.result) {
            if decision.mechanism != ori_repr::CompiledAllocationMechanism::StackSlot {
                return false;
            }
            if decision.requires_runtime_header {
                self.emit_local_yield_free(self.var(builder_var), decision.elem_size);
            }
        }
        let unit = self.builder.const_i64(0);
        self.def_var(dst, EmittedValue::Immediate(unit));
        true
    }

    fn emit_length_only_yield_builder(&mut self, capacity: u64) -> ValueId {
        let narrow = i32::try_from(capacity).is_ok();
        let count_ty = if narrow {
            self.builder.i32_type()
        } else {
            self.builder.i64_type()
        };

        let builder = self.builder.create_entry_alloca_aligned(
            self.current_function,
            "yield.length_only.count",
            count_ty,
            8,
        );

        let zero = if narrow {
            self.builder.const_i32(0)
        } else {
            self.builder.const_i64(0)
        };
        self.builder.store(zero, builder);
        builder
    }

    fn emit_length_only_yield_push(&mut self, builder: ValueId, capacity: u64) {
        let narrow = i32::try_from(capacity).is_ok();
        let count_ty = if narrow {
            self.builder.i32_type()
        } else {
            self.builder.i64_type()
        };

        let len = self
            .builder
            .load(count_ty, builder, "yield.length_only.push.len");

        // The private projection clone exists only for a yield whose neutral
        // extent fact supplies a static upper bound. This builder owns no
        // element storage, so incrementing its count cannot cross a memory
        // boundary; the ordinary materializing function retains the guarded
        // push that enforces the physical allocation contract.
        let one = if narrow {
            self.builder.const_i32(1)
        } else {
            self.builder.const_i64(1)
        };

        let next_len = self
            .builder
            .add(len, one, "yield.length_only.push.next_len");
        self.builder.store(next_len, builder);
    }

    fn emit_length_only_yield_take(&mut self, builder: ValueId, capacity: u64) -> ValueId {
        let narrow = i32::try_from(capacity).is_ok();
        let count_ty = if narrow {
            self.builder.i32_type()
        } else {
            self.builder.i64_type()
        };

        let count = self
            .builder
            .load(count_ty, builder, "yield.length_only.result.count");

        let count = if narrow {
            let i64_ty = self.builder.i64_type();
            self.builder
                .zext(count, i64_ty, "yield.length_only.result.len")
        } else {
            count
        };
        let list_ty = self.fat_ptr_llvm_type();
        let cap = self.builder.const_i64(capacity as i64);
        let data = self.builder.const_null_ptr();
        self.builder
            .build_struct(list_ty, &[count, cap, data], "yield.length_only.result")
    }

    fn emit_local_yield_push(
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

        self.emit_unwrap_branch(
            has_capacity,
            "compiler's bounded-yield capacity proof was violated; report this compiler bug",
            "yield.local.push.capacity",
        )
        .expect("local yield capacity guard emits its continuation");
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

    fn emit_local_yield_builder(
        &mut self,
        capacity: u64,
        elem_size: u64,
        requires_runtime_header: bool,
    ) -> ValueId {
        const RC_HEADER_SIZE: u64 = 32;
        const LOCAL_DATA_SIZE: i64 = -1;

        let bytes = capacity
            .checked_mul(elem_size.max(1))
            .and_then(|size| {
                size.checked_add(if requires_runtime_header {
                    RC_HEADER_SIZE
                } else {
                    0
                })
            })
            .expect("representation plan admitted a checked local yield size");
        let bytes = u32::try_from(bytes).expect("local yield threshold fits LLVM array length");
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
            let offset = self.builder.const_i64(RC_HEADER_SIZE as i64);
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
        let capacity = self.builder.const_i64(capacity as i64);
        self.builder.store(capacity, cap_ptr);
        self.builder.store(data, data_ptr);
        builder
    }

    fn emit_local_yield_take(&mut self, builder: ValueId) -> ValueId {
        let list_ty = self.fat_ptr_llvm_type();
        self.builder.load(list_ty, builder, "yield.local.list")
    }

    fn emit_local_yield_free(&mut self, builder: ValueId, elem_size: u64) {
        let list_ty = self.fat_ptr_llvm_type();
        let list = self
            .builder
            .load(list_ty, builder, "yield.local.cleanup.list");

        let Some(data) =
            self.builder
                .extract_value(list, ori_ir::FIELD_DATA, "yield.local.cleanup.data")
        else {
            return;
        };

        let Some(len) =
            self.builder
                .extract_value(list, ori_ir::FIELD_LEN, "yield.local.cleanup.len")
        else {
            return;
        };

        let Some(cap) =
            self.builder
                .extract_value(list, ori_ir::FIELD_CAP, "yield.local.cleanup.cap")
        else {
            return;
        };
        let elem_size = self.builder.const_i64(elem_size as i64);
        let null = self.builder.const_null_ptr();
        let free = self.builder.runtime_fn("ori_buffer_rc_dec");
        self.builder
            .call(free, &[data, len, cap, elem_size, null], "");
    }
}
