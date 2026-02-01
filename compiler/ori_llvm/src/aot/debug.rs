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

use std::cell::RefCell;
use std::collections::HashMap;
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
    /// Debug info is disabled.
    Disabled,
}

impl std::fmt::Display for DebugInfoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BasicType { name, message } => {
                write!(f, "failed to create debug type '{name}': {message}")
            }
            Self::Disabled => write!(f, "debug info is disabled"),
        }
    }
}

impl std::error::Error for DebugInfoError {}

/// Cached debug type information.
struct TypeCache<'ctx> {
    /// Primitive type cache (int, float, bool, etc.).
    primitives: HashMap<&'static str, DIBasicType<'ctx>>,
}

impl TypeCache<'_> {
    fn new() -> Self {
        Self {
            primitives: HashMap::new(),
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
    pub fn int_type(&self) -> DIBasicType<'ctx> {
        self.get_or_create_basic_type("int", 64, 0x05) // DW_ATE_signed
    }

    /// Get or create a debug type for `float` (64-bit float).
    pub fn float_type(&self) -> DIBasicType<'ctx> {
        self.get_or_create_basic_type("float", 64, 0x04) // DW_ATE_float
    }

    /// Get or create a debug type for `bool` (1-bit boolean).
    pub fn bool_type(&self) -> DIBasicType<'ctx> {
        self.get_or_create_basic_type("bool", 8, 0x02) // DW_ATE_boolean (8-bit for DWARF)
    }

    /// Get or create a debug type for `char` (32-bit Unicode).
    pub fn char_type(&self) -> DIBasicType<'ctx> {
        self.get_or_create_basic_type("char", 32, 0x08) // DW_ATE_unsigned_char
    }

    /// Get or create a debug type for `byte` (8-bit unsigned).
    pub fn byte_type(&self) -> DIBasicType<'ctx> {
        self.get_or_create_basic_type("byte", 8, 0x08) // DW_ATE_unsigned_char
    }

    /// Get or create a debug type for `void`.
    pub fn void_type(&self) -> DIBasicType<'ctx> {
        // DWARF doesn't have a void type, use unspecified
        self.get_or_create_basic_type("void", 0, 0x00)
    }

    /// Get or create a basic type with caching.
    fn get_or_create_basic_type(
        &self,
        name: &'static str,
        size_bits: u64,
        encoding: u32,
    ) -> DIBasicType<'ctx> {
        let mut cache = self.type_cache.borrow_mut();
        if let Some(&ty) = cache.primitives.get(name) {
            return ty;
        }

        // Create the type (void types need special handling)
        let ty = if size_bits == 0 {
            // For void, create a minimal type
            self.inner
                .create_basic_type("void", 0, encoding, DIFlags::ZERO)
                .unwrap_or_else(|_| {
                    // Fallback: create as "unspecified" with 1 bit
                    self.inner
                        .create_basic_type("void", 1, 0x00, DIFlags::ZERO)
                        .expect("failed to create void debug type")
                })
        } else {
            self.inner
                .create_basic_type(name, size_bits, encoding, DIFlags::ZERO)
                .expect("failed to create basic debug type")
        };

        cache.primitives.insert(name, ty);
        ty
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
    pub fn string_type(&self) -> DICompositeType<'ctx> {
        let int_ty = self.int_type().as_type();
        let ptr_ty = self.create_pointer_type("*byte", self.byte_type().as_type(), 64);

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

        self.create_struct_type("str", 0, 128, 64, &fields)
    }

    /// Create debug info for Option<T>: { tag: byte, payload: T }.
    pub fn option_type(
        &self,
        payload_ty: DIType<'ctx>,
        payload_size_bits: u64,
    ) -> DICompositeType<'ctx> {
        let byte_ty = self.byte_type().as_type();

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
        self.create_struct_type("Option", 0, total_size, align_bits, &fields)
    }

    /// Create debug info for Result<T, E>: { tag: byte, payload: union }.
    pub fn result_type(
        &self,
        ok_ty: DIType<'ctx>,
        ok_size_bits: u64,
        _err_ty: DIType<'ctx>,
        _err_size_bits: u64,
    ) -> DICompositeType<'ctx> {
        let byte_ty = self.byte_type().as_type();

        // Result enum: Ok=0, Err=1
        let tag_ty = self.create_enum_type("ResultTag", 0, 8, 8, &[("Ok", 0), ("Err", 1)], byte_ty);

        // For simplicity, we use the larger payload size
        // In practice, this would be a union
        let payload_size = ok_size_bits.max(64); // min 64 for Error type

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
                ty: ok_ty,
                size_bits: payload_size,
                offset_bits: 64,
                line: 0,
            },
        ];

        let total_size = 64 + payload_size;
        self.create_struct_type("Result", 0, total_size, 64, &fields)
    }

    /// Create debug info for a list type: { len, cap, data }.
    pub fn list_type(&self, element_ty: DIType<'ctx>) -> DICompositeType<'ctx> {
        let int_ty = self.int_type().as_type();
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

        self.create_struct_type("[T]", 0, 192, 64, &fields)
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
    pub fn create_simple_function(&self, name: &str, line: u32) -> DISubprogram<'ctx> {
        let void_type = self.void_type();
        let subroutine = self.create_subroutine_type(Some(void_type.as_type()), &[]);
        self.create_function(name, None, line, subroutine, false, true)
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
    pub fn create_function_at_offset(&self, name: &str, span_start: u32) -> DISubprogram<'ctx> {
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

    #[test]
    fn test_debug_level_emission_kind() {
        assert_eq!(DebugLevel::None.to_emission_kind(), DWARFEmissionKind::None);
        assert_eq!(
            DebugLevel::LineTablesOnly.to_emission_kind(),
            DWARFEmissionKind::LineTablesOnly
        );
        assert_eq!(DebugLevel::Full.to_emission_kind(), DWARFEmissionKind::Full);
    }

    #[test]
    fn test_debug_level_is_enabled() {
        assert!(!DebugLevel::None.is_enabled());
        assert!(DebugLevel::LineTablesOnly.is_enabled());
        assert!(DebugLevel::Full.is_enabled());
    }

    #[test]
    fn test_debug_info_config_default() {
        let config = DebugInfoConfig::default();
        assert_eq!(config.level, DebugLevel::None);
        assert!(!config.optimized);
        assert_eq!(config.dwarf_version, 4);
    }

    #[test]
    fn test_debug_info_config_development() {
        let config = DebugInfoConfig::development();
        assert_eq!(config.level, DebugLevel::Full);
        assert!(!config.optimized);
    }

    #[test]
    fn test_debug_info_config_release() {
        let config = DebugInfoConfig::release_with_debug();
        assert_eq!(config.level, DebugLevel::LineTablesOnly);
        assert!(config.optimized);
    }

    #[test]
    fn test_debug_info_config_builder() {
        let config = DebugInfoConfig::new(DebugLevel::Full)
            .with_optimized(true)
            .with_dwarf_version(5);

        assert_eq!(config.level, DebugLevel::Full);
        assert!(config.optimized);
        assert_eq!(config.dwarf_version, 5);
    }

    #[test]
    fn test_line_map_simple() {
        let source = "line1\nline2\nline3";
        let map = LineMap::new(source);

        assert_eq!(map.line_count(), 3);

        // First character of each line
        assert_eq!(map.offset_to_line_col(0), (1, 1)); // 'l' in line1
        assert_eq!(map.offset_to_line_col(6), (2, 1)); // 'l' in line2
        assert_eq!(map.offset_to_line_col(12), (3, 1)); // 'l' in line3

        // Middle of lines
        assert_eq!(map.offset_to_line_col(2), (1, 3)); // 'n' in line1
        assert_eq!(map.offset_to_line_col(8), (2, 3)); // 'n' in line2
    }

    #[test]
    fn test_line_map_empty() {
        let source = "";
        let map = LineMap::new(source);
        assert_eq!(map.line_count(), 1);
        assert_eq!(map.offset_to_line_col(0), (1, 1));
    }

    #[test]
    fn test_line_map_single_line() {
        let source = "hello";
        let map = LineMap::new(source);
        assert_eq!(map.line_count(), 1);
        assert_eq!(map.offset_to_line_col(0), (1, 1));
        assert_eq!(map.offset_to_line_col(4), (1, 5));
    }

    #[test]
    fn test_line_map_trailing_newline() {
        let source = "line1\nline2\n";
        let map = LineMap::new(source);
        assert_eq!(map.line_count(), 3); // Empty line after trailing newline
    }

    #[test]
    fn test_debug_info_error_display() {
        let err = DebugInfoError::BasicType {
            name: "int".to_string(),
            message: "encoding error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to create debug type 'int': encoding error"
        );

        let err = DebugInfoError::Disabled;
        assert_eq!(err.to_string(), "debug info is disabled");
    }

    #[test]
    fn test_debug_info_builder_disabled() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::None);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".");
        assert!(builder.is_none());
    }

    #[test]
    fn test_debug_info_builder_creation() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", "src");
        assert!(builder.is_some());

        let builder = builder.unwrap();
        assert_eq!(builder.level(), DebugLevel::Full);
    }

    #[test]
    fn test_debug_info_builder_basic_types() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        // Create basic types (should not panic)
        let _int_ty = builder.int_type();
        let _float_ty = builder.float_type();
        let _bool_ty = builder.bool_type();
        let _char_ty = builder.char_type();
        let _byte_ty = builder.byte_type();

        // Second call should return cached type
        let int_ty1 = builder.int_type();
        let int_ty2 = builder.int_type();
        // Types should be equal (same pointer) - use as_mut_ptr for comparison
        assert_eq!(int_ty1.as_mut_ptr(), int_ty2.as_mut_ptr());

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_info_builder_function() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        // Create a function
        let fn_type = context.void_type().fn_type(&[], false);
        let fn_val = module.add_function("test_func", fn_type, None);

        // Create function debug info
        let void_ty = builder.void_type();
        let subroutine = builder.create_subroutine_type(Some(void_ty.as_type()), &[]);
        let subprogram = builder.create_function("test_func", None, 1, subroutine, false, true);

        // Attach to function
        builder.attach_function(fn_val, subprogram);

        // Add entry block
        let entry = context.append_basic_block(fn_val, "entry");
        let ir_builder = context.create_builder();
        ir_builder.position_at_end(entry);

        // Set debug location
        builder.set_location(&ir_builder, 2, 1, subprogram.as_debug_info_scope());

        ir_builder.build_return(None).unwrap();

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_info_builder_lexical_block() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        // Create function
        let subprogram = builder.create_simple_function("test_func", 1);

        // Create lexical block
        let block = builder.create_lexical_block(subprogram.as_debug_info_scope(), 2, 1);

        // Use scope stack
        builder.push_scope(subprogram.as_debug_info_scope());
        assert!(!builder.scope_stack.borrow().is_empty());

        builder.push_scope(block.as_debug_info_scope());
        assert_eq!(builder.scope_stack.borrow().len(), 2);

        builder.pop_scope();
        assert_eq!(builder.scope_stack.borrow().len(), 1);

        builder.finalize();
    }

    #[test]
    fn test_debug_info_builder_from_path() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let path = Path::new("/home/user/project/src/main.ori");
        let builder = DebugInfoBuilder::from_path(&module, &context, config, path);
        assert!(builder.is_some());
    }

    #[test]
    fn test_debug_info_builder_scope_management() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        // Initial scope should be compile unit
        let initial = builder.current_scope();
        assert_eq!(
            initial.as_mut_ptr(),
            builder.compile_unit.as_debug_info_scope().as_mut_ptr()
        );

        // Push function scope
        let subprogram = builder.create_simple_function("func", 1);
        builder.push_scope(subprogram.as_debug_info_scope());

        // Current scope should be function
        let current = builder.current_scope();
        assert_eq!(
            current.as_mut_ptr(),
            subprogram.as_debug_info_scope().as_mut_ptr()
        );

        // Pop should restore to compile unit
        builder.pop_scope();
        let after_pop = builder.current_scope();
        assert_eq!(
            after_pop.as_mut_ptr(),
            builder.compile_unit.as_debug_info_scope().as_mut_ptr()
        );

        builder.finalize();
    }

    #[test]
    fn test_debug_context_creation() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let source = "line1\nline2\nline3";
        let path = Path::new("/src/test.ori");

        let ctx = DebugContext::new(&module, &context, config, path, source);
        assert!(ctx.is_some());

        let ctx = ctx.unwrap();

        // Test offset to line/col
        assert_eq!(ctx.offset_to_line_col(0), (1, 1)); // Start of line 1
        assert_eq!(ctx.offset_to_line_col(6), (2, 1)); // Start of line 2
    }

    #[test]
    fn test_debug_context_function_at_offset() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let source = "// comment\n@main () -> void = {}";
        let path = Path::new("/src/main.ori");

        let ctx = DebugContext::new(&module, &context, config, path, source)
            .expect("debug info should be enabled");

        // Create function at offset 11 (start of @main on line 2)
        let _subprogram = ctx.create_function_at_offset("main", 11);

        ctx.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_context_scope_management() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let source = "@outer () -> void = {\n  let x = 1\n}";
        let path = Path::new("/src/test.ori");

        let ctx = DebugContext::new(&module, &context, config, path, source)
            .expect("debug info should be enabled");

        // Create function at line 1
        let subprogram = ctx.create_function_at_offset("outer", 0);

        // Enter function scope
        ctx.enter_function(subprogram);

        // Create lexical block for the body (line 2)
        let _block = ctx.create_lexical_block_at_offset(
            subprogram.as_debug_info_scope(),
            22, // Start of "let x = 1"
        );

        // Exit function scope
        ctx.exit_function();

        ctx.finalize();
    }

    #[test]
    fn test_debug_context_location_from_offset() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let source = "let x = 42\nlet y = x + 1";
        let path = Path::new("/src/test.ori");

        let ctx = DebugContext::new(&module, &context, config, path, source)
            .expect("debug info should be enabled");

        // Create a function
        let fn_type = context.void_type().fn_type(&[], false);
        let fn_val = module.add_function("test_func", fn_type, None);
        let subprogram = ctx.create_function_at_offset("test_func", 0);
        fn_val.set_subprogram(subprogram);

        // Create entry block
        let entry = context.append_basic_block(fn_val, "entry");
        let ir_builder = context.create_builder();
        ir_builder.position_at_end(entry);

        // Enter function scope
        ctx.enter_function(subprogram);

        // Set location at offset 0 (line 1, col 1)
        ctx.set_location_from_offset_in_current_scope(&ir_builder, 0);

        // Build an instruction with this location
        ir_builder.build_return(None).unwrap();

        ctx.exit_function();
        ctx.finalize();

        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_context_disabled() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::None);

        let source = "let x = 1";
        let path = Path::new("/src/test.ori");

        let ctx = DebugContext::new(&module, &context, config, path, source);
        assert!(ctx.is_none());
    }

    // -- Type debug info tests --

    #[test]
    fn test_debug_struct_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().as_type();
        let float_ty = builder.float_type().as_type();

        // Create Point { x: int, y: float }
        let fields = [
            FieldInfo {
                name: "x",
                ty: int_ty,
                size_bits: 64,
                offset_bits: 0,
                line: 1,
            },
            FieldInfo {
                name: "y",
                ty: float_ty,
                size_bits: 64,
                offset_bits: 64,
                line: 2,
            },
        ];

        let _struct_ty = builder.create_struct_type("Point", 1, 128, 64, &fields);

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_enum_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let byte_ty = builder.byte_type().as_type();

        // Create Color enum
        let _enum_ty = builder.create_enum_type(
            "Color",
            1,
            8,
            8,
            &[("Red", 0), ("Green", 1), ("Blue", 2)],
            byte_ty,
        );

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_pointer_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().as_type();
        let _ptr_ty = builder.create_pointer_type("*int", int_ty, 64);

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_array_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().as_type();
        // [int; 10] - array of 10 ints, each 64 bits = 640 bits total
        let _array_ty = builder.create_array_type(int_ty, 10, 640, 64);

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_typedef() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().as_type();
        // type UserId = int
        let _typedef = builder.create_typedef("UserId", int_ty, 1, 64);

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_string_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let _str_ty = builder.string_type();

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_option_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().as_type();
        let _option_ty = builder.option_type(int_ty, 64);

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_result_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().as_type();
        let str_ty = builder.string_type().as_type();
        let _result_ty = builder.result_type(int_ty, 64, str_ty, 128);

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    #[test]
    fn test_debug_list_type() {
        let context = Context::create();
        let module = context.create_module("test");
        let config = DebugInfoConfig::new(DebugLevel::Full);

        let builder = DebugInfoBuilder::new(&module, &context, config, "test.ori", ".")
            .expect("debug info should be enabled");

        let int_ty = builder.int_type().as_type();
        let _list_ty = builder.list_type(int_ty);

        builder.finalize();
        assert!(module.verify().is_ok());
    }

    // -- Debug format tests --

    #[test]
    fn test_debug_format_for_target_linux() {
        let format = DebugFormat::for_target("x86_64-unknown-linux-gnu");
        assert!(format.is_dwarf());
        assert!(!format.is_codeview());
    }

    #[test]
    fn test_debug_format_for_target_macos() {
        let format = DebugFormat::for_target("aarch64-apple-darwin");
        assert!(format.is_dwarf());
    }

    #[test]
    fn test_debug_format_for_target_windows_msvc() {
        let format = DebugFormat::for_target("x86_64-pc-windows-msvc");
        assert!(format.is_codeview());
        assert!(!format.is_dwarf());
    }

    #[test]
    fn test_debug_format_for_target_windows_gnu() {
        // MinGW uses DWARF, not CodeView
        let format = DebugFormat::for_target("x86_64-pc-windows-gnu");
        assert!(format.is_dwarf());
    }

    #[test]
    fn test_debug_format_for_target_wasm() {
        let format = DebugFormat::for_target("wasm32-unknown-unknown");
        assert!(format.is_dwarf());
    }

    #[test]
    fn test_debug_config_for_target() {
        let config = DebugInfoConfig::for_target(DebugLevel::Full, "x86_64-pc-windows-msvc");
        assert_eq!(config.level, DebugLevel::Full);
        assert!(config.format.is_codeview());

        let config = DebugInfoConfig::for_target(DebugLevel::Full, "x86_64-unknown-linux-gnu");
        assert!(config.format.is_dwarf());
    }

    #[test]
    fn test_debug_config_needs_dsym() {
        let config = DebugInfoConfig::new(DebugLevel::Full).with_split_debug_info(true);

        assert!(config.needs_dsym("aarch64-apple-darwin"));
        assert!(config.needs_dsym("x86_64-apple-darwin"));
        assert!(!config.needs_dsym("x86_64-unknown-linux-gnu"));
        assert!(!config.needs_dsym("x86_64-pc-windows-msvc"));

        // Without split debug, no dSYM needed
        let config = DebugInfoConfig::new(DebugLevel::Full);
        assert!(!config.needs_dsym("aarch64-apple-darwin"));
    }

    #[test]
    fn test_debug_config_needs_pdb() {
        let config = DebugInfoConfig::for_target(DebugLevel::Full, "x86_64-pc-windows-msvc");

        assert!(config.needs_pdb("x86_64-pc-windows-msvc"));
        assert!(!config.needs_pdb("x86_64-pc-windows-gnu"));
        assert!(!config.needs_pdb("x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn test_debug_config_with_profiling() {
        let config = DebugInfoConfig::new(DebugLevel::Full).with_profiling(true);

        assert!(config.debug_info_for_profiling);
    }

    #[test]
    fn test_debug_config_release_with_debug() {
        let config = DebugInfoConfig::release_with_debug();

        assert_eq!(config.level, DebugLevel::LineTablesOnly);
        assert!(config.optimized);
        assert!(config.split_debug_info);
    }
}
