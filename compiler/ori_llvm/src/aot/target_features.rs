//! Target Triple Parsing, Validation, and CPU Feature Detection
//!
//! This module handles:
//! - Target triple parsing into components (`TargetTripleComponents`)
//! - Supported target validation (`SUPPORTED_TARGETS`, `is_supported_target`)
//! - LLVM target initialization per architecture
//! - Host CPU name and feature detection
//! - Feature string parsing and validation
//!
//! The main `TargetConfig` struct (in `target.rs`) uses these primitives
//! to configure LLVM target machines.

use std::fmt;
use std::sync::Once;

use inkwell::targets::{InitializationConfig, Target, TargetMachine};

/// Error type for target configuration operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetError {
    /// Target triple is not in the supported list.
    UnsupportedTarget {
        triple: String,
        supported: Vec<&'static str>,
    },
    /// Failed to initialize LLVM target.
    InitializationFailed(String),
    /// Failed to create target machine.
    TargetMachineCreationFailed(String),
    /// Invalid target triple format.
    InvalidTripleFormat { triple: String, reason: String },
    /// Invalid CPU name.
    InvalidCpu { cpu: String, target: String },
    /// Invalid feature specification.
    InvalidFeature { feature: String, reason: String },
}

impl fmt::Display for TargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTarget { triple, supported } => {
                write!(
                    f,
                    "unsupported target '{triple}'. Supported targets: {}",
                    supported.join(", ")
                )
            }
            Self::InitializationFailed(msg) => {
                write!(f, "failed to initialize LLVM target: {msg}")
            }
            Self::TargetMachineCreationFailed(msg) => {
                write!(f, "failed to create target machine: {msg}")
            }
            Self::InvalidTripleFormat { triple, reason } => {
                write!(f, "invalid target triple '{triple}': {reason}")
            }
            Self::InvalidCpu { cpu, target } => {
                write!(f, "invalid CPU '{cpu}' for target '{target}'")
            }
            Self::InvalidFeature { feature, reason } => {
                write!(f, "invalid feature '{feature}': {reason}")
            }
        }
    }
}

impl std::error::Error for TargetError {}

/// Supported target triples for AOT compilation.
///
/// These are the officially supported targets. Cross-compilation requires
/// the appropriate sysroot to be installed via `ori target add`.
pub const SUPPORTED_TARGETS: &[&str] = &[
    // Linux
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-gnu",
    "aarch64-unknown-linux-musl",
    // macOS
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    // Windows
    "x86_64-pc-windows-msvc",
    "x86_64-pc-windows-gnu",
    // WebAssembly
    "wasm32-unknown-unknown",
    "wasm32-wasi",
];

/// Check if a target triple is in the supported list.
///
/// Uses a match statement for O(1) lookup instead of linear scan.
#[must_use]
pub fn is_supported_target(triple: &str) -> bool {
    matches!(
        triple,
        "x86_64-unknown-linux-gnu"
            | "x86_64-unknown-linux-musl"
            | "aarch64-unknown-linux-gnu"
            | "aarch64-unknown-linux-musl"
            | "x86_64-apple-darwin"
            | "aarch64-apple-darwin"
            | "x86_64-pc-windows-msvc"
            | "x86_64-pc-windows-gnu"
            | "wasm32-unknown-unknown"
            | "wasm32-wasi"
    )
}

/// Parsed components of a target triple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetTripleComponents {
    /// CPU architecture (e.g., `x86_64`, `aarch64`, `wasm32`)
    pub arch: String,
    /// Hardware vendor (e.g., `unknown`, `apple`, `pc`)
    pub vendor: String,
    /// Operating system (e.g., `linux`, `darwin`, `windows`, `wasi`)
    pub os: String,
    /// Environment/ABI (e.g., `gnu`, `musl`, `msvc`) - optional
    pub env: Option<String>,
}

impl TargetTripleComponents {
    /// Parse a target triple string into components.
    ///
    /// Format: `<arch>-<vendor>-<os>[-<env>]`
    pub fn parse(triple: &str) -> Result<Self, TargetError> {
        let parts: Vec<&str> = triple.split('-').collect();

        if parts.len() < 3 {
            return Err(TargetError::InvalidTripleFormat {
                triple: triple.to_string(),
                reason: "expected at least 3 components: <arch>-<vendor>-<os>".to_string(),
            });
        }

        Ok(Self {
            arch: parts[0].to_string(),
            vendor: parts[1].to_string(),
            os: parts[2].to_string(),
            env: parts.get(3).map(|s| (*s).to_string()),
        })
    }

