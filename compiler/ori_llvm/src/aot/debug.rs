//! Debug Information Generation for AOT Compilation
//!
//! This module provides DWARF/CodeView debug information generation using LLVM's
//! `DIBuilder` infrastructure. Debug info enables source-level debugging with tools
//! like GDB, LLDB, and Visual Studio.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
//! │  Source File    │────▶│  DebugInfoBuilder │────▶│  DWARF/CodeView │
//! │  (spans, names) │     │  (DIBuilder)      │     │  (in object)    │
//! └─────────────────┘     └──────────────────┘     └─────────────────┘
//! ```
//!
//! # Debug Levels
//!
//! - `None`: No debug info (smallest output, fastest compile)
//! - `LineTablesOnly`: Line numbers only (small overhead, basic debugging)
//! - `Full`: Complete debug info (types, variables, full debugging)
//!
//! # Usage
//!
//! ```ignore
//! use ori_llvm::aot::debug::{DebugInfoBuilder, DebugInfoConfig, DebugLevel};
//!
//! let config = DebugInfoConfig::new(DebugLevel::Full);
//! let di = DebugInfoBuilder::new(&module, &context, config, "src/main.ori", "src")?;
//!
//! // Create function debug info
//! let func_di = di.create_function("my_func", 10, &fn_type);
//! fn_val.set_subprogram(func_di);
//!
//! // Set debug location for instructions
//! di.set_location(&builder, 15, 4);
//!
//! // Finalize before emission
//! di.finalize();
//! ```

use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::path::Path;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::debug_info::{
    AsDIScope, DIBasicType, DICompileUnit, DICompositeType, DIFile, DIFlags, DIFlagsConstants,
    DILexicalBlock, DIScope, DISubprogram, DISubroutineType, DIType, DWARFEmissionKind,
    DWARFSourceLanguage, DebugInfoBuilder as InkwellDIBuilder,
};
use inkwell::module::{FlagBehavior, Module};
use inkwell::values::FunctionValue;

/// Debug information detail level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugLevel {
    /// No debug information.
    #[default]
    None,
    /// Line tables only (file/line/column, no type info).
    /// Good balance of debug capability and compile speed.
    LineTablesOnly,
    /// Full debug information (types, variables, scopes).
    /// Maximum debugging capability, slowest compile.
    Full,
}

impl DebugLevel {
    /// Convert to LLVM DWARF emission kind.
    fn to_emission_kind(self) -> DWARFEmissionKind {
        match self {
            Self::None => DWARFEmissionKind::None,
            Self::LineTablesOnly => DWARFEmissionKind::LineTablesOnly,
            Self::Full => DWARFEmissionKind::Full,
        }
    }

    /// Check if debug info should be generated.
    #[must_use]
    pub fn is_enabled(self) -> bool {
        !matches!(self, Self::None)
    }
}

impl std::fmt::Display for DebugLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::LineTablesOnly => write!(f, "line-tables"),
            Self::Full => write!(f, "full"),
        }
    }
}

/// Debug format for different platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugFormat {
    /// DWARF format (Linux, macOS, WASM).
    /// This is the default for Unix-like systems.
    #[default]
    Dwarf,
    /// `CodeView` format (Windows).
    /// Used with MSVC toolchain.
    CodeView,
}

impl DebugFormat {
    /// Determine the appropriate debug format for a target triple.
    #[must_use]
    pub fn for_target(target: &str) -> Self {
        if target.contains("windows") && target.contains("msvc") {
            Self::CodeView
        } else {
            Self::Dwarf
        }
    }

    /// Check if this format produces DWARF debug info.
    #[must_use]
    pub fn is_dwarf(&self) -> bool {
        matches!(self, Self::Dwarf)
    }

    /// Check if this format produces `CodeView` debug info.
    #[must_use]
    pub fn is_codeview(&self) -> bool {
        matches!(self, Self::CodeView)
    }
}

