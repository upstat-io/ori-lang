//! LLVM Instruction Builder
//!
//! Follows Rust's `Builder` pattern from `rustc_codegen_llvm/src/builder.rs`.
//!
//! The Builder wraps an LLVM `IRBuilder` and provides methods for generating
//! LLVM IR instructions. It is scoped to a single basic block and provides
//! a clean API for code generation.
//!
//! Key differences from having methods on `CodegenCx`:
//! - Builder is scoped to a basic block (position tracking)
//! - Instructions are generated in the builder's current position
//! - Clean separation between type-level operations (`CodegenCx`) and
//!   instruction generation (Builder)
//!
//! # Code Organization
//!
//! Code is organized by concern, not by syntax element:
//!
//! | Concern | Location |
//! |---------|----------|
//! | Low-level LLVM ops | `builder.rs` (this file) |
//! | Pattern binding (`bind_pattern`) | `functions/sequences.rs` |
//! | Match expressions | `matching.rs` |
//! | Struct/tuple/list creation | `collections/*.rs` |
//! | Function codegen | `functions/*.rs` |
//!
//! **Important**: `bind_pattern()` lives in `sequences.rs`, not here.
//! The `compile_let()` method in this file calls `self.bind_pattern()`
//! which resolves to the implementation in `sequences.rs`.

use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder as LLVMBuilder;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{
    BasicValue, BasicValueEnum, FunctionValue, IntValue, PhiValue, PointerValue, StructValue,
};
use inkwell::IntPredicate;
use rustc_hash::FxHashMap;
use tracing::{instrument, warn};

use ori_ir::ast::patterns::BindingPattern;
use ori_ir::ast::{BinaryOp, ExprKind};
use ori_ir::{ExprArena, ExprId, Name};
use ori_types::Idx;

use crate::context::CodegenCx;
use crate::LoopContext;

// Local Variable Storage

/// Storage strategy for a local variable.
///
/// LLVM uses SSA (Static Single Assignment) where each value is assigned once.
/// For mutable variables in loops, we need stack allocation with load/store
/// so that reassignment updates the memory location, not an immutable SSA value.
#[derive(Debug, Clone, Copy)]
pub enum LocalStorage<'ctx> {
    /// Immutable: SSA value in register (efficient, but can't be reassigned)
    Immutable(BasicValueEnum<'ctx>),
    /// Mutable: stack-allocated via alloca (supports reassignment via load/store)
    Mutable {
        /// Pointer to the stack slot
        ptr: PointerValue<'ctx>,
        /// Type of the stored value (needed for load instruction)
        ty: BasicTypeEnum<'ctx>,
    },
}

/// Manages local variable bindings with mutability awareness.
///
/// Replaces the simple `FxHashMap<Name, BasicValueEnum>` to track whether
/// each variable needs SSA (immutable) or alloca/load/store (mutable) semantics.
#[derive(Debug, Clone, Default)]
pub struct Locals<'ctx> {
    bindings: FxHashMap<Name, LocalStorage<'ctx>>,
}

impl<'ctx> Locals<'ctx> {
    /// Create a new empty locals map.
    pub fn new() -> Self {
        Self {
            bindings: FxHashMap::default(),
        }
    }

    /// Bind an immutable variable (SSA value).
    pub fn bind_immutable(&mut self, name: Name, value: BasicValueEnum<'ctx>) {
        self.bindings.insert(name, LocalStorage::Immutable(value));
    }

    /// Bind a mutable variable (stack-allocated).
    pub fn bind_mutable(&mut self, name: Name, ptr: PointerValue<'ctx>, ty: BasicTypeEnum<'ctx>) {
        self.bindings
            .insert(name, LocalStorage::Mutable { ptr, ty });
    }

    /// Get the storage for a variable.
    pub fn get_storage(&self, name: &Name) -> Option<&LocalStorage<'ctx>> {
        self.bindings.get(name)
    }

    /// Check if a variable exists.
    pub fn contains(&self, name: &Name) -> bool {
        self.bindings.contains_key(name)
    }
}

/// LLVM instruction builder.
///
/// Wraps an LLVM `IRBuilder` and provides methods for generating instructions.
/// The builder maintains a current insertion point (basic block) and all
/// instruction generation methods insert at that point.
pub struct Builder<'a, 'll, 'tcx> {
    /// The underlying LLVM builder.
    llbuilder: LLVMBuilder<'ll>,
    /// Reference to the codegen context.
    cx: &'a CodegenCx<'ll, 'tcx>,
}

/// RAII guard that restores the builder's position when dropped.
///
/// Use this when temporarily repositioning the builder (e.g., for lambda compilation)
/// to ensure the original position is restored, even on early returns or panics.
pub struct BuilderPositionGuard<'a, 'b, 'll, 'tcx> {
    builder: &'a Builder<'b, 'll, 'tcx>,
    saved_block: Option<BasicBlock<'ll>>,
}

impl Drop for BuilderPositionGuard<'_, '_, '_, '_> {
    fn drop(&mut self) {
        if let Some(block) = self.saved_block {
            self.builder.position_at_end(block);
        }
    }
}

impl<'a, 'll, 'tcx> Builder<'a, 'll, 'tcx> {
    /// Create a new builder positioned at the end of the given basic block.
    pub fn build(cx: &'a CodegenCx<'ll, 'tcx>, bb: BasicBlock<'ll>) -> Self {
        let llbuilder = cx.llcx().create_builder();
        llbuilder.position_at_end(bb);
        Self { llbuilder, cx }
    }

