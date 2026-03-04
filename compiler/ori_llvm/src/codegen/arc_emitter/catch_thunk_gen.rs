//! Catch thunk function generation for SEH `catch(expr:)`.
//!
//! Generates `void @_ori_catch_thunk$N(ptr %ctx)` LLVM functions that load
//! arguments from a context struct, call the real function, and store the
//! result back. Used by [`super::catch_thunk`] to emit SEH-safe catch invokes.

use super::ArcIrEmitter;
use crate::codegen::abi::{ParamPassing, ReturnPassing};
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Generate a catch thunk for a compiled function with full ABI awareness.
    ///
    /// The thunk loads arguments from the context struct respecting each
    /// parameter's passing mode (Direct/Indirect/Reference), calls the
    /// function with proper ABI, and stores the result back.
    pub(super) fn generate_catch_thunk(
        &mut self,
        callee_id: FunctionId,
        params: &[crate::codegen::abi::ParamAbi],
        ret_abi: &crate::codegen::abi::ReturnAbi,
        ctx_struct_ty: LLVMTypeId,
        result_field_idx: u32,
    ) -> FunctionId {
        let ptr_ty = self.builder.ptr_type();
        let counter = self.catch_thunk_counter;
        self.catch_thunk_counter += 1;

        let name = format!("_ori_catch_thunk${counter}");
        let thunk_id = self.builder.declare_void_function(&name, &[ptr_ty]);
        self.builder.set_ccc(thunk_id);
        // NOT nounwind — the callee may panic, and the unwind must propagate
        // through this thunk so that ori_try_call's catch can intercept it.
        self.builder.add_uwtable_attribute(thunk_id);

        // Save builder state
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        let saved_emitter_func = self.current_function;
        let saved_funclet_pad = self.current_funclet_pad.take();

        let entry = self.builder.append_block(thunk_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(thunk_id);
        self.current_function = thunk_id;

        let ctx_ptr = self.builder.get_param(thunk_id, 0);

        // Load args from context and build call args
        let mut call_args: Vec<ValueId> = Vec::new();
        let has_sret = matches!(ret_abi.passing, ReturnPassing::Sret { .. });

        // If sret, allocate local for the return value
        let sret_alloca = if has_sret {
            let ret_ty = self.resolve_type(ret_abi.ty);
            let alloca = self.builder.alloca(ret_ty, "thunk.sret");
            call_args.push(alloca);
            Some(alloca)
        } else {
            None
        };

        let mut field_idx: u32 = 0;
        for param_abi in params {
            if matches!(param_abi.passing, ParamPassing::Void) {
                continue;
            }
            let field_ptr = self.builder.struct_gep(
                ctx_struct_ty,
                ctx_ptr,
                field_idx,
                &format!("thunk.arg.{field_idx}"),
            );

            match &param_abi.passing {
                ParamPassing::Direct => {
                    let field_ty = self.resolve_type(param_abi.ty);
                    let val = self.builder.load(field_ty, field_ptr, "thunk.load");
                    call_args.push(val);
                }
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    // Callee expects a pointer — pass the address of the field
                    call_args.push(field_ptr);
                }
                ParamPassing::Void => unreachable!(),
            }
            field_idx += 1;
        }

        // Call the real function
        let result = self.builder.call(callee_id, &call_args, "thunk.call");

        // Store result into context
        match &ret_abi.passing {
            ReturnPassing::Direct => {
                if let Some(val) = result {
                    let result_ptr = self.builder.struct_gep(
                        ctx_struct_ty,
                        ctx_ptr,
                        result_field_idx,
                        "thunk.result.ptr",
                    );
                    self.builder.store(val, result_ptr);
                }
            }
            ReturnPassing::Sret { .. } => {
                if let Some(sret) = sret_alloca {
                    let ret_ty = self.resolve_type(ret_abi.ty);
                    let val = self.builder.load(ret_ty, sret, "thunk.sret.load");
                    let result_ptr = self.builder.struct_gep(
                        ctx_struct_ty,
                        ctx_ptr,
                        result_field_idx,
                        "thunk.result.ptr",
                    );
                    self.builder.store(val, result_ptr);
                }
            }
            ReturnPassing::Void => {}
        }

        self.builder.ret_void();

        // Restore builder state
        self.current_funclet_pad = saved_funclet_pad;
        self.current_function = saved_emitter_func;
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        thunk_id
    }

    /// Generate a catch thunk for a runtime function call.
    ///
    /// Simplified path: all runtime function args are `ptr` type (after coercion).
    pub(super) fn generate_rt_catch_thunk(
        &mut self,
        callee_id: FunctionId,
        num_args: usize,
        ctx_struct_ty: LLVMTypeId,
        result_field_idx: u32,
    ) -> FunctionId {
        let ptr_ty = self.builder.ptr_type();
        let counter = self.catch_thunk_counter;
        self.catch_thunk_counter += 1;

        let name = format!("_ori_catch_thunk${counter}");
        let thunk_id = self.builder.declare_void_function(&name, &[ptr_ty]);
        self.builder.set_ccc(thunk_id);
        // NOT nounwind — callee may panic; uwtable for SEH frame info
        self.builder.add_uwtable_attribute(thunk_id);

        // Save builder state
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        let saved_emitter_func = self.current_function;
        let saved_funclet_pad = self.current_funclet_pad.take();

        let entry = self.builder.append_block(thunk_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(thunk_id);
        self.current_function = thunk_id;

        let ctx_ptr = self.builder.get_param(thunk_id, 0);

        // Load args from context
        let mut call_args: Vec<ValueId> = Vec::with_capacity(num_args);
        for i in 0..num_args {
            let field_ptr = self.builder.struct_gep(
                ctx_struct_ty,
                ctx_ptr,
                i as u32,
                &format!("thunk.arg.{i}"),
            );
            let val = self.builder.load(ptr_ty, field_ptr, "thunk.load");
            call_args.push(val);
        }

        // Call the runtime function
        let result = self.builder.call(callee_id, &call_args, "thunk.call");

        // Store result
        if let Some(val) = result {
            let result_ptr = self.builder.struct_gep(
                ctx_struct_ty,
                ctx_ptr,
                result_field_idx,
                "thunk.result.ptr",
            );
            self.builder.store(val, result_ptr);
        }

        self.builder.ret_void();

        // Restore builder state
        self.current_funclet_pad = saved_funclet_pad;
        self.current_function = saved_emitter_func;
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        thunk_id
    }
}
