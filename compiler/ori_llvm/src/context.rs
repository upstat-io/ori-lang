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
//!
//! # Arc vs Borrowed References
//!
//! This module uses **borrowed references** (`&'tcx T`) for shared data, not `Arc`.
//!
//! **Use `&'tcx T` (borrowed) when:**
//! - One owner exists and others borrow (codegen borrows from type-check phase)
//! - The data's lifetime is well-defined (created before, lives until after)
//! - Zero runtime cost is required (no atomic ref counting)
//!
//! **Use `Arc<T>` only when:**
//! - Ownership is truly shared with no clear single owner
//! - Data must outlive its creator (e.g., spawned tasks)
//! - Cross-thread sharing with independent lifetimes
//!
//! Codegen receives data from type-checking which outlives the codegen phase.
//! Borrowed references are correct, zero-cost, and Rust-idiomatic here.

use std::cell::RefCell;

use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicType, BasicTypeEnum, IntType, PointerType, StructType};
use inkwell::values::FunctionValue;
use inkwell::AddressSpace;
use rustc_hash::FxHashMap;

use ori_ir::{Name, StringInterner, TypeId};
use ori_types::{TypeData, TypeInterner};

/// Layout information for a user-defined struct type.
///
/// Tracks field names and their indices for field access code generation.
#[derive(Clone, Debug)]
pub struct StructLayout {
    /// Field names in declaration order (index = LLVM struct field index).
    pub fields: Vec<Name>,
    /// Map from field name to index for O(1) lookup.
    pub field_indices: FxHashMap<Name, u32>,
}

impl StructLayout {
    /// Create a new struct layout from field names.
    pub fn new(fields: Vec<Name>) -> Self {
        let field_indices = fields
            .iter()
            .enumerate()
            .map(|(i, &name)| (name, i as u32))
            .collect();
        Self {
            fields,
            field_indices,
        }
    }

    /// Get the field index for a given field name.
    pub fn field_index(&self, name: Name) -> Option<u32> {
        self.field_indices.get(&name).copied()
    }
}

