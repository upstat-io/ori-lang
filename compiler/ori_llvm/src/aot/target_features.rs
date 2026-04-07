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
    // WASI Preview1 (modern Rust 1.78+ canonical naming).
    // The historical 2-component spelling `wasm32-wasi` was deprecated by
    // Rust upstream in May 2024 in favor of `wasm32-wasip1` because the
    // upstream WASI specification fragmented (Preview1 vs Preview2). Ori
    // uses LLVM canonical 3-component triples; this is `unknown` vendor +
    // `wasip1` os. Ori's Preview1 codegen targets `wasi_snapshot_preview1`
    // imports (see `aot/wasm/wasi.rs`).
    "wasm32-unknown-wasip1",
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
            | "wasm32-unknown-wasip1"
    )
}

/// Canonical CPU architecture.
///
/// Parsed from target triples with all known alias spellings normalized at
/// the boundary: `arm64 → Aarch64`, `amd64 → X86_64`, `i486|i586|i686 → I386`.
/// This is the single source of truth for arch identity — comparisons, cross
/// detection, and code-model decisions operate on `Arch`, never on raw strings.
///
/// The bug class "raw arch string compare across an aliased namespace" is
/// prevented at compile time by making the canonical type un-bypassable.
/// See and the TPR prior-art pass documented in
/// `plans/bug-tracker/fix-BUG-04-045.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arch {
    /// 64-bit x86 (`x86_64`, `amd64`).
    X86_64,
    /// 32-bit x86 (`i386`, `i486`, `i586`, `i686`).
    I386,
    /// 64-bit ARM (`aarch64`, `arm64`). Apple platforms historically spell
    /// this `arm64`; the spec canonical spelling is `aarch64`.
    Aarch64,
    /// 32-bit WebAssembly (`wasm32`).
    Wasm32,
    /// 64-bit WebAssembly (`wasm64`).
    Wasm64,
}

impl Arch {
    /// Parse an arch name as it may appear in an LLVM target triple.
    ///
    /// Accepts every known alias and normalizes to the canonical variant.
    /// Returns `None` for architectures Ori does not support.
    #[must_use]
    pub fn parse_llvm_name(name: &str) -> Option<Self> {
        match name {
            "x86_64" | "amd64" => Some(Self::X86_64),
            "i386" | "i486" | "i586" | "i686" => Some(Self::I386),
            "aarch64" | "arm64" => Some(Self::Aarch64),
            "wasm32" => Some(Self::Wasm32),
            "wasm64" => Some(Self::Wasm64),
            _ => None,
        }
    }

    /// The canonical (spec) display name for this architecture.
    ///
    /// Used for display, cross-toolchain naming, and boundary emission.
    /// Always the spec spelling, never an LLVM/Apple alias.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::I386 => "i386",
            Self::Aarch64 => "aarch64",
            Self::Wasm32 => "wasm32",
            Self::Wasm64 => "wasm64",
        }
    }

    /// Pointer size in bytes for this architecture.
    #[must_use]
    pub fn pointer_size_bytes(self) -> u32 {
        match self {
            Self::X86_64 | Self::Aarch64 | Self::Wasm64 => 8,
            Self::I386 | Self::Wasm32 => 4,
        }
    }

    /// Is this a 64-bit non-WebAssembly architecture?
    ///
    /// Used by syslib path discovery to decide whether to include `lib64`
    /// directories in the search path.
    #[must_use]
    pub fn is_64_bit_non_wasm(self) -> bool {
        matches!(self, Self::X86_64 | Self::Aarch64)
    }

    /// Is this a WebAssembly architecture (wasm32 or wasm64)?
    #[must_use]
    pub fn is_wasm(self) -> bool {
        matches!(self, Self::Wasm32 | Self::Wasm64)
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

/// Host operating system family, determined at compile time via `cfg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostOs {
    Linux,
    Darwin,
    Windows,
    /// Ori compiler does not officially build on other hosts; this variant
    /// exists only as a safe fallback for unrecognized `cfg(target_os)`.
    Unknown,
}

impl HostOs {
    /// The host OS the compiler is running on, selected at compile time.
    #[must_use]
    pub const fn current() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
        #[cfg(target_os = "macos")]
        {
            Self::Darwin
        }
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Self::Unknown
        }
    }

    /// The spec spelling of this OS, used for cross-target equality checks.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Darwin => "darwin",
            Self::Windows => "windows",
            Self::Unknown => "unknown",
        }
    }
}

/// A typed host platform — the combination of arch and OS used for
/// host-vs-target comparisons.
///
/// All cross-compilation detection flows through a `HostPlatform` rather
/// than ad-hoc `cfg!` string comparisons, ensuring a single canonical home
/// for host identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HostPlatform {
    pub arch: Arch,
    pub os: HostOs,
}

impl HostPlatform {
    /// Construct a host platform explicitly. Used by tests to simulate a
    /// host that isn't the one the compiler is currently running on.
    #[must_use]
    pub const fn new(arch: Arch, os: HostOs) -> Self {
        Self { arch, os }
    }

    /// The host platform the compiler is running on, selected at compile time.
    #[must_use]
    pub const fn current() -> Self {
        Self::new(host_arch(), HostOs::current())
    }
}

