//! LLVM Codegen Context Hierarchy
//!
//! Follows Rust's `rustc_codegen_llvm` pattern:
//! - `SimpleCx`: Minimal LLVM context (module, context, basic types)
//! - `CodegenCx`: Full context with Ori-specific state (interner, caches)
//!
//! This separation allows:
//! - Type building code to work with minimal context
//! - Full codegen to have access to caches and Ori types
//! - Future extension for parallel codegen (one context per unit)

use std::cell::RefCell;
use std::collections::HashMap;

use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicType, BasicTypeEnum, IntType, PointerType, StructType};
use inkwell::values::FunctionValue;
use inkwell::AddressSpace;

use ori_ir::{Name, StringInterner, TypeId};

/// Type cache for avoiding repeated LLVM type construction.
///
/// Two-level cache following Rust's pattern:
/// - `scalars`: Primitive types (fast path)
/// - `complex`: Compound types (structs, arrays, etc.)
#[derive(Default)]
pub struct TypeCache<'ll> {
    /// Cache for scalar types (int, float, bool, etc.)
    pub scalars: HashMap<TypeId, BasicTypeEnum<'ll>>,
    /// Cache for complex types (structs, arrays, etc.)
    pub complex: HashMap<TypeId, BasicTypeEnum<'ll>>,
    /// Named struct types for forward references.
    ///
    /// Uses interned `Name` as key for O(1) lookup without string hashing.
    pub named_structs: HashMap<Name, StructType<'ll>>,
}

impl<'ll> TypeCache<'ll> {
    /// Create a new empty type cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a cached type.
    #[must_use]
    pub fn get(&self, type_id: TypeId) -> Option<BasicTypeEnum<'ll>> {
        self.scalars
            .get(&type_id)
            .or_else(|| self.complex.get(&type_id))
            .copied()
    }

    /// Cache a scalar type.
    pub fn cache_scalar(&mut self, type_id: TypeId, ty: BasicTypeEnum<'ll>) {
        self.scalars.insert(type_id, ty);
    }

    /// Cache a complex type.
    pub fn cache_complex(&mut self, type_id: TypeId, ty: BasicTypeEnum<'ll>) {
        self.complex.insert(type_id, ty);
    }

    /// Get or create a named struct type for forward references.
    ///
    /// Takes both the interned `Name` (for caching) and the string representation
    /// (for LLVM's `opaque_struct_type` call). Call from `CodegenCx` which has
    /// access to the interner.
    pub fn get_or_create_named_struct(
        &mut self,
        context: &'ll Context,
        name: Name,
        name_str: &str,
    ) -> StructType<'ll> {
        if let Some(&ty) = self.named_structs.get(&name) {
            ty
        } else {
            let ty = context.opaque_struct_type(name_str);
            self.named_structs.insert(name, ty);
            ty
        }
    }
}

/// Simple LLVM context with minimal state.
///
/// Contains only the LLVM module, context, and commonly-used types.
/// Used as the base for `CodegenCx` and can be used independently
/// for operations that don't need full Ori context.
pub struct SimpleCx<'ll> {
    /// The LLVM context (owns all LLVM types and values).
    pub llcx: &'ll Context,
    /// The LLVM module being compiled.
    pub llmod: Module<'ll>,
    /// Commonly used pointer type (opaque pointer).
    pub ptr_type: PointerType<'ll>,
    /// Machine word size type (i64 on 64-bit).
    pub isize_ty: IntType<'ll>,
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

    // -- Type constructors (available on minimal context) --

    /// Get the i1 (boolean) type.
    #[inline]
    pub fn type_i1(&self) -> IntType<'ll> {
        self.llcx.bool_type()
    }

    /// Get the i8 type.
    #[inline]
    pub fn type_i8(&self) -> IntType<'ll> {
        self.llcx.i8_type()
    }

    /// Get the i32 type.
    #[inline]
    pub fn type_i32(&self) -> IntType<'ll> {
        self.llcx.i32_type()
    }

    /// Get the i64 type.
    #[inline]
    pub fn type_i64(&self) -> IntType<'ll> {
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

