//! Closure-to-C-ABI trampolines for iterator operations.
//!
//! Each trampoline unpacks an Ori `{fn_ptr, env_ptr}` closure, loads pointer-based
//! inputs, invokes the Ori function, and writes or returns the C-ABI result.

#[cfg(any(test, doc))]
pub(super) const REGISTERED: &[super::BuiltinRegistration] = &[];

use ori_ir::{CLOSURE_FIELD_ENV, CLOSURE_FIELD_FN};
use ori_types::Idx;

use crate::codegen::abi::{
    compute_closure_param_passing, compute_return_passing, ParamPassing, ReturnPassing,
};
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

use super::super::ArcIrEmitter;

/// Trampoline variant for different iterator adapter calling conventions.
#[derive(Clone, Copy, Debug)]
pub(crate) enum TrampolineKind {
    /// `map`: `(env, in_ptr, out_ptr) -> void`
    /// Reads input element, calls closure, writes output element.
    Map,
    /// `filter`/`any`/`all`: `(env, elem_ptr) -> i8`
    /// Reads input element, calls closure, returns bool as i8.
    Predicate,
    /// `for_each`: `(env, elem_ptr) -> void`
    /// Reads input element, calls closure, discards result.
    ForEach,
    /// `fold`: `(env, acc_ptr, elem_ptr, out_ptr) -> void`
    /// Reads accumulator + element, calls closure, writes new accumulator.
    Fold,
}

struct TrampolineBody {
    func_id: FunctionId,
    ptr_ty: LLVMTypeId,
    i8_ty: LLVMTypeId,
    ori_fn: ValueId,
    ori_env: ValueId,
    elem_llvm_ty: LLVMTypeId,
    elem_passing: ParamPassing,
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Build a trampoline and return its function pointer plus closure environment.
    /// Element and result types come from the validated parent ARC function.
    pub(crate) fn build_trampoline(
        &mut self,
        closure_val: ValueId,
        elem_ty: Idx,
        kind: TrampolineKind,
        result_ty: Option<Idx>,
    ) -> (ValueId, ValueId) {
        let closure_ty = self.builder.closure_type();
        let closure_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "tramp.closure", closure_ty);
        self.builder.store(closure_val, closure_alloca);

        let tramp_fn_id = self.generate_trampoline_fn(elem_ty, kind, result_ty);
        let tramp_fn_ptr = self.builder.get_function_ptr(tramp_fn_id);