/// Detect the host CPU architecture at compile time.
///
/// Ori's compiler builds on `x86_64` / `aarch64` / `i386` hosts; any other
/// host is unsupported and falls back to [`Arch::X86_64`] (unreachable in
/// practice).
const fn host_arch() -> Arch {
    #[cfg(target_arch = "x86_64")]
    {
        Arch::X86_64
    }
    #[cfg(target_arch = "aarch64")]
    {
        Arch::Aarch64
    }
    #[cfg(target_arch = "x86")]
    {
        Arch::I386
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86")))]
    {
        Arch::X86_64
    }
}

/// Parsed components of a target triple.
///
/// The `arch` field is a typed [`Arch`] enum, parsed and canonicalized at the
/// `parse()` boundary. All host-vs-target equality checks operate on the
/// typed field — raw string compares against `arch` are a compile error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetTripleComponents {
    /// CPU architecture (canonical; aliases normalized at parse time).
    pub arch: Arch,
    /// Hardware vendor (e.g., `unknown`, `apple`, `pc`).
    pub vendor: String,
    /// Operating system (e.g., `linux`, `darwin`, `windows`, `wasi`).
    /// May include a version suffix for darwin (e.g., `darwin25.2.0`).
    pub os: String,
    /// Environment/ABI (e.g., `gnu`, `musl`, `msvc`) — optional.
    pub env: Option<String>,
}

impl TargetTripleComponents {
    /// Parse a target triple string into components.
    ///
    /// Format: `<arch>-<vendor>-<os>[-<env>]`. The `arch` is normalized to
    /// the canonical [`Arch`] variant at the boundary — `arm64` becomes
    /// [`Arch::Aarch64`], `amd64` becomes [`Arch::X86_64`], etc.
    pub fn parse(triple: &str) -> Result<Self, TargetError> {
        let parts: Vec<&str> = triple.split('-').collect();

        if parts.len() < 3 {
            return Err(TargetError::InvalidTripleFormat {
                triple: triple.to_string(),
                reason: "expected at least 3 components: <arch>-<vendor>-<os>".to_string(),
            });
        }

        let arch =
            Arch::parse_llvm_name(parts[0]).ok_or_else(|| TargetError::InvalidTripleFormat {
                triple: triple.to_string(),
                reason: format!("unknown architecture '{}'", parts[0]),
            })?;

        Ok(Self {
            arch,
            vendor: parts[1].to_string(),
            os: parts[2].to_string(),
            env: parts.get(3).map(|s| (*s).to_string()),
        })
    }

    /// Check if this is a WebAssembly target.
    #[must_use]
    pub fn is_wasm(&self) -> bool {
        self.arch.is_wasm()
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

    /// Is this triple a cross-compilation target for the given host?
    ///
    /// Operates on typed `Arch` / `HostOs` values — never on raw strings.
    /// Darwin OS version suffixes (e.g., `darwin25.2.0`) are handled by
    /// delegating the OS check to [`Self::is_macos`], which is the
    /// canonical "is this a macOS variant" query.
    #[must_use]
    pub fn is_cross_for(&self, host: HostPlatform) -> bool {
        if self.arch != host.arch {
            return true;
        }
        // Use typed `is_macos()` rather than raw `starts_with("darwin")`
        // so the OS-suffix-stripping rule lives in exactly one place.
        if self.is_macos() {
            return host.os != HostOs::Darwin;
        }
        self.os != host.os.display_name()
    }

    /// Is this triple the native target for the given host?
    #[must_use]
    pub fn is_native_for(&self, host: HostPlatform) -> bool {
        !self.is_cross_for(host)
    }

    /// Canonical key used to look this triple up in `SUPPORTED_TARGETS`.
    ///
    /// Returns the form `<canonical-arch>-<vendor>-<canonical-os>[-<env>]`,
    /// where:
    /// - The arch is the spec spelling ([`Arch::display_name`]) — already
    ///   normalized at parse time.
    /// - The OS is canonicalized via the typed [`Self::is_macos`] predicate:
    ///   any darwin-family OS (`darwin`, `darwin25.2.0`, `macos`) maps to
    ///   the bare `darwin` token. This is the SSOT for "what string does the
    ///   support-targets list use?" — versioned darwin spellings parsed from
    ///   LLVM's default triple (e.g., `arm64-apple-darwin25.2.0` on Apple
    ///   Silicon) match the unversioned `aarch64-apple-darwin` entry.
    ///
    /// Distinct from [`Self::to_string`], which preserves the OS version
    /// suffix for downstream consumers (LLVM `TargetMachine`) that need
    /// the exact LLVM-emitted spelling.
    #[must_use]
    pub fn support_key(&self) -> String {
        let arch = self.arch.display_name();
        let os: &str = if self.is_macos() {
            "darwin"
        } else {
            self.os.as_str()
        };
        match &self.env {
            Some(env) => format!("{arch}-{}-{os}-{env}", self.vendor),
            None => format!("{arch}-{}-{os}", self.vendor),
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
///
/// Exhaustive match on the typed [`Arch`] — no string fallthrough, so a new
/// `Arch` variant is a compile-time error until every initializer is wired
/// up.
pub(crate) fn initialize_target_for_triple(
    components: &TargetTripleComponents,
) -> Result<(), TargetError> {
    match components.arch {
        Arch::X86_64 | Arch::I386 => {
            X86_TARGET_INIT.call_once(|| {
                Target::initialize_x86(&InitializationConfig::default());
            });
        }
        Arch::Aarch64 => {
            AARCH64_TARGET_INIT.call_once(|| {
                Target::initialize_aarch64(&InitializationConfig::default());
            });
        }
        Arch::Wasm32 | Arch::Wasm64 => {
            WASM_TARGET_INIT.call_once(|| {
                Target::initialize_webassembly(&InitializationConfig::default());
            });
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

#[cfg(test)]
mod tests;
