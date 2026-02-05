//! Backend Traits for Codegen Abstraction
//!
//! Follows Rust's `rustc_codegen_ssa/src/traits/` pattern.
//!
//! These traits define the interface for code generation backends.
//! Currently only LLVM is implemented, but this abstraction allows
//! future backends (Cranelift, etc.) to implement the same interface.
//!
//! Trait hierarchy:
//! - `BackendTypes`: Associated types for backend IR (Value, Type, etc.)
//! - `TypeMethods`: Type construction and lookup
//! - `BuilderMethods`: Instruction generation
//! - `CodegenMethods`: High-level codegen operations

use inkwell::basic_block::BasicBlock;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValueEnum, FunctionValue};
use inkwell::IntPredicate;
use ori_types::Idx;

/// Associated types for a codegen backend.
///
/// Each backend defines its own IR types:
/// - Value: An SSA value (result of computation)
/// - Type: A type in the IR
/// - Function: A function definition
/// - `BasicBlock`: A control flow node
pub trait BackendTypes {
    type Value: Copy;
    type Type: Copy;
    type Function: Copy;
    type BasicBlock: Copy;
}

/// Type construction and lookup methods.
pub trait TypeMethods: BackendTypes {
    /// Get the LLVM type for an Ori `Idx`.
    fn llvm_type(&self, type_id: Idx) -> Self::Type;

    /// Get a default value for a type.
    fn default_value(&self, type_id: Idx) -> Self::Value;

    /// Check if a type is void.
    fn is_void_type(&self, type_id: Idx) -> bool {
        type_id == Idx::UNIT || type_id == Idx::NEVER
    }
}

/// Instruction builder methods.
///
/// This is the main interface for generating IR instructions.
/// Each method corresponds to an IR instruction or pattern.
pub trait BuilderMethods<'a>: BackendTypes {
    type CodegenCx;

    /// Create a new builder positioned at the given basic block.
    fn build(cx: &'a Self::CodegenCx, bb: Self::BasicBlock) -> Self;

    /// Get the codegen context.
    fn cx(&self) -> &'a Self::CodegenCx;

    /// Get the current basic block.
    fn current_block(&self) -> Option<Self::BasicBlock>;

    /// Position at the end of a basic block.
    fn position_at_end(&self, bb: Self::BasicBlock);

    // -- Terminators --

    /// Build a void return.
    fn ret_void(&self);

    /// Build a return with value.
    fn ret(&self, val: Self::Value);

    /// Build unconditional branch.
    fn br(&self, dest: Self::BasicBlock);

    /// Build conditional branch.
    fn cond_br(&self, cond: Self::Value, then_bb: Self::BasicBlock, else_bb: Self::BasicBlock);

    // -- Arithmetic --

    /// Integer addition.
    fn add(&self, lhs: Self::Value, rhs: Self::Value, name: &str) -> Self::Value;

    /// Integer subtraction.
    fn sub(&self, lhs: Self::Value, rhs: Self::Value, name: &str) -> Self::Value;

    /// Integer multiplication.
    fn mul(&self, lhs: Self::Value, rhs: Self::Value, name: &str) -> Self::Value;

    /// Signed integer division.
    fn sdiv(&self, lhs: Self::Value, rhs: Self::Value, name: &str) -> Self::Value;

    // -- Comparisons --

    /// Integer comparison.
    fn icmp(
        &self,
        pred: IntPredicate,
        lhs: Self::Value,
        rhs: Self::Value,
        name: &str,
    ) -> Self::Value;

    // -- Calls --

    /// Function call.
    fn call(&self, callee: Self::Function, args: &[Self::Value], name: &str)
        -> Option<Self::Value>;

    // -- Phi nodes --

    /// Create a phi node.
    fn phi(&self, ty: Self::Type, name: &str) -> Self::Value;
}

/// Codegen context methods.
///
/// High-level operations on the codegen context.
pub trait CodegenMethods<'tcx>: TypeMethods {
    /// Declare a function.
    fn declare_fn(&self, name: &str, param_types: &[Idx], return_type: Idx) -> Self::Function;

    /// Get a declared function by name.
    fn get_fn(&self, name: &str) -> Option<Self::Function>;

    /// Append a basic block to a function.
    fn append_block(&self, function: Self::Function, name: &str) -> Self::BasicBlock;
}

// -- LLVM Implementation --

/// LLVM backend types.
impl<'ll> BackendTypes for crate::context::CodegenCx<'ll, '_> {
    type Value = BasicValueEnum<'ll>;
    type Type = BasicTypeEnum<'ll>;
    type Function = FunctionValue<'ll>;
    type BasicBlock = BasicBlock<'ll>;
}

/// LLVM type methods.
impl TypeMethods for crate::context::CodegenCx<'_, '_> {
    fn llvm_type(&self, type_id: Idx) -> Self::Type {
        // Delegate to CodegenCx method
        crate::context::CodegenCx::llvm_type(self, type_id)
    }

    fn default_value(&self, type_id: Idx) -> Self::Value {
        crate::context::CodegenCx::default_value(self, type_id)
    }
}

/// LLVM builder methods.
impl<'ll> BackendTypes for crate::builder::Builder<'_, 'll, '_> {
    type Value = BasicValueEnum<'ll>;
    type Type = BasicTypeEnum<'ll>;
    type Function = FunctionValue<'ll>;
    type BasicBlock = BasicBlock<'ll>;
}

