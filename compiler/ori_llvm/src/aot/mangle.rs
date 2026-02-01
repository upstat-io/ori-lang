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
//! | `impl Eq for int` | `_ori_int$$Eq$equals` |
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
//! // Demangle
//! let demangled = demangle("_ori_math$add");
//! assert_eq!(demangled, Some("math::add".to_string()));
//! ```

use std::fmt::Write;

/// The prefix for all Ori mangled symbols.
pub const MANGLE_PREFIX: &str = "_ori_";

/// Separator for module path components.
const MODULE_SEP: char = '$';

/// Separator for trait implementations.
const TRAIT_SEP: &str = "$$";

/// Marker for extension methods.
const EXT_MARKER: &str = "$$ext$";

/// Symbol mangler for generating unique linker names.
#[derive(Debug, Clone, Default)]
pub struct Mangler {
    /// Whether to use Windows-style decorated names (no leading underscore on some platforms).
    /// Reserved for future use when Windows-specific mangling is needed.
    #[allow(dead_code)]
    windows_compat: bool,
}

impl Mangler {
    /// Create a new mangler with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a mangler for Windows targets (affects name decoration).
    #[must_use]
    pub fn for_windows() -> Self {
        Self {
            windows_compat: true,
        }
    }

    /// Mangle a simple function name.
    ///
    /// # Arguments
    ///
    /// * `module_path` - The module path (e.g., "math", "data/utils"), empty for root
    /// * `function_name` - The function name (e.g., "add", "main")
    ///
    /// # Returns
    ///
    /// The mangled symbol name suitable for object file emission.
    #[must_use]
    pub fn mangle_function(&self, module_path: &str, function_name: &str) -> String {
        let mut result = String::with_capacity(64);
        result.push_str(MANGLE_PREFIX);

        if !module_path.is_empty() {
            self.encode_module_path(&mut result, module_path);
            result.push(MODULE_SEP);
        }

        self.encode_identifier(&mut result, function_name);
        result
    }

    /// Mangle a trait method implementation.
    ///
    /// # Arguments
    ///
    /// * `type_name` - The implementing type (e.g., "int", "Point")
    /// * `trait_name` - The trait name (e.g., "Eq", "Clone")
    /// * `method_name` - The method name (e.g., "equals", "clone")
    ///
    /// # Returns
    ///
    /// The mangled symbol name for the trait implementation.
    #[must_use]
    pub fn mangle_trait_impl(
        &self,
        type_name: &str,
        trait_name: &str,
        method_name: &str,
    ) -> String {
        let mut result = String::with_capacity(64);
        result.push_str(MANGLE_PREFIX);

        self.encode_identifier(&mut result, type_name);
        result.push_str(TRAIT_SEP);
        self.encode_identifier(&mut result, trait_name);
        result.push(MODULE_SEP);
        self.encode_identifier(&mut result, method_name);

        result
    }

    /// Mangle an extension method.
    ///
    /// # Arguments
    ///
    /// * `type_name` - The extended type (e.g., "[int]", "str")
    /// * `method_name` - The extension method name
    /// * `module_path` - The module where the extension is defined
    ///
    /// # Returns
    ///
    /// The mangled symbol name for the extension method.
    #[must_use]
    pub fn mangle_extension(
        &self,
        type_name: &str,
        method_name: &str,
        module_path: &str,
    ) -> String {
        let mut result = String::with_capacity(64);
        result.push_str(MANGLE_PREFIX);

        // Encode the type name (with special handling for collections)
        self.encode_type_name(&mut result, type_name);
        result.push_str(EXT_MARKER);

        if !module_path.is_empty() {
            self.encode_module_path(&mut result, module_path);
            result.push(MODULE_SEP);
        }

        self.encode_identifier(&mut result, method_name);
        result
    }

    /// Mangle a generic function instantiation.
    ///
    /// # Arguments
    ///
    /// * `module_path` - The module path
    /// * `function_name` - The function name
    /// * `type_args` - The type arguments for this instantiation
    ///
    /// # Returns
    ///
    /// The mangled symbol name for the specific instantiation.
    #[must_use]
    pub fn mangle_generic(
        &self,
        module_path: &str,
        function_name: &str,
        type_args: &[&str],
    ) -> String {
        let mut result = self.mangle_function(module_path, function_name);

        if !type_args.is_empty() {
            result.push_str("$G");
            for (i, type_arg) in type_args.iter().enumerate() {
                if i > 0 {
                    result.push('_');
                }
                self.encode_type_name(&mut result, type_arg);
            }
        }

        result
    }

