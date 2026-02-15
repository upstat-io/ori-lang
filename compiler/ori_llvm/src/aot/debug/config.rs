//! Debug info configuration types and error definitions.

use inkwell::debug_info::DWARFEmissionKind;

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
    pub(crate) fn to_emission_kind(self) -> DWARFEmissionKind {
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
pub(super) fn basic_type_creation_error(name: &str) -> DebugInfoError {
    DebugInfoError::BasicTypeCreation {
        name: name.to_string(),
    }
}