        (tramp_fn_ptr, closure_alloca)
    }

    /// Generate a trampoline function.
    fn generate_trampoline_fn(
        &mut self,
        elem_ty: Idx,
        kind: TrampolineKind,
        result_ty: Option<Idx>,
    ) -> FunctionId {
        let tramp_id = self.partial_apply_counter;
        self.partial_apply_counter += 1;
        let name = format!("_ori_tramp_{tramp_id}");
        let saved_position = self.builder.save_position();
        let saved_function = self.builder.current_function();
        let saved_emitter_function = self.current_function;
        let ptr_ty = self.builder.ptr_type();
        let i8_ty = self.builder.i8_type();
        let func_id = self.declare_trampoline(&name, kind, ptr_ty, i8_ty);

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);
        self.current_function = func_id;
        let (ori_fn, ori_env) = self.unpack_trampoline_closure(func_id, ptr_ty);
        let elem_llvm_ty = self.resolve_type(elem_ty);
        let elem_passing =
            compute_closure_param_passing(elem_ty, self.type_info, self.repr_plan, self.classifier);
        let body = TrampolineBody {
            func_id,
            ptr_ty,
            i8_ty,
            ori_fn,
            ori_env,
            elem_llvm_ty,
            elem_passing,
        };

        match kind {
            TrampolineKind::Map => self.emit_map_trampoline(&body, elem_ty, result_ty),
            TrampolineKind::Predicate => self.emit_predicate_trampoline(&body),
            TrampolineKind::ForEach => self.emit_for_each_trampoline(&body, result_ty),
            TrampolineKind::Fold => self.emit_fold_trampoline(&body, elem_ty, result_ty),
        }

        self.verify_trampoline(func_id, &name);
        self.current_function = saved_emitter_function;
        self.builder.restore_position(saved_position);
        if let Some(function) = saved_function {
            self.builder.set_current_function(function);
        }
        func_id
    }

    fn declare_trampoline(
        &mut self,
        name: &str,
        kind: TrampolineKind,
        ptr_ty: LLVMTypeId,
        i8_ty: LLVMTypeId,
    ) -> FunctionId {
        let func_id = match kind {
            TrampolineKind::Map => self
                .builder
                .declare_void_function(name, &[ptr_ty, ptr_ty, ptr_ty]),
            TrampolineKind::Predicate => {
                self.builder
                    .declare_function(name, &[ptr_ty, ptr_ty], i8_ty)
            }
            TrampolineKind::ForEach => self.builder.declare_void_function(name, &[ptr_ty, ptr_ty]),
            TrampolineKind::Fold => self
                .builder
                .declare_void_function(name, &[ptr_ty, ptr_ty, ptr_ty, ptr_ty]),
        };
        self.builder.set_module_local(func_id);
        self.builder.add_uwtable_attribute(func_id);

        let param_count = match kind {
            TrampolineKind::Map => 3,
            TrampolineKind::Predicate | TrampolineKind::ForEach => 2,
            TrampolineKind::Fold => 4,
        };
        for index in 0..param_count {
            self.builder.add_noundef_param_attribute(func_id, index);
        }
        func_id
    }

    fn unpack_trampoline_closure(
        &mut self,
        func_id: FunctionId,
        ptr_ty: LLVMTypeId,
    ) -> (ValueId, ValueId) {
        let env_ptr = self.builder.get_param(func_id, 0);
        let closure_ty = self.builder.closure_type();
        let fn_ptr =
            self.builder
                .struct_gep(closure_ty, env_ptr, CLOSURE_FIELD_FN, "tramp.fn_ptr.gep");
        let ori_fn = self.builder.load(ptr_ty, fn_ptr, "tramp.fn_ptr");
        let env = self
            .builder
            .struct_gep(closure_ty, env_ptr, CLOSURE_FIELD_ENV, "tramp.env.gep");
        let ori_env = self.builder.load(ptr_ty, env, "tramp.env");
        (ori_fn, ori_env)
    }

    fn append_trampoline_argument(
        &mut self,
        body: &TrampolineBody,
        passing: ParamPassing,
        source_ptr: ValueId,
        llvm_ty: LLVMTypeId,
        label: &str,
        param_types: &mut Vec<LLVMTypeId>,
        args: &mut Vec<ValueId>,
    ) {
        match passing {
            ParamPassing::Direct => {
                param_types.push(llvm_ty);
                args.push(self.builder.load(llvm_ty, source_ptr, label));
            }
            ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                param_types.push(body.ptr_ty);
                args.push(source_ptr);
            }
            ParamPassing::Void => {}
        }
    }

    fn emit_map_trampoline(&mut self, body: &TrampolineBody, elem_ty: Idx, result_ty: Option<Idx>) {
        let input_ptr = self.builder.get_param(body.func_id, 1);
        let output_ptr = self.builder.get_param(body.func_id, 2);
        let result_idx = result_ty.unwrap_or(elem_ty);
        let result_llvm_ty = result_ty.map_or(body.elem_llvm_ty, |ty| self.resolve_type(ty));
        let mut param_types = vec![body.ptr_ty];
        let mut args = vec![body.ori_env];
        self.append_trampoline_argument(
            body,
            body.elem_passing,
            input_ptr,
            body.elem_llvm_ty,
            "tramp.elem",
            &mut param_types,
            &mut args,
        );

        match compute_return_passing(result_idx, self.type_info, self.repr_plan) {
            ReturnPassing::Direct => {
                let result = self.builder.call_indirect(
                    result_llvm_ty,
                    &param_types,
                    body.ori_fn,
                    &args,
                    "tramp.result",
                );
                if let Some(value) = result {
                    self.builder.store(value, output_ptr);
                }
            }
            ReturnPassing::Sret { .. } => {
                self.builder.call_indirect_with_sret(
                    result_llvm_ty,
                    &param_types,
                    body.ori_fn,
                    output_ptr,
                    &args,
                );
            }
            ReturnPassing::Void => {
                self.builder
                    .call_indirect_void(&param_types, body.ori_fn, &args);
            }
        }
        self.builder.ret_void();
    }

    fn emit_predicate_trampoline(&mut self, body: &TrampolineBody) {
        let elem_ptr = self.builder.get_param(body.func_id, 1);
        let bool_ty = self.builder.bool_type();
        let mut param_types = vec![body.ptr_ty];
        let mut args = vec![body.ori_env];
        self.append_trampoline_argument(
            body,
            body.elem_passing,
            elem_ptr,
            body.elem_llvm_ty,
            "tramp.elem",
            &mut param_types,
            &mut args,
        );
        let result =
            self.builder
                .call_indirect(bool_ty, &param_types, body.ori_fn, &args, "tramp.pred");

        if let Some(predicate) = result {
            let result_i8 = self.builder.zext(predicate, body.i8_ty, "tramp.pred.i8");
            self.builder.ret(result_i8);
        } else {
            let zero = self.builder.const_i64(0);
            let zero_i8 = self.builder.trunc(zero, body.i8_ty, "zero");
            self.builder.ret(zero_i8);
        }
    }

    fn emit_for_each_trampoline(&mut self, body: &TrampolineBody, result_ty: Option<Idx>) {
        let elem_ptr = self.builder.get_param(body.func_id, 1);
        let mut param_types = vec![body.ptr_ty];
        let mut args = vec![body.ori_env];
        self.append_trampoline_argument(
            body,
            body.elem_passing,
            elem_ptr,
            body.elem_llvm_ty,
            "tramp.elem",
            &mut param_types,
            &mut args,
        );
        let result_idx = result_ty.unwrap_or(Idx::UNIT);
        let result_llvm_ty = self.resolve_type(result_idx);
        match compute_return_passing(result_idx, self.type_info, self.repr_plan) {
            ReturnPassing::Direct => {
                if let Some(result) = self.builder.call_indirect(
                    result_llvm_ty,
                    &param_types,
                    body.ori_fn,
                    &args,
                    "tramp.foreach",
                ) {
                    self.dec_value_rc(result, result_idx);
                }
            }
            ReturnPassing::Sret { .. } => {
                let result = self.builder.alloca(result_llvm_ty, "tramp.foreach.sret");
                self.builder.call_indirect_with_sret(
                    result_llvm_ty,
                    &param_types,
                    body.ori_fn,
                    result,
                    &args,
                );
                let value = self
                    .builder
                    .load(result_llvm_ty, result, "tramp.foreach.result");
                self.dec_value_rc(value, result_idx);
            }
            ReturnPassing::Void => {
                self.builder
                    .call_indirect_void(&param_types, body.ori_fn, &args);
            }
        }
        self.builder.ret_void();
    }

    fn emit_fold_trampoline(
        &mut self,
        body: &TrampolineBody,
        elem_ty: Idx,
        result_ty: Option<Idx>,
    ) {
        let acc_ptr = self.builder.get_param(body.func_id, 1);
        let elem_ptr = self.builder.get_param(body.func_id, 2);
        let output_ptr = self.builder.get_param(body.func_id, 3);
        let acc_idx = result_ty.unwrap_or(elem_ty);
        let acc_llvm_ty = result_ty.map_or(body.elem_llvm_ty, |ty| self.resolve_type(ty));
        let acc_passing =
            compute_closure_param_passing(acc_idx, self.type_info, self.repr_plan, self.classifier);
        let mut param_types = vec![body.ptr_ty];
        let mut args = vec![body.ori_env];
        self.append_trampoline_argument(
            body,
            acc_passing,
            acc_ptr,
            acc_llvm_ty,
            "tramp.acc",
            &mut param_types,
            &mut args,
        );
        self.append_trampoline_argument(
            body,
            body.elem_passing,
            elem_ptr,
            body.elem_llvm_ty,
            "tramp.elem",
            &mut param_types,
            &mut args,
        );

        match compute_return_passing(acc_idx, self.type_info, self.repr_plan) {
            ReturnPassing::Direct => {
                let result = self.builder.call_indirect(
                    acc_llvm_ty,
                    &param_types,
                    body.ori_fn,
                    &args,
                    "tramp.fold",
                );
                if let Some(value) = result {
                    self.builder.store(value, output_ptr);
                }
            }
            ReturnPassing::Sret { .. } => {
                self.builder.call_indirect_with_sret(
                    acc_llvm_ty,
                    &param_types,
                    body.ori_fn,
                    output_ptr,
                    &args,
                );
            }
            ReturnPassing::Void => {
                self.builder
                    .call_indirect_void(&param_types, body.ori_fn, &args);
            }
        }
        self.builder.ret_void();
    }

    fn verify_trampoline(&mut self, func_id: FunctionId, name: &str) {
        if !self.verify_arc {
            return;
        }
        let function = self.builder.get_function_value(func_id);
        if !function.verify(true) {
            tracing::error!(name, "LLVM IR verification failed (generate_trampoline_fn)");
            self.builder.record_codegen_error();
        }
    }

    /// Generate a sign-extension widening trampoline for narrowed list iterators.
    ///
    /// The trampoline reads a narrowed integer element (i8/i16/i32) from `in_ptr`,
    /// sign-extends it to canonical i64, and stores the i64 to `out_ptr`. This is
    /// injected at the `iter()` boundary for narrowed lists so the entire iterator
    /// pipeline operates on canonical element types.
    ///
    /// Signature: `(env: ptr, in_ptr: ptr, out_ptr: ptr) -> void`
    /// (env is unused — null passed by the caller)
    pub(crate) fn generate_sext_widening_trampoline(
        &mut self,
        narrowed_width: ori_repr::IntWidth,
    ) -> FunctionId {
        let tramp_id = self.partial_apply_counter;
        self.partial_apply_counter += 1;
        let tramp_name = format!("_ori_sext_widen_{tramp_id}");

        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        let ptr_ty = self.builder.ptr_type();

        // (env: ptr, in_ptr: ptr, out_ptr: ptr) -> void
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

        let in_ptr = self.builder.get_param(func_id, 1);
        let out_ptr = self.builder.get_param(func_id, 2);

        // Load narrowed type from in_ptr
        let narrowed_llvm_ty = self.llvm_type_for_int_width(narrowed_width);
        let raw = self.builder.load(narrowed_llvm_ty, in_ptr, "sext.raw");

        let i64_ty = self.builder.i64_type();
        let widened = self.builder.sext(raw, i64_ty, "sext.wide");

        self.builder.store(widened, out_ptr);
        self.builder.ret_void();

        if self.verify_arc {
            let fn_val = self.builder.get_function_value(func_id);
            if !fn_val.verify(true) {
                tracing::error!(
                    name = tramp_name,
                    "LLVM IR verification failed (generate_sext_widening_trampoline)"
                );
                self.builder.record_codegen_error();
            }
        }

        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        func_id
    }
}
