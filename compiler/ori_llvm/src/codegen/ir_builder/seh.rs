//! SEH (Structured Exception Handling) instruction builders for Windows MSVC.
//!
//! SEH uses `catchswitch` / `catchpad` / `cleanuppad` instead of Itanium's
//! `landingpad` / `resume`. These instructions return LLVM `token` type,
//! which is stored via [`TokenId`] in the arena.
//!
//! All function calls and invokes inside a funclet pad must carry an operand
//! bundle `"funclet"(token %pad)` — LLVM's verifier rejects calls without it.

use std::ffi::CString;

use inkwell::types::{AnyType, AsTypeRef, BasicType};
use inkwell::values::{AnyValueEnum, AsValueRef, InstructionValue};
use llvm_sys::core::{
    LLVMAddHandler, LLVMBuildCallWithOperandBundles, LLVMBuildCatchPad, LLVMBuildCatchRet,
    LLVMBuildCatchSwitch, LLVMBuildCleanupPad, LLVMBuildCleanupRet,
    LLVMBuildInvokeWithOperandBundles,
};
use llvm_sys::prelude::{LLVMBasicBlockRef, LLVMValueRef};

use super::IrBuilder;
use crate::codegen::value_id::{BlockId, FunctionId, LLVMTypeId, TokenId, ValueId};