/// Full codegen context with Ori-specific state.
///
/// Wraps `SimpleCx` and adds:
/// - String interner for name resolution
/// - Function instance cache
/// - Type cache for efficient type lookups
/// - Test function registry
pub struct CodegenCx<'ll, 'tcx> {
    /// The underlying simple context.
    pub scx: SimpleCx<'ll>,
    /// String interner for name lookup.
    pub interner: &'tcx StringInterner,
    /// Cache of compiled functions by name.
    pub instances: RefCell<HashMap<Name, FunctionValue<'ll>>>,
    /// Cache of compiled test functions.
    pub tests: RefCell<HashMap<Name, FunctionValue<'ll>>>,
    /// Type cache for efficient lookups.
    pub type_cache: RefCell<TypeCache<'ll>>,
}

impl<'ll, 'tcx> CodegenCx<'ll, 'tcx> {
    /// Create a new full codegen context.
    pub fn new(context: &'ll Context, interner: &'tcx StringInterner, module_name: &str) -> Self {
        let scx = SimpleCx::new(context, module_name);

        Self {
            scx,
            interner,
            instances: RefCell::new(HashMap::new()),
            tests: RefCell::new(HashMap::new()),
            type_cache: RefCell::new(TypeCache::new()),
        }
    }

    // -- Delegate to SimpleCx --

