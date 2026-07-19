//! `FormatFields` derive strategy — Debug / Printable string rendering.

use ori_ir::{DerivedMethodInfo, FormatOpen};

use super::super::Interpreter;
use crate::{EvalResult, Value};

impl Interpreter<'_> {
    /// Format struct/variant fields into a string representation.
    ///
    /// Uses `format_value_printable` (Printable) or `debug_value` (Debug)
    /// based on `include_names`.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Consistent strategy-driven dispatch signature"
    )]
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Returns EvalResult for consistent strategy-driven interface"
    )]
    pub(super) fn eval_format_fields(
        &self,
        receiver: Value,
        info: &DerivedMethodInfo,
        open: FormatOpen,
        separator: &str,
        suffix: &str,
        include_names: bool,
    ) -> EvalResult {
        let fmt = |val: &Value| -> String {
            if include_names {
                crate::methods::debug_format::debug_value(val, self.interner())
            } else {
                self.format_value_printable(val, 0)
            }
        };

        match &receiver {
            Value::Struct(struct_val) => {
                let type_name = self.interner.lookup(struct_val.type_name).to_string();
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "capacity estimation, overflow is safe"
                )]
                let capacity = type_name.len() + 4 + info.field_names.len() * 12;
                let mut result = String::with_capacity(capacity);

                result.push_str(&type_name);
                match open {
                    FormatOpen::TypeNameParen => result.push('('),
                    FormatOpen::TypeNameBrace => result.push_str(" { "),
                }

                let mut first = true;
                for field_name in &info.field_names {
                    if let Some(val) = struct_val.get_field(*field_name) {
                        if !first {
                            result.push_str(separator);
                        }
                        first = false;
                        if include_names {
                            let name_str = self.interner.lookup(*field_name);
                            result.push_str(name_str);
                            result.push_str(": ");
                        }
                        result.push_str(&fmt(val));
                    }
                }

                result.push_str(suffix);
                Ok(Value::string(result))
            }
            Value::Variant {
                variant_name,
                fields,
                ..
            } => {
                let vname = self.interner.lookup(*variant_name);
                if fields.is_empty() {
                    return Ok(Value::string(vname.to_string()));
                }
                let mut result = vname.to_string();
                result.push('(');
                for (i, val) in fields.iter().enumerate() {
                    if i > 0 {
                        result.push_str(separator);
                    }
                    result.push_str(&fmt(val));
                }
                result.push(')');
                Ok(Value::string(result))
            }
            _ => Ok(Value::string(fmt(&receiver))),
        }
    }

    /// Format a value in Printable style (human-readable, no quotes on strings).
    ///
    /// Unlike `Value::Display` which wraps strings in quotes and shows raw
    /// struct debug info, this produces the human-readable Printable format:
    /// - Strings: content directly (no quotes)
    /// - Chars: character directly (no quotes)
    /// - Structs: `TypeName(val1, val2)` via recursive lookup
    /// - Other values: standard Display format
    fn format_value_printable(&self, val: &Value, depth: usize) -> String {
        if depth > 256 {
            return "...".to_string();
        }
        match val {
            Value::Str(s) => (**s).to_string(),
            Value::Char(c) => c.to_string(),
            Value::Struct(sv) => {
                let to_str_name = self.interner.intern("to_str");
                let derived_info = self
                    .user_method_registry
                    .read()
                    .lookup_derived(sv.type_name, to_str_name)
                    .cloned();

                let mut result = self.interner.lookup(sv.type_name).to_string();
                result.push('(');

                if let Some(ref info) = derived_info {
                    let mut first = true;
                    for field_name in &info.field_names {
                        if let Some(fv) = sv.get_field(*field_name) {
                            if !first {
                                result.push_str(", ");
                            }
                            first = false;
                            result.push_str(
                                &self.format_value_printable(fv, depth.saturating_add(1)),
                            );
                        }
                    }
                }

                result.push(')');
                result
            }
            Value::Variant {
                variant_name,
                fields,
                ..
            } => {
                let vname = self.interner.lookup(*variant_name);
                if fields.is_empty() {
                    vname.to_string()
                } else {
                    let mut result = vname.to_string();
                    result.push('(');
                    for (i, fv) in fields.iter().enumerate() {
                        if i > 0 {
                            result.push_str(", ");
                        }
                        result.push_str(&self.format_value_printable(fv, depth.saturating_add(1)));
                    }
                    result.push(')');
                    result
                }
            }
            _ => format!("{val}"),
        }
    }
}
