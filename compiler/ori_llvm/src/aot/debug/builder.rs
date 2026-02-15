//! `DebugInfoBuilder` struct definition, constructor, and type-creation methods.

use std::cell::RefCell;
use std::path::Path;

use inkwell::context::Context;
use inkwell::debug_info::{
    AsDIScope, DIBasicType, DICompileUnit, DICompositeType, DIFile, DIFlags, DIFlagsConstants,
    DIScope, DISubroutineType, DIType, DWARFSourceLanguage, DebugInfoBuilder as InkwellDIBuilder,
};
use inkwell::module::{FlagBehavior, Module};
use rustc_hash::FxHashMap;

use super::config::{basic_type_creation_error, DebugInfoConfig, DebugInfoError, DebugLevel};

/// Cached debug type information.
pub(super) struct TypeCache<'ctx> {
    /// Primitive type cache (int, float, bool, etc.).
    pub(super) primitives: FxHashMap<&'static str, DIBasicType<'ctx>>,
    /// Composite type cache for deduplication (keyed by type pool `Idx`).
    pub(super) composites: FxHashMap<u32, DIType<'ctx>>,
}

impl TypeCache<'_> {
    pub(super) fn new() -> Self {
        Self {
            primitives: FxHashMap::default(),
            composites: FxHashMap::default(),
        }
    }
}

/// Field information for struct debug type creation.
#[derive(Debug, Clone)]
pub struct FieldInfo<'a, 'ctx> {
    /// Field name.
    pub name: &'a str,
    /// Field type.
    pub ty: DIType<'ctx>,
    /// Size in bits.
    pub size_bits: u64,
    /// Offset from struct start in bits.
    pub offset_bits: u64,
    /// Line number where field is defined.
    pub line: u32,
}

/// Debug information builder for AOT compilation.
///
/// Wraps LLVM's `DIBuilder` to generate DWARF/CodeView debug information.
/// Created per-module and must be finalized before object emission.
pub struct DebugInfoBuilder<'ctx> {
    /// The underlying LLVM `DIBuilder`.
    pub(super) inner: InkwellDIBuilder<'ctx>,
    /// The compile unit for this module.
    pub(super) compile_unit: DICompileUnit<'ctx>,
    /// The LLVM context.
    pub(super) context: &'ctx Context,
    /// Configuration for debug info generation.
    pub(super) config: DebugInfoConfig,
    /// Cached debug types.
    pub(super) type_cache: RefCell<TypeCache<'ctx>>,
    /// Current scope stack for lexical blocks.
    pub(super) scope_stack: RefCell<Vec<DIScope<'ctx>>>,
}

impl<'ctx> DebugInfoBuilder<'ctx> {
    /// Producer string identifying the Ori compiler.
    const PRODUCER: &'static str = "Ori Compiler";

    /// Create a new debug info builder for a module.
    ///
    /// # Arguments
    ///
    /// * `module` - The LLVM module to add debug info to
    /// * `context` - The LLVM context
    /// * `config` - Debug info configuration
    /// * `source_file` - Path to the source file being compiled
    /// * `source_dir` - Directory containing the source file
    ///
    /// # Returns
    ///
    /// Returns `None` if debug info is disabled in the config.
    #[must_use]
    pub fn new(
        module: &Module<'ctx>,
        context: &'ctx Context,
        config: DebugInfoConfig,
        source_file: &str,
        source_dir: &str,
    ) -> Option<Self> {
        if !config.level.is_enabled() {
            return None;
        }

        // Add debug info version flag to module
        let debug_metadata_version = context.i32_type().const_int(3, false);
        module.add_basic_value_flag(
            "Debug Info Version",
            FlagBehavior::Warning,
            debug_metadata_version,
        );

        // Add DWARF version flag
        let dwarf_version = context
            .i32_type()
            .const_int(u64::from(config.dwarf_version), false);
        module.add_basic_value_flag("Dwarf Version", FlagBehavior::Warning, dwarf_version);

        // Create the DIBuilder and compile unit
        let (inner, compile_unit) = module.create_debug_info_builder(
            /* allow_unresolved */ true,
            /* language */ DWARFSourceLanguage::C, // Closest to Ori's semantics
            /* filename */ source_file,
            /* directory */ source_dir,
            /* producer */ Self::PRODUCER,
            /* is_optimized */ config.optimized,
            /* flags */ "",
            /* runtime_ver */ 0,
            /* split_name */ "",
            /* kind */ config.level.to_emission_kind(),
            /* dwo_id */ 0,
            /* split_debug_inlining */ false,
            /* debug_info_for_profiling */ config.debug_info_for_profiling,
            /* sysroot */ "",
            /* sdk */ "",
        );

        Some(Self {
            inner,
            compile_unit,
            context,
            config,
            type_cache: RefCell::new(TypeCache::new()),
            scope_stack: RefCell::new(Vec::new()),
        })
    }