impl std::fmt::Display for DebugFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dwarf => write!(f, "DWARF"),
            Self::CodeView => write!(f, "CodeView"),
        }
    }
}

/// Configuration for debug information generation.
#[derive(Debug, Clone)]
pub struct DebugInfoConfig {
    /// Debug information detail level.
    pub level: DebugLevel,
    /// Whether this is an optimized build.
    /// When true, debug info may be less accurate due to optimizations.
    pub optimized: bool,
    /// DWARF version to emit (4 or 5).
    /// Only applicable when format is DWARF.
    pub dwarf_version: u32,
    /// Debug format (DWARF or `CodeView`).
    pub format: DebugFormat,
    /// Whether to generate split debug info (dSYM on macOS, .dwo on Linux).
    /// Split debug info keeps symbols separate from the main binary.
    pub split_debug_info: bool,
    /// Whether to enable debug info for profiling tools.
    pub debug_info_for_profiling: bool,
}

impl Default for DebugInfoConfig {
    fn default() -> Self {
        Self {
            level: DebugLevel::None,
            optimized: false,
            dwarf_version: 4,
            format: DebugFormat::Dwarf,
            split_debug_info: false,
            debug_info_for_profiling: false,
        }
    }
}

impl DebugInfoConfig {
    /// Create a new debug info configuration with the given level.
    #[must_use]
    pub fn new(level: DebugLevel) -> Self {
        Self {
            level,
            ..Default::default()
        }
    }

    /// Create a debug configuration for development (full debug, unoptimized).
    #[must_use]
    pub fn development() -> Self {
        Self {
            level: DebugLevel::Full,
            optimized: false,
            dwarf_version: 4,
            format: DebugFormat::Dwarf,
            split_debug_info: false,
            debug_info_for_profiling: false,
        }
    }

    /// Create a debug configuration for release with debug info.
    #[must_use]
    pub fn release_with_debug() -> Self {
        Self {
            level: DebugLevel::LineTablesOnly,
            optimized: true,
            dwarf_version: 4,
            format: DebugFormat::Dwarf,
            split_debug_info: true, // Keep release binaries smaller
            debug_info_for_profiling: false,
        }
    }

    /// Create a configuration appropriate for a target triple.
    #[must_use]
    pub fn for_target(level: DebugLevel, target: &str) -> Self {
        Self {
            level,
            optimized: false,
            dwarf_version: 4,
            format: DebugFormat::for_target(target),
            split_debug_info: false,
            debug_info_for_profiling: false,
        }
    }

    /// Set whether this is an optimized build.
    #[must_use]
    pub fn with_optimized(mut self, optimized: bool) -> Self {
        self.optimized = optimized;
        self
    }

    /// Set the DWARF version (4 or 5).
    #[must_use]
    pub fn with_dwarf_version(mut self, version: u32) -> Self {
        self.dwarf_version = version;
        self
    }

    /// Set the debug format.
    #[must_use]
    pub fn with_format(mut self, format: DebugFormat) -> Self {
        self.format = format;
        self
    }

    /// Enable split debug info (dSYM on macOS, .dwo on Linux).
    #[must_use]
    pub fn with_split_debug_info(mut self, split: bool) -> Self {
        self.split_debug_info = split;
        self
    }

    /// Enable debug info for profiling tools.
    #[must_use]
    pub fn with_profiling(mut self, profiling: bool) -> Self {
        self.debug_info_for_profiling = profiling;
        self
    }

    /// Check if dSYM bundle should be generated (macOS with split debug).
    #[must_use]
    pub fn needs_dsym(&self, target: &str) -> bool {
        self.level.is_enabled() && self.split_debug_info && target.contains("apple")
    }

    /// Check if PDB file should be generated (Windows with `CodeView`).
    #[must_use]
    pub fn needs_pdb(&self, target: &str) -> bool {
        self.level.is_enabled()
            && self.format.is_codeview()
            && target.contains("windows")
            && target.contains("msvc")
    }
}

