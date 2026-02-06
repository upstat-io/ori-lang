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

use ori_ir::{Name, StringInterner};
use ori_types::{Idx, PatternKey, PatternResolution, Pool, Tag};

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

/// Layout for a user-defined sum type (tagged union).
///
/// Sum types are represented as `{ i8 tag, [M x i64] payload }` where:
/// - `tag`: variant discriminant (0..n for n variants)
/// - `payload`: fixed-size array large enough for the largest variant's fields
///
/// Using `[M x i64]` (not `[N x i8]`) ensures 8-byte alignment. LLVM auto-pads
/// 7 bytes between the i8 tag and the i64 array, giving all stores natural alignment.
#[derive(Clone, Debug)]
pub struct SumTypeLayout {
    /// The sum type's name.
    pub type_name: Name,
    /// Variants in declaration order (index = tag value).
    pub variants: Vec<SumVariantLayout>,
    /// Payload size in i64 units: `ceil(max_payload_bytes / 8)`.
    ///
    /// For built-in sum types (Option/Result), this is 0 as a sentinel —
    /// their LLVM layout comes from Pool-based generation, not from this system.
    pub payload_i64_count: u32,
}

/// Layout for a single variant of a sum type.
#[derive(Clone, Debug)]
pub struct SumVariantLayout {
    /// Variant name (e.g., `Pending`, `Running`, `Done`).
    pub name: Name,
    /// Tag value (= index in parent's variants vec).
    pub tag: u8,
    /// Field type `Idx`s (empty for unit variants).
    pub field_types: Vec<Idx>,
}

/// Type cache for avoiding repeated LLVM type construction.
///
/// Two-level cache following Rust's pattern:
/// - `scalars`: Primitive types (fast path)
/// - `complex`: Compound types (structs, arrays, etc.)
#[derive(Default)]
pub struct TypeCache<'ll> {
    /// Cache for scalar types (int, float, bool, etc.)
    pub scalars: FxHashMap<Idx, BasicTypeEnum<'ll>>,
    /// Cache for complex types (structs, arrays, etc.)
    pub complex: FxHashMap<Idx, BasicTypeEnum<'ll>>,
    /// Named struct types for forward references.
    ///
    /// Uses interned `Name` as key for O(1) lookup without string hashing.
    pub named_structs: FxHashMap<Name, StructType<'ll>>,
    /// Struct field layouts for user-defined types.
    ///
    /// Maps type name to field layout for field access code generation.
    pub struct_layouts: FxHashMap<Name, StructLayout>,
    /// Sum type layouts for user-defined and built-in tagged unions.
    ///
    /// Maps type name to layout for tag/variant lookup.
    pub sum_types: FxHashMap<Name, SumTypeLayout>,
}