    /// Create a debug info builder from a file path.
    ///
    /// Extracts the filename and directory from the path.
    #[must_use]
    pub fn from_path(
        module: &Module<'ctx>,
        context: &'ctx Context,
        config: DebugInfoConfig,
        path: &Path,
    ) -> Option<Self> {
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown.ori");
        let dir = path.parent().and_then(|p| p.to_str()).unwrap_or(".");

        Self::new(module, context, config, file_name, dir)
    }

    /// Get the compile unit for this module.
    #[must_use]
    pub fn compile_unit(&self) -> DICompileUnit<'ctx> {
        self.compile_unit
    }

    /// Get the source file for the compile unit.
    #[must_use]
    pub fn file(&self) -> DIFile<'ctx> {
        self.compile_unit.get_file()
    }

    /// Get the debug level.
    #[must_use]
    pub fn level(&self) -> DebugLevel {
        self.config.level
    }

    // -- Type Creation --

    /// Get or create a debug type for `int` (64-bit signed integer).
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the type, which indicates an LLVM internal error.
    pub fn int_type(&self) -> Result<DIBasicType<'ctx>, DebugInfoError> {
        self.get_or_create_basic_type("int", 64, 0x05) // DW_ATE_signed
    }

    /// Get or create a debug type for `float` (64-bit float).
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the type, which indicates an LLVM internal error.
    pub fn float_type(&self) -> Result<DIBasicType<'ctx>, DebugInfoError> {
        self.get_or_create_basic_type("float", 64, 0x04) // DW_ATE_float
    }

    /// Get or create a debug type for `bool` (1-bit boolean).
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the type, which indicates an LLVM internal error.
    pub fn bool_type(&self) -> Result<DIBasicType<'ctx>, DebugInfoError> {
        self.get_or_create_basic_type("bool", 8, 0x02) // DW_ATE_boolean (8-bit for DWARF)
    }

    /// Get or create a debug type for `char` (32-bit Unicode).
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the type, which indicates an LLVM internal error.
    pub fn char_type(&self) -> Result<DIBasicType<'ctx>, DebugInfoError> {
        self.get_or_create_basic_type("char", 32, 0x08) // DW_ATE_unsigned_char
    }

    /// Get or create a debug type for `byte` (8-bit unsigned).
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the type, which indicates an LLVM internal error.
    pub fn byte_type(&self) -> Result<DIBasicType<'ctx>, DebugInfoError> {
        self.get_or_create_basic_type("byte", 8, 0x08) // DW_ATE_unsigned_char
    }

    /// Get or create a debug type for `void`.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the type, which indicates an LLVM internal error.
    pub fn void_type(&self) -> Result<DIBasicType<'ctx>, DebugInfoError> {
        // DWARF doesn't have a void type, use unspecified
        self.get_or_create_basic_type("void", 0, 0x00)
    }

    /// Get or create a basic type with caching.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the type. This indicates a serious LLVM internal error and should
    /// not happen with valid inputs.
    fn get_or_create_basic_type(
        &self,
        name: &'static str,
        size_bits: u64,
        encoding: u32,
    ) -> Result<DIBasicType<'ctx>, DebugInfoError> {
        let mut cache = self.type_cache.borrow_mut();
        if let Some(&ty) = cache.primitives.get(name) {
            return Ok(ty);
        }

        // Create the type (void types need special handling)
        let ty = if size_bits == 0 {
            // For void, create a minimal type. Try zero-size first, then fallback.
            self.inner
                .create_basic_type("void", 0, encoding, DIFlags::ZERO)
                .or_else(|_| {
                    // Fallback: create as "unspecified" with 1 bit
                    self.inner.create_basic_type("void", 1, 0x00, DIFlags::ZERO)
                })
                .map_err(|_| basic_type_creation_error("void"))?
        } else {
            self.inner
                .create_basic_type(name, size_bits, encoding, DIFlags::ZERO)
                .map_err(|_| basic_type_creation_error(name))?
        };

        cache.primitives.insert(name, ty);
        Ok(ty)
    }

    /// Create a subroutine (function) type.
    pub fn create_subroutine_type(
        &self,
        return_type: Option<DIType<'ctx>>,
        param_types: &[DIType<'ctx>],
    ) -> DISubroutineType<'ctx> {
        self.inner
            .create_subroutine_type(self.file(), return_type, param_types, DIFlags::ZERO)
    }

    // -- Composite Type Creation --

    /// Create a struct type with fields.
    ///
    /// # Arguments
    ///
    /// * `name` - Struct type name
    /// * `line` - Line number where struct is defined
    /// * `size_bits` - Total size of struct in bits
    /// * `align_bits` - Alignment in bits
    /// * `fields` - Field information
    ///
    /// # Returns
    ///
    /// The `DICompositeType` representing the struct.
    pub fn create_struct_type(
        &self,
        name: &str,
        line: u32,
        size_bits: u64,
        align_bits: u32,
        fields: &[FieldInfo<'_, 'ctx>],
    ) -> DICompositeType<'ctx> {
        // Create member types for each field
        let member_types: Vec<DIType<'ctx>> = fields
            .iter()
            .map(|field| {
                self.inner
                    .create_member_type(
                        self.compile_unit.as_debug_info_scope(),
                        field.name,
                        self.file(),
                        field.line,
                        field.size_bits,
                        align_bits,
                        field.offset_bits,
                        DIFlags::ZERO,
                        field.ty,
                    )
                    .as_type()
            })
            .collect();

        self.inner.create_struct_type(
            self.compile_unit.as_debug_info_scope(),
            name,
            self.file(),
            line,
            size_bits,
            align_bits,
            DIFlags::ZERO,
            None, // No base type
            &member_types,
            0,    // Runtime language
            None, // No vtable holder
            name, // Unique identifier
        )
    }

    /// Create an enum/sum type.
    ///
    /// For Ori's sum types, we create an enumeration for the discriminant
    /// and represent the overall type as a struct with tag + payload.
    ///
    /// # Arguments
    ///
    /// * `name` - Enum type name
    /// * `line` - Line number where enum is defined
    /// * `variants` - Variant names and their discriminant values
    /// * `underlying_type` - The type of the discriminant (usually int or byte)
    ///
    /// # Returns
    ///
    /// The `DICompositeType` representing the enumeration.
    pub fn create_enum_type(
        &self,
        name: &str,
        line: u32,
        size_bits: u64,
        align_bits: u32,
        variants: &[(&str, i64)],
        underlying_type: DIType<'ctx>,
    ) -> DICompositeType<'ctx> {
        // Create enumerator values for each variant
        let enumerators: Vec<_> = variants
            .iter()
            .map(|(variant_name, value)| self.inner.create_enumerator(variant_name, *value, false))
            .collect();

        self.inner.create_enumeration_type(
            self.compile_unit.as_debug_info_scope(),
            name,
            self.file(),
            line,
            size_bits,
            align_bits,
            &enumerators,
            underlying_type,
        )
    }

    /// Create a pointer type.
    ///
    /// # Arguments
    ///
    /// * `name` - Optional name for the pointer type
    /// * `pointee` - The type being pointed to
    /// * `size_bits` - Size of the pointer in bits (typically 64)
    pub fn create_pointer_type(
        &self,
        name: &str,
        pointee: DIType<'ctx>,
        size_bits: u64,
    ) -> DIType<'ctx> {
        self.inner
            .create_pointer_type(
                name,
                pointee,
                size_bits,
                size_bits as u32, // alignment = size for pointers
                inkwell::AddressSpace::default(),
            )
            .as_type()
    }

    /// Create an array type.
    ///
    /// # Arguments
    ///
    /// * `element_type` - Type of array elements
    /// * `count` - Number of elements
    /// * `size_bits` - Total size in bits
    /// * `align_bits` - Alignment in bits
    // Single-element vec with range is intentional here for LLVM's debug info API
    // which requires a slice of subscript ranges even for 1D arrays.
    #[allow(
        clippy::single_range_in_vec_init,
        reason = "LLVM debug API requires a slice of subscript ranges even for 1D arrays"
    )]
    pub fn create_array_type(
        &self,
        element_type: DIType<'ctx>,
        count: u64,
        size_bits: u64,
        align_bits: u32,
    ) -> DICompositeType<'ctx> {
        // Subscript ranges for a 1D array with `count` elements
        let subscripts = if count > 0 {
            vec![0..(count as i64)]
        } else {
            vec![]
        };

        self.inner
            .create_array_type(element_type, size_bits, align_bits, &subscripts)
    }

    /// Create a typedef (type alias).
    ///
    /// # Arguments
    ///
    /// * `name` - The alias name
    /// * `underlying` - The underlying type
    /// * `line` - Line number where typedef is defined
    pub fn create_typedef(
        &self,
        name: &str,
        underlying: DIType<'ctx>,
        line: u32,
        size_bits: u64,
    ) -> DIType<'ctx> {
        self.inner
            .create_typedef(
                underlying,
                name,
                self.file(),
                line,
                self.compile_unit.as_debug_info_scope(),
                size_bits as u32,
            )
            .as_type()
    }

    // -- Ori-specific type helpers --

    /// Create debug info for Ori's string type: { len: int, data: ptr }.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the underlying int or byte types.
    pub fn string_type(&self) -> Result<DICompositeType<'ctx>, DebugInfoError> {
        let int_ty = self.int_type()?.as_type();
        let ptr_ty = self.create_pointer_type("*byte", self.byte_type()?.as_type(), 64);

        let fields = [
            FieldInfo {
                name: "len",
                ty: int_ty,
                size_bits: 64,
                offset_bits: 0,
                line: 0,
            },
            FieldInfo {
                name: "data",
                ty: ptr_ty,
                size_bits: 64,
                offset_bits: 64,
                line: 0,
            },
        ];

        Ok(self.create_struct_type("str", 0, 128, 64, &fields))
    }

    /// Create debug info for Option<T>: { tag: byte, payload: T }.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the underlying byte type for the tag.
    pub fn option_type(
        &self,
        payload_ty: DIType<'ctx>,
        payload_size_bits: u64,
    ) -> Result<DICompositeType<'ctx>, DebugInfoError> {
        let byte_ty = self.byte_type()?.as_type();

        // Alignment: max of tag (8) and payload alignment
        let align_bits = 64u32; // Assume 8-byte alignment for simplicity

        // Option enum: None=0, Some=1
        let tag_ty =
            self.create_enum_type("OptionTag", 0, 8, 8, &[("None", 0), ("Some", 1)], byte_ty);

        let fields = [
            FieldInfo {
                name: "tag",
                ty: tag_ty.as_type(),
                size_bits: 8,
                offset_bits: 0,
                line: 0,
            },
            FieldInfo {
                name: "payload",
                ty: payload_ty,
                size_bits: payload_size_bits,
                offset_bits: 64, // Aligned to 8 bytes
                line: 0,
            },
        ];

        let total_size = 64 + payload_size_bits; // tag + padding + payload
        Ok(self.create_struct_type("Option", 0, total_size, align_bits, &fields))
    }

    /// Create debug info for Result<T, E>: { tag: byte, payload: union }.
    ///
    /// The payload size is the maximum of ok and error sizes, representing
    /// the union semantics of a sum type where either variant can occupy the space.
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the underlying byte type for the tag.
    pub fn result_type(
        &self,
        ok_ty: DIType<'ctx>,
        ok_size_bits: u64,
        err_ty: DIType<'ctx>,
        err_size_bits: u64,
    ) -> Result<DICompositeType<'ctx>, DebugInfoError> {
        let byte_ty = self.byte_type()?.as_type();

        // Result enum: Ok=0, Err=1
        let tag_ty = self.create_enum_type("ResultTag", 0, 8, 8, &[("Ok", 0), ("Err", 1)], byte_ty);

        // Use the larger of ok and error sizes for proper union semantics
        let payload_size = ok_size_bits.max(err_size_bits);
        // Use the type with the larger size for the payload field in debug info
        let (payload_ty, payload_name) = if ok_size_bits >= err_size_bits {
            (ok_ty, "ok_payload")
        } else {
            (err_ty, "err_payload")
        };

        let fields = [
            FieldInfo {
                name: "tag",
                ty: tag_ty.as_type(),
                size_bits: 8,
                offset_bits: 0,
                line: 0,
            },
            FieldInfo {
                name: payload_name,
                ty: payload_ty,
                size_bits: payload_size,
                offset_bits: 64,
                line: 0,
            },
        ];

        let total_size = 64 + payload_size;
        Ok(self.create_struct_type("Result", 0, total_size, 64, &fields))
    }

    /// Create debug info for a list type: { len, cap, data }.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the underlying int type for length and capacity.
    pub fn list_type(
        &self,
        element_ty: DIType<'ctx>,
    ) -> Result<DICompositeType<'ctx>, DebugInfoError> {
        let int_ty = self.int_type()?.as_type();
        let ptr_ty = self.create_pointer_type("*elem", element_ty, 64);

        let fields = [
            FieldInfo {
                name: "len",
                ty: int_ty,
                size_bits: 64,
                offset_bits: 0,
                line: 0,
            },
            FieldInfo {
                name: "cap",
                ty: int_ty,
                size_bits: 64,
                offset_bits: 64,
                line: 0,
            },
            FieldInfo {
                name: "data",
                ty: ptr_ty,
                size_bits: 64,
                offset_bits: 128,
                line: 0,
            },
        ];

        Ok(self.create_struct_type("[T]", 0, 192, 64, &fields))
    }
}