    /// Mangle an associated function (no `self` parameter).
    ///
    /// # Arguments
    ///
    /// * `type_name` - The type name (e.g., "Option", "Result")
    /// * `function_name` - The associated function name (e.g., "new", "from")
    ///
    /// # Returns
    ///
    /// The mangled symbol name.
    #[must_use]
    pub fn mangle_associated_function(&self, type_name: &str, function_name: &str) -> String {
        let mut result = String::with_capacity(64);
        result.push_str(MANGLE_PREFIX);
        self.encode_type_name(&mut result, type_name);
        result.push_str("$A$");
        self.encode_identifier(&mut result, function_name);
        result
    }

    // -- Internal encoding helpers --
    // Note: These take &self for future extensibility (e.g., windows_compat flag)
    // even though they don't currently use instance state.

    /// Encode a module path, replacing path separators.
    #[allow(clippy::unused_self)]
    fn encode_module_path(&self, out: &mut String, path: &str) {
        for c in path.chars() {
            match c {
                '/' | '\\' | '.' | ':' => out.push(MODULE_SEP),
                c if c.is_alphanumeric() || c == '_' => out.push(c),
                _ => {
                    // Escape other characters as hex
                    let _ = write!(out, "${:02x}", c as u32);
                }
            }
        }
    }

    /// Encode an identifier (function/type name).
    #[allow(clippy::unused_self)]
    fn encode_identifier(&self, out: &mut String, name: &str) {
        for c in name.chars() {
            match c {
                c if c.is_alphanumeric() || c == '_' => out.push(c),
                '<' => out.push_str("$LT"),
                '>' => out.push_str("$GT"),
                ',' => out.push_str("$C"),
                ' ' => out.push('_'),
                '[' => out.push_str("$LB"),
                ']' => out.push_str("$RB"),
                '(' => out.push_str("$LP"),
                ')' => out.push_str("$RP"),
                ':' => out.push_str("$CC"),
                '-' => out.push_str("$D"),
                _ => {
                    let _ = write!(out, "${:02x}", c as u32);
                }
            }
        }
    }

    /// Encode a type name with special handling for compound types.
    #[allow(clippy::unused_self)]
    fn encode_type_name(&self, out: &mut String, type_name: &str) {
        // Handle common collection types
        let encoded = match type_name {
            "int" => "int",
            "float" => "float",
            "bool" => "bool",
            "str" => "str",
            "char" => "char",
            "byte" => "byte",
            "void" => "void",
            "Never" => "Never",
            _ => {
                // For complex types, encode the full name
                self.encode_identifier(out, type_name);
                return;
            }
        };
        out.push_str(encoded);
    }
}

