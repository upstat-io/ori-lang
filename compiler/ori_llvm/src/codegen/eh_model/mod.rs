//! Exception handling model selection.
//!
//! Different platforms use different unwinding conventions:
//! - **Itanium**: Linux, macOS, MinGW — `landingpad` / `resume`
//! - **SEH**: Windows MSVC — `catchswitch` / `catchpad` / `cleanuppad`
//!
//! The model determines which personality function is used and how
//! unwind blocks are emitted in the ARC IR → LLVM IR pipeline.

/// Exception handling model for the target platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EhModel {
    /// Itanium EH ABI (Linux, macOS, MinGW).
    ///
    /// Uses `landingpad` / `resume` instructions with
    /// `rust_eh_personality` as the personality function.
    Itanium,

    /// Windows Structured Exception Handling (MSVC).
    ///
    /// Uses `catchswitch` / `catchpad` / `cleanuppad` with
    /// `__CxxFrameHandler3` as the personality function.
    /// All calls inside funclet pads require operand bundles.
    Seh,
}

impl EhModel {
    /// Detect the EH model from a target triple string.
    ///
    /// Returns `Seh` for `*-windows-msvc` targets, `Itanium` for everything else
    /// (including `*-windows-gnu` which uses Itanium-style unwinding).
    pub fn from_triple(triple: &str) -> Self {
        if triple.contains("windows-msvc") {
            Self::Seh
        } else {
            Self::Itanium
        }
    }

    /// The personality function name for this EH model.
    pub fn personality_name(&self) -> &'static str {
        match self {
            Self::Itanium => "rust_eh_personality",
            Self::Seh => "__CxxFrameHandler3",
        }
    }
}

#[cfg(test)]
mod tests;
