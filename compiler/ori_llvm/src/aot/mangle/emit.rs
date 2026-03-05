//! Mangling — symbol name emission.
//!
//! Contains the [`Mangler`] struct and all `mangle_*` methods that produce
//! linker-safe symbol names from Ori identifiers.

use std::fmt::Write;

use super::{EXT_MARKER, MANGLE_PREFIX, MODULE_SEP, TRAIT_SEP};

/// Symbol mangler for generating unique linker names.
#[derive(Debug, Clone, Default)]
pub struct Mangler {
    /// Whether to use Windows-style decorated names (no leading underscore on some platforms).
    /// Reserved for future use when Windows-specific mangling is needed.
    #[expect(
        dead_code,
        reason = "Reserved for future Windows-specific mangling; used in for_windows constructor"
    )]
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

    /// Mangle an inherent method (method defined directly on a type, not via trait).
    ///
    /// # Arguments
    ///
    /// * `module_path` - The module path (empty for root module)
    /// * `type_name` - The type name (e.g., "Point", "Line")
    /// * `method_name` - The method name (e.g., "distance", "length")
    ///
    /// # Returns
    ///
    /// The mangled symbol name: `_ori_[<module>$]<type>$<method>`.
    #[must_use]
    pub fn mangle_method(&self, module_path: &str, type_name: &str, method_name: &str) -> String {
        let mut result = String::with_capacity(64);
        result.push_str(MANGLE_PREFIX);

        if !module_path.is_empty() {
            self.encode_module_path(&mut result, module_path);
            result.push(MODULE_SEP);
        }

        self.encode_type_name(&mut result, type_name);
        result.push(MODULE_SEP);
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
    #[allow(
        clippy::unused_self,
        reason = "API consistency with other encode methods; future platform-specific encoding"
    )]
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
    #[allow(
        clippy::unused_self,
        reason = "API consistency with other encode methods; future platform-specific encoding"
    )]
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
    #[allow(
        clippy::unused_self,
        reason = "API consistency with other encode methods; future platform-specific encoding"
    )]
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