/// Type cache for avoiding repeated LLVM type construction.
///
/// Two-level cache following Rust's pattern:
/// - `scalars`: Primitive types (fast path)
/// - `complex`: Compound types (structs, arrays, etc.)
#[derive(Default)]
pub struct TypeCache<'ll> {
    /// Cache for scalar types (int, float, bool, etc.)
    pub scalars: FxHashMap<TypeId, BasicTypeEnum<'ll>>,
    /// Cache for complex types (structs, arrays, etc.)
    pub complex: FxHashMap<TypeId, BasicTypeEnum<'ll>>,
    /// Named struct types for forward references.
    ///
    /// Uses interned `Name` as key for O(1) lookup without string hashing.
    pub named_structs: FxHashMap<Name, StructType<'ll>>,
    /// Struct field layouts for user-defined types.
    ///
    /// Maps type name to field layout for field access code generation.
    pub struct_layouts: FxHashMap<Name, StructLayout>,
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
/// - Type interner for resolving compound types
/// - Function instance cache
/// - Type cache for efficient type lookups
/// - Test function registry
pub struct CodegenCx<'ll, 'tcx> {
    /// The underlying simple context.
    pub scx: SimpleCx<'ll>,
    /// String interner for name lookup.
    pub interner: &'tcx StringInterner,
    /// Type interner for resolving `TypeId` to `TypeData`.
    ///
    /// Used by `compute_llvm_type` to determine LLVM representation
    /// for compound types (List, Map, Tuple, etc.).
    pub type_interner: Option<&'tcx TypeInterner>,
    /// Cache of compiled functions by name.
    pub instances: RefCell<FxHashMap<Name, FunctionValue<'ll>>>,
    /// Cache of compiled test functions.
    pub tests: RefCell<FxHashMap<Name, FunctionValue<'ll>>>,
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
            type_interner: None,
            instances: RefCell::new(FxHashMap::default()),
            tests: RefCell::new(FxHashMap::default()),
            type_cache: RefCell::new(TypeCache::new()),
        }
    }

    /// Create a codegen context with a type interner for compound type resolution.
    pub fn with_type_interner(
        context: &'ll Context,
        interner: &'tcx StringInterner,
        type_interner: &'tcx TypeInterner,
        module_name: &str,
    ) -> Self {
        let scx = SimpleCx::new(context, module_name);

        Self {
            scx,
            interner,
            type_interner: Some(type_interner),
            instances: RefCell::new(FxHashMap::default()),
            tests: RefCell::new(FxHashMap::default()),
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
    ///
    /// For compound types, uses the type interner to look up the `TypeData`
    /// and return the appropriate LLVM struct type.
    fn compute_llvm_type(&self, type_id: TypeId) -> BasicTypeEnum<'ll> {
        // Fast path: primitive types by TypeId constant
        match type_id {
            TypeId::FLOAT => return self.scx.type_f64().into(),
            TypeId::BOOL => return self.scx.type_i1().into(),
            TypeId::CHAR => return self.scx.type_i32().into(), // Unicode codepoint
            TypeId::STR => return self.string_type().into(),
            // BYTE and ORDERING are both i8 (Ordering: Less=0, Equal=1, Greater=2)
            TypeId::BYTE | TypeId::ORDERING => return self.scx.type_i8().into(),
            // INT, DURATION, SIZE, VOID, NEVER use i64
            TypeId::INT | TypeId::DURATION | TypeId::SIZE | TypeId::VOID | TypeId::NEVER => {
                return self.scx.type_i64().into();
            }
            _ => {}
        }

        // Use type interner for compound types
        if let Some(interner) = self.type_interner {
            let type_data = interner.lookup(type_id);
            match type_data {
                // Primitives and fallback types all use i64 representation.
                // Primitives: Int, Duration, Size, Unit, Never are intentionally i64.
                // Fallback: Var, Projection, ModuleNamespace, Error shouldn't appear at codegen
                // but we fall back to i64 if they do (shouldn't happen in well-typed code).
                TypeData::Int
                | TypeData::Duration
                | TypeData::Size
                | TypeData::Unit
                | TypeData::Never
                | TypeData::Var(_)
                | TypeData::Projection { .. }
                | TypeData::ModuleNamespace { .. }
                | TypeData::Error => self.scx.type_i64().into(),
                TypeData::Float => self.scx.type_f64().into(),
                TypeData::Bool => self.scx.type_i1().into(),
                TypeData::Str => self.string_type().into(),
                TypeData::Char => self.scx.type_i32().into(),
                TypeData::Byte | TypeData::Ordering => self.scx.type_i8().into(),

                // Compound types (Set uses same layout as List)
                TypeData::List(_) | TypeData::Set(_) => self.list_type().into(),
                TypeData::Map { .. } => self.map_type().into(),
                TypeData::Range(_) => self.range_type().into(),
                // Channel and Function are handles (pointers)
                TypeData::Channel(_) | TypeData::Function { .. } => self.scx.type_ptr().into(),

                // Option and Result need payload type
                TypeData::Option(inner) => {
                    let payload = self.llvm_type(inner);
                    self.option_type(payload).into()
                }
                TypeData::Result { ok, err: _ } => {
                    // For Result, use the larger of ok/err as payload
                    // For simplicity, use ok type (error handling TBD)
                    let payload = self.llvm_type(ok);
                    self.result_type(payload).into()
                }

                // Tuple: struct of element types
                TypeData::Tuple(elements) => {
                    let field_types: Vec<BasicTypeEnum<'ll>> =
                        elements.iter().map(|&id| self.llvm_type(id)).collect();
                    self.scx.type_struct(&field_types, false).into()
                }

                // Named types: look up struct or fall back to i64
                TypeData::Named(name) => {
                    if let Some(struct_ty) = self.get_struct_type(name) {
                        struct_ty.into()
                    } else {
                        self.scx.type_i64().into()
                    }
                }
                // Applied (generic) types: compute layout from type arguments
                // Don't use named structs because Container<int> and Container<str>
                // have different layouts despite sharing the same name.
                TypeData::Applied { name, ref args } => {
                    // For now, create an anonymous struct with the resolved arg types
                    // This is a simplified approach - full support needs field definitions
                    if args.is_empty() {
                        // No type args = non-generic, can use named struct
                        if let Some(struct_ty) = self.get_struct_type(name) {
                            struct_ty.into()
                        } else {
                            self.scx.type_i64().into()
                        }
                    } else {
                        // Generic instantiation: create anonymous struct from type args
                        // This assumes a simple pattern where type args map to fields
                        let field_types: Vec<BasicTypeEnum<'ll>> =
                            args.iter().map(|&arg| self.llvm_type(arg)).collect();
                        self.scx.type_struct(&field_types, false).into()
                    }
                }
            }
        } else {
            // No type interner available: fall back to i64 for unknown types
            self.scx.type_i64().into()
        }
    }

    /// Get the Ordering type (i8).
    ///
    /// Ordering is represented as i8: Less=0, Equal=1, Greater=2.
    #[inline]
    pub fn ordering_type(&self) -> inkwell::types::IntType<'ll> {
        self.scx.type_i8()
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

    /// Check if a `TypeId` represents an Option type.
    ///
    /// Returns true if the type is `Option<T>`, false otherwise.
    pub fn is_option_type(&self, type_id: TypeId) -> bool {
        use ori_types::TypeData;

        if let Some(interner) = self.type_interner {
            matches!(interner.lookup(type_id), TypeData::Option(_))
        } else {
            false
        }
    }

    /// Check if a `TypeId` represents a Result type.
    ///
    /// Returns true if the type is `Result<T, E>`, false otherwise.
    /// This is needed for coalesce (`??`) where Option and Result have
    /// different tag semantics (Option: tag=1 for Some, Result: tag=0 for Ok).
    pub fn is_result_type(&self, type_id: TypeId) -> bool {
        use ori_types::TypeData;

        if let Some(interner) = self.type_interner {
            matches!(interner.lookup(type_id), TypeData::Result { .. })
        } else {
            false
        }
    }

    /// Check if a `TypeId` represents a coercible wrapper type (Option or Result).
    ///
    /// These types support the `??` coalesce operator.
    pub fn is_wrapper_type(&self, type_id: TypeId) -> bool {
        self.is_option_type(type_id) || self.is_result_type(type_id)
    }

    /// Get the inner type of an Option<T>.
    ///
    /// Returns `Some(T)` if the type is `Option<T>`, `None` otherwise.
    pub fn option_inner_type(&self, type_id: TypeId) -> Option<TypeId> {
        use ori_types::TypeData;

        if let Some(interner) = self.type_interner {
            if let TypeData::Option(inner) = interner.lookup(type_id) {
                return Some(inner);
            }
        }
        None
    }

    /// Get the Ok type of a Result<T, E>.
    ///
    /// Returns `Some(T)` if the type is `Result<T, E>`, `None` otherwise.
    pub fn result_ok_type(&self, type_id: TypeId) -> Option<TypeId> {
        use ori_types::TypeData;

        if let Some(interner) = self.type_interner {
            if let TypeData::Result { ok, .. } = interner.lookup(type_id) {
                return Some(ok);
            }
        }
        None
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

    // -- Struct layout management --

    /// Register a user-defined struct type with its field layout.
    ///
    /// This creates an LLVM named struct type and records the field names
    /// for later field access code generation.
    pub fn register_struct(
        &self,
        name: Name,
        field_names: Vec<Name>,
        field_types: &[BasicTypeEnum<'ll>],
    ) {
        // Create or get the named struct type
        let struct_ty = self.get_or_create_named_struct(name);

        // Set the struct body with field types
        self.scx.set_struct_body(struct_ty, field_types, false);

        // Record the field layout for field access
        let layout = StructLayout::new(field_names);
        self.type_cache
            .borrow_mut()
            .struct_layouts
            .insert(name, layout);
    }

    /// Look up a registered struct type by name.
    pub fn get_struct_type(&self, name: Name) -> Option<StructType<'ll>> {
        self.type_cache.borrow().named_structs.get(&name).copied()
    }

    /// Look up the field index for a struct field.
    pub fn get_field_index(&self, struct_name: Name, field_name: Name) -> Option<u32> {
        self.type_cache
            .borrow()
            .struct_layouts
            .get(&struct_name)
            .and_then(|layout| layout.field_index(field_name))
    }

    /// Get the struct layout for a type.
    pub fn get_struct_layout(&self, name: Name) -> Option<StructLayout> {
        self.type_cache.borrow().struct_layouts.get(&name).cloned()
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
    pub fn all_tests(&self) -> FxHashMap<Name, FunctionValue<'ll>> {
        self.tests.borrow().clone()
    }

    // -- Default values --

    /// Get a default value for a type.
    pub fn default_value(&self, type_id: TypeId) -> inkwell::values::BasicValueEnum<'ll> {
        use ori_ir::builtin_constants::ordering::unsigned as ord;
        match type_id {
            TypeId::INT => self.scx.type_i64().const_int(0, false).into(),
            TypeId::FLOAT => self.scx.type_f64().const_float(0.0).into(),
            TypeId::BOOL => self.scx.type_i1().const_int(0, false).into(),
            TypeId::CHAR => self.scx.type_i32().const_int(0, false).into(),
            TypeId::BYTE => self.scx.type_i8().const_int(0, false).into(),
            // Ordering defaults to Equal (as per spec)
            TypeId::ORDERING => self.scx.type_i8().const_int(ord::EQUAL, false).into(),
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