/// Error type for debug info operations.
#[derive(Debug, Clone)]
pub enum DebugInfoError {
    /// Failed to create basic type.
    BasicType { name: String, message: String },
    /// Failed to create a basic type during LLVM debug info generation.
    /// This indicates an LLVM internal error and should not happen with valid inputs.
    BasicTypeCreation { name: String },
    /// Debug info is disabled.
    Disabled,
}

impl std::fmt::Display for DebugInfoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BasicType { name, message } => {
                write!(f, "failed to create debug type '{name}': {message}")
            }
            Self::BasicTypeCreation { name } => {
                write!(f, "LLVM failed to create basic debug type '{name}'")
            }
            Self::Disabled => write!(f, "debug info is disabled"),
        }
    }
}

impl std::error::Error for DebugInfoError {}

/// Create a `DebugInfoError::BasicTypeCreation` error (cold path).
///
/// This function is marked `#[cold]` because basic type creation should
/// never fail under normal circumstances. A failure here indicates a
/// serious LLVM internal error.
#[cold]
#[inline(never)]
fn basic_type_creation_error(name: &str) -> DebugInfoError {
    DebugInfoError::BasicTypeCreation {
        name: name.to_string(),
    }
}

/// Cached debug type information.
struct TypeCache<'ctx> {
    /// Primitive type cache (int, float, bool, etc.).
    primitives: FxHashMap<&'static str, DIBasicType<'ctx>>,
}

impl TypeCache<'_> {
    fn new() -> Self {
        Self {
            primitives: FxHashMap::default(),
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
    inner: InkwellDIBuilder<'ctx>,
    /// The compile unit for this module.
    compile_unit: DICompileUnit<'ctx>,
    /// The LLVM context.
    context: &'ctx Context,
    /// Configuration for debug info generation.
    config: DebugInfoConfig,
    /// Cached debug types.
    type_cache: RefCell<TypeCache<'ctx>>,
    /// Current scope stack for lexical blocks.
    scope_stack: RefCell<Vec<DIScope<'ctx>>>,
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
    #[allow(clippy::single_range_in_vec_init)]
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

    // -- Function Debug Info --

    /// Create debug info for a function.
    ///
    /// # Arguments
    ///
    /// * `name` - Function name as it appears in source
    /// * `linkage_name` - Mangled name (or None to use `name`)
    /// * `line` - Line number where function is defined
    /// * `subroutine_type` - Function's type signature
    /// * `is_local` - Whether function has internal linkage
    /// * `is_definition` - Whether this is the function definition (not declaration)
    ///
    /// # Returns
    ///
    /// The `DISubprogram` to attach to the LLVM function.
    pub fn create_function(
        &self,
        name: &str,
        linkage_name: Option<&str>,
        line: u32,
        subroutine_type: DISubroutineType<'ctx>,
        is_local: bool,
        is_definition: bool,
    ) -> DISubprogram<'ctx> {
        self.inner.create_function(
            self.compile_unit.as_debug_info_scope(),
            name,
            linkage_name,
            self.file(),
            line,
            subroutine_type,
            is_local,
            is_definition,
            line, // scope_line = definition line
            DIFlags::ZERO,
            self.config.optimized,
        )
    }

    /// Create a simple function debug info entry.
    ///
    /// Convenience method that creates a void-returning function with no parameters.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the void type for the return type.
    pub fn create_simple_function(
        &self,
        name: &str,
        line: u32,
    ) -> Result<DISubprogram<'ctx>, DebugInfoError> {
        let void_type = self.void_type()?;
        let subroutine = self.create_subroutine_type(Some(void_type.as_type()), &[]);
        Ok(self.create_function(name, None, line, subroutine, false, true))
    }

    /// Attach debug info to a function value.
    pub fn attach_function(&self, func: FunctionValue<'ctx>, subprogram: DISubprogram<'ctx>) {
        func.set_subprogram(subprogram);
    }

    // -- Scope Management --

    /// Create a lexical block (scope) within a function or other scope.
    pub fn create_lexical_block(
        &self,
        scope: DIScope<'ctx>,
        line: u32,
        column: u32,
    ) -> DILexicalBlock<'ctx> {
        self.inner
            .create_lexical_block(scope, self.file(), line, column)
    }

    /// Push a scope onto the scope stack.
    pub fn push_scope(&self, scope: DIScope<'ctx>) {
        self.scope_stack.borrow_mut().push(scope);
    }

    /// Pop a scope from the scope stack.
    pub fn pop_scope(&self) -> Option<DIScope<'ctx>> {
        self.scope_stack.borrow_mut().pop()
    }

    /// Get the current scope (top of stack or compile unit).
    pub fn current_scope(&self) -> DIScope<'ctx> {
        self.scope_stack
            .borrow()
            .last()
            .copied()
            .unwrap_or_else(|| self.compile_unit.as_debug_info_scope())
    }

    // -- Location Setting --

    /// Set the current debug location for subsequent instructions.
    ///
    /// # Arguments
    ///
    /// * `builder` - The LLVM IR builder
    /// * `line` - Source line number (1-indexed)
    /// * `column` - Source column number (1-indexed)
    /// * `scope` - The debug scope for this location
    pub fn set_location(
        &self,
        builder: &Builder<'ctx>,
        line: u32,
        column: u32,
        scope: DIScope<'ctx>,
    ) {
        let loc = self.inner.create_debug_location(
            self.context,
            line,
            column,
            scope,
            None, // No inlined-at
        );
        builder.set_current_debug_location(loc);
    }

    /// Set debug location using the current scope from the stack.
    pub fn set_location_in_current_scope(&self, builder: &Builder<'ctx>, line: u32, column: u32) {
        self.set_location(builder, line, column, self.current_scope());
    }

    /// Clear the current debug location.
    pub fn clear_location(&self, builder: &Builder<'ctx>) {
        builder.unset_current_debug_location();
    }

    // -- Finalization --

    /// Finalize the debug info.
    ///
    /// Must be called before emitting the module as object code.
    /// This resolves forward references and validates the debug info.
    pub fn finalize(&self) {
        self.inner.finalize();
    }
}

