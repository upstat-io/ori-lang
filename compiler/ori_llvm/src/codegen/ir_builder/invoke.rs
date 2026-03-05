//! Invoke and exception handling operations for `IrBuilder`.
//!
//! Contains methods for building `invoke` instructions (calls that may
//! unwind), landing pads, and resume instructions — the Itanium EH ABI
//! surface. For SEH (Windows MSVC), see [`super::seh`].

use inkwell::types::{BasicMetadataTypeEnum, BasicType};
use inkwell::values::BasicValueEnum;

use super::IrBuilder;
use crate::codegen::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

impl<'ctx> IrBuilder<'_, 'ctx> {
    /// Build a direct invoke (call that may unwind).
    ///
    /// On normal return, execution continues at `then_block`.
    /// On unwind (exception), execution continues at `catch_block`.
    ///
    /// Returns `None` for void-returning functions, `Some(ValueId)` otherwise.
    /// The result value is only valid in `then_block`.
    pub fn invoke(
        &mut self,
        callee: FunctionId,
        args: &[ValueId],
        then_block: BlockId,
        catch_block: BlockId,
        name: &str,
    ) -> Option<ValueId> {
        let func = self.arena.get_function(callee);
        let arg_vals: Vec<BasicValueEnum<'ctx>> =
            args.iter().map(|&id| self.arena.get_value(id)).collect();
        let then_bb = self.arena.get_block(then_block);
        let catch_bb = self.arena.get_block(catch_block);
        let call_val = self
            .builder
            .build_invoke(func, &arg_vals, then_bb, catch_bb, name)
            .expect("invoke");
        // inkwell's build_invoke does not reliably copy the calling convention
        // from the callee. Without this, fastcc callees get invoked with the
        // default ccc, causing SIGSEGV or wrong results.
        call_val.set_call_convention(func.get_call_conventions());
        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build an indirect invoke through a function pointer.
    ///
    /// Like [`invoke`], but the callee is a function pointer with an
    /// explicit type signature.
    pub fn invoke_indirect(
        &mut self,
        return_type: LLVMTypeId,
        param_types: &[LLVMTypeId],
        fn_ptr: ValueId,
        args: &[ValueId],
        then_block: BlockId,
        catch_block: BlockId,
        name: &str,
    ) -> Option<ValueId> {
        let raw = self.arena.get_value(fn_ptr);
        if !raw.is_pointer_value() {
            tracing::error!(val_type = ?raw.get_type(), "invoke_indirect on non-pointer");
            self.record_codegen_error();
            return None;
        }
        let ptr = raw.into_pointer_value();
        let arg_vals: Vec<BasicValueEnum<'ctx>> =
            args.iter().map(|&id| self.arena.get_value(id)).collect();

        let ret_ty = self.arena.get_type(return_type);
        let param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();
        let func_ty = ret_ty.fn_type(&param_tys, false);

        let then_bb = self.arena.get_block(then_block);
        let catch_bb = self.arena.get_block(catch_block);
        let call_val = self
            .builder
            .build_indirect_invoke(func_ty, ptr, &arg_vals, then_bb, catch_bb, name)
            .expect("invoke_indirect");
        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build a `landingpad` instruction for exception handling cleanup.
    ///
    /// `personality` is the personality function (typically `__gxx_personality_v0`
    /// for C++/Rust Itanium EH ABI). `is_cleanup` should be `true` for cleanup
    /// pads that don't catch specific exceptions.
    ///
    /// Returns the landing pad value (an `{ i8*, i32 }` struct) as a `ValueId`.
    pub fn landingpad(&mut self, personality: FunctionId, is_cleanup: bool, name: &str) -> ValueId {
        let personality_fn = self.arena.get_function(personality);

        // Landing pad type is { ptr, i32 } (Itanium ABI convention).
        let i8_ptr_ty = self.scx.ptr_type;
        let i32_ty = self.scx.llcx.i32_type();
        let lp_ty = self
            .scx
            .llcx
            .struct_type(&[i8_ptr_ty.into(), i32_ty.into()], false);

        let lp_val = self
            .builder
            .build_landing_pad(lp_ty, personality_fn, &[], is_cleanup, name)
            .expect("landingpad");
        self.arena.push_value(lp_val)
    }

    /// Build a `landingpad catch null` instruction for catch-all exception handling.
    ///
    /// Unlike [`landingpad`](Self::landingpad) (cleanup-only), this emits a
    /// catch-all clause (`catch ptr null`) that tells the unwinder "I handle
    /// all exceptions." The caught exception must be acknowledged with
    /// `__cxa_begin_catch` / `__cxa_end_catch` before normal execution resumes.
    ///
    /// Used by `catch(expr:)` unwind blocks.
    pub fn landingpad_catch_all(&mut self, personality: FunctionId, name: &str) -> ValueId {
        let personality_fn = self.arena.get_function(personality);
        let i8_ptr_ty = self.scx.ptr_type;
        let i32_ty = self.scx.llcx.i32_type();
        let lp_ty = self
            .scx
            .llcx
            .struct_type(&[i8_ptr_ty.into(), i32_ty.into()], false);

        // catch null = catch-all (Itanium ABI: null typeinfo matches everything)
        let null_clause = i8_ptr_ty.const_null();
        let lp_val = self
            .builder
            .build_landing_pad(lp_ty, personality_fn, &[null_clause.into()], false, name)
            .expect("landingpad_catch_all");
        self.arena.push_value(lp_val)
    }

    /// Build a `resume` instruction to re-raise an exception.
    ///
    /// `value` must be the result of a `landingpad` instruction.
    /// This terminates the current basic block.
    pub fn resume(&mut self, value: ValueId) {
        let v = self.arena.get_value(value);
        self.builder.build_resume(v).expect("resume");
    }

    /// Set the personality function on an LLVM function.
    ///
    /// Required for any function containing `invoke`/`landingpad`.
    /// Typically `__gxx_personality_v0` (Itanium EH ABI on Linux/macOS).
    pub fn set_personality(&mut self, func: FunctionId, personality: FunctionId) {
        let func_val = self.arena.get_function(func);
        let personality_fn = self.arena.get_function(personality);
        func_val.set_personality_function(personality_fn);
    }
}
