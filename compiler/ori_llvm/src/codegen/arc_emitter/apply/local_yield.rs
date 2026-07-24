//! Bounded local and length-only yield allocation emission.

mod storage;

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::Name;

use crate::codegen::value_id::ValueId;

use super::super::{ArcIrEmitter, EmittedValue};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emits an apply whose frozen yield plan selects local or length-only storage.
    ///
    /// Returns `false` when the apply requires the canonical runtime path.
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

        let ori_arc::ir::YieldExtent::StaticExact(capacity) = decision.mechanism.extent() else {
            return false;
        };

        let builder = if self.length_only_yield_result == Some(decision.result) {
            self.emit_length_only_yield_builder(capacity)
        } else {
            if !decision.mechanism.is_stack() {
                return false;
            }
            self.emit_local_yield_builder(
                capacity,
                decision.elem_size,
                decision.mechanism.requires_runtime_header(),
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
        let exact_heap = matches!(
            decision.mechanism,
            ori_repr::CompiledAllocationMechanism::RuntimeHeap {
                extent: ori_arc::ir::YieldExtent::StaticExact(_)
            }
        );
        if self.length_only_yield_result == Some(decision.result) {
            let ori_arc::ir::YieldExtent::StaticExact(capacity) = decision.mechanism.extent()
            else {
                return false;
            };
            self.emit_length_only_yield_push(self.var(args[0]), capacity);
        } else {
            if !decision.mechanism.is_stack() && !exact_heap {
                return false;
            }
            self.emit_local_yield_push(
                self.var(args[0]),
                self.var(args[1]),
                func.var_type(decision.result),
                func.var_type(args[1]),
                decision.elem_size,
                decision.mechanism.requires_runtime_header(),
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
            let ori_arc::ir::YieldExtent::StaticExact(capacity) = decision.mechanism.extent()
            else {
                return false;
            };
            self.emit_length_only_yield_take(self.var(builder_var), capacity)
        } else {
            if !decision.mechanism.is_stack() {
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
            if !decision.mechanism.is_stack() {
                return false;
            }
            if decision.mechanism.requires_runtime_header() {
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

        // Why: Static extent bounds this storage-free counter without an allocation guard.
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
        let cap = self
            .builder
            .const_i64(planned_runtime_i64(capacity, "yield capacity"));
        let data = self.builder.const_null_ptr();
        self.builder
            .build_struct(list_ty, &[count, cap, data], "yield.length_only.result")
    }
}

fn planned_runtime_i64(value: u64, subject: &str) -> i64 {
    let Ok(value) = i64::try_from(value) else {
        // Why: Representation planning admits only values that fit runtime list ABI fields.
        unreachable!("planned {subject} must fit the runtime i64 ABI field");
    };
    value
}