    /// Get the codegen context.
    #[inline]
    pub fn cx(&self) -> &'a CodegenCx<'ll, 'tcx> {
        self.cx
    }

    /// Get the current basic block.
    pub fn current_block(&self) -> Option<BasicBlock<'ll>> {
        self.llbuilder.get_insert_block()
    }

    /// Get the function that contains the current insertion point.
    pub fn get_current_function(&self) -> FunctionValue<'ll> {
        self.current_block()
            .expect("builder has insertion point")
            .get_parent()
            .expect("block has parent function")
    }

    /// Position at the end of a basic block.
    pub fn position_at_end(&self, bb: BasicBlock<'ll>) {
        self.llbuilder.position_at_end(bb);
    }

    /// Save the current builder position and return an RAII guard.
    ///
    /// When the guard is dropped, the builder is repositioned to the saved block.
    /// Use this when temporarily repositioning the builder (e.g., for lambda compilation).
    pub fn save_position(&self) -> BuilderPositionGuard<'_, '_, 'll, 'tcx> {
        BuilderPositionGuard {
            builder: self,
            saved_block: self.current_block(),
        }
    }

    /// Append a new basic block to the given function.
    pub fn append_block(&self, function: FunctionValue<'ll>, name: &str) -> BasicBlock<'ll> {
        self.cx.llcx().append_basic_block(function, name)
    }

    // -- Terminators --

    /// Build a return with no value (void return).
    pub fn ret_void(&self) {
        self.llbuilder.build_return(None).expect("build_return");
    }

    /// Build a return with a value.
    pub fn ret(&self, val: BasicValueEnum<'ll>) {
        self.llbuilder
            .build_return(Some(&val))
            .expect("build_return");
    }

    /// Build an unconditional branch.
    pub fn br(&self, dest: BasicBlock<'ll>) {
        self.llbuilder
            .build_unconditional_branch(dest)
            .expect("build_br");
    }

    /// Build a conditional branch.
    pub fn cond_br(&self, cond: IntValue<'ll>, then_bb: BasicBlock<'ll>, else_bb: BasicBlock<'ll>) {
        self.llbuilder
            .build_conditional_branch(cond, then_bb, else_bb)
            .expect("build_cond_br");
    }

    /// Build an unreachable terminator.
    pub fn unreachable(&self) {
        self.llbuilder
            .build_unreachable()
            .expect("build_unreachable");
    }

    /// Build a global string pointer (C-style null-terminated string).
    ///
    /// Returns a pointer to the string data that can be passed to C functions.
    pub fn build_global_string_ptr(&self, value: &str, name: &str) -> PointerValue<'ll> {
        self.llbuilder
            .build_global_string_ptr(value, name)
            .expect("build_global_string_ptr")
            .as_pointer_value()
    }

    // -- Arithmetic --

    /// Build integer addition.
    pub fn add(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder.build_int_add(lhs, rhs, name).expect("add")
    }

    /// Build integer subtraction.
    pub fn sub(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder.build_int_sub(lhs, rhs, name).expect("sub")
    }

    /// Build integer multiplication.
    pub fn mul(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder.build_int_mul(lhs, rhs, name).expect("mul")
    }

    /// Build signed integer division.
    pub fn sdiv(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder
            .build_int_signed_div(lhs, rhs, name)
            .expect("sdiv")
    }

    /// Build unsigned integer division.
    pub fn udiv(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder
            .build_int_unsigned_div(lhs, rhs, name)
            .expect("udiv")
    }

    /// Build signed integer remainder.
    pub fn srem(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder
            .build_int_signed_rem(lhs, rhs, name)
            .expect("srem")
    }

    /// Build unsigned integer remainder.
    pub fn urem(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder
            .build_int_unsigned_rem(lhs, rhs, name)
            .expect("urem")
    }

    /// Build integer negation.
    pub fn neg(&self, val: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder.build_int_neg(val, name).expect("neg")
    }

    /// Build integer NOT (bitwise complement).
    pub fn not(&self, val: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder.build_not(val, name).expect("not")
    }

    // -- Floating point arithmetic --

    /// Build floating-point addition.
    pub fn fadd(
        &self,
        lhs: inkwell::values::FloatValue<'ll>,
        rhs: inkwell::values::FloatValue<'ll>,
        name: &str,
    ) -> inkwell::values::FloatValue<'ll> {
        self.llbuilder
            .build_float_add(lhs, rhs, name)
            .expect("fadd")
    }

    /// Build floating-point subtraction.
    pub fn fsub(
        &self,
        lhs: inkwell::values::FloatValue<'ll>,
        rhs: inkwell::values::FloatValue<'ll>,
        name: &str,
    ) -> inkwell::values::FloatValue<'ll> {
        self.llbuilder
            .build_float_sub(lhs, rhs, name)
            .expect("fsub")
    }

    /// Build floating-point multiplication.
    pub fn fmul(
        &self,
        lhs: inkwell::values::FloatValue<'ll>,
        rhs: inkwell::values::FloatValue<'ll>,
        name: &str,
    ) -> inkwell::values::FloatValue<'ll> {
        self.llbuilder
            .build_float_mul(lhs, rhs, name)
            .expect("fmul")
    }

    /// Build floating-point division.
    pub fn fdiv(
        &self,
        lhs: inkwell::values::FloatValue<'ll>,
        rhs: inkwell::values::FloatValue<'ll>,
        name: &str,
    ) -> inkwell::values::FloatValue<'ll> {
        self.llbuilder
            .build_float_div(lhs, rhs, name)
            .expect("fdiv")
    }

    /// Build floating-point remainder.
    pub fn frem(
        &self,
        lhs: inkwell::values::FloatValue<'ll>,
        rhs: inkwell::values::FloatValue<'ll>,
        name: &str,
    ) -> inkwell::values::FloatValue<'ll> {
        self.llbuilder
            .build_float_rem(lhs, rhs, name)
            .expect("frem")
    }

    /// Build floating-point negation.
    pub fn fneg(
        &self,
        val: inkwell::values::FloatValue<'ll>,
        name: &str,
    ) -> inkwell::values::FloatValue<'ll> {
        self.llbuilder.build_float_neg(val, name).expect("fneg")
    }

    // -- Bitwise operations --

    /// Build bitwise AND.
    pub fn and(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder.build_and(lhs, rhs, name).expect("and")
    }

    /// Build bitwise OR.
    pub fn or(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder.build_or(lhs, rhs, name).expect("or")
    }

    /// Build bitwise XOR.
    pub fn xor(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder.build_xor(lhs, rhs, name).expect("xor")
    }

    /// Build left shift.
    pub fn shl(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder
            .build_left_shift(lhs, rhs, name)
            .expect("shl")
    }

    /// Build arithmetic right shift (sign-extending).
    pub fn ashr(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder
            .build_right_shift(lhs, rhs, true, name)
            .expect("ashr")
    }

    /// Build logical right shift (zero-extending).
    pub fn lshr(&self, lhs: IntValue<'ll>, rhs: IntValue<'ll>, name: &str) -> IntValue<'ll> {
        self.llbuilder
            .build_right_shift(lhs, rhs, false, name)
            .expect("lshr")
    }

    // -- Comparisons --

    /// Build integer comparison.
    pub fn icmp(
        &self,
        pred: IntPredicate,
        lhs: IntValue<'ll>,
        rhs: IntValue<'ll>,
        name: &str,
    ) -> IntValue<'ll> {
        self.llbuilder
            .build_int_compare(pred, lhs, rhs, name)
            .expect("icmp")
    }

    /// Build floating-point comparison.
    pub fn fcmp(
        &self,
        pred: inkwell::FloatPredicate,
        lhs: inkwell::values::FloatValue<'ll>,
        rhs: inkwell::values::FloatValue<'ll>,
        name: &str,
    ) -> IntValue<'ll> {
        self.llbuilder
            .build_float_compare(pred, lhs, rhs, name)
            .expect("fcmp")
    }

    // -- Memory operations --

    /// Build alloca (stack allocation).
    pub fn alloca(&self, ty: BasicTypeEnum<'ll>, name: &str) -> PointerValue<'ll> {
        self.llbuilder.build_alloca(ty, name).expect("alloca")
    }

    /// Build load from pointer.
    pub fn load(
        &self,
        ty: BasicTypeEnum<'ll>,
        ptr: PointerValue<'ll>,
        name: &str,
    ) -> BasicValueEnum<'ll> {
        self.llbuilder.build_load(ty, ptr, name).expect("load")
    }

    /// Build store to pointer.
    pub fn store(&self, val: BasicValueEnum<'ll>, ptr: PointerValue<'ll>) {
        self.llbuilder.build_store(ptr, val).expect("store");
    }

    // -- Aggregate operations --

    /// Build extract value from aggregate (struct, array).
    ///
    /// Returns `None` if the index is out of range for the struct.
    pub fn extract_value(
        &self,
        agg: StructValue<'ll>,
        index: u32,
        name: &str,
    ) -> Option<BasicValueEnum<'ll>> {
        self.llbuilder.build_extract_value(agg, index, name).ok()
    }

    /// Build insert value into aggregate.
    pub fn insert_value(
        &self,
        agg: StructValue<'ll>,
        val: BasicValueEnum<'ll>,
        index: u32,
        name: &str,
    ) -> StructValue<'ll> {
        self.llbuilder
            .build_insert_value(agg, val, index, name)
            .expect("insert_value")
            .into_struct_value()
    }

    /// Build struct from values.
    pub fn build_struct(
        &self,
        ty: inkwell::types::StructType<'ll>,
        values: &[BasicValueEnum<'ll>],
        name: &str,
    ) -> StructValue<'ll> {
        let mut result = ty.get_undef();
        for (i, val) in values.iter().enumerate() {
            result = self.insert_value(result, *val, i as u32, &format!("{name}.{i}"));
        }
        result
    }

    // -- Casts --

    /// Build truncate (to smaller integer).
    pub fn trunc(
        &self,
        val: IntValue<'ll>,
        ty: inkwell::types::IntType<'ll>,
        name: &str,
    ) -> IntValue<'ll> {
        self.llbuilder
            .build_int_truncate(val, ty, name)
            .expect("trunc")
    }

    /// Build zero-extend (to larger integer).
    pub fn zext(
        &self,
        val: IntValue<'ll>,
        ty: inkwell::types::IntType<'ll>,
        name: &str,
    ) -> IntValue<'ll> {
        self.llbuilder
            .build_int_z_extend(val, ty, name)
            .expect("zext")
    }

    /// Build sign-extend (to larger integer).
    pub fn sext(
        &self,
        val: IntValue<'ll>,
        ty: inkwell::types::IntType<'ll>,
        name: &str,
    ) -> IntValue<'ll> {
        self.llbuilder
            .build_int_s_extend(val, ty, name)
            .expect("sext")
    }

    /// Build signed int to float.
    pub fn sitofp(
        &self,
        val: IntValue<'ll>,
        ty: inkwell::types::FloatType<'ll>,
        name: &str,
    ) -> inkwell::values::FloatValue<'ll> {
        self.llbuilder
            .build_signed_int_to_float(val, ty, name)
            .expect("sitofp")
    }

    /// Build unsigned int to float.
    pub fn uitofp(
        &self,
        val: IntValue<'ll>,
        ty: inkwell::types::FloatType<'ll>,
        name: &str,
    ) -> inkwell::values::FloatValue<'ll> {
        self.llbuilder
            .build_unsigned_int_to_float(val, ty, name)
            .expect("uitofp")
    }

    /// Build float to signed int.
    pub fn fptosi(
        &self,
        val: inkwell::values::FloatValue<'ll>,
        ty: inkwell::types::IntType<'ll>,
        name: &str,
    ) -> IntValue<'ll> {
        self.llbuilder
            .build_float_to_signed_int(val, ty, name)
            .expect("fptosi")
    }

    /// Build float to unsigned int.
    pub fn fptoui(
        &self,
        val: inkwell::values::FloatValue<'ll>,
        ty: inkwell::types::IntType<'ll>,
        name: &str,
    ) -> IntValue<'ll> {
        self.llbuilder
            .build_float_to_unsigned_int(val, ty, name)
            .expect("fptoui")
    }

    /// Build bitcast.
    pub fn bitcast(
        &self,
        val: BasicValueEnum<'ll>,
        ty: BasicTypeEnum<'ll>,
        name: &str,
    ) -> BasicValueEnum<'ll> {
        self.llbuilder
            .build_bit_cast(val, ty, name)
            .expect("bitcast")
    }

    /// Build pointer to int conversion.
    pub fn ptr_to_int(
        &self,
        ptr: PointerValue<'ll>,
        ty: inkwell::types::IntType<'ll>,
        name: &str,
    ) -> IntValue<'ll> {
        self.llbuilder
            .build_ptr_to_int(ptr, ty, name)
            .expect("ptr_to_int")
    }

    /// Build int to pointer conversion.
    pub fn int_to_ptr(
        &self,
        val: IntValue<'ll>,
        ty: inkwell::types::PointerType<'ll>,
        name: &str,
    ) -> PointerValue<'ll> {
        self.llbuilder
            .build_int_to_ptr(val, ty, name)
            .expect("int_to_ptr")
    }

    // -- Calls --

    /// Build a function call.
    pub fn call(
        &self,
        callee: FunctionValue<'ll>,
        args: &[BasicValueEnum<'ll>],
        name: &str,
    ) -> Option<BasicValueEnum<'ll>> {
        let args_meta: Vec<inkwell::values::BasicMetadataValueEnum> =
            args.iter().map(|v| (*v).into()).collect();

        let call_val = self
            .llbuilder
            .build_call(callee, &args_meta, name)
            .expect("call");

        call_val.try_as_basic_value().basic()
    }

    /// Build an indirect call through a function pointer.
    pub fn call_indirect(
        &self,
        fn_type: inkwell::types::FunctionType<'ll>,
        fn_ptr: PointerValue<'ll>,
        args: &[BasicValueEnum<'ll>],
        name: &str,
    ) -> Option<BasicValueEnum<'ll>> {
        let args_meta: Vec<inkwell::values::BasicMetadataValueEnum> =
            args.iter().map(|v| (*v).into()).collect();

        let call_val = self
            .llbuilder
            .build_indirect_call(fn_type, fn_ptr, &args_meta, name)
            .expect("call_indirect");

        call_val.try_as_basic_value().basic()
    }

    // -- Phi nodes --

    /// Build a phi node.
    pub fn phi(&self, ty: BasicTypeEnum<'ll>, name: &str) -> PhiValue<'ll> {
        self.llbuilder.build_phi(ty, name).expect("phi")
    }

    /// Add incoming values to a phi node.
    pub fn add_incoming(
        &self,
        phi: PhiValue<'ll>,
        incoming: &[(&dyn BasicValue<'ll>, BasicBlock<'ll>)],
    ) {
        phi.add_incoming(incoming);
    }

    // -- Select --

    /// Build a select (ternary) instruction.
    pub fn select(
        &self,
        cond: IntValue<'ll>,
        then_val: BasicValueEnum<'ll>,
        else_val: BasicValueEnum<'ll>,
        name: &str,
    ) -> BasicValueEnum<'ll> {
        self.llbuilder
            .build_select(cond, then_val, else_val, name)
            .expect("select")
    }

    // -- GEP (GetElementPtr) --

    /// Build a struct GEP (field access).
    pub fn struct_gep(
        &self,
        ty: inkwell::types::StructType<'ll>,
        ptr: PointerValue<'ll>,
        index: u32,
        name: &str,
    ) -> PointerValue<'ll> {
        self.llbuilder
            .build_struct_gep(ty, ptr, index, name)
            .expect("struct_gep")
    }

    /// Build an in-bounds GEP.
    ///
    /// # Safety
    /// The caller must ensure that the indices are valid for the given type
    /// and that the resulting pointer is within bounds.
    #[allow(unsafe_code)]
    pub fn gep(
        &self,
        ty: BasicTypeEnum<'ll>,
        ptr: PointerValue<'ll>,
        indices: &[IntValue<'ll>],
        name: &str,
    ) -> PointerValue<'ll> {
        // SAFETY: The GEP operation requires that indices are valid for the type.
        // This is ensured by the caller who constructs valid indices based on type layout.
        unsafe {
            self.llbuilder
                .build_in_bounds_gep(ty, ptr, indices, name)
                .expect("gep")
        }
    }

    // -- Raw builder access for complex operations --

    /// Get the raw LLVM builder for complex operations.
    ///
    /// Use this sparingly - prefer the typed methods above.
    pub(crate) fn raw_builder(&self) -> &LLVMBuilder<'ll> {
        &self.llbuilder
    }

    // -- Mutable Variable Support --

    /// Create an alloca at function entry block.
    ///
    /// Placing allocas at the entry block is required for LLVM's `mem2reg` pass
    /// to optimize stack allocations back to SSA registers when possible.
    pub fn create_entry_alloca(
        &self,
        function: FunctionValue<'ll>,
        name: &str,
        ty: BasicTypeEnum<'ll>,
    ) -> PointerValue<'ll> {
        // Get the entry block
        let entry = function
            .get_first_basic_block()
            .expect("function has entry block");

        // Save current position
        let current_block = self.current_block();

        // Position at the start of entry block (after any existing allocas)
        // We position at the end of entry, then move to start if there's no terminator yet
        if let Some(first_instr) = entry.get_first_instruction() {
            self.llbuilder.position_before(&first_instr);
        } else {
            self.position_at_end(entry);
        }

        // Create the alloca
        let ptr = self.alloca(ty, name);

        // Restore position
        if let Some(block) = current_block {
            self.position_at_end(block);
        }

        ptr
    }

    /// Load a variable, handling both immutable (SSA) and mutable (alloca) storage.
    pub fn load_variable(&self, name: Name, locals: &Locals<'ll>) -> Option<BasicValueEnum<'ll>> {
        match locals.get_storage(&name)? {
            LocalStorage::Immutable(value) => Some(*value),
            LocalStorage::Mutable { ptr, ty } => {
                let name_str = self.cx().interner.lookup(name);
                Some(self.load(*ty, *ptr, name_str))
            }
        }
    }

    /// Store to a mutable variable.
    ///
    /// Returns `None` if the variable doesn't exist or is immutable.
    pub fn store_variable(
        &self,
        name: Name,
        value: BasicValueEnum<'ll>,
        locals: &Locals<'ll>,
    ) -> Option<()> {
        match locals.get_storage(&name)? {
            LocalStorage::Immutable(_) => {
                // Cannot assign to immutable variable - this is a type error
                // that should have been caught earlier
                None
            }
            LocalStorage::Mutable { ptr, ty: _ } => {
                self.store(value, *ptr);
                Some(())
            }
        }
    }

    // -- Not Implemented Trap --

    /// Emit a runtime trap for an unimplemented LLVM backend feature.
    ///
    /// Generates a call to `ori_panic_cstr` with a descriptive message,
    /// followed by an `unreachable` terminator. Returns `None` because
    /// the block is now terminated (same semantics as `panic`/`todo`).
    ///
    /// This replaces silent `None` returns that previously caused SIGSEGV
    /// when callers tried to use the missing value in phi nodes or returns.
    pub fn emit_not_implemented(&self, feature: &str) -> Option<BasicValueEnum<'ll>> {
        warn!(feature, "LLVM backend: not yet implemented");
        if let Some(panic_fn) = self.cx().llmod().get_function("ori_panic_cstr") {
            let msg = format!("LLVM backend: '{feature}' not yet implemented");
            let msg_ptr = self.build_global_string_ptr(&msg, "not_impl_msg");
            self.call(panic_fn, &[msg_ptr.into()], "not_impl_panic");
        }
        self.unreachable();
        None
    }

    // -- Expression Compilation --

    /// Compile an expression, dispatching to the appropriate helper method.
    ///
    /// This is the main entry point for expression compilation in the LLVM backend.
    ///
    /// # Parameters
    /// - `id`: The expression to compile
    /// - `arena`: The expression arena containing all AST nodes
    /// - `expr_types`: Type of each expression (indexed by `ExprId`)
    /// - `locals`: Local variable bindings
    /// - `function`: The LLVM function being compiled
    /// - `loop_ctx`: Current loop context for break/continue
    ///
    /// # Design Note
    /// The many parameters follow codegen conventions (see `rustc_codegen_llvm`).
    /// Using a `CompileCtx` struct was considered but rejected because:
    /// - The crate already allows `clippy::too_many_arguments` at crate level
    /// - The parameters are explicit about what each function needs
    /// - Lifetime handling is simpler without reborrowing gymnastics
    #[expect(
        clippy::too_many_lines,
        reason = "large match on ExprKind - splitting would obscure the dispatch logic"
    )]
    #[instrument(
        skip(self, arena, expr_types, locals, function, loop_ctx),
        level = "trace"
    )]
    pub fn compile_expr(
        &self,
        id: ExprId,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        let expr = arena.get_expr(id);
        let type_id = expr_types.get(id.index()).copied().unwrap_or(Idx::NONE);

        match &expr.kind {
            // Literals
            ExprKind::Int(n) => Some(self.cx().scx.type_i64().const_int(*n as u64, true).into()),

            ExprKind::Float(bits) => Some(
                self.cx()
                    .scx
                    .type_f64()
                    .const_float(f64::from_bits(*bits))
                    .into(),
            ),

            ExprKind::Bool(b) => Some(
                self.cx()
                    .scx
                    .type_i1()
                    .const_int(u64::from(*b), false)
                    .into(),
            ),

            ExprKind::Char(c) => Some(
                self.cx()
                    .scx
                    .type_i32()
                    .const_int(u64::from(*c), false)
                    .into(),
            ),

            // String literal
            ExprKind::String(name) | ExprKind::TemplateFull(name) => self.compile_string(*name),

            // Variables or unit variant constructors
            ExprKind::Ident(name) => {
                // Check if this identifier is a unit variant constructor
                if let Some((type_name, variant)) = self.cx().lookup_variant_constructor(*name) {
                    if variant.field_types.is_empty() {
                        return self.compile_unit_variant(type_name, &variant, function);
                    }
                }
                self.load_variable(*name, locals)
            }

            // Binary operations
            ExprKind::Binary { op, left, right } => {
                // Short-circuit evaluation for logical and coalescing operators
                match op {
                    BinaryOp::And => {
                        return self.compile_short_circuit_and(
                            *left, *right, arena, expr_types, locals, function, loop_ctx,
                        );
                    }
                    BinaryOp::Or => {
                        return self.compile_short_circuit_or(
                            *left, *right, arena, expr_types, locals, function, loop_ctx,
                        );
                    }
                    BinaryOp::Coalesce => {
                        return self.compile_short_circuit_coalesce(
                            *left, *right, type_id, arena, expr_types, locals, function, loop_ctx,
                        );
                    }
                    _ => {}
                }

                // Non-short-circuit operators: evaluate both sides
                let lhs =
                    self.compile_expr(*left, arena, expr_types, locals, function, loop_ctx)?;
                let rhs =
                    self.compile_expr(*right, arena, expr_types, locals, function, loop_ctx)?;
                // Pass the left operand's type to help distinguish struct types
                let left_type = expr_types.get(left.index()).copied().unwrap_or(Idx::NONE);
                self.compile_binary_op(*op, lhs, rhs, left_type)
            }

            // Unary operations
            ExprKind::Unary { op, operand } => {
                let val =
                    self.compile_expr(*operand, arena, expr_types, locals, function, loop_ctx)?;
                self.compile_unary_op(*op, val, type_id)
            }

            // Let binding
            ExprKind::Let {
                pattern,
                init,
                mutable,
                ..
            } => {
                let pattern = arena.get_binding_pattern(*pattern);
                self.compile_let(
                    pattern, *init, *mutable, arena, expr_types, locals, function, loop_ctx,
                )
            }

            // If/else expression
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.compile_if(
                *cond,
                *then_branch,
                *else_branch,
                type_id,
                arena,
                expr_types,
                locals,
                function,
                loop_ctx,
            ),

            // Loop
            ExprKind::Loop { body } => {
                self.compile_loop(*body, type_id, arena, expr_types, locals, function)
            }

            // Break
            ExprKind::Break(value) => {
                self.compile_break(*value, arena, expr_types, locals, function, loop_ctx)
            }

            // Continue
            ExprKind::Continue(value) => {
                self.compile_continue(*value, arena, expr_types, locals, function, loop_ctx)
            }

            // Tuple
            ExprKind::Tuple(range) => {
                self.compile_tuple(*range, arena, expr_types, locals, function, loop_ctx)
            }

            // Struct literal
            ExprKind::Struct { name, fields } => self.compile_struct(
                *name, *fields, arena, expr_types, locals, function, loop_ctx,
            ),

            // Struct literal with spread (not yet implemented in LLVM backend)
            ExprKind::StructWithSpread { .. } => self.emit_not_implemented("struct spread syntax"),

            // Field access
            ExprKind::Field { receiver, field } => self.compile_field_access(
                *receiver, *field, arena, expr_types, locals, function, loop_ctx,
            ),

            // Option type constructors
            ExprKind::Some(inner) => self.compile_some(
                *inner, type_id, arena, expr_types, locals, function, loop_ctx,
            ),

            ExprKind::None => self.compile_none(type_id),

            // Result type constructors
            ExprKind::Ok(inner) => self.compile_ok(
                *inner, type_id, arena, expr_types, locals, function, loop_ctx,
            ),

            ExprKind::Err(inner) => self.compile_err(
                *inner, type_id, arena, expr_types, locals, function, loop_ctx,
            ),

            // Match expression
            ExprKind::Match { scrutinee, arms } => self.compile_match(
                *scrutinee, *arms, type_id, arena, expr_types, locals, function, loop_ctx,
            ),

            // Function call (positional args) or data variant constructor
            ExprKind::Call { func, args } => {
                // Check if the callee is a data variant constructor
                if let ExprKind::Ident(func_name) = &arena.get_expr(*func).kind {
                    if let Some((type_name, variant)) =
                        self.cx().lookup_variant_constructor(*func_name)
                    {
                        if !variant.field_types.is_empty() {
                            return self.compile_data_variant_positional(
                                type_name, &variant, *args, arena, expr_types, locals, function,
                                loop_ctx,
                            );
                        }
                    }
                }
                self.compile_call(*func, *args, arena, expr_types, locals, function, loop_ctx)
            }

            // Function call (named args) or data variant constructor
            ExprKind::CallNamed { func, args } => {
                // Check if the callee is a data variant constructor
                if let ExprKind::Ident(func_name) = &arena.get_expr(*func).kind {
                    if let Some((type_name, variant)) =
                        self.cx().lookup_variant_constructor(*func_name)
                    {
                        if !variant.field_types.is_empty() {
                            return self.compile_data_variant(
                                type_name, &variant, *args, arena, expr_types, locals, function,
                                loop_ctx,
                            );
                        }
                    }
                }
                self.compile_call_named(*func, *args, arena, expr_types, locals, function, loop_ctx)
            }

            // Template literal with interpolation (requires runtime string concatenation)
            ExprKind::TemplateLiteral { .. } => {
                tracing::warn!(
                    feature = "template literal interpolation",
                    "LLVM backend: not yet implemented"
                );
                None
            }

            // Unit produces no value; Error is a placeholder that shouldn't be reached
            ExprKind::Unit | ExprKind::Error => None,

            // Constant (compile-time constant)
            ExprKind::Const(name) => self.compile_const(*name, locals),

            // Self reference (for recursion)
            ExprKind::SelfRef => {
                // Return pointer to current function
                Some(function.as_global_value().as_pointer_value().into())
            }

            // Function reference: @name
            ExprKind::FunctionRef(name) => self.compile_function_ref(*name),

            // Hash length: # (refers to length in index context)
            ExprKind::HashLength => self.emit_not_implemented("hash length (#)"),

            // Duration literal: 100ms, 5s
            ExprKind::Duration { value, unit } => self.compile_duration(*value, *unit),

            // Size literal: 4kb, 10mb
            ExprKind::Size { value, unit } => self.compile_size(*value, *unit),

            // Block: { stmts; result }
            ExprKind::Block { stmts, result } => self.compile_block(
                *stmts, *result, arena, expr_types, locals, function, loop_ctx,
            ),

            // Assignment: target = value
            ExprKind::Assign { target, value } => self.compile_assign(
                *target, *value, arena, expr_types, locals, function, loop_ctx,
            ),

            // List literal: [a, b, c]
            ExprKind::List(range) => {
                self.compile_list(*range, arena, expr_types, locals, function, loop_ctx)
            }

            // Map literal: {k: v, ...}
            ExprKind::Map(entries) => {
                self.compile_map(*entries, arena, expr_types, locals, function, loop_ctx)
            }

            // Range: start..end or start..end by step
            ExprKind::Range {
                start,
                end,
                step: _,
                inclusive,
            } => self.compile_range(
                *start, *end, *inclusive, arena, expr_types, locals, function, loop_ctx,
            ),

            // Index access: receiver[index]
            ExprKind::Index { receiver, index } => self.compile_index(
                *receiver, *index, arena, expr_types, locals, function, loop_ctx,
            ),

            // Method call: receiver.method(args)
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => self.compile_method_call(
                *receiver, *method, *args, arena, expr_types, locals, function, loop_ctx,
            ),

            // Method call with named args
            ExprKind::MethodCallNamed {
                receiver,
                method,
                args,
            } => self.compile_method_call_named(
                *receiver, *method, *args, arena, expr_types, locals, function, loop_ctx,
            ),

            // Lambda: params -> body
            ExprKind::Lambda {
                params,
                ret_ty: _,
                body,
            } => self.compile_lambda(*params, *body, arena, expr_types, locals, function),

            // For loop: for x in iter do/yield body
            ExprKind::For {
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => self.compile_for(
                *binding, *iter, *guard, *body, *is_yield, type_id, arena, expr_types, locals,
                function,
            ),

            // Await (no-op for sync runtime)
            ExprKind::Await(inner) => {
                // Just compile the inner expression - no async support yet
                self.compile_expr(*inner, arena, expr_types, locals, function, loop_ctx)
            }

            // Try expression: expr?
            ExprKind::Try(inner) => {
                self.compile_try(*inner, arena, expr_types, locals, function, loop_ctx)
            }

            // With capability provision
            ExprKind::WithCapability {
                capability: _,
                provider: _,
                body,
            } => {
                // For now, just compile the body (capability system not yet implemented)
                self.compile_expr(*body, arena, expr_types, locals, function, loop_ctx)
            }

            // Sequential expression patterns (run, try, match)
            ExprKind::FunctionSeq(seq_id) => {
                let seq = arena.get_function_seq(*seq_id);
                self.compile_function_seq(
                    seq, type_id, arena, expr_types, locals, function, loop_ctx,
                )
            }

            // Named expression patterns (recurse, parallel, etc.)
            ExprKind::FunctionExp(exp_id) => {
                let exp = arena.get_function_exp(*exp_id);
                self.compile_function_exp(
                    exp, type_id, arena, expr_types, locals, function, loop_ctx,
                )
            }

            // Type cast: expr as Type or expr as? Type
            ExprKind::Cast {
                expr: inner,
                fallible,
                ..
            } => {
                // For now, compile the inner expression and rely on type checker
                // to ensure cast validity. Full cast implementation requires
                // knowing source and target types to select appropriate LLVM casts.
                let val =
                    self.compile_expr(*inner, arena, expr_types, locals, function, loop_ctx)?;
                if *fallible {
                    // as? returns Option<T> - wrap value in Some
                    // Use standardized Option type with i64 payload
                    let opt_type = self.cx().option_type(self.cx().scx.type_i64().into());
                    let payload = self.coerce_to_i64(val)?;
                    let tag = self.cx().scx.type_i8().const_int(1, false); // 1 = Some
                    let struct_val =
                        self.build_struct(opt_type, &[tag.into(), payload.into()], "cast_some");
                    Some(struct_val.into())
                } else {
                    // as returns the converted value directly
                    Some(val)
                }
            }

            // List with spread: [...a, b, ...c]
            ExprKind::ListWithSpread(_elements) => self.emit_not_implemented("list spread syntax"),

            // Map with spread: {...a, k: v, ...b}
            ExprKind::MapWithSpread(_elements) => self.emit_not_implemented("map spread syntax"),
        }
    }

    // -- Sum Type Variant Construction --

    /// Compile a unit variant constructor (e.g., `Pending`).
    ///
    /// Produces `{ i8 tag, [M x i64] zeroed_payload }` for the sum type.
    fn compile_unit_variant(
        &self,
        type_name: Name,
        variant: &crate::context::SumVariantLayout,
        function: FunctionValue<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        let layout = self.cx().get_sum_type_layout(type_name)?;

        // Get the LLVM struct type for this sum type
        let struct_ty = self.cx().get_struct_type(type_name)?;

        // Alloca-based construction: alloca the struct, set tag, zero payload
        let alloca = self.create_entry_alloca(function, "variant", struct_ty.into());

        // Store tag (field 0)
        let tag_ptr = self.struct_gep(struct_ty, alloca, 0, "tag_ptr");
        let tag_val = self
            .cx()
            .scx
            .type_i8()
            .const_int(u64::from(variant.tag), false);
        self.store(tag_val.into(), tag_ptr);

        // Zero the payload (field 1) if it exists
        if layout.payload_i64_count > 0 {
            let payload_ptr = self.struct_gep(struct_ty, alloca, 1, "payload_ptr");
            let payload_ty = self
                .cx()
                .scx
                .type_i64()
                .array_type(layout.payload_i64_count);
            let zero = payload_ty.const_zero();
            self.store(zero.into(), payload_ptr);
        }

        // Load the complete struct
        let result = self.load(struct_ty.into(), alloca, "variant_val");
        Some(result)
    }

    /// Compile a data variant constructor (e.g., `Failed(reason: "oops")`).
    ///
    /// Produces `{ i8 tag, [M x i64] payload }` where payload fields are stored
    /// at byte offsets via GEP through the payload array pointer.
    fn compile_data_variant(
        &self,
        type_name: Name,
        variant: &crate::context::SumVariantLayout,
        args: ori_ir::ast::CallArgRange,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        let layout = self.cx().get_sum_type_layout(type_name)?;
        let struct_ty = self.cx().get_struct_type(type_name)?;

        // Compile argument values (named args: use value order)
        let call_args = arena.get_call_args(args);
        let mut arg_values = Vec::with_capacity(call_args.len());
        for arg in call_args {
            let val =
                self.compile_expr(arg.value, arena, expr_types, locals, function, loop_ctx)?;
            arg_values.push(val);
        }

        self.build_data_variant_from_values(
            type_name,
            variant,
            &layout,
            struct_ty,
            &arg_values,
            function,
        )
    }

    /// Compile a data variant constructor with positional args.
    ///
    /// Same as `compile_data_variant` but takes `ExprRange` (positional) instead
    /// of `CallArgRange` (named).
    fn compile_data_variant_positional(
        &self,
        type_name: Name,
        variant: &crate::context::SumVariantLayout,
        args: ori_ir::ExprRange,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        let layout = self.cx().get_sum_type_layout(type_name)?;
        let struct_ty = self.cx().get_struct_type(type_name)?;

        // Compile positional argument values
        let arg_exprs = arena.get_expr_list(args);
        let mut arg_values = Vec::with_capacity(arg_exprs.len());
        for &expr_id in arg_exprs {
            let val = self.compile_expr(expr_id, arena, expr_types, locals, function, loop_ctx)?;
            arg_values.push(val);
        }

        // Reuse the same alloca-based construction logic
        self.build_data_variant_from_values(
            type_name,
            variant,
            &layout,
            struct_ty,
            &arg_values,
            function,
        )
    }

    /// Shared implementation for data variant construction from compiled values.
    fn build_data_variant_from_values(
        &self,
        _type_name: Name,
        variant: &crate::context::SumVariantLayout,
        layout: &crate::context::SumTypeLayout,
        struct_ty: inkwell::types::StructType<'ll>,
        arg_values: &[BasicValueEnum<'ll>],
        function: FunctionValue<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        let alloca = self.create_entry_alloca(function, "variant", struct_ty.into());

        // Store tag (field 0)
        let tag_ptr = self.struct_gep(struct_ty, alloca, 0, "tag_ptr");
        let tag_val = self
            .cx()
            .scx
            .type_i8()
            .const_int(u64::from(variant.tag), false);
        self.store(tag_val.into(), tag_ptr);

        // Store payload fields at byte offsets
        if layout.payload_i64_count > 0 {
            let payload_ptr = self.struct_gep(struct_ty, alloca, 1, "payload_ptr");

            // Zero the payload first
            let payload_ty = self
                .cx()
                .scx
                .type_i64()
                .array_type(layout.payload_i64_count);
            let zero = payload_ty.const_zero();
            self.store(zero.into(), payload_ptr);

            // Store each field value through byte-addressed GEP
            let mut byte_offset: u32 = 0;
            for (i, val) in arg_values.iter().enumerate() {
                let field_ty = variant.field_types.get(i).copied().unwrap_or(Idx::INT);
                let field_size = crate::module::field_byte_size(field_ty);

                let i8_ty = self.cx().scx.type_i8();
                let offset = i8_ty.const_int(u64::from(byte_offset), false);
                let field_ptr = self.gep(
                    i8_ty.into(),
                    payload_ptr,
                    &[offset],
                    &format!("field_{i}_ptr"),
                );

                self.store(*val, field_ptr);

                byte_offset += field_size;
            }
        }

        let result = self.load(struct_ty.into(), alloca, "variant_val");
        Some(result)
    }

    /// Compile a let binding.
    ///
    /// For mutable bindings (`let mut x = ...`), creates stack allocation with
    /// `alloca`/`store` so the variable can be reassigned. For immutable bindings,
    /// uses direct SSA values which are more efficient but cannot be reassigned.
    #[instrument(
        skip(self, pattern, arena, expr_types, locals, function, loop_ctx),
        level = "debug"
    )]
    pub(crate) fn compile_let(
        &self,
        pattern: &BindingPattern,
        init: ExprId,
        mutable: bool,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile the initializer
        let value = self.compile_expr(init, arena, expr_types, locals, function, loop_ctx)?;
        let ty = value.get_type();

        // Bind the value based on the pattern (implementation in sequences.rs)
        self.bind_pattern(pattern, value, mutable, ty, function, locals);

        // Let bindings produce the bound value
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::context::Context;
    use ori_ir::StringInterner;

    #[test]
    fn test_builder_arithmetic() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");

        // Create a simple function to have a basic block
        let fn_type = cx.scx.type_i64().fn_type(&[], false);
        let function = cx.llmod().add_function("test_fn", fn_type, None);
        let entry = cx.llcx().append_basic_block(function, "entry");

        let bx = Builder::build(&cx, entry);

        let a = cx.scx.type_i64().const_int(5, false);
        let b = cx.scx.type_i64().const_int(3, false);

        let sum = bx.add(a, b, "sum");
        let diff = bx.sub(a, b, "diff");
        let prod = bx.mul(a, b, "prod");

        // Verify instructions were created
        assert!(sum.is_const());
        assert!(diff.is_const());
        assert!(prod.is_const());
    }

    #[test]
    fn test_builder_control_flow() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");

        let fn_type = cx.scx.type_i64().fn_type(&[], false);
        let function = cx.llmod().add_function("test_fn", fn_type, None);
        let entry = cx.llcx().append_basic_block(function, "entry");
        let then_bb = cx.llcx().append_basic_block(function, "then");
        let else_bb = cx.llcx().append_basic_block(function, "else");

        let bx = Builder::build(&cx, entry);

        let cond = cx.scx.type_i1().const_int(1, false);
        bx.cond_br(cond, then_bb, else_bb);

        // Verify branching instruction exists
        assert!(entry.get_terminator().is_some());
    }

    #[test]
    fn test_builder_struct_operations() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");

        let fn_type = cx.scx.type_void().fn_type(&[], false);
        let function = cx.llmod().add_function("test_fn", fn_type, None);
        let entry = cx.llcx().append_basic_block(function, "entry");

        let bx = Builder::build(&cx, entry);

        // Create a struct type
        let struct_ty = cx
            .scx
            .type_struct(&[cx.scx.type_i64().into(), cx.scx.type_i64().into()], false);

        // Build a struct value
        let val1 = cx.scx.type_i64().const_int(1, false).into();
        let val2 = cx.scx.type_i64().const_int(2, false).into();
        let struct_val = bx.build_struct(struct_ty, &[val1, val2], "pair");

        // Extract values
        let _extracted = bx.extract_value(struct_val, 0, "first");

        bx.ret_void();
    }
}
