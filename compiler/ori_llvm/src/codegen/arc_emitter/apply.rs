//! ABI-aware direct and indirect ARC call emission.
//! Internal protocols, casts, and method resolution precede ordinary ABI dispatch.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::canon::MonoInstanceId;
use ori_ir::{Name, CLOSURE_FIELD_ENV, CLOSURE_FIELD_FN, FIELD_DATA, FIELD_LEN};
use ori_types::Idx;

use super::{ArcIrEmitter, EmittedValue, StringRuntimeReturnAbi};
use crate::codegen::abi::{ParamAbi, ReturnAbi, ReturnPassing};
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit an `Apply` instruction (ABI-aware direct call).
    pub(super) fn emit_apply(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
        mono_instance_id: Option<MonoInstanceId>,
    ) {
        let runtime_projection_allowed = self.runtime_projection_allowed(func, dst);

        if runtime_projection_allowed && self.try_emit_local_yield_apply(dst, callee, args, func) {
            return;
        }

        if runtime_projection_allowed && self.try_emit_apply_special(dst, callee, args, func) {
            return;
        }

        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();

        let result = match self.resolve_callee(callee, args, dst, func, mono_instance_id) {
            Some((func_id, params, ret_abi)) => {
                self.emit_resolved_direct_call(func_id, &params, ret_abi, &arg_vals, args)
            }
            None if runtime_projection_allowed => {
                self.emit_runtime_projection_fallback(dst, callee, args, &arg_vals, func)
            }
            None => self.record_unresolved_direct_call(dst, callee, func),
        };

        // INVARIANT: Record destructor metadata after each push because reallocation can
        // change the scratch buffer before an unwind cleanup releases its elements.
        if runtime_projection_allowed && callee == self.list_rt_names.push && args.len() == 3 {
            self.record_list_builder_element_header(arg_vals[0], func.var_type(args[1]));
        }

        if let Some(val) = result {
            self.def_var_repr(dst, val, func);
        } else if !self.builder.has_codegen_errors() {
            // INVARIANT: Every Apply defines its destination, including void calls.
            let unit = self.builder.const_i64(0);
            self.def_var(dst, EmittedValue::Immediate(unit));
        }
    }

    /// Emit an `ApplyIndirect` instruction (indirect call through closure).
    pub(super) fn emit_apply_indirect(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        closure: ArcVarId,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) {
        let closure_val = self.var(closure);
        tracing::trace!(
            ?ty,
            tag = ?self.pool.tag(ty),
            closure_var = closure.raw(),
            args = args.len(),
            "emit_apply_indirect"
        );
        let fn_ptr = self
            .builder
            .extract_value(closure_val, CLOSURE_FIELD_FN, "closure.fn_ptr");
        let env_ptr = self
            .builder
            .extract_value(closure_val, CLOSURE_FIELD_ENV, "closure.env_ptr");
        let (Some(fn_ptr), Some(env_ptr)) = (fn_ptr, env_ptr) else {
            tracing::error!(
                closure_var = closure.raw(),
                "emit_apply_indirect: extract_value failed — fn_ptr or env_ptr is None"
            );
            return;
        };

        let (arg_vals, param_types) = self.marshal_indirect_call_args(env_ptr, args, func);

        let ret_ty = self.resolve_type(ty);
        let ret_is_indirect =
            crate::codegen::abi::abi_size(ty, self.type_info, self.repr_plan) > 16;
        tracing::trace!(
            ?ty,
            resolved_llvm_ty = ?self.builder.arena.get_type(ret_ty),
            ret_is_indirect,
            "emit_apply_indirect: resolved return type"
        );

        if ret_is_indirect {
            self.emit_indirect_call_sret(dst, ret_ty, &param_types, fn_ptr, &arg_vals, func);
        } else {
            self.emit_indirect_call_direct(dst, ret_ty, &param_types, fn_ptr, &arg_vals, func);
        }
    }

    // String runtime call helpers

    /// Call a string runtime function: `ori_str_concat`, `ori_str_eq`, `ori_str_ne`.
    ///
    /// String values are `{ i64, i64, ptr }` structs passed by pointer to the runtime.
    /// `return_abi` selects sret `{ i64, i64, ptr }` or direct `i1` return.
    pub(super) fn emit_str_runtime_call(
        &mut self,
        func_name: &'static str,
        lhs: ValueId,
        rhs: ValueId,
        return_abi: StringRuntimeReturnAbi,
    ) -> ValueId {
        let func_id = self.builder.runtime_fn(func_name);

        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let lhs_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_op.lhs", str_ty);
        self.builder.store(lhs, lhs_ptr);
        let rhs_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_op.rhs", str_ty);
        self.builder.store(rhs, rhs_ptr);

        match return_abi {
            StringRuntimeReturnAbi::StringSret => self
                .builder
                .call_with_sret(func_id, &[lhs_ptr, rhs_ptr], str_ty, func_name)
                .expect("str-returning runtime call uses sret; builder yields the loaded value"),
            StringRuntimeReturnAbi::BoolDirect => {
                let result = self.emit_rt_call(func_id, &[lhs_ptr, rhs_ptr], func_name);
                result.expect("str comparison runtime fn is non-void; builder.call returns Some")
            }
        }
    }

    // Apply emission helpers

    fn try_emit_local_yield_apply(
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

    /// Emit a special-case `Apply` that bypasses ordinary callee resolution:
    /// protocol builtins, format calls, prelude functions, and traceless
    /// `Traceable` accessors. Returns `true` when the call was fully emitted
    /// and `dst` defined.
    fn try_emit_apply_special(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> bool {
        if self.try_emit_protocol(dst, callee, args, func) {
            return true;
        }

        let special = self
            .try_emit_format_call(callee, args, func)
            .or_else(|| {
                let callee_name = self.interner.lookup(callee);
                super::builtins::prelude::try_emit_prelude_function(
                    &mut *self,
                    callee_name,
                    args,
                    func,
                )
            })
            // Why: Traceless accessors have no backend declaration for normal resolution.
            .or_else(|| self.try_emit_traceless_traceable(callee, args, func, func.var_type(dst)));

        match special {
            Some(val) => {
                self.def_var_repr(dst, val, func);
                true
            }
            None => false,
        }
    }

    /// Emit a direct call to a resolved callee per its declared ABI.
    fn emit_resolved_direct_call(
        &mut self,
        func_id: FunctionId,
        params: &[ParamAbi],
        ret_abi: ReturnAbi,
        arg_vals: &[ValueId],
        args: &[ArcVarId],
    ) -> Option<ValueId> {
        let passed_args = self.apply_param_passing(arg_vals, Some(args), params);
        match &ret_abi.passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(ret_abi.ty);
                self.call_with_sret(func_id, &passed_args, ret_ty, "call")
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.emit_rt_call(func_id, &passed_args, "call")
            }
        }
    }

    /// Fallback chain for an unresolved callee when runtime projection is
    /// allowed: builtin method, builtin associated function, then a named
    /// `ori_*` runtime function; records a codegen error when nothing matches.
    fn emit_runtime_projection_fallback(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        arg_vals: &[ValueId],
        func: &ArcFunction,
    ) -> Option<ValueId> {
        if let Some(val) = self.try_emit_builtin_method(callee, args, func, func.var_type(dst)) {
            return Some(val);
        }
        if let Some(val) = self.try_emit_builtin_associated(callee, args, func.var_type(dst)) {
            return Some(val);
        }
        let callee_name = self.interner.lookup(callee);
        if let Some(func_id) = self.builder.try_runtime_fn(callee_name) {
            return self.emit_coerced_runtime_fn_call(func_id, dst, callee, args, arg_vals, func);
        }
        self.record_unresolved_direct_call(dst, callee, func)
    }

    /// Emit a call to a declared `ori_*` runtime function with coerced
    /// arguments, via sret when the function's declaration requires it.
    fn emit_coerced_runtime_fn_call(
        &mut self,
        func_id: FunctionId,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
        arg_vals: &[ValueId],
        func: &ArcFunction,
    ) -> Option<ValueId> {
        let coerced_args = self.coerce_runtime_fn_args(callee, args, arg_vals, func);
        let callee_name = self.interner.lookup(callee);

        if crate::codegen::runtime_decl::rt_fn_needs_sret(callee_name) {
            let ret_ty = self.resolve_type(func.var_type(dst));
            self.call_with_sret(func_id, &coerced_args, ret_ty, "call")
        } else {
            self.emit_rt_call(func_id, &coerced_args, "call")
        }
    }

    /// Record the unresolved-direct-call codegen error; always yields `None`.
    fn record_unresolved_direct_call(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        func: &ArcFunction,
    ) -> Option<ValueId> {
        let callee_name = self.interner.lookup(callee);
        let msg = self.unresolved_direct_call_message(func, dst, callee_name, "apply");
        tracing::warn!("{msg}");
        self.builder.record_codegen_error_with_msg(msg);
        None
    }

    fn record_list_builder_element_header(&mut self, list_ptr: ValueId, element_ty: Idx) {
        let list_struct_ty = self.fat_ptr_llvm_type();
        let len_ptr =
            self.builder
                .struct_gep(list_struct_ty, list_ptr, FIELD_LEN, "list_builder.len_ptr");
        let data_ptr = self.builder.struct_gep(
            list_struct_ty,
            list_ptr,
            FIELD_DATA,
            "list_builder.data_ptr",
        );
        let i64_ty = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();
        let len = self.builder.load(i64_ty, len_ptr, "list_builder.len");
        let data = self.builder.load(ptr_ty, data_ptr, "list_builder.data");
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(element_ty);
        let store_dec = self.builder.runtime_fn("ori_buffer_store_elem_dec");
        self.builder.call(store_dec, &[data, elem_dec_fn], "");
        let store_count = self.builder.runtime_fn("ori_buffer_store_elem_count");
        self.builder.call(store_count, &[data, len], "");
    }

    // Indirect-call emission helpers

    /// Marshal explicit closure arguments under the uniform borrowed ABI.
    pub(super) fn marshal_indirect_call_args(
        &mut self,
        env_ptr: ValueId,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> (Vec<ValueId>, Vec<LLVMTypeId>) {
        let ptr_ty = self.builder.ptr_type();
        let mut arg_vals = Vec::with_capacity(1 + args.len());
        let mut param_types = Vec::with_capacity(1 + args.len());
        arg_vals.push(env_ptr);
        param_types.push(ptr_ty);

        for &a in args {
            let arg_ty = func.var_type(a);
            let passing = crate::codegen::abi::compute_closure_param_passing(
                arg_ty,
                self.type_info,
                self.repr_plan,
                self.classifier,
            );
            match passing {
                crate::codegen::abi::ParamPassing::Indirect { .. }
                | crate::codegen::abi::ParamPassing::Reference => {
                    let llvm_ty = self.resolve_type(arg_ty);
                    let alloca = self.builder.alloca(llvm_ty, "icall.arg.tmp");
                    self.builder.store(self.var(a), alloca);
                    arg_vals.push(alloca);
                    param_types.push(ptr_ty);
                }
                crate::codegen::abi::ParamPassing::Void => {}
                crate::codegen::abi::ParamPassing::Direct => {
                    arg_vals.push(self.var(a));
                    param_types.push(self.resolve_type(arg_ty));
                }
            }
        }

        (arg_vals, param_types)
    }

    /// Emit an indirect call whose return is passed via sret.
    fn emit_indirect_call_sret(
        &mut self,
        dst: ArcVarId,
        ret_ty: LLVMTypeId,
        param_types: &[LLVMTypeId],
        fn_ptr: ValueId,
        arg_vals: &[ValueId],
        func: &ArcFunction,
    ) {
        // Why: ARM64 passes the closure sret pointer in X8, not as an argument.
        let sret_alloca = self.builder.alloca(ret_ty, "icall.sret");
        if let Some(pad) = self.current_cleanup_pad {
            // Why: Calls inside an SEH funclet require its operand bundle.
            self.builder.call_indirect_with_sret_and_funclet(
                ret_ty,
                param_types,
                fn_ptr,
                sret_alloca,
                arg_vals,
                pad,
            );
        } else {
            self.builder.call_indirect_with_sret(
                ret_ty,
                param_types,
                fn_ptr,
                sret_alloca,
                arg_vals,
            );
        }
        let loaded = self.builder.load(ret_ty, sret_alloca, "icall.sret.load");
        self.def_var_repr(dst, loaded, func);
    }

    /// Emit an indirect call whose return is passed directly (or is void).
    fn emit_indirect_call_direct(
        &mut self,
        dst: ArcVarId,
        ret_ty: LLVMTypeId,
        param_types: &[LLVMTypeId],
        fn_ptr: ValueId,
        arg_vals: &[ValueId],
        func: &ArcFunction,
    ) {
        let result = if let Some(pad) = self.current_cleanup_pad {
            self.builder.call_indirect_with_funclet(
                ret_ty,
                param_types,
                fn_ptr,
                arg_vals,
                pad,
                "icall",
            )
        } else {
            self.builder
                .call_indirect(ret_ty, param_types, fn_ptr, arg_vals, "icall")
        };
        if let Some(val) = result {
            self.def_var_repr(dst, val, func);
        }
    }
}

pub(super) fn closed_target_projection_message(target: &str, site: &str) -> String {
    format!(
        "LLVM did not declare closed executable target `{target}` before {site}; rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug"
    )
}

#[cfg(test)]
mod diagnostic_tests {
    use super::closed_target_projection_message;

    #[test]
    fn closed_target_diagnostic_states_cause_and_action() {
        let message = closed_target_projection_message("clone$derived$7", "apply");
        assert!(message.contains("did not declare closed executable target"));
        assert!(message.contains("ORI_VERIFY_ARC=1"));
        assert!(message.contains("report this compiler bug"));
        assert!(!message.contains("missing mono instance"));
    }
}