    /// Check if this is a WebAssembly target.
    #[must_use]
    pub fn is_wasm(&self) -> bool {
        self.arch == "wasm32" || self.arch == "wasm64"
    }

    /// Check if this is a Windows target.
    #[must_use]
    pub fn is_windows(&self) -> bool {
        self.os == "windows"
    }

    /// Check if this is a Windows MSVC target (uses SEH exception handling).
    #[must_use]
    pub fn is_msvc(&self) -> bool {
        self.env.as_deref() == Some("msvc")
    }

    /// Check if this is a macOS target.
    ///
    /// The OS component may include a version suffix (e.g., `darwin25.2.0`
    /// from `arm64-apple-darwin25.2.0`), so we use `starts_with`.
    #[must_use]
    pub fn is_macos(&self) -> bool {
        self.os.starts_with("darwin") || self.os == "macos"
    }

    /// Check if this is a Linux target.
    #[must_use]
    pub fn is_linux(&self) -> bool {
        self.os == "linux"
    }

    /// Get the target family (unix, windows, or wasm).
    #[must_use]
    pub fn family(&self) -> &'static str {
        if self.is_wasm() {
            "wasm"
        } else if self.is_windows() {
            "windows"
        } else {
            "unix"
        }
    }
}

impl fmt::Display for TargetTripleComponents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}-{}", self.arch, self.vendor, self.os)?;
        if let Some(env) = &self.env {
            write!(f, "-{env}")?;
        }
        Ok(())
    }
}

// -- LLVM Target Initialization --

static NATIVE_TARGET_INIT: Once = Once::new();
static X86_TARGET_INIT: Once = Once::new();
static AARCH64_TARGET_INIT: Once = Once::new();
static WASM_TARGET_INIT: Once = Once::new();

/// Initialize the native LLVM target.
///
/// Safe to call multiple times; initialization happens once.
pub(crate) fn initialize_native_target() -> Result<(), TargetError> {
    let mut result = Ok(());

    NATIVE_TARGET_INIT.call_once(|| {
        if let Err(e) = Target::initialize_native(&InitializationConfig::default()) {
            result = Err(TargetError::InitializationFailed(e.clone()));
        }
    });

    result
}

/// Initialize LLVM targets for a given triple.
pub(crate) fn initialize_target_for_triple(
    components: &TargetTripleComponents,
) -> Result<(), TargetError> {
    match components.arch.as_str() {
        "x86_64" | "i686" | "i386" => {
            X86_TARGET_INIT.call_once(|| {
                Target::initialize_x86(&InitializationConfig::default());
            });
        }
        "aarch64" | "arm64" => {
            AARCH64_TARGET_INIT.call_once(|| {
                Target::initialize_aarch64(&InitializationConfig::default());
            });
        }
        "wasm32" | "wasm64" => {
            WASM_TARGET_INIT.call_once(|| {
                Target::initialize_webassembly(&InitializationConfig::default());
            });
        }
        arch => {
            return Err(TargetError::InitializationFailed(format!(
                "unsupported architecture: {arch}"
            )));
        }
    }

    Ok(())
}

/// Get the host CPU name as detected by LLVM.
///
/// Returns "generic" if detection fails.
pub fn get_host_cpu_name() -> String {
    TargetMachine::get_host_cpu_name().to_string()
}

/// Get the host CPU features as a comma-separated string.
///
/// Returns an empty string if detection fails.
pub fn get_host_cpu_features() -> String {
    TargetMachine::get_host_cpu_features().to_string()
}

/// Parse a features string and validate format.
///
/// Features are comma-separated with `+` to enable and `-` to disable.
/// Example: `"+avx2,+fma,-sse4.1"`
///
/// # Errors
///
/// Returns an error if a feature doesn't start with `+` or `-`.
pub fn parse_features(features: &str) -> Result<Vec<(&str, bool)>, TargetError> {
    if features.is_empty() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();

    for feature in features.split(',') {
        let feature = feature.trim();
        if feature.is_empty() {
            continue;
        }

        if let Some(name) = feature.strip_prefix('+') {
            result.push((name, true));
        } else if let Some(name) = feature.strip_prefix('-') {
            result.push((name, false));
        } else {
            return Err(TargetError::InvalidFeature {
                feature: feature.to_string(),
                reason: "feature must start with '+' (enable) or '-' (disable)".to_string(),
            });
        }
    }

    Ok(result)
}
