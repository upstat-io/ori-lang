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
//! // Demangle (Ori-style output)
//! let demangled = demangle("_ori_math$add");
//! assert_eq!(demangled, Some("math.@add".to_string()));
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
    //
    // These helpers share a common pattern:
    // 1. Alphanumeric and '_' pass through unchanged
    // 2. Special characters get named escapes (context-dependent)
    // 3. Other characters get hex-escaped via encode_char_hex
    //
    // The different methods have different special character mappings:
    // - Module paths: path separators become MODULE_SEP
    // - Identifiers: brackets, generics, etc. get named escapes

    /// Encode a character as hex escape (e.g., '@' -> "$40").
    #[inline]
    fn encode_char_hex(out: &mut String, c: char) {
        let _ = write!(out, "${:02x}", c as u32);
    }

    /// Encode a module path, replacing path separators.
    // Takes &self for API consistency and future extensibility (e.g., windows_compat
    // platform-specific encoding using self.windows_compat).
    #[allow(clippy::unused_self)]
    fn encode_module_path(&self, out: &mut String, path: &str) {
        for c in path.chars() {
            match c {
                '/' | '\\' | '.' | ':' => out.push(MODULE_SEP),
                c if c.is_alphanumeric() || c == '_' => out.push(c),
                _ => Self::encode_char_hex(out, c),
            }
        }
    }

    /// Encode an identifier (function/type name).
    // Takes &self for API consistency and future extensibility (e.g., windows_compat
    // platform-specific encoding using self.windows_compat).
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
                _ => Self::encode_char_hex(out, c),
            }
        }
    }

    /// Encode a type name with special handling for compound types.
    // Takes &self for API consistency and future extensibility (e.g., windows_compat
    // platform-specific encoding using self.windows_compat).
    #[allow(clippy::unused_self)]
    fn encode_type_name(&self, out: &mut String, type_name: &str) {
        // Primitive types are passed through unchanged for readability
        match type_name {
            "int" | "float" | "bool" | "str" | "char" | "byte" | "void" | "Never" => {
                out.push_str(type_name);
            }
            // Complex types get full identifier encoding
            _ => self.encode_identifier(out, type_name),
        }
    }
}

/// Demangle an Ori symbol name back to its original Ori-style form.
///
/// # Arguments
///
/// * `mangled` - The mangled symbol name
///
/// # Returns
///
/// The demangled name in Ori syntax (e.g., `math.@add`), or `None` if not a valid Ori symbol.
///
/// # Output Format
///
/// - Module functions: `_ori_math$add` → `math.@add`
/// - Nested modules: `_ori_http$client$connect` → `http/client.@connect`
/// - Trait impls: `_ori_int$$Eq$equals` → `int::Eq.@equals`
/// - Associated fns: `_ori_Option$A$some` → `Option.@some`
#[must_use]
pub fn demangle(mangled: &str) -> Option<String> {
    let rest = mangled.strip_prefix(MANGLE_PREFIX)?;
    let parsed = DemangleParser::parse(rest)?;
    Some(parsed.format())
}

/// Internal parser state for demangling.
struct DemangleParser {
    segments: Vec<String>,
    is_trait_impl: bool,
    is_associated: bool,
}

