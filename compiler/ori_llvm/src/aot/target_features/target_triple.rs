//! Target triple component parsing and typed queries.

use std::fmt;

use super::{Arch, HostOs, HostPlatform, TargetError};

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
    /// Distinct from `ToString::to_string`, which preserves the OS version
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
