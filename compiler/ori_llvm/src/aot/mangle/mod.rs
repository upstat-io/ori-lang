//! Symbol Mangling for AOT Compilation
//!
//! Provides a mangling scheme for Ori symbols to ensure unique, linkable names
//! in object files across all target platforms.
//!
//! # Mangling Scheme
//!
//! The Ori mangling scheme follows a structured format:
//!
//! ```text
//! _ori_<module>_<function>[_<suffix>]
//! ```
//!
//! Where:
//! - `_ori_` is the prefix identifying Ori symbols
//! - `<module>` is the module path with `/` replaced by `$`
//! - `<function>` is the function name
//! - `<suffix>` is optional type information for overloads
//!
//! # Examples
//!
//! | Ori Symbol | Mangled Name |
//! |------------|--------------|
//! | `@main` in root | `_ori_main` |
//! | `@add` in `math` | `_ori_math$add` |
//! | `@process` in `data/utils` | `_ori_data$utils$process` |
//! | `impl int: Eq` | `_ori_int$$Eq$equals` |
//! | `extend [int]` | `_ori_list_int_$$ext$count` |
//!
//! # Usage
//!
//! ```ignore
//! use ori_llvm::aot::mangle::{Mangler, demangle};
//!
//! let mangler = Mangler::new();
//!
//! // Simple function
//! let mangled = mangler.mangle_function("", "main");
//! assert_eq!(mangled, "_ori_main");
//!
//! // Module function
//! let mangled = mangler.mangle_function("math", "add");
//! assert_eq!(mangled, "_ori_math$add");
//!
//! // Demangle (Ori-style output)
//! let demangled = demangle("_ori_math$add");
//! assert_eq!(demangled, Some("math.@add".to_string()));
//! ```
//!
//! # Submodules
//!
//! - [`emit`] — `Mangler` struct and all mangle methods
//! - [`parse`] — `demangle()` function and `DemangleParser`

mod emit;
mod parse;

pub use emit::Mangler;
pub use parse::demangle;

/// The prefix for all Ori mangled symbols.
pub const MANGLE_PREFIX: &str = "_ori_";

/// Separator for module path components.
pub(crate) const MODULE_SEP: char = '$';

/// Separator for trait implementations.
pub(crate) const TRAIT_SEP: &str = "$$";

/// Marker for extension methods.
pub(crate) const EXT_MARKER: &str = "$$ext$";

/// Check if a symbol name is a mangled Ori symbol.
#[must_use]
pub fn is_ori_symbol(name: &str) -> bool {
    name.starts_with(MANGLE_PREFIX)
}

/// Extract just the function name from a mangled symbol (without module path).
#[must_use]
pub fn extract_function_name(mangled: &str) -> Option<&str> {
    let rest = mangled.strip_prefix(MANGLE_PREFIX)?;

    // Find the last module separator
    if let Some(pos) = rest.rfind(MODULE_SEP) {
        // Skip trait/extension markers
        let after_sep = &rest[pos + 1..];
        if after_sep.starts_with('$') {
            // This is a special marker, not a function name
            None
        } else {
            Some(after_sep)
        }
    } else {
        // No separator - the whole thing is the function name
        Some(rest)
    }
}