impl DemangleParser {
    /// Parse a mangled symbol (without prefix) into segments and flags.
    fn parse(input: &str) -> Option<Self> {
        let mut parser = Self {
            segments: Vec::new(),
            is_trait_impl: false,
            is_associated: false,
        };
        let mut current = String::new();
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                MODULE_SEP => parser.handle_separator(&mut chars, &mut current),
                '_' if current.ends_with('<') || current.ends_with(", ") => {
                    current.push_str(", ");
                }
                c => current.push(c),
            }
        }

        if !current.is_empty() {
            parser.segments.push(current);
        }

        if parser.segments.is_empty() {
            return None;
        }
        Some(parser)
    }

    /// Handle `$` separator and its escape sequences.
    fn handle_separator(
        &mut self,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        current: &mut String,
    ) {
        match chars.peek().copied() {
            Some('$') => {
                chars.next();
                self.push_segment(current);
                self.is_trait_impl = true;
            }
            Some('L') => Self::decode_left_bracket(chars, current),
            Some('G') => {
                chars.next();
                current.push('<');
            }
            Some('R') => Self::decode_right_bracket(chars, current),
            Some('C') => Self::decode_comma_or_colons(chars, current),
            Some('D') => {
                chars.next();
                current.push('-');
            }
            Some('A') => self.handle_associated_marker(chars, current),
            Some('e') => self.handle_extension_marker(chars, current),
            _ => self.push_segment(current),
        }
    }

    fn decode_left_bracket(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        current: &mut String,
    ) {
        chars.next();
        match chars.next() {
            Some('T') => current.push('<'),
            Some('B') => current.push('['),
            Some('P') => current.push('('),
            _ => current.push_str("$L"),
        }
    }

    fn decode_right_bracket(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        current: &mut String,
    ) {
        chars.next();
        match chars.next() {
            Some('B') => current.push(']'),
            Some('P') => current.push(')'),
            _ => current.push_str("$R"),
        }
    }

    fn decode_comma_or_colons(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        current: &mut String,
    ) {
        chars.next();
        if chars.peek() == Some(&'C') {
            chars.next();
            current.push_str("::");
        } else {
            current.push(',');
        }
    }

    fn handle_associated_marker(
        &mut self,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        current: &mut String,
    ) {
        chars.next();
        if chars.peek() == Some(&'$') {
            chars.next();
            self.push_segment(current);
            self.is_associated = true;
        }
    }

    fn handle_extension_marker(
        &mut self,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        current: &mut String,
    ) {
        let peek: String = chars.clone().take(3).collect();
        if peek == "xt$" {
            chars.next(); // x
            chars.next(); // t
            chars.next(); // $
            self.push_segment(current);
            self.is_trait_impl = true;
        } else {
            self.push_segment(current);
        }
    }

    fn push_segment(&mut self, current: &mut String) {
        if !current.is_empty() {
            self.segments.push(std::mem::take(current));
        }
    }

    /// Format parsed segments into Ori-style output.
    fn format(mut self) -> String {
        let mut result = String::with_capacity(self.segments.iter().map(String::len).sum());

        if self.segments.len() == 1 {
            result.push('@');
            result.push_str(&self.segments[0]);
        } else if self.is_trait_impl {
            self.format_trait_impl(&mut result);
        } else if self.is_associated {
            self.format_associated(&mut result);
        } else {
            self.format_module_function(&mut result);
        }

        if result.contains('<') && !result.contains('>') {
            result.push('>');
        }
        result
    }

    fn format_trait_impl(&mut self, result: &mut String) {
        let method = self.segments.pop().unwrap();
        let trait_name = self.segments.pop();
        let type_name = self.segments.pop();
        if let Some(tn) = type_name {
            result.push_str(&tn);
        }
        if let Some(tr) = trait_name {
            result.push_str("::");
            result.push_str(&tr);
        }
        result.push_str(".@");
        result.push_str(&method);
    }

    fn format_associated(&mut self, result: &mut String) {
        let method = self.segments.pop().unwrap();
        result.push_str(&self.segments.join("/"));
        result.push_str(".@");
        result.push_str(&method);
    }

    fn format_module_function(&mut self, result: &mut String) {
        let function = self.segments.pop().unwrap();
        if self.segments.is_empty() {
            result.push('@');
        } else {
            result.push_str(&self.segments.join("/"));
            result.push_str(".@");
        }
        result.push_str(&function);
    }
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
        assert_eq!(demangle("_ori_main"), Some("@main".to_string()));
        assert_eq!(demangle("_ori_add"), Some("@add".to_string()));
    }

    #[test]
    fn test_demangle_module() {
        // Ori-style: module.@function
        assert_eq!(demangle("_ori_math$add"), Some("math.@add".to_string()));
        // Nested modules: module/submodule.@function
        assert_eq!(
            demangle("_ori_data$utils$process"),
            Some("data/utils.@process".to_string())
        );
    }

    #[test]
    fn test_demangle_trait_impl() {
        // Trait impl: type::Trait.@method
        assert_eq!(
            demangle("_ori_int$$Eq$equals"),
            Some("int::Eq.@equals".to_string())
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

        // Test that demangling produces Ori-style readable output
        let cases = [
            ("", "main", "@main"),
            ("math", "add", "math.@add"),
            ("std.io", "read", "std/io.@read"),
        ];

        for (module, func, expected) in cases {
            let mangled_name = mangler.mangle_function(module, func);
            let demangled = demangle(&mangled_name).expect("should demangle");
            assert_eq!(
                demangled, expected,
                "demangled '{demangled}' should equal '{expected}'"
            );
        }
    }

    #[test]
    fn test_mangler_for_windows() {
        let mangler = Mangler::for_windows();
        // Windows mangler should still produce valid output
        assert_eq!(mangler.mangle_function("", "main"), "_ori_main");
    }

    #[test]
    fn test_mangle_special_characters_extended() {
        let mangler = Mangler::new();

        // Comma in generic types (e.g., Map<int, str>)
        assert!(mangler.mangle_function("", "Map<int, str>").contains("$C"));

        // Parentheses in function types
        assert!(mangler.mangle_function("", "(int) -> str").contains("$LP"));
        assert!(mangler.mangle_function("", "(int) -> str").contains("$RP"));

        // Colon in qualified paths
        assert!(mangler.mangle_function("", "Foo::Bar").contains("$CC"));

        // Dash in identifiers
        assert!(mangler.mangle_function("", "my-func").contains("$D"));

        // Space gets converted to underscore
        assert!(mangler.mangle_function("", "my func").contains('_'));
    }

    #[test]
    fn test_mangle_hex_escape() {
        let mangler = Mangler::new();

        // Characters not in the allowed set get hex-escaped
        // '@' = 0x40 = 64
        let result = mangler.mangle_function("", "foo@bar");
        assert!(result.contains("$40"));

        // '#' = 0x23 = 35
        let result = mangler.mangle_function("", "foo#bar");
        assert!(result.contains("$23"));
    }

    #[test]
    fn test_mangle_module_with_special_paths() {
        let mangler = Mangler::new();

        // Module path with backslash (Windows-style)
        let result = mangler.mangle_function("foo\\bar", "baz");
        assert_eq!(result, "_ori_foo$bar$baz");

        // Module path with colon gets encoded as module separator
        let result = mangler.mangle_function("C:/foo", "bar");
        // Colon becomes $ (module separator), so C: becomes C$
        assert!(result.contains('$'));
    }

    #[test]
    fn test_demangle_special_characters() {
        // Demangle symbols with special character encodings

        // Array type with brackets
        let demangled = demangle("_ori_$LBint$RB");
        assert!(demangled.is_some());
        assert!(demangled.as_ref().unwrap().contains('['));
        assert!(demangled.as_ref().unwrap().contains(']'));

        // Function type with parentheses
        let demangled = demangle("_ori_$LPint$RP");
        assert!(demangled.is_some());
        assert!(demangled.as_ref().unwrap().contains('('));
        assert!(demangled.as_ref().unwrap().contains(')'));

        // Comma in generics
        let demangled = demangle("_ori_Map$Cint");
        assert!(demangled.is_some());
        assert!(demangled.unwrap().contains(','));

        // Dash in identifier
        let demangled = demangle("_ori_my$Dfunc");
        assert!(demangled.is_some());
        assert!(demangled.unwrap().contains('-'));

        // Generic marker $G adds opening angle bracket
        let demangled = demangle("_ori_identity$Gint");
        assert!(demangled.is_some());
        assert!(demangled.unwrap().contains('<'));

        // Associated function marker
        let demangled = demangle("_ori_Option$A$some");
        assert!(demangled.is_some());
        assert!(demangled.unwrap().contains('.'));

        // Qualified path with double colon $CC
        let demangled = demangle("_ori_foo$CCbar");
        assert!(demangled.is_some());
        assert!(demangled.unwrap().contains("::"));

        // Open angle bracket via $LT
        let demangled = demangle("_ori_Option$LTint");
        assert!(demangled.is_some());
        assert!(demangled.unwrap().contains('<'));
    }

    #[test]
    fn test_demangle_incomplete_escapes() {
        // Test incomplete escape sequences - these should fallback gracefully

        // Incomplete $L (no following char) - falls back to $L
        let demangled = demangle("_ori_test$L");
        assert!(demangled.is_some());

        // Incomplete $R (no following char) - falls back to $R
        let demangled = demangle("_ori_test$R");
        assert!(demangled.is_some());

        // Incomplete $C with CC check
        let demangled = demangle("_ori_test$C");
        assert!(demangled.is_some());
        // Should be treated as comma
        assert!(demangled.unwrap().contains(','));
    }
}