impl<'ctx> IrBuilder<'_, 'ctx> {
    // -- SEH pad instructions --

    /// Build a `catchswitch` instruction.
    ///
    /// The `catchswitch` acts as a dispatch: it lists handler blocks that
    /// will receive `catchpad` instructions. `unwind_dest` is where to
    /// resume unwinding if no handler matches (`None` = unwind to caller).
    ///
    /// Returns a `TokenId` for the catchswitch token. This terminates the
    /// current basic block.
    pub fn catchswitch(
        &mut self,
        parent_pad: Option<TokenId>,
        handlers: &[BlockId],
        unwind_dest: Option<BlockId>,
    ) -> TokenId {
        let parent = parent_pad.map_or(std::ptr::null_mut(), |id| {
            self.arena.get_token(id).as_value_ref()
        });
        let unwind_bb: LLVMBasicBlockRef = unwind_dest.map_or(std::ptr::null_mut(), |id| {
            self.arena.get_block(id).as_mut_ptr()
        });

        let name = c"catchswitch";
        // SAFETY: Builder is positioned at a valid insertion point. parent, unwind_bb,
        // and name are valid LLVM refs (or null for optional params) within this module.
        let cs = unsafe {
            LLVMBuildCatchSwitch(
                self.builder.as_mut_ptr(),
                parent,
                unwind_bb,
                handlers.len().try_into().unwrap_or(1),
                name.as_ptr(),
            )
        };

        for &handler_id in handlers {
            let handler_bb = self.arena.get_block(handler_id);
            // SAFETY: cs is the non-null catchswitch just built; handler_bb is a valid block in this function.
            unsafe { LLVMAddHandler(cs, handler_bb.as_mut_ptr()) };
        }

        // SAFETY: cs is a non-null instruction value returned by LLVMBuildCatchSwitch.
        let inst = unsafe { InstructionValue::new(cs) };
        self.arena.push_token(inst)
    }

    /// Build a `catchpad` instruction.
    ///
    /// Must be the first non-phi instruction in a handler block listed by
    /// a `catchswitch`. The `parent_pad` is the `TokenId` from the
    /// `catchswitch`. `args` are the catch filter arguments.
    ///
    /// For C++ catch-all: `[ptr null, i32 64, ptr null]`.
    pub fn catchpad(&mut self, parent_pad: TokenId, args: &[ValueId]) -> TokenId {
        let parent = self.arena.get_token(parent_pad).as_value_ref();
        let mut arg_vals: Vec<LLVMValueRef> = args
            .iter()
            .map(|&id| self.arena.get_value(id).as_value_ref())
            .collect();
        let name = c"catchpad";

        // SAFETY: Builder is positioned at a valid insertion point. parent is a valid
        // catchswitch token from the arena. arg_vals are valid LLVM values in this module.
        let pad = unsafe {
            LLVMBuildCatchPad(
                self.builder.as_mut_ptr(),
                parent,
                arg_vals.as_mut_ptr(),
                arg_vals.len().try_into().unwrap_or(0),
                name.as_ptr(),
            )
        };

        // SAFETY: pad is a non-null instruction value returned by LLVMBuildCatchPad.
        let inst = unsafe { InstructionValue::new(pad) };
        self.arena.push_token(inst)
    }

    /// Build a `cleanuppad` instruction.
    ///
    /// Must be the first non-phi instruction in an unwind block that
    /// performs cleanup (RC decrements) then re-raises. `parent_pad`
    /// is `None` for the outermost cleanup (token `none`).
    pub fn cleanuppad(&mut self, parent_pad: Option<TokenId>, args: &[ValueId]) -> TokenId {
        let parent = parent_pad.map_or(std::ptr::null_mut(), |id| {
            self.arena.get_token(id).as_value_ref()
        });
        let mut arg_vals: Vec<LLVMValueRef> = args
            .iter()
            .map(|&id| self.arena.get_value(id).as_value_ref())
            .collect();
        let name = c"cleanuppad";

        // SAFETY: Builder is positioned at a valid insertion point. parent is a valid
        // token (or null for outermost cleanup). arg_vals are valid LLVM values.
        let pad = unsafe {
            LLVMBuildCleanupPad(
                self.builder.as_mut_ptr(),
                parent,
                arg_vals.as_mut_ptr(),
                arg_vals.len().try_into().unwrap_or(0),
                name.as_ptr(),
            )
        };

        // SAFETY: pad is a non-null instruction value returned by LLVMBuildCleanupPad.
        let inst = unsafe { InstructionValue::new(pad) };
        self.arena.push_token(inst)
    }

    // -- SEH terminator instructions --

    /// Build a `catchret` instruction.
    ///
    /// Returns from a `catchpad` to a normal block. This terminates the
    /// current basic block.
    pub fn catchret(&mut self, pad: TokenId, dest: BlockId) {
        let pad_val = self.arena.get_token(pad).as_value_ref();
        let dest_bb = self.arena.get_block(dest);
        // SAFETY: Builder is positioned at a valid insertion point. pad_val is a valid
        // catchpad token and dest_bb is a valid block, both from the arena.
        unsafe {
            LLVMBuildCatchRet(self.builder.as_mut_ptr(), pad_val, dest_bb.as_mut_ptr());
        }
    }

    /// Build a `cleanupret` instruction.
    ///
    /// Returns from a `cleanuppad`, re-raising the exception. `unwind_dest`
    /// is the next unwind target (`None` = unwind to caller).
    /// This terminates the current basic block.
    pub fn cleanupret(&mut self, pad: TokenId, unwind_dest: Option<BlockId>) {
        let pad_val = self.arena.get_token(pad).as_value_ref();
        let unwind_bb: LLVMBasicBlockRef = unwind_dest.map_or(std::ptr::null_mut(), |id| {
            self.arena.get_block(id).as_mut_ptr()
        });
        // SAFETY: Builder is positioned at a valid insertion point. pad_val is a valid
        // cleanuppad token. unwind_bb is a valid block or null (unwind to caller).
        unsafe {
            LLVMBuildCleanupRet(self.builder.as_mut_ptr(), pad_val, unwind_bb);
        }
    }

    // -- Funclet-aware call/invoke --

    /// Build a function call with a `"funclet"` operand bundle.
    ///
    /// Required for all calls inside a SEH pad (catchpad/cleanuppad).
    /// LLVM's verifier rejects calls inside funclets without the bundle.
    ///
    /// Replicates the calling convention copy and `last_call_site` tracking
    /// from [`call()`](Self::call) — without these, fastcc calls inside
    /// SEH pads silently degrade and COW noalias hints are lost.
    pub fn call_with_funclet(
        &mut self,
        callee: FunctionId,
        args: &[ValueId],
        pad: TokenId,
        name: &str,
    ) -> Option<ValueId> {
        let func = self.arena.get_function(callee);
        let pad_inst = self.arena.get_token(pad);
        let pad_any: AnyValueEnum<'ctx> = pad_inst.into();

        let bundle = inkwell::values::OperandBundle::create("funclet", &[pad_any]);

        let arg_vals: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args
            .iter()
            .map(|&id| self.arena.get_value(id).into())
            .collect();

        let call_val = self
            .builder
            .build_direct_call_with_operand_bundles(func, &arg_vals, &[bundle], name)
            .expect("call_with_funclet");

        // CC copy — critical for fastcc inside funclets
        call_val.set_call_convention(func.get_call_conventions());
        self.last_call_site = Some(call_val);

        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build an invoke with a `"funclet"` operand bundle.
    ///
    /// Required for invokes inside a SEH pad. Uses `llvm_sys` directly
    /// since inkwell doesn't wrap `LLVMBuildInvokeWithOperandBundles`.
    pub fn invoke_with_funclet(
        &mut self,
        callee: FunctionId,
        args: &[ValueId],
        pad: TokenId,
        then_block: BlockId,
        catch_block: BlockId,
        name: &str,
    ) -> Option<ValueId> {
        let func = self.arena.get_function(callee);
        let pad_inst = self.arena.get_token(pad);
        let then_bb = self.arena.get_block(then_block);
        let catch_bb = self.arena.get_block(catch_block);

        // Create "funclet" operand bundle via inkwell, extract raw ptr
        let pad_any: AnyValueEnum<'ctx> = pad_inst.into();
        let bundle = inkwell::values::OperandBundle::create("funclet", &[pad_any]);

        let mut arg_vals: Vec<LLVMValueRef> = args
            .iter()
            .map(|&id| self.arena.get_value(id).as_value_ref())
            .collect();

        let fn_ty = func.get_type();
        // LLVM doesn't like named void calls
        let effective_name = if fn_ty.get_return_type().is_none() {
            ""
        } else {
            name
        };
        let c_name = CString::new(effective_name).unwrap_or_default();
        let mut bundles = [bundle.as_mut_ptr()];

        // SAFETY: Builder is positioned at a valid insertion point. fn_ty, func, args,
        // then_bb, catch_bb are valid LLVM refs from the arena. The funclet operand bundle
        // is valid for the lifetime of this call. c_name is a valid null-terminated C string.
        let invoke_val = unsafe {
            LLVMBuildInvokeWithOperandBundles(
                self.builder.as_mut_ptr(),
                fn_ty.as_type_ref(),
                func.as_value_ref(),
                arg_vals.as_mut_ptr(),
                arg_vals.len().try_into().unwrap_or(0),
                then_bb.as_mut_ptr(),
                catch_bb.as_mut_ptr(),
                bundles.as_mut_ptr(),
                1,
                c_name.as_ptr(),
            )
        };

        // Don't dispose bundle — inkwell's OperandBundle Drop handles it.
        // But we used `as_mut_ptr()` which doesn't consume the OperandBundle,
        // so Drop will run and clean up.

        // SAFETY: invoke_val is a non-null invoke instruction returned by LLVMBuildInvokeWithOperandBundles.
        let call_site = unsafe { inkwell::values::CallSiteValue::new(invoke_val) };
        // CC copy — critical for fastcc inside funclets
        call_site.set_call_convention(func.get_call_conventions());
        self.last_call_site = Some(call_site);

        call_site
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build an indirect function call with a `"funclet"` operand bundle.
    ///
    /// Required for `ApplyIndirect` (closure calls) inside a SEH pad.
    /// Uses `LLVMBuildCallWithOperandBundles` from `llvm_sys` since inkwell
    /// doesn't wrap indirect calls with operand bundles.
    pub fn call_indirect_with_funclet(
        &mut self,
        return_type: LLVMTypeId,
        param_types: &[LLVMTypeId],
        fn_ptr: ValueId,
        args: &[ValueId],
        pad: TokenId,
        name: &str,
    ) -> Option<ValueId> {
        let raw = self.arena.get_value(fn_ptr);
        if !raw.is_pointer_value() {
            tracing::error!(
                val_type = ?raw.get_type(),
                "call_indirect_with_funclet on non-pointer"
            );
            self.record_codegen_error();
            return None;
        }

        let ret_ty = self.arena.get_type(return_type);
        let param_tys: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();
        let func_ty = ret_ty.fn_type(&param_tys, false);

        let pad_inst = self.arena.get_token(pad);
        let pad_any: AnyValueEnum<'ctx> = pad_inst.into();
        let bundle = inkwell::values::OperandBundle::create("funclet", &[pad_any]);

        let mut arg_vals: Vec<LLVMValueRef> = args
            .iter()
            .map(|&id| self.arena.get_value(id).as_value_ref())
            .collect();

        // LLVM doesn't like named void calls
        let effective_name = if func_ty.get_return_type().is_none() {
            ""
        } else {
            name
        };
        let c_name = CString::new(effective_name).unwrap_or_default();
        let mut bundles = [bundle.as_mut_ptr()];

        // SAFETY: Builder is positioned at a valid insertion point. func_ty and raw (fn_ptr)
        // are valid LLVM refs. arg_vals are valid values. The funclet operand bundle is valid
        // for the lifetime of this call. c_name is a valid null-terminated C string.
        let call_val = unsafe {
            LLVMBuildCallWithOperandBundles(
                self.builder.as_mut_ptr(),
                func_ty.as_type_ref(),
                raw.as_value_ref(),
                arg_vals.as_mut_ptr(),
                arg_vals.len().try_into().unwrap_or(0),
                bundles.as_mut_ptr(),
                1,
                c_name.as_ptr(),
            )
        };

        // SAFETY: call_val is a non-null call instruction returned by LLVMBuildCallWithOperandBundles.
        let call_site = unsafe { inkwell::values::CallSiteValue::new(call_val) };
        self.last_call_site = Some(call_site);

        call_site
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build an indirect function call with both `sret` return and a `"funclet"`
    /// operand bundle.
    ///
    /// Combines `call_indirect_with_sret` (void return, sret pointer as first
    /// arg with sret+noalias attributes) with `call_indirect_with_funclet`
    /// (funclet operand bundle for SEH). Required when a closure returning a
    /// large type (>16 bytes) is called inside a SEH catchpad or cleanuppad.
    pub fn call_indirect_with_sret_and_funclet(
        &mut self,
        sret_type: LLVMTypeId,
        param_types: &[LLVMTypeId],
        fn_ptr: ValueId,
        sret_ptr: ValueId,
        args: &[ValueId],
        pad: TokenId,
    ) {
        let raw = self.arena.get_value(fn_ptr);
        if !raw.is_pointer_value() {
            tracing::error!(
                val_type = ?raw.get_type(),
                "call_indirect_with_sret_and_funclet on non-pointer"
            );
            self.record_codegen_error();
            return;
        }

        // Build full param list: sret_ptr + remaining args
        let mut all_args: Vec<LLVMValueRef> = Vec::with_capacity(1 + args.len());
        all_args.push(self.arena.get_value(sret_ptr).as_value_ref());
        for &id in args {
            all_args.push(self.arena.get_value(id).as_value_ref());
        }

        // Build function type: void(ptr, param_types...)
        let ptr_ty: inkwell::types::BasicMetadataTypeEnum<'ctx> = self.scx.type_ptr().into();
        let mut all_param_tys: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(1 + param_types.len());
        all_param_tys.push(ptr_ty);
        for &id in param_types {
            all_param_tys.push(self.arena.get_type(id).into());
        }
        let fn_type = self.scx.type_void_func(&all_param_tys);

        // Build funclet operand bundle
        let pad_inst = self.arena.get_token(pad);
        let pad_any: AnyValueEnum<'ctx> = pad_inst.into();
        let bundle = inkwell::values::OperandBundle::create("funclet", &[pad_any]);
        let mut bundles = [bundle.as_mut_ptr()];

        let c_name = CString::new("").unwrap_or_default();

        // SAFETY: Builder is positioned, fn_type/fn_ptr/args/pad are valid LLVM refs.
        let call_val = unsafe {
            LLVMBuildCallWithOperandBundles(
                self.builder.as_mut_ptr(),
                fn_type.as_type_ref(),
                raw.as_value_ref(),
                all_args.as_mut_ptr(),
                all_args.len().try_into().unwrap_or(0),
                bundles.as_mut_ptr(),
                1,
                c_name.as_ptr(),
            )
        };

        // SAFETY: call_val is a non-null call instruction.
        let call_site = unsafe { inkwell::values::CallSiteValue::new(call_val) };
        self.last_call_site = Some(call_site);

        // Add sret + noalias attributes on the first parameter.
        let sret_ty = self.arena.get_type(sret_type);
        let sret_kind = inkwell::attributes::Attribute::get_named_enum_kind_id("sret");
        let sret_attr = self
            .scx
            .llcx
            .create_type_attribute(sret_kind, sret_ty.as_any_type_enum());
        call_site.add_attribute(inkwell::attributes::AttributeLoc::Param(0), sret_attr);

        let noalias_kind = inkwell::attributes::Attribute::get_named_enum_kind_id("noalias");
        let noalias_attr = self.scx.llcx.create_enum_attribute(noalias_kind, 0);
        call_site.add_attribute(inkwell::attributes::AttributeLoc::Param(0), noalias_attr);
    }
}
