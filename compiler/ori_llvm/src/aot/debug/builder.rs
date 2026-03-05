//! `DebugInfoBuilder` struct definition, constructor, and type-creation methods.

use std::cell::RefCell;
use std::path::Path;

use inkwell::context::Context;
use inkwell::debug_info::{
    DIBasicType, DICompileUnit, DIFile, DIFlags, DIFlagsConstants, DIScope, DIType,
    DWARFSourceLanguage, DebugInfoBuilder as InkwellDIBuilder,
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
}