/// Demangle an Ori symbol name back to its original form.
///
/// # Arguments
///
/// * `mangled` - The mangled symbol name
///
/// # Returns
///
/// The demangled name in Ori syntax, or `None` if not a valid Ori symbol.
#[must_use]
pub fn demangle(mangled: &str) -> Option<String> {
    // Check prefix
    let rest = mangled.strip_prefix(MANGLE_PREFIX)?;

    let mut result = String::with_capacity(mangled.len());
    let mut chars = rest.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            MODULE_SEP => {
                // Check for escape sequences (multi-character escapes start with $)
                match chars.peek().copied() {
                    Some('$') => {
                        // Trait separator $$
                        chars.next();
                        result.push_str("::");
                    }
                    Some('L') => {
                        chars.next();
                        match chars.next() {
                            Some('T') => result.push('<'),
                            Some('B') => result.push('['),
                            Some('P') => result.push('('),
                            _ => result.push_str("$L"),
                        }
                    }
                    Some('G') => {
                        // Generic marker $G
                        chars.next();
                        result.push('<');
                    }
                    Some('R') => {
                        chars.next();
                        match chars.next() {
                            Some('B') => result.push(']'),
                            Some('P') => result.push(')'),
                            _ => result.push_str("$R"),
                        }
                    }
                    Some('C') => {
                        chars.next();
                        match chars.peek() {
                            Some('C') => {
                                chars.next();
                                result.push_str("::");
                            }
                            _ => result.push(','),
                        }
                    }
                    Some('D') => {
                        chars.next();
                        result.push('-');
                    }
                    Some('A') => {
                        // Associated function marker $A$
                        chars.next();
                        if chars.peek() == Some(&'$') {
                            chars.next();
                            result.push('.');
                        }
                    }
                    Some('e') => {
                        // Could be extension marker $ext$ or just a regular separator before 'e'
                        // Peek ahead to check for 'xt$'
                        // Since peeking multiple chars is tricky, just treat as separator
                        // Extension markers are $$ext$ which starts with $$, handled above
                        result.push_str("::");
                    }
                    _ => {
                        // Plain module separator $ -> ::
                        result.push_str("::");
                    }
                }
            }
            '_' => {
                // Underscore in generic type separator becomes ", "
                if result.ends_with('<') || result.ends_with(", ") {
                    // Already in generic context, this is type separator
                    result.push_str(", ");
                } else {
                    result.push('_');
                }
            }
            c => result.push(c),
        }
    }

    // Clean up trailing generic bracket if needed
    if result.contains('<') && !result.contains('>') {
        result.push('>');
    }

    Some(result)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mangle_simple_function() {
        let mangler = Mangler::new();
        assert_eq!(mangler.mangle_function("", "main"), "_ori_main");
        assert_eq!(mangler.mangle_function("", "add"), "_ori_add");
    }

    #[test]
    fn test_mangle_module_function() {
        let mangler = Mangler::new();
        assert_eq!(mangler.mangle_function("math", "add"), "_ori_math$add");
        assert_eq!(
            mangler.mangle_function("data/utils", "process"),
            "_ori_data$utils$process"
        );
        assert_eq!(
            mangler.mangle_function("std.io", "read"),
            "_ori_std$io$read"
        );
    }

    #[test]
    fn test_mangle_trait_impl() {
        let mangler = Mangler::new();
        assert_eq!(
            mangler.mangle_trait_impl("int", "Eq", "equals"),
            "_ori_int$$Eq$equals"
        );
        assert_eq!(
            mangler.mangle_trait_impl("Point", "Clone", "clone"),
            "_ori_Point$$Clone$clone"
        );
    }

    #[test]
    fn test_mangle_extension() {
        let mangler = Mangler::new();
        assert_eq!(
            mangler.mangle_extension("[int]", "sum", ""),
            "_ori_$LBint$RB$$ext$sum"
        );
        assert_eq!(
            mangler.mangle_extension("str", "to_upper", "string_utils"),
            "_ori_str$$ext$string_utils$to_upper"
        );
    }

    #[test]
    fn test_mangle_generic() {
        let mangler = Mangler::new();
        assert_eq!(
            mangler.mangle_generic("", "identity", &["int"]),
            "_ori_identity$Gint"
        );
        assert_eq!(
            mangler.mangle_generic("", "map", &["int", "str"]),
            "_ori_map$Gint_str"
        );
    }

    #[test]
    fn test_mangle_associated_function() {
        let mangler = Mangler::new();
        assert_eq!(
            mangler.mangle_associated_function("Option", "some"),
            "_ori_Option$A$some"
        );
        assert_eq!(
            mangler.mangle_associated_function("Result", "ok"),
            "_ori_Result$A$ok"
        );
    }

    #[test]
    fn test_demangle_simple() {
        assert_eq!(demangle("_ori_main"), Some("main".to_string()));
        assert_eq!(demangle("_ori_add"), Some("add".to_string()));
    }

    #[test]
    fn test_demangle_module() {
        assert_eq!(demangle("_ori_math$add"), Some("math::add".to_string()));
        assert_eq!(
            demangle("_ori_data$utils$process"),
            Some("data::utils::process".to_string())
        );
    }

    #[test]
    fn test_demangle_trait_impl() {
        assert_eq!(
            demangle("_ori_int$$Eq$equals"),
            Some("int::Eq::equals".to_string())
        );
    }

    #[test]
    fn test_demangle_not_ori_symbol() {
        assert_eq!(demangle("_ZN3foo3barE"), None);
        assert_eq!(demangle("printf"), None);
        assert_eq!(demangle(""), None);
    }

    #[test]
    fn test_is_ori_symbol() {
        assert!(is_ori_symbol("_ori_main"));
        assert!(is_ori_symbol("_ori_math$add"));
        assert!(!is_ori_symbol("_ZN3foo3barE"));
        assert!(!is_ori_symbol("printf"));
    }

    #[test]
    fn test_extract_function_name() {
        assert_eq!(extract_function_name("_ori_main"), Some("main"));
        assert_eq!(extract_function_name("_ori_math$add"), Some("add"));
        assert_eq!(
            extract_function_name("_ori_data$utils$process"),
            Some("process")
        );
    }

    #[test]
    fn test_mangle_special_characters() {
        let mangler = Mangler::new();
        // Generic types
        assert_eq!(
            mangler.mangle_function("", "Option<int>"),
            "_ori_Option$LTint$GT"
        );
        // Array types
        assert_eq!(mangler.mangle_function("", "[int]"), "_ori_$LBint$RB");
    }

    #[test]
    fn test_roundtrip() {
        let mangler = Mangler::new();

        // Test that demangling produces readable output
        let cases = [("", "main"), ("math", "add"), ("std.io", "read")];

        for (module, func) in cases {
            let mangled = mangler.mangle_function(module, func);
            let demangled = demangle(&mangled).expect("should demangle");
            // The demangled form should contain the function name
            assert!(
                demangled.contains(func),
                "demangled '{}' should contain '{}'",
                demangled,
                func
            );
        }
    }

    #[test]
    fn test_mangler_for_windows() {
        let mangler = Mangler::for_windows();
        // Windows mangler should still produce valid output
        assert_eq!(mangler.mangle_function("", "main"), "_ori_main");
    }
}
