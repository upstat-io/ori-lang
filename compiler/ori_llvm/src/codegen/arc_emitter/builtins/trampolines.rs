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

use ori_types::Idx;

use crate::codegen::abi::abi_size;
use crate::codegen::value_id::{FunctionId, ValueId};

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

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Build a trampoline function and return `(trampoline_fn_ptr, closure_as_env_ptr)`.
    ///
    /// The closure struct `{fn_ptr, env_ptr}` is stored to an alloca and
    /// passed as the `env` argument to the runtime. The trampoline unpacks it.
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
    #[expect(
        clippy::too_many_lines,
        reason = "trampoline emits sequential LLVM IR for runtime builtins"
    )]
    fn generate_trampoline_fn(
        &mut self,
        elem_ty: Idx,
        kind: TrampolineKind,
        result_ty: Option<Idx>,
    ) -> FunctionId {
        let tramp_id = self.partial_apply_counter;
        self.partial_apply_counter += 1;
        let tramp_name = format!("_ori_tramp_{tramp_id}");

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        let ptr_ty = self.builder.ptr_type();
        let i8_ty = self.builder.i8_type();

        // Declare trampoline based on kind
        let func_id = match kind {
            TrampolineKind::Map => {
                // (env: ptr, in_ptr: ptr, out_ptr: ptr) -> void
                self.builder
                    .declare_void_function(&tramp_name, &[ptr_ty, ptr_ty, ptr_ty])
            }
            TrampolineKind::Predicate => {
                // (env: ptr, elem_ptr: ptr) -> i8
                self.builder
                    .declare_function(&tramp_name, &[ptr_ty, ptr_ty], i8_ty)
            }
            TrampolineKind::ForEach => {
                // (env: ptr, elem_ptr: ptr) -> void
                self.builder
                    .declare_void_function(&tramp_name, &[ptr_ty, ptr_ty])
            }
            TrampolineKind::Fold => {
                // (env: ptr, acc_ptr: ptr, elem_ptr: ptr, out_ptr: ptr) -> void
                self.builder
                    .declare_void_function(&tramp_name, &[ptr_ty, ptr_ty, ptr_ty, ptr_ty])
            }
        };
        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);
        self.builder.add_uwtable_attribute(func_id);
        // All trampoline params are pointers passed by the runtime — always
        // valid, defined addresses (never undef/poison).
        let param_count = match kind {
            TrampolineKind::Map => 3,
            TrampolineKind::Predicate | TrampolineKind::ForEach => 2,
            TrampolineKind::Fold => 4,
        };
        for i in 0..param_count {
            self.builder.add_noundef_param_attribute(func_id, i);
        }

        // Generate body
        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);

        let env_ptr = self.builder.get_param(func_id, 0);

        // Unpack the Ori closure from env_ptr: { fn_ptr: ptr, env_ptr: ptr }
        let closure_struct_ty = self.builder.closure_type();
        let fn_ptr_gep = self
            .builder
            .struct_gep(closure_struct_ty, env_ptr, 0, "tramp.fn_ptr.gep");
        let ori_fn = self.builder.load(ptr_ty, fn_ptr_gep, "tramp.fn_ptr");
        let env_gep = self
            .builder
            .struct_gep(closure_struct_ty, env_ptr, 1, "tramp.env.gep");
        let ori_env = self.builder.load(ptr_ty, env_gep, "tramp.env");

        // Resolve element LLVM type and determine ABI passing mode.
        // Types > 16 bytes (e.g. str = 24 bytes) use indirect parameter
        // passing and sret return in Ori's fastcc ABI.
        let elem_llvm_ty = self.resolve_type(elem_ty);
        let elem_is_indirect = abi_size(elem_ty, self.type_info) > 16;

        match kind {
            TrampolineKind::Map => {
                let in_ptr = self.builder.get_param(func_id, 1);
                let out_ptr = self.builder.get_param(func_id, 2);

                let result_idx = result_ty.unwrap_or(elem_ty);
                let result_llvm_ty = result_ty.map_or(elem_llvm_ty, |ty| self.resolve_type(ty));
                let result_is_indirect = abi_size(result_idx, self.type_info) > 16;

                if elem_is_indirect && result_is_indirect {
                    // Both param and return are indirect (sret + ptr param).
                    // Call: ori_fn(out_ptr, ori_env, in_ptr) -> void
                    self.builder.call_indirect_void(
                        &[ptr_ty, ptr_ty, ptr_ty],
                        ori_fn,
                        &[out_ptr, ori_env, in_ptr],
                    );
                } else if elem_is_indirect {
                    // Param indirect, return direct (small result from large input).
                    // Call: result = ori_fn(ori_env, in_ptr) -> T
                    let result = self.builder.call_indirect(
                        result_llvm_ty,
                        &[ptr_ty, ptr_ty],
                        ori_fn,
                        &[ori_env, in_ptr],
                        "tramp.result",
                    );
                    if let Some(result_val) = result {
                        self.builder.store(result_val, out_ptr);
                    }
                } else if result_is_indirect {
                    // Param direct, return indirect (large result from small input).
                    let elem = self.builder.load(elem_llvm_ty, in_ptr, "tramp.elem");
                    // Call: ori_fn(out_ptr, ori_env, elem) -> void
                    self.builder.call_indirect_void(
                        &[ptr_ty, ptr_ty, elem_llvm_ty],
                        ori_fn,
                        &[out_ptr, ori_env, elem],
                    );
                } else {
                    // Both direct — small types (original path).
                    let elem = self.builder.load(elem_llvm_ty, in_ptr, "tramp.elem");
                    let result = self.builder.call_indirect(
                        result_llvm_ty,
                        &[ptr_ty, elem_llvm_ty],
                        ori_fn,
                        &[ori_env, elem],
                        "tramp.result",
                    );
                    if let Some(result_val) = result {
                        self.builder.store(result_val, out_ptr);
                    }
                }
                self.builder.ret_void();
            }

            TrampolineKind::Predicate => {
                let elem_ptr = self.builder.get_param(func_id, 1);

                // Predicate always returns i1 (direct). Only elem may be indirect.
                let bool_ty = self.builder.bool_type();
                let result = if elem_is_indirect {
                    // Pass pointer directly — closure expects indirect param.
                    self.builder.call_indirect(
                        bool_ty,
                        &[ptr_ty, ptr_ty],
                        ori_fn,
                        &[ori_env, elem_ptr],
                        "tramp.pred",
                    )
                } else {
                    let elem = self.builder.load(elem_llvm_ty, elem_ptr, "tramp.elem");
                    self.builder.call_indirect(
                        bool_ty,
                        &[ptr_ty, elem_llvm_ty],
                        ori_fn,
                        &[ori_env, elem],
                        "tramp.pred",
                    )
                };

                // Convert i1 -> i8 for C ABI
                if let Some(pred_val) = result {
                    let result_i8 = self.builder.zext(pred_val, i8_ty, "tramp.pred.i8");
                    self.builder.ret(result_i8);
                } else {
                    let zero = self.builder.const_i64(0);
                    let zero_i8 = self.builder.trunc(zero, i8_ty, "zero");
                    self.builder.ret(zero_i8);
                }
            }

            TrampolineKind::ForEach => {
                let elem_ptr = self.builder.get_param(func_id, 1);

                if elem_is_indirect {
                    // Pass pointer directly — closure expects indirect param.
                    // ForEach discards result; closure returns void for unit.
                    self.builder.call_indirect_void(
                        &[ptr_ty, ptr_ty],
                        ori_fn,
                        &[ori_env, elem_ptr],
                    );
                } else {
                    let elem = self.builder.load(elem_llvm_ty, elem_ptr, "tramp.elem");
                    let unit_ty = self.builder.i64_type(); // Unit = i64
                    self.builder.call_indirect(
                        unit_ty,
                        &[ptr_ty, elem_llvm_ty],
                        ori_fn,
                        &[ori_env, elem],
                        "tramp.foreach",
                    );
                }
                self.builder.ret_void();
            }

            TrampolineKind::Fold => {
                let acc_ptr = self.builder.get_param(func_id, 1);
                let elem_ptr = self.builder.get_param(func_id, 2);
                let out_ptr = self.builder.get_param(func_id, 3);

                let acc_idx = result_ty.unwrap_or(elem_ty);
                let acc_llvm_ty = result_ty.map_or(elem_llvm_ty, |ty| self.resolve_type(ty));
                let acc_is_indirect = abi_size(acc_idx, self.type_info) > 16;

                // Load/pass accumulator and element based on ABI
                let acc_arg = if acc_is_indirect {
                    acc_ptr
                } else {
                    self.builder.load(acc_llvm_ty, acc_ptr, "tramp.acc")
                };
                let elem_arg = if elem_is_indirect {
                    elem_ptr
                } else {
                    self.builder.load(elem_llvm_ty, elem_ptr, "tramp.elem")
                };

                let acc_arg_ty = if acc_is_indirect { ptr_ty } else { acc_llvm_ty };
                let elem_arg_ty = if elem_is_indirect {
                    ptr_ty
                } else {
                    elem_llvm_ty
                };

                if acc_is_indirect {
                    // Accumulator is indirect → sret return.
                    // Call: ori_fn(out_ptr, ori_env, acc_ptr, elem_arg) -> void
                    self.builder.call_indirect_void(
                        &[ptr_ty, ptr_ty, acc_arg_ty, elem_arg_ty],
                        ori_fn,
                        &[out_ptr, ori_env, acc_arg, elem_arg],
                    );
                } else {
                    // Accumulator is direct → direct return.
                    let result = self.builder.call_indirect(
                        acc_llvm_ty,
                        &[ptr_ty, acc_arg_ty, elem_arg_ty],
                        ori_fn,
                        &[ori_env, acc_arg, elem_arg],
                        "tramp.fold",
                    );
                    if let Some(result_val) = result {
                        self.builder.store(result_val, out_ptr);
                    }
                }
                self.builder.ret_void();
            }
        }

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        func_id
    }
}