impl<'a, 'll, 'tcx> BuilderMethods<'a> for crate::builder::Builder<'a, 'll, 'tcx> {
    type CodegenCx = crate::context::CodegenCx<'ll, 'tcx>;

    fn build(cx: &'a Self::CodegenCx, bb: Self::BasicBlock) -> Self {
        crate::builder::Builder::build(cx, bb)
    }

    fn cx(&self) -> &'a Self::CodegenCx {
        crate::builder::Builder::cx(self)
    }

    fn current_block(&self) -> Option<Self::BasicBlock> {
        crate::builder::Builder::current_block(self)
    }

    fn position_at_end(&self, bb: Self::BasicBlock) {
        crate::builder::Builder::position_at_end(self, bb);
    }

    fn ret_void(&self) {
        crate::builder::Builder::ret_void(self);
    }

    fn ret(&self, val: Self::Value) {
        crate::builder::Builder::ret(self, val);
    }

    fn br(&self, dest: Self::BasicBlock) {
        crate::builder::Builder::br(self, dest);
    }

    fn cond_br(&self, cond: Self::Value, then_bb: Self::BasicBlock, else_bb: Self::BasicBlock) {
        let cond_int = cond.into_int_value();
        crate::builder::Builder::cond_br(self, cond_int, then_bb, else_bb);
    }

    fn add(&self, lhs: Self::Value, rhs: Self::Value, name: &str) -> Self::Value {
        crate::builder::Builder::add(self, lhs.into_int_value(), rhs.into_int_value(), name).into()
    }

    fn sub(&self, lhs: Self::Value, rhs: Self::Value, name: &str) -> Self::Value {
        crate::builder::Builder::sub(self, lhs.into_int_value(), rhs.into_int_value(), name).into()
    }

    fn mul(&self, lhs: Self::Value, rhs: Self::Value, name: &str) -> Self::Value {
        crate::builder::Builder::mul(self, lhs.into_int_value(), rhs.into_int_value(), name).into()
    }

    fn sdiv(&self, lhs: Self::Value, rhs: Self::Value, name: &str) -> Self::Value {
        crate::builder::Builder::sdiv(self, lhs.into_int_value(), rhs.into_int_value(), name).into()
    }

    fn icmp(
        &self,
        pred: IntPredicate,
        lhs: Self::Value,
        rhs: Self::Value,
        name: &str,
    ) -> Self::Value {
        crate::builder::Builder::icmp(self, pred, lhs.into_int_value(), rhs.into_int_value(), name)
            .into()
    }

    fn call(
        &self,
        callee: Self::Function,
        args: &[Self::Value],
        name: &str,
    ) -> Option<Self::Value> {
        crate::builder::Builder::call(self, callee, args, name)
    }

    fn phi(&self, ty: Self::Type, name: &str) -> Self::Value {
        crate::builder::Builder::phi(self, ty, name).as_basic_value()
    }
}

/// LLVM codegen methods.
impl<'tcx> CodegenMethods<'tcx> for crate::context::CodegenCx<'_, 'tcx> {
    fn declare_fn(&self, name: &str, param_types: &[Idx], return_type: Idx) -> Self::Function {
        let fn_name = self.interner.intern(name);
        crate::context::CodegenCx::declare_fn(self, fn_name, param_types, return_type)
    }

    fn get_fn(&self, name: &str) -> Option<Self::Function> {
        self.scx.llmod.get_function(name)
    }

    fn append_block(&self, function: Self::Function, name: &str) -> Self::BasicBlock {
        self.llcx().append_basic_block(function, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::context::Context;
    use ori_ir::StringInterner;

    #[test]
    fn test_backend_types() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = crate::context::CodegenCx::new(&context, &interner, "test");

        // Test TypeMethods
        let int_ty = TypeMethods::llvm_type(&cx, Idx::INT);
        assert!(matches!(int_ty, BasicTypeEnum::IntType(_)));

        let default = TypeMethods::default_value(&cx, Idx::INT);
        assert!(matches!(default, BasicValueEnum::IntValue(_)));
    }

    #[test]
    fn test_codegen_methods() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = crate::context::CodegenCx::new(&context, &interner, "test");

        // Test CodegenMethods
        let func = CodegenMethods::declare_fn(&cx, "test_fn", &[Idx::INT], Idx::INT);
        assert_eq!(func.get_name().to_str().unwrap(), "test_fn");

        let retrieved = CodegenMethods::get_fn(&cx, "test_fn");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), func);
    }

    #[test]
    fn test_builder_methods() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = crate::context::CodegenCx::new(&context, &interner, "test");

        let func = CodegenMethods::declare_fn(&cx, "test_fn", &[], Idx::INT);
        let entry = CodegenMethods::append_block(&cx, func, "entry");

        let bx = <crate::builder::Builder as BuilderMethods>::build(&cx, entry);

        // Test builder operations via trait
        let a: BasicValueEnum = cx.scx.type_i64().const_int(5, false).into();
        let b: BasicValueEnum = cx.scx.type_i64().const_int(3, false).into();

        let _sum = BuilderMethods::add(&bx, a, b, "sum");

        BuilderMethods::ret(&bx, a);
    }
}
