//! Closure (partial application) emission for [`ArcIrEmitter`].
//!
//! Handles `PartialApply` instructions: allocating closure environments
//! and generating environment drop functions. Wrapper function generation
//! lives in the sibling [`closure_wrappers`](super::closure_wrappers) module.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_arc::ownership::Ownership;
use ori_arc::DropKind;
use ori_ir::Name;
use ori_types::Idx;

use super::context::EmittedValue;
use super::ArcIrEmitter;
use crate::codegen::abi::ParamAbi;
use crate::codegen::type_info::TypeLayoutResolver;
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a `PartialApply` instruction (closure creation).
    ///
    /// For **non-capturing lambdas** (callee is in `non_capturing_lambdas`),
    /// the lambda was declared with closure-compatible ABI (`ccc` + phantom
    /// `ptr %_env`), so we can use the lambda's function pointer directly as
    /// `fn_ptr` without generating a `_ori_partial_N` trampoline. The closure
    /// is `{ lambda_fn_ptr, null }`.
    ///
    /// For **capturing lambdas**, generates a wrapper function that bridges
    /// the closure calling convention `(env_ptr, user_args...)` to the
    /// lambda's flat convention `(captures..., user_args...)`, allocates an
    /// RC-tracked environment struct, and builds `{ wrapper_fn_ptr, env_ptr }`.
    pub(super) fn emit_partial_apply(
        &mut self,
        dst: ArcVarId,
        _ty: Idx,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) {
        let callee_name_str = self.interner.lookup(callee);
        let is_non_capturing = self.ctx.non_capturing_lambdas.contains(&callee);

        tracing::debug!(
            name = callee_name_str,
            captures = args.len(),
            non_capturing = is_non_capturing,
            "ArcIrEmitter: PartialApply — closure creation"
        );

        // Look up the callee (lambda function), already compiled and registered
        let Some(&(callee_func_id, ref callee_abi)) = self.ctx.functions.get(&callee) else {
            tracing::warn!(
                name = callee_name_str,
                "emit_partial_apply: callee not found"
            );
            let closure_ty = self.builder.closure_type();
            let null_ptr = self.builder.const_null_ptr();
            let closure =
                self.builder
                    .build_struct(closure_ty, &[null_ptr, null_ptr], "partial_apply");
            self.def_var(dst, EmittedValue::Aggregate(closure));
            return;
        };

        // Non-capturing fast path: lambda already has closure-compatible ABI,
        // so use its function pointer directly — no wrapper needed.
        if is_non_capturing && args.is_empty() {
            let fn_ptr = self.builder.get_function_ptr(callee_func_id);
            let null_env = self.builder.const_null_ptr();
            let closure_ty = self.builder.closure_type();
            let closure =
                self.builder
                    .build_struct(closure_ty, &[fn_ptr, null_env], "partial_apply.direct");
            self.def_var(dst, EmittedValue::Aggregate(closure));
            return;
        }

        let callee_abi = callee_abi.clone();
        let num_captures = args.len();

        // Capture types (from ARC IR variable types)
        let capture_types: Vec<Idx> = args.iter().map(|&v| func.var_type(v)).collect();

        // Remaining user params (the closure awaits these)
        let remaining_params: Vec<ParamAbi> = callee_abi.params[num_captures..].to_vec();

        // Capture ownership: which captures are borrowed (skip RcInc in wrapper — body borrows from env).
        let capture_ownership: Vec<Ownership> = self
            .ctx
            .lambda_capture_ownership
            .get(&callee)
            .cloned()
            .unwrap_or_else(|| {
                tracing::warn!(
                    name = callee_name_str,
                    captures = num_captures,
                    "lambda_capture_ownership missing — defaulting to all-Owned (conservative)"
                );
                vec![Ownership::Owned; num_captures]
            });

        // == Allocate and pack the environment ==
        let env_ptr = if capture_types.is_empty() {
            self.builder.const_null_ptr()
        } else {
            self.build_closure_env(args, &capture_types)
        };

        // == Generate wrapper function ==
        let target_is_nounwind = self.ctx.nounwind_functions.contains(&callee);
        let wrapper_fn_ptr = self.generate_closure_wrapper(
            callee_func_id,
            &callee_abi,
            &capture_types,
            &capture_ownership,
            &remaining_params,
            target_is_nounwind,
        );

        // == Build fat-pointer closure { wrapper_fn_ptr, env_ptr } ==
        let closure_ty = self.builder.closure_type();
        let closure =
            self.builder
                .build_struct(closure_ty, &[wrapper_fn_ptr, env_ptr], "partial_apply");
        self.def_var(dst, EmittedValue::Aggregate(closure));
    }

    /// Allocate and pack a closure environment struct.
    ///
    /// Layout: `{ ptr drop_fn, cap_0_ty, cap_1_ty, ... }`
    /// Allocated via `ori_rc_alloc` (RC-tracked heap memory).
    fn build_closure_env(&mut self, capture_vars: &[ArcVarId], capture_types: &[Idx]) -> ValueId {
        // Build env struct type: { drop_fn: ptr, cap_0, cap_1, ... }
        let ptr_llvm = self.builder.scx().type_ptr().into();
        let mut env_fields: Vec<inkwell::types::BasicTypeEnum<'_>> = vec![ptr_llvm];
        for &cap_ty in capture_types {
            env_fields.push(self.type_resolver.resolve(cap_ty));
        }
        let env_struct = self.builder.scx().type_struct(&env_fields, false);
        let env_struct_ty_id = self.builder.register_type(env_struct.into());

        // Compute size via LLVM's target layout, falling back to
        // summing field sizes for compound captures (str, tuple, struct).
        let env_size = env_struct
            .size_of()
            .and_then(inkwell::values::IntValue::get_zero_extended_constant)
            .unwrap_or_else(|| TypeLayoutResolver::type_store_size(env_struct.into()));

        // Allocate via ori_rc_alloc(size, align=8)
        let size_val = self.builder.const_i64(env_size as i64);
        let align_val = self.builder.const_i64(8);
        let rc_alloc_func = self.builder.runtime_fn("ori_rc_alloc");
        let data_ptr = self
            .emit_rt_call(rc_alloc_func, &[size_val, align_val], "env.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());

        // Generate drop function for this environment.
        let drop_fn_id = self.generate_env_drop_fn(env_struct_ty_id, capture_types, env_size);
        let drop_fn_ptr = self.builder.get_function_ptr(drop_fn_id);

        // Store drop_fn at field 0
        let drop_field = self
            .builder
            .struct_gep(env_struct_ty_id, data_ptr, 0, "env.drop_fn");
        self.builder.store(drop_fn_ptr, drop_field);

        // Store each capture at fields 1..N
        #[expect(
            clippy::cast_possible_truncation,
            reason = "capture count bounded by lambda arity, well within u32 range"
        )]
        for (i, &cap_var) in capture_vars.iter().enumerate() {
            let cap_val = self.var(cap_var);
            let field_ptr = self.builder.struct_gep(
                env_struct_ty_id,
                data_ptr,
                (i + 1) as u32,
                &format!("env.cap.{i}"),
            );
            self.builder.store(cap_val, field_ptr);
        }

        data_ptr
    }

    /// Generate a drop function for a closure environment.
    ///
    /// Builds the `DropKind::ClosureEnv(fields)` descriptor via the
    /// `ori_arc::compute_closure_env_drop` SSOT (the single source of truth
    /// for which captures need RC and their logical indices), then walks
    /// those fields decrementing each through the shared `dec_value_rc`
    /// helper — the single tag-aware inline-value RC-dec dispatch (buffer dec
    /// for List/Set/Map, inline-enum dec for Option/Result/Enum, aggregate
    /// fields for Struct/Tuple, dynamic env-header dec for closures). Finally
    /// frees the environment via `ori_rc_free`.
    ///
    /// Closure-env-specific concerns kept local: the per-instance env struct
    /// type (closure envs are not interned in the type pool), the +1 field
    /// offset (field 0 is the `drop_fn` slot; capture `i` lives at field
    /// `i + 1`), and the `ori_rc_free` payload size.
    fn generate_env_drop_fn(
        &mut self,
        env_struct_ty_id: LLVMTypeId,
        capture_types: &[Idx],
        env_size: u64,
    ) -> FunctionId {
        let partial_id = self.partial_apply_counter;
        self.partial_apply_counter += 1;
        let func_name = format!("_ori_partial_{partial_id}_drop");

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        // Save emitter's tracked current_function so helpers that append
        // blocks (emit_drop_rc_dec → emit_closure_field_rc_dec) use the drop
        // function's id, not the caller's.
        let saved_current_function = self.current_function;

        // Declare: void @_ori_partial_N_drop(ptr %data)
        let ptr_ty = self.builder.ptr_type();
        let func_id = self.builder.declare_void_function(&func_name, &[ptr_ty]);
        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);
        self.builder.add_cold_attribute(func_id);
        self.builder.add_uwtable_attribute(func_id);
        // noundef on data pointer param — Ori never passes poison pointers.
        self.builder.add_noundef_param_attribute(func_id, 0);

        // Generate body
        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);
        self.current_function = func_id;

        let data_ptr = self.builder.get_param(func_id, 0);

        self.emit_closure_env_field_decs(env_struct_ty_id, data_ptr, capture_types);

        // Free the env struct
        let size_val = self.builder.const_i64(env_size as i64);
        let align_val = self.builder.const_i64(8);
        let rc_free_id = self.builder.runtime_fn("ori_rc_free");
        self.builder
            .call(rc_free_id, &[data_ptr, size_val, align_val], "");
        self.builder.ret_void();

        // Function-level LLVM IR verification.
        if self.verify_arc {
            let fn_val = self.builder.get_function_value(func_id);
            if !fn_val.verify(true) {
                tracing::error!(
                    name = func_name,
                    "LLVM IR verification failed (generate_env_drop_fn)"
                );
                self.builder.record_codegen_error();
            }
        }

        // Restore builder position and emitter's current_function trackers
        self.current_function = saved_current_function;
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        func_id
    }

    /// Decrement each RC-owning captured field of a closure environment.
    ///
    /// The set of fields needing RC and their logical capture indices come
    /// from `ori_arc::compute_closure_env_drop` — the SSOT shared with the
    /// `DropKind::ClosureEnv` codegen arm. Each field is loaded from its
    /// physical slot (`+1` past the `drop_fn` header) and decremented via the
    /// shared `dec_value_rc` SSOT. `dec_value_rc` itself dispatches per tag
    /// (buffer dec for List/Set/Map, inline-enum dec for Option/Result/Enum,
    /// aggregate-field dec for Struct/Tuple, dynamic env-header dec for
    /// closure captures), so collection captures route through the same
    /// buffer-dec path as every other inline collection value — no parallel
    /// per-tag dispatch in the closure path.
    fn emit_closure_env_field_decs(
        &mut self,
        env_struct_ty_id: LLVMTypeId,
        data_ptr: ValueId,
        capture_types: &[Idx],
    ) {
        let DropKind::ClosureEnv(fields) =
            ori_arc::compute_closure_env_drop(capture_types, self.classifier)
        else {
            // No captured variable needs RC — nothing to walk before free.
            return;
        };

        for (capture_index, field_type) in fields {
            // Physical env layout: field 0 is the drop_fn slot, captures
            // start at field 1. The logical capture index from the burden
            // spec maps to physical field `capture_index + 1`.
            let physical_index = capture_index + 1;
            let field_llvm_ty = self.resolve_type(field_type);
            let field_ptr = self.builder.struct_gep(
                env_struct_ty_id,
                data_ptr,
                physical_index,
                &format!("cap.{capture_index}.ptr"),
            );
            let field_val =
                self.builder
                    .load(field_llvm_ty, field_ptr, &format!("cap.{capture_index}"));
            self.dec_value_rc(field_val, field_type);
        }
    }
}