impl<'ll> TypeCache<'ll> {
    /// Create a new empty type cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a cached type.
    #[must_use]
    pub fn get(&self, idx: Idx) -> Option<BasicTypeEnum<'ll>> {
        self.scalars
            .get(&idx)
            .or_else(|| self.complex.get(&idx))
            .copied()
    }

    /// Cache a scalar type.
    pub fn cache_scalar(&mut self, idx: Idx, ty: BasicTypeEnum<'ll>) {
        self.scalars.insert(idx, ty);
    }

    /// Cache a complex type.
    pub fn cache_complex(&mut self, idx: Idx, ty: BasicTypeEnum<'ll>) {
        self.complex.insert(idx, ty);
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
/// - sret tracking for functions returning large structs
pub struct CodegenCx<'ll, 'tcx> {
    /// The underlying simple context.
    pub scx: SimpleCx<'ll>,
    /// String interner for name lookup.
    pub interner: &'tcx StringInterner,
    /// Type pool for resolving `Idx` to type information.
    ///
    /// Used by `compute_llvm_type` to determine LLVM representation
    /// for compound types (List, Map, Tuple, etc.).
    pub pool: Option<&'tcx Pool>,
    /// Cache of compiled functions by name.
    pub instances: RefCell<FxHashMap<Name, FunctionValue<'ll>>>,
    /// Cache of compiled test functions.
    pub tests: RefCell<FxHashMap<Name, FunctionValue<'ll>>>,
    /// Type cache for efficient lookups.
    pub type_cache: RefCell<TypeCache<'ll>>,
    /// Functions that use the sret (structured return) calling convention.
    ///
    /// On x86-64 `SysV` ABI, structs >16 bytes cannot be returned in registers.
    /// These functions have their return type transformed: the original struct
    /// return becomes a hidden first parameter (`ptr sret(T) noalias`), and the
    /// function returns void. Maps function name → original LLVM struct type.
    pub sret_types: RefCell<FxHashMap<Name, StructType<'ll>>>,

    /// Resolved pattern bindings from the type checker.
    ///
    /// Sorted by `PatternKey` for O(log n) binary search.
    /// Used by the matching compiler to distinguish unit variant patterns
    /// from regular variable bindings.
    pub pattern_resolutions: Vec<(PatternKey, PatternResolution)>,
}

impl<'ll, 'tcx> CodegenCx<'ll, 'tcx> {
    /// Create a new full codegen context.
    pub fn new(context: &'ll Context, interner: &'tcx StringInterner, module_name: &str) -> Self {
        let scx = SimpleCx::new(context, module_name);

        let cx = Self {
            scx,
            interner,
            pool: None,
            instances: RefCell::new(FxHashMap::default()),
            tests: RefCell::new(FxHashMap::default()),
            type_cache: RefCell::new(TypeCache::new()),
            sret_types: RefCell::new(FxHashMap::default()),
            pattern_resolutions: Vec::new(),
        };
        cx.register_builtin_sum_types();
        cx
    }

    /// Create a codegen context with a type pool for compound type resolution.
    pub fn with_pool(
        context: &'ll Context,
        interner: &'tcx StringInterner,
        pool: &'tcx Pool,
        module_name: &str,
    ) -> Self {
        let scx = SimpleCx::new(context, module_name);

        let cx = Self {
            scx,
            interner,
            pool: Some(pool),
            instances: RefCell::new(FxHashMap::default()),
            tests: RefCell::new(FxHashMap::default()),
            type_cache: RefCell::new(TypeCache::new()),
            sret_types: RefCell::new(FxHashMap::default()),
            pattern_resolutions: Vec::new(),
        };
        cx.register_builtin_sum_types();
        cx
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

    /// Get the LLVM type for an Ori type `Idx`.
    ///
    /// Uses two-level cache: scalars first, then complex types.
    /// Fast path (cache hit): single `borrow()` check.
    /// Slow path (cache miss): compute + `borrow_mut()` to cache.
    pub fn llvm_type(&self, idx: Idx) -> BasicTypeEnum<'ll> {
        // Fast path: check cache with read-only borrow
        if let Some(ty) = self.type_cache.borrow().get(idx) {
            return ty;
        }

        // Slow path: compute and cache
        let ty = self.compute_llvm_type(idx);

        // Cache in appropriate level
        let mut cache = self.type_cache.borrow_mut();
        if idx.is_primitive() {
            cache.cache_scalar(idx, ty);
        } else {
            cache.cache_complex(idx, ty);
        }

        ty
    }

    /// Compute the LLVM type for an `Idx` (uncached).
    ///
    /// For compound types, uses the type pool to look up the type information
    /// and return the appropriate LLVM struct type.
    fn compute_llvm_type(&self, idx: Idx) -> BasicTypeEnum<'ll> {
        // Fast path: primitive types by constant
        match idx {
            Idx::FLOAT => return self.scx.type_f64().into(),
            Idx::BOOL => return self.scx.type_i1().into(),
            Idx::CHAR => return self.scx.type_i32().into(), // Unicode codepoint
            Idx::STR => return self.string_type().into(),
            // BYTE and ORDERING are both i8 (Ordering: Less=0, Equal=1, Greater=2)
            Idx::BYTE | Idx::ORDERING => return self.scx.type_i8().into(),
            // INT, DURATION, SIZE, UNIT, NEVER use i64
            Idx::INT | Idx::DURATION | Idx::SIZE | Idx::UNIT | Idx::NEVER => {
                return self.scx.type_i64().into();
            }
            _ => {}
        }

        // Use type pool for compound types
        if let Some(pool) = self.pool {
            match pool.tag(idx) {
                Tag::Float => self.scx.type_f64().into(),
                Tag::Bool => self.scx.type_i1().into(),
                Tag::Str => self.string_type().into(),
                Tag::Char => self.scx.type_i32().into(),
                Tag::Byte | Tag::Ordering => self.scx.type_i8().into(),

                // Compound types (Set uses same layout as List)
                Tag::List | Tag::Set => self.list_type().into(),
                Tag::Map => self.map_type().into(),
                Tag::Range => self.range_type().into(),
                // Channel and Function are handles (pointers)
                Tag::Channel | Tag::Function => self.scx.type_ptr().into(),

                // Option and Result need payload type
                Tag::Option => {
                    let inner = pool.option_inner(idx);
                    let payload = self.llvm_type(inner);
                    self.option_type(payload).into()
                }
                Tag::Result => {
                    // For Result, use the larger of ok/err as payload
                    // For simplicity, use ok type (error handling TBD)
                    let ok = pool.result_ok(idx);
                    let payload = self.llvm_type(ok);
                    self.result_type(payload).into()
                }

                // Tuple: struct of element types
                Tag::Tuple => {
                    let elements = pool.tuple_elems(idx);
                    let field_types: Vec<BasicTypeEnum<'ll>> =
                        elements.iter().map(|&id| self.llvm_type(id)).collect();
                    self.scx.type_struct(&field_types, false).into()
                }

                // Named types: look up struct, then sum type, then fall back to i64
                Tag::Named => {
                    let name = pool.named_name(idx);
                    if let Some(struct_ty) = self.get_struct_type(name) {
                        struct_ty.into()
                    } else {
                        // Sum types are also registered as named structs via
                        // register_sum_type, so get_struct_type should find them.
                        // This fallback handles edge cases.
                        self.scx.type_i64().into()
                    }
                }
                // Applied (generic) types: compute layout from type arguments
                // Don't use named structs because Container<int> and Container<str>
                // have different layouts despite sharing the same name.
                Tag::Applied => {
                    let name = pool.applied_name(idx);
                    let args = pool.applied_args(idx);
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

                // All other tags (Int, Duration, Size, Unit, Never, Var, Error, etc.)
                // use i64 representation. Var/Error shouldn't appear at codegen
                // but we fall back to i64 if they do.
                _ => self.scx.type_i64().into(),
            }
        } else {
            // No type pool available: fall back to i64 for unknown types
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

    /// Check if an `Idx` represents an Option type.
    pub fn is_option_type(&self, idx: Idx) -> bool {
        if let Some(pool) = self.pool {
            matches!(pool.tag(idx), Tag::Option)
        } else {
            false
        }
    }

    /// Check if an `Idx` represents a Result type.
    ///
    /// This is needed for coalesce (`??`) where Option and Result have
    /// different tag semantics (Option: tag=1 for Some, Result: tag=0 for Ok).
    pub fn is_result_type(&self, idx: Idx) -> bool {
        if let Some(pool) = self.pool {
            matches!(pool.tag(idx), Tag::Result)
        } else {
            false
        }
    }

    /// Check if an `Idx` represents a coercible wrapper type (Option or Result).
    ///
    /// These types support the `??` coalesce operator.
    pub fn is_wrapper_type(&self, idx: Idx) -> bool {
        self.is_option_type(idx) || self.is_result_type(idx)
    }

    /// Get the inner type of an Option<T>.
    ///
    /// Returns `Some(T)` if the type is `Option<T>`, `None` otherwise.
    pub fn option_inner_type(&self, idx: Idx) -> Option<Idx> {
        if let Some(pool) = self.pool {
            if matches!(pool.tag(idx), Tag::Option) {
                return Some(pool.option_inner(idx));
            }
        }
        None
    }

    /// Get the Ok type of a Result<T, E>.
    ///
    /// Returns `Some(T)` if the type is `Result<T, E>`, `None` otherwise.
    pub fn result_ok_type(&self, idx: Idx) -> Option<Idx> {
        if let Some(pool) = self.pool {
            if matches!(pool.tag(idx), Tag::Result) {
                return Some(pool.result_ok(idx));
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

    // -- Sum type layout management --

    /// Register a user-defined sum type.
    ///
    /// Creates an LLVM struct type: `{ i8 tag, [M x i64] payload }` where
    /// `M = layout.payload_i64_count`.
    pub fn register_sum_type(&self, layout: SumTypeLayout) {
        let name = layout.type_name;
        let tag_ty = self.scx.type_i8();
        let payload_ty = self.scx.type_i64().array_type(layout.payload_i64_count);
        let named = self.get_or_create_named_struct(name);
        self.scx
            .set_struct_body(named, &[tag_ty.into(), payload_ty.into()], false);
        self.type_cache.borrow_mut().sum_types.insert(name, layout);
    }

    /// Register a built-in sum type (Option/Result) for tag lookup only.
    ///
    /// Does NOT create an LLVM struct — their types come from Pool-based generation
    /// (`option_type()`/`result_type()` in `compute_llvm_type`). This only provides
    /// unified tag lookup so `matching.rs` doesn't need hardcoded variant→tag mappings.
    fn register_builtin_sum_type(&self, layout: SumTypeLayout) {
        self.type_cache
            .borrow_mut()
            .sum_types
            .insert(layout.type_name, layout);
    }

    /// Register Option and Result as built-in sum types.
    ///
    /// Called once during initialization. Provides unified tag lookup so
    /// `matching.rs` can use `lookup_variant_constructor` for all sum types
    /// without hardcoded variant→tag mappings.
    fn register_builtin_sum_types(&self) {
        let intern = |s: &str| self.interner.intern(s);

        // Option<T>: None=0, Some=1
        self.register_builtin_sum_type(SumTypeLayout {
            type_name: intern("Option"),
            variants: vec![
                SumVariantLayout {
                    name: intern("None"),
                    tag: 0,
                    field_types: vec![],
                },
                SumVariantLayout {
                    name: intern("Some"),
                    tag: 1,
                    field_types: vec![],
                },
            ],
            payload_i64_count: 0,
        });

        // Result<T, E>: Ok=0, Err=1
        self.register_builtin_sum_type(SumTypeLayout {
            type_name: intern("Result"),
            variants: vec![
                SumVariantLayout {
                    name: intern("Ok"),
                    tag: 0,
                    field_types: vec![],
                },
                SumVariantLayout {
                    name: intern("Err"),
                    tag: 1,
                    field_types: vec![],
                },
            ],
            payload_i64_count: 0,
        });
    }

    /// Look up sum type layout by type name.
    pub fn get_sum_type_layout(&self, name: Name) -> Option<SumTypeLayout> {
        self.type_cache.borrow().sum_types.get(&name).cloned()
    }

    /// Check if a variant name belongs to a registered sum type.
    ///
    /// Returns `(type_name, variant_layout)` if found.
    /// Works for both user-defined and built-in (Option/Result) sum types.
    pub fn lookup_variant_constructor(
        &self,
        variant_name: Name,
    ) -> Option<(Name, SumVariantLayout)> {
        let cache = self.type_cache.borrow();
        for (type_name, layout) in &cache.sum_types {
            for variant in &layout.variants {
                if variant.name == variant_name {
                    return Some((*type_name, variant.clone()));
                }
            }
        }
        None
    }

    // -- Pattern resolution --

    /// Look up a pattern resolution by key.
    ///
    /// Returns `Some(&PatternResolution)` if the pattern was resolved to a
    /// unit variant, `None` if it's a normal variable binding.
    pub fn resolve_pattern(&self, key: PatternKey) -> Option<PatternResolution> {
        self.pattern_resolutions
            .binary_search_by_key(&key, |(k, _)| *k)
            .ok()
            .map(|idx| self.pattern_resolutions[idx].1)
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

    // -- sret (structured return) management --

    /// Check if a return type needs the sret calling convention.
    ///
    /// On x86-64 `SysV` ABI, structs >16 bytes (i.e., >2 eight-byte fields)
    /// cannot be returned in registers and must use a hidden pointer parameter.
    /// Ori struct fields are each 8 bytes (i64/f64/ptr), so >2 fields = >16 bytes.
    pub fn needs_sret(&self, return_type: Idx) -> bool {
        if return_type == Idx::UNIT || return_type == Idx::NEVER {
            return false;
        }
        let llvm_ty = self.llvm_type(return_type);
        matches!(llvm_ty, BasicTypeEnum::StructType(st) if st.count_fields() > 2)
    }

    /// Record that a function uses the sret convention.
    pub fn mark_sret(&self, name: Name, ty: StructType<'ll>) {
        self.sret_types.borrow_mut().insert(name, ty);
    }

    /// Check if a function uses the sret convention.
    pub fn is_sret(&self, name: Name) -> bool {
        self.sret_types.borrow().contains_key(&name)
    }

    /// Get the original struct return type for an sret function.
    pub fn get_sret_type(&self, name: Name) -> Option<StructType<'ll>> {
        self.sret_types.borrow().get(&name).copied()
    }

    // -- Default values --

    /// Get a default value for a type.
    pub fn default_value(&self, idx: Idx) -> inkwell::values::BasicValueEnum<'ll> {
        use ori_ir::builtin_constants::ordering::unsigned as ord;
        match idx {
            Idx::INT => self.scx.type_i64().const_int(0, false).into(),
            Idx::FLOAT => self.scx.type_f64().const_float(0.0).into(),
            Idx::BOOL => self.scx.type_i1().const_int(0, false).into(),
            Idx::CHAR => self.scx.type_i32().const_int(0, false).into(),
            Idx::BYTE => self.scx.type_i8().const_int(0, false).into(),
            // Ordering defaults to Equal (as per spec)
            Idx::ORDERING => self.scx.type_i8().const_int(ord::EQUAL, false).into(),
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
        let int_ty = cx.llvm_type(Idx::INT);

        // Second lookup should hit cache
        let int_ty2 = cx.llvm_type(Idx::INT);

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
