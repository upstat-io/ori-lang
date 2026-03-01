//! Prelude registration — built-in functions, type constructors, and enum variants.
//!
//! Extracted from `interpreter/mod.rs` to keep the main module focused on
//! interpreter structure and core dispatch.

use crate::Value;

use super::Interpreter;

impl Interpreter<'_> {
    /// Register a `function_val` (type conversion function).
    pub fn register_function_val(
        &mut self,
        name: &str,
        func: crate::FunctionValFn,
        display_name: &'static str,
    ) {
        let name = self.interner.intern(name);
        self.env
            .define_global(name, Value::FunctionVal(func, display_name));
    }

    /// Register all `function_val` (type conversion) functions and built-in values.
    ///
    /// Includes:
    /// - Type conversion functions like int(x), str(x), float(x) (positional args per spec)
    /// - Built-in enum variants like Less, Equal, Greater (Ordering type)
    pub fn register_prelude(&mut self) {
        use crate::{
            function_val_byte, function_val_error, function_val_float, function_val_hash_combine,
            function_val_int, function_val_repeat, function_val_str, function_val_thread_id,
        };
        tracing::debug!("registering prelude");

        // Type conversion functions (positional args allowed per spec)
        self.register_function_val("str", function_val_str, "str");
        self.register_function_val("int", function_val_int, "int");
        self.register_function_val("float", function_val_float, "float");
        self.register_function_val("byte", function_val_byte, "byte");

        // Error constructor (Traceable errors with trace storage)
        self.register_function_val("Error", function_val_error, "Error");

        // Iterator constructors
        self.register_function_val("repeat", function_val_repeat, "repeat");

        // Hash utility (wrapping arithmetic — can't be pure Ori due to overflow)
        self.register_function_val("hash_combine", function_val_hash_combine, "hash_combine");

        // Thread/parallel introspection (internal use)
        self.register_function_val("thread_id", function_val_thread_id, "thread_id");

        // Built-in Ordering enum variants (Less, Equal, Greater)
        // These are first-class Ordering values, used by compare() and comparison operators
        let less_name = self.interner.intern("Less");
        let equal_name = self.interner.intern("Equal");
        let greater_name = self.interner.intern("Greater");

        self.env.define_global(less_name, Value::ordering_less());
        self.env.define_global(equal_name, Value::ordering_equal());
        self.env
            .define_global(greater_name, Value::ordering_greater());

        // Built-in format spec enum variants (§3.16 Formattable)
        self.register_format_variants();
    }

    /// Register `Alignment`, `Sign`, and `FormatType` enum variants as globals.
    ///
    /// These unit variants are used by the `Formattable` trait's `FormatSpec` struct.
    /// Uses the generic `Value::Variant` representation (not a dedicated Value variant)
    /// since format spec types are only used in formatting, not in hot-path operators.
    fn register_format_variants(&mut self) {
        let alignment = self.interner.intern("Alignment");
        for name in ["Left", "Center", "Right"] {
            let n = self.interner.intern(name);
            self.env
                .define_global(n, Value::variant(alignment, n, vec![]));
        }

        let sign = self.interner.intern("Sign");
        for name in ["Plus", "Minus", "Space"] {
            let n = self.interner.intern(name);
            self.env.define_global(n, Value::variant(sign, n, vec![]));
        }

        let format_type = self.interner.intern("FormatType");
        for name in [
            "Binary", "Octal", "Hex", "HexUpper", "Exp", "ExpUpper", "Fixed", "Percent",
        ] {
            let n = self.interner.intern(name);
            self.env
                .define_global(n, Value::variant(format_type, n, vec![]));
        }
    }
}