/// Helper to convert byte offset spans to line/column.
///
/// This structure pre-computes line start offsets for efficient lookup.
#[derive(Debug, Clone)]
pub struct LineMap {
    /// Byte offsets where each line starts (0-indexed).
    /// `line_starts[0]` is always 0 (start of file).
    /// `line_starts[n]` is the byte offset of line n+1.
    line_starts: Vec<u32>,
}

impl LineMap {
    /// Create a line map from source text.
    #[must_use]
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, c) in source.char_indices() {
            if c == '\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self { line_starts }
    }

    /// Convert a byte offset to (line, column).
    ///
    /// Both line and column are 1-indexed (standard for debug info).
    #[must_use]
    pub fn offset_to_line_col(&self, offset: u32) -> (u32, u32) {
        // Binary search for the line containing this offset
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,      // Exact match at line start
            Err(i) => i - 1, // Between line starts
        };

        let line = (line_idx + 1) as u32; // 1-indexed
        let col = offset - self.line_starts[line_idx] + 1; // 1-indexed

        (line, col)
    }

    /// Get the number of lines in the source.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

/// Combined debug info context for a compilation unit.
///
/// This combines the `DebugInfoBuilder` with a `LineMap` to provide
/// convenient span-based location setting.
pub struct DebugContext<'ctx> {
    /// The debug info builder.
    pub builder: DebugInfoBuilder<'ctx>,
    /// Line map for span-to-location conversion.
    pub line_map: LineMap,
}

