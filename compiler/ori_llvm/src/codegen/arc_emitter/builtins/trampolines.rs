//! Closure-to-C-ABI trampoline generation for iterator adapters.
//!
//! Iterator adapters like `map(f)` and `filter(f)` receive Ori closures
//! `{fn_ptr, env_ptr}` but the runtime expects C-ABI function pointers.
//! A trampoline bridges them:
//!
//! ```text
//! trampoline(env_ptr: ptr, in_ptr: ptr, out_ptr: ptr) -> void:
//!   // env_ptr IS the Ori closure {fn_ptr, env_ptr}
//!   ori_fn  = load ptr from env_ptr[0]
//!   ori_env = load ptr from env_ptr[1]
//!   elem    = load T from in_ptr
//!   result  = call ori_fn(ori_env, elem)
//!   store result to out_ptr
//! ```

// No method registrations — trampolines are helper functions, not method handlers.
declare_builtins! { _emitter, _ctx; }

use ori_ir::{CLOSURE_FIELD_ENV, CLOSURE_FIELD_FN};
use ori_types::Idx;

use crate::codegen::abi::abi_size;
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

use super::super::ArcIrEmitter;

/// Trampoline variant for different iterator adapter calling conventions.
#[derive(Clone, Copy)]
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
    elem_is_indirect: bool,
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Build a trampoline function and return `(trampoline_fn_ptr, closure_as_env_ptr)`.
    ///
    /// The closure struct `{fn_ptr, env_ptr}` is stored to an alloca and
    /// passed as the `env` argument to the runtime. The trampoline unpacks it.
    ///
    /// PC-2 upstream guarantor: `elem_ty` and `result_ty` originate from
    /// `TypeInfo::Iterator { element }` extracted at iterator-emission sites
    /// (e.g. `arc_emitter/builtins/iterator.rs::emit_iter_map`) based on the
    /// receiver's parent `ArcFunction` type indices — they are NOT
    /// independent `ArcInstr` operands. Coverage is provided by the
    /// `assert_no_unresolved_type_vars` walker on the parent function's
    /// `var_types` / `params` / `return_type` / block-params. No additional
    /// `assert_no_unresolved_idx` guard is needed here.
    pub(crate) fn build_trampoline(
        &mut self,
        closure_val: ValueId,
        elem_ty: Idx,
        kind: TrampolineKind,
        result_ty: Option<Idx>,
    ) -> (ValueId, ValueId) {
        // Store the closure to an alloca so we can pass its pointer as env
        let closure_ty = self.builder.closure_type();
        let closure_alloca =
            self.builder
                .create_entry_alloca(self.current_function, "tramp.closure", closure_ty);
        self.builder.store(closure_val, closure_alloca);

        // Generate the trampoline function
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
        let ptr_ty = self.builder.ptr_type();
        let i8_ty = self.builder.i8_type();
        let func_id = self.declare_trampoline(&name, kind, ptr_ty, i8_ty);

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);
        let (ori_fn, ori_env) = self.unpack_trampoline_closure(func_id, ptr_ty);
        let elem_llvm_ty = self.resolve_type(elem_ty);
        let elem_is_indirect = abi_size(elem_ty, self.type_info, self.repr_plan) > 16;
        let body = TrampolineBody {
            func_id,
            ptr_ty,
            i8_ty,
            ori_fn,
            ori_env,
            elem_llvm_ty,
            elem_is_indirect,
        };

        match kind {
            TrampolineKind::Map => self.emit_map_trampoline(&body, elem_ty, result_ty),
            TrampolineKind::Predicate => self.emit_predicate_trampoline(&body),
            TrampolineKind::ForEach => self.emit_for_each_trampoline(&body),
            TrampolineKind::Fold => self.emit_fold_trampoline(&body, elem_ty, result_ty),
        }

        self.verify_trampoline(func_id, &name);
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

    fn emit_map_trampoline(&mut self, body: &TrampolineBody, elem_ty: Idx, result_ty: Option<Idx>) {
        let input_ptr = self.builder.get_param(body.func_id, 1);
        let output_ptr = self.builder.get_param(body.func_id, 2);
        let result_idx = result_ty.unwrap_or(elem_ty);
        let result_llvm_ty = result_ty.map_or(body.elem_llvm_ty, |ty| self.resolve_type(ty));
        let result_is_indirect = abi_size(result_idx, self.type_info, self.repr_plan) > 16;
        let elem_arg_ty = if body.elem_is_indirect {
            body.ptr_ty
        } else {
            body.elem_llvm_ty
        };
        let elem_arg = if body.elem_is_indirect {
            input_ptr
        } else {
            self.builder
                .load(body.elem_llvm_ty, input_ptr, "tramp.elem")
        };

        if result_is_indirect {
            self.builder.call_indirect_with_sret(
                result_llvm_ty,
                &[body.ptr_ty, elem_arg_ty],
                body.ori_fn,
                output_ptr,
                &[body.ori_env, elem_arg],
            );
        } else {
            let result = self.builder.call_indirect(
                result_llvm_ty,
                &[body.ptr_ty, elem_arg_ty],
                body.ori_fn,
                &[body.ori_env, elem_arg],
                "tramp.result",
            );
            if let Some(value) = result {
                self.builder.store(value, output_ptr);
            }
        }
        self.builder.ret_void();
    }

    fn emit_predicate_trampoline(&mut self, body: &TrampolineBody) {
        let elem_ptr = self.builder.get_param(body.func_id, 1);
        let bool_ty = self.builder.bool_type();
        let result = if body.elem_is_indirect {
            self.builder.call_indirect(
                bool_ty,
                &[body.ptr_ty, body.ptr_ty],
                body.ori_fn,
                &[body.ori_env, elem_ptr],
                "tramp.pred",
            )
        } else {
            let elem = self.builder.load(body.elem_llvm_ty, elem_ptr, "tramp.elem");
            self.builder.call_indirect(
                bool_ty,
                &[body.ptr_ty, body.elem_llvm_ty],
                body.ori_fn,
                &[body.ori_env, elem],
                "tramp.pred",
            )
        };

        if let Some(predicate) = result {
            let result_i8 = self.builder.zext(predicate, body.i8_ty, "tramp.pred.i8");
            self.builder.ret(result_i8);
        } else {
            let zero = self.builder.const_i64(0);
            let zero_i8 = self.builder.trunc(zero, body.i8_ty, "zero");
            self.builder.ret(zero_i8);
        }
    }

    fn emit_for_each_trampoline(&mut self, body: &TrampolineBody) {
        let elem_ptr = self.builder.get_param(body.func_id, 1);
        if body.elem_is_indirect {
            self.builder.call_indirect_void(
                &[body.ptr_ty, body.ptr_ty],
                body.ori_fn,
                &[body.ori_env, elem_ptr],
            );
        } else {
            let elem = self.builder.load(body.elem_llvm_ty, elem_ptr, "tramp.elem");
            let unit_ty = self.builder.i64_type();
            self.builder.call_indirect(
                unit_ty,
                &[body.ptr_ty, body.elem_llvm_ty],
                body.ori_fn,
                &[body.ori_env, elem],
                "tramp.foreach",
            );
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
        let acc_is_indirect = abi_size(acc_idx, self.type_info, self.repr_plan) > 16;
        let acc_arg = if acc_is_indirect {
            acc_ptr
        } else {
            self.builder.load(acc_llvm_ty, acc_ptr, "tramp.acc")
        };
        let elem_arg = if body.elem_is_indirect {
            elem_ptr
        } else {
            self.builder.load(body.elem_llvm_ty, elem_ptr, "tramp.elem")
        };
        let acc_arg_ty = if acc_is_indirect {
            body.ptr_ty
        } else {
            acc_llvm_ty
        };
        let elem_arg_ty = if body.elem_is_indirect {
            body.ptr_ty
        } else {
            body.elem_llvm_ty
        };

        if acc_is_indirect {
            self.builder.call_indirect_with_sret(
                acc_llvm_ty,
                &[body.ptr_ty, acc_arg_ty, elem_arg_ty],
                body.ori_fn,
                output_ptr,
                &[body.ori_env, acc_arg, elem_arg],
            );
        } else {
            let result = self.builder.call_indirect(
                acc_llvm_ty,
                &[body.ptr_ty, acc_arg_ty, elem_arg_ty],
                body.ori_fn,
                &[body.ori_env, acc_arg, elem_arg],
                "tramp.fold",
            );
            if let Some(value) = result {
                self.builder.store(value, output_ptr);
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

        // Sign-extend to canonical i64
        let i64_ty = self.builder.i64_type();
        let widened = self.builder.sext(raw, i64_ty, "sext.wide");

        // Store canonical i64 to out_ptr
        self.builder.store(widened, out_ptr);
        self.builder.ret_void();

        // Verify
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