    /// Get the LLVM context.
    #[inline]
    pub fn llcx(&self) -> &'ll Context {
        self.scx.llcx
    }

    /// Get the LLVM module.
    #[inline]
    pub fn llmod(&self) -> &Module<'ll> {
        &self.scx.llmod
    }

    // -- Type methods (with caching) --

    /// Get the LLVM type for an Ori `TypeId`.
    ///
    /// Uses two-level cache: scalars first, then complex types.
    /// Fast path (cache hit): single `borrow()` check.
    /// Slow path (cache miss): compute + `borrow_mut()` to cache.
    pub fn llvm_type(&self, type_id: TypeId) -> BasicTypeEnum<'ll> {
        // Fast path: check cache with read-only borrow
        if let Some(ty) = self.type_cache.borrow().get(type_id) {
            return ty;
        }

        // Slow path: compute and cache
        let ty = self.compute_llvm_type(type_id);

        // Cache in appropriate level
        let mut cache = self.type_cache.borrow_mut();
        if type_id.is_primitive() {
            cache.cache_scalar(type_id, ty);
        } else {
            cache.cache_complex(type_id, ty);
        }

        ty
    }

    /// Compute the LLVM type for a `TypeId` (uncached).
    fn compute_llvm_type(&self, type_id: TypeId) -> BasicTypeEnum<'ll> {
        match type_id {
            TypeId::FLOAT => self.scx.type_f64().into(),
            TypeId::BOOL => self.scx.type_i1().into(),
            TypeId::CHAR => self.scx.type_i32().into(), // Unicode codepoint
            TypeId::BYTE => self.scx.type_i8().into(),
            TypeId::STR => self.string_type().into(),
            // INT, VOID, NEVER, and unknown types (including unresolved type variables)
            // default to i64. This handles generic functions where T is not yet resolved.
            // Using i64 as the fallback ensures type compatibility with extracted
            // values from Option/Result structs.
            _ => self.scx.type_i64().into(),
        }
    }

    /// Get the string type: { i64 len, ptr data }
    pub fn string_type(&self) -> StructType<'ll> {
        self.scx.type_struct(
            &[self.scx.type_i64().into(), self.scx.type_ptr().into()],
            false,
        )
    }

    /// Create an Option type: { i8 tag, T payload }
    /// tag = 0: None, tag = 1: Some
    pub fn option_type(&self, payload: BasicTypeEnum<'ll>) -> StructType<'ll> {
        self.scx
            .type_struct(&[self.scx.type_i8().into(), payload], false)
    }

    /// Create a Result type: { i8 tag, T payload }
    /// tag = 0: Ok, tag = 1: Err
    pub fn result_type(&self, payload: BasicTypeEnum<'ll>) -> StructType<'ll> {
        self.scx
            .type_struct(&[self.scx.type_i8().into(), payload], false)
    }

    /// Get the list type: { i64 len, i64 cap, ptr data }
    pub fn list_type(&self) -> StructType<'ll> {
        self.scx.type_struct(
            &[
                self.scx.type_i64().into(),
                self.scx.type_i64().into(),
                self.scx.type_ptr().into(),
            ],
            false,
        )
    }

    /// Get the map type: { i64 len, i64 cap, ptr keys, ptr vals }
    pub fn map_type(&self) -> StructType<'ll> {
        self.scx.type_struct(
            &[
                self.scx.type_i64().into(),
                self.scx.type_i64().into(),
                self.scx.type_ptr().into(),
                self.scx.type_ptr().into(),
            ],
            false,
        )
    }

    /// Get the range type: { i64 start, i64 end, i1 inclusive }
    pub fn range_type(&self) -> StructType<'ll> {
        self.scx.type_struct(
            &[
                self.scx.type_i64().into(),
                self.scx.type_i64().into(),
                self.scx.type_i1().into(),
            ],
            false,
        )
    }

    /// Get or create a named struct type for forward references.
    ///
    /// Uses interned `Name` for O(1) cache lookup without string hashing.
    pub fn get_or_create_named_struct(&self, name: Name) -> StructType<'ll> {
        let name_str = self.interner.lookup(name);
        self.type_cache
            .borrow_mut()
            .get_or_create_named_struct(self.llcx(), name, name_str)
    }

    // -- Function instance management --

    /// Look up a function by name (checking cache first).
    pub fn get_function(&self, name: Name) -> Option<FunctionValue<'ll>> {
        // Check instance cache
        if let Some(func) = self.instances.borrow().get(&name).copied() {
            return Some(func);
        }

        // Fall back to module lookup
        let fn_name = self.interner.lookup(name);
        self.scx.llmod.get_function(fn_name)
    }

    /// Register a function in the instance cache.
    pub fn register_function(&self, name: Name, func: FunctionValue<'ll>) {
        self.instances.borrow_mut().insert(name, func);
    }

    /// Register a test function.
    pub fn register_test(&self, name: Name, func: FunctionValue<'ll>) {
        self.tests.borrow_mut().insert(name, func);
    }

    /// Get a test function by name.
    pub fn get_test(&self, name: Name) -> Option<FunctionValue<'ll>> {
        self.tests.borrow().get(&name).copied()
    }

    /// Get all registered test functions.
    ///
    /// Returns a cloned `HashMap` because the test registry is stored in a `RefCell`
    /// and we cannot return a reference with the borrow guard's lifetime.
    /// The clone is cheap since `FunctionValue` is a thin pointer wrapper.
    pub fn all_tests(&self) -> HashMap<Name, FunctionValue<'ll>> {
        self.tests.borrow().clone()
    }

    // -- Default values --

    /// Get a default value for a type.
    pub fn default_value(&self, type_id: TypeId) -> inkwell::values::BasicValueEnum<'ll> {
        match type_id {
            TypeId::INT => self.scx.type_i64().const_int(0, false).into(),
            TypeId::FLOAT => self.scx.type_f64().const_float(0.0).into(),
            TypeId::BOOL => self.scx.type_i1().const_int(0, false).into(),
            TypeId::CHAR => self.scx.type_i32().const_int(0, false).into(),
            TypeId::BYTE => self.scx.type_i8().const_int(0, false).into(),
            _ => self.scx.type_ptr().const_null().into(),
        }
    }

    /// Get a default value for an LLVM type.
    pub fn default_value_for_type(
        &self,
        ty: BasicTypeEnum<'ll>,
    ) -> inkwell::values::BasicValueEnum<'ll> {
        match ty {
            BasicTypeEnum::IntType(t) => t.const_int(0, false).into(),
            BasicTypeEnum::FloatType(t) => t.const_float(0.0).into(),
            BasicTypeEnum::PointerType(t) => t.const_null().into(),
            BasicTypeEnum::StructType(t) => t.get_undef().into(),
            BasicTypeEnum::ArrayType(t) => t.get_undef().into(),
            BasicTypeEnum::VectorType(t) => t.get_undef().into(),
            BasicTypeEnum::ScalableVectorType(t) => t.get_undef().into(),
        }
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

    #[test]
    fn test_type_cache() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");

        // First lookup should compute
        let int_ty = cx.llvm_type(TypeId::INT);

        // Second lookup should hit cache
        let int_ty2 = cx.llvm_type(TypeId::INT);

        // Should be the same type
        assert_eq!(int_ty, int_ty2);
    }

    #[test]
    fn test_struct_types() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");

        let str_ty = cx.string_type();
        assert_eq!(str_ty.count_fields(), 2);

        let list_ty = cx.list_type();
        assert_eq!(list_ty.count_fields(), 3);

        let map_ty = cx.map_type();
        assert_eq!(map_ty.count_fields(), 4);
    }
}