impl<'ctx> DebugContext<'ctx> {
    /// Create a new debug context.
    ///
    /// # Arguments
    ///
    /// * `module` - The LLVM module
    /// * `context` - The LLVM context
    /// * `config` - Debug info configuration
    /// * `source_path` - Path to the source file
    /// * `source_text` - The source text (for line map building)
    ///
    /// # Returns
    ///
    /// Returns `None` if debug info is disabled.
    #[must_use]
    pub fn new(
        module: &Module<'ctx>,
        context: &'ctx Context,
        config: DebugInfoConfig,
        source_path: &Path,
        source_text: &str,
    ) -> Option<Self> {
        let builder = DebugInfoBuilder::from_path(module, context, config, source_path)?;
        let line_map = LineMap::new(source_text);
        Some(Self { builder, line_map })
    }

    /// Set debug location from a span's start offset.
    ///
    /// # Arguments
    ///
    /// * `ir_builder` - The LLVM IR builder
    /// * `span_start` - The byte offset of the span start
    /// * `scope` - The debug scope for this location
    pub fn set_location_from_offset(
        &self,
        ir_builder: &inkwell::builder::Builder<'ctx>,
        span_start: u32,
        scope: DIScope<'ctx>,
    ) {
        let (line, col) = self.line_map.offset_to_line_col(span_start);
        self.builder.set_location(ir_builder, line, col, scope);
    }

    /// Set debug location from a span using the current scope.
    pub fn set_location_from_offset_in_current_scope(
        &self,
        ir_builder: &inkwell::builder::Builder<'ctx>,
        span_start: u32,
    ) {
        let (line, col) = self.line_map.offset_to_line_col(span_start);
        self.builder
            .set_location_in_current_scope(ir_builder, line, col);
    }

    /// Get the line and column for a byte offset.
    #[must_use]
    pub fn offset_to_line_col(&self, offset: u32) -> (u32, u32) {
        self.line_map.offset_to_line_col(offset)
    }

    /// Create debug info for a function at a given span offset.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the void type for the return type.
    pub fn create_function_at_offset(
        &self,
        name: &str,
        span_start: u32,
    ) -> Result<DISubprogram<'ctx>, DebugInfoError> {
        let (line, _col) = self.line_map.offset_to_line_col(span_start);
        self.builder.create_simple_function(name, line)
    }

    /// Create debug info for a function with full signature.
    pub fn create_function_with_type(
        &self,
        name: &str,
        linkage_name: Option<&str>,
        span_start: u32,
        subroutine_type: DISubroutineType<'ctx>,
        is_local: bool,
    ) -> DISubprogram<'ctx> {
        let (line, _col) = self.line_map.offset_to_line_col(span_start);
        self.builder
            .create_function(name, linkage_name, line, subroutine_type, is_local, true)
    }

    /// Create a lexical block at a given span offset.
    pub fn create_lexical_block_at_offset(
        &self,
        scope: DIScope<'ctx>,
        span_start: u32,
    ) -> DILexicalBlock<'ctx> {
        let (line, col) = self.line_map.offset_to_line_col(span_start);
        self.builder.create_lexical_block(scope, line, col)
    }

    /// Push a function scope for the given subprogram.
    pub fn enter_function(&self, subprogram: DISubprogram<'ctx>) {
        self.builder.push_scope(subprogram.as_debug_info_scope());
    }

    /// Pop the current function scope.
    pub fn exit_function(&self) {
        self.builder.pop_scope();
    }

    /// Get the debug info builder.
    #[must_use]
    pub fn di(&self) -> &DebugInfoBuilder<'ctx> {
        &self.builder
    }

    /// Finalize debug info (must be called before emission).
    pub fn finalize(&self) {
        self.builder.finalize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that `to_emission_kind` correctly maps `DebugLevel` variants.
    /// This test must remain inline as it tests a private method.
    #[test]
    fn test_debug_level_emission_kind() {
        assert_eq!(DebugLevel::None.to_emission_kind(), DWARFEmissionKind::None);
        assert_eq!(
            DebugLevel::LineTablesOnly.to_emission_kind(),
            DWARFEmissionKind::LineTablesOnly
        );
        assert_eq!(DebugLevel::Full.to_emission_kind(), DWARFEmissionKind::Full);
    }
}
