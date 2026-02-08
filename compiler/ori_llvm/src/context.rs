//! LLVM Codegen Context
//!
//! Provides `SimpleCx`: a minimal LLVM context wrapping the LLVM module,
//! context reference, and commonly-used types. Used as the foundation
//! for the V2 codegen pipeline (`IrBuilder`, `FunctionCompiler`, etc.).
//!
//! # Architecture
//!
//! Following Rust's `rustc_codegen_llvm` pattern, `SimpleCx` is a thin
//! wrapper around LLVM's `Context` + `Module`. Type computation is handled
//! by `TypeInfoStore` + `TypeLayoutResolver` in `codegen/type_info.rs`.

use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicType, BasicTypeEnum, PointerType, StructType};
use inkwell::AddressSpace;

/// Minimal LLVM context with the module and commonly-used types.
///
/// Contains only the LLVM module, context, and commonly-used types.
/// Used by `IrBuilder` and `FunctionCompiler` for all code generation.
pub struct SimpleCx<'ll> {
    /// The LLVM context (owns all LLVM types and values).
    pub llcx: &'ll Context,
    /// The LLVM module being compiled.
    pub llmod: Module<'ll>,
    /// Commonly used pointer type (opaque pointer).
    pub ptr_type: PointerType<'ll>,
    /// Machine word size type (i64 on 64-bit).
    pub isize_ty: inkwell::types::IntType<'ll>,
}

impl<'ll> SimpleCx<'ll> {
    /// Create a new simple context.
    #[must_use]
    pub fn new(context: &'ll Context, module_name: &str) -> Self {
        let llmod = context.create_module(module_name);
        let ptr_type = context.ptr_type(AddressSpace::default());
        let isize_ty = context.i64_type(); // 64-bit target

        Self {
            llcx: context,
            llmod,
            ptr_type,
            isize_ty,
        }
    }

    /// Consume this context and return the LLVM module.
    ///
    /// Use this when the compilation pipeline is done and you need the module
    /// for JIT execution or AOT output.
    pub fn into_module(self) -> Module<'ll> {
        self.llmod
    }

    // -- Type constructors --

    /// Get the i1 (boolean) type.
    #[inline]
    pub fn type_i1(&self) -> inkwell::types::IntType<'ll> {
        self.llcx.bool_type()
    }

    /// Get the i8 type.
    #[inline]
    pub fn type_i8(&self) -> inkwell::types::IntType<'ll> {
        self.llcx.i8_type()
    }

    /// Get the i32 type.
    #[inline]
    pub fn type_i32(&self) -> inkwell::types::IntType<'ll> {
        self.llcx.i32_type()
    }

    /// Get the i64 type.
    #[inline]
    pub fn type_i64(&self) -> inkwell::types::IntType<'ll> {
        self.llcx.i64_type()
    }

    /// Get the f64 type.
    #[inline]
    pub fn type_f64(&self) -> inkwell::types::FloatType<'ll> {
        self.llcx.f64_type()
    }

    /// Get the void type.
    #[inline]
    pub fn type_void(&self) -> inkwell::types::VoidType<'ll> {
        self.llcx.void_type()
    }

    /// Get the pointer type.
    #[inline]
    pub fn type_ptr(&self) -> PointerType<'ll> {
        self.ptr_type
    }

    /// Create a struct type from fields.
    pub fn type_struct(&self, fields: &[BasicTypeEnum<'ll>], packed: bool) -> StructType<'ll> {
        self.llcx.struct_type(fields, packed)
    }

    /// Create a named struct type (for forward references).
    pub fn type_named_struct(&self, name: &str) -> StructType<'ll> {
        self.llcx.opaque_struct_type(name)
    }

    /// Set the body of a named struct type.
    pub fn set_struct_body(
        &self,
        ty: StructType<'ll>,
        fields: &[BasicTypeEnum<'ll>],
        packed: bool,
    ) {
        ty.set_body(fields, packed);
    }

    /// Create a function type.
    pub fn type_func(
        &self,
        args: &[inkwell::types::BasicMetadataTypeEnum<'ll>],
        ret: inkwell::types::BasicTypeEnum<'ll>,
    ) -> inkwell::types::FunctionType<'ll> {
        ret.fn_type(args, false)
    }

    /// Create a void function type.
    pub fn type_void_func(
        &self,
        args: &[inkwell::types::BasicMetadataTypeEnum<'ll>],
    ) -> inkwell::types::FunctionType<'ll> {
        self.type_void().fn_type(args, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_cx_types() {
        let context = Context::create();
        let scx = SimpleCx::new(&context, "test");

        assert_eq!(scx.type_i64().get_bit_width(), 64);
        assert_eq!(scx.type_i32().get_bit_width(), 32);
        assert_eq!(scx.type_i8().get_bit_width(), 8);
        assert_eq!(scx.type_i1().get_bit_width(), 1);
    }
}
