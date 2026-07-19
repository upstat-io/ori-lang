//! Debug-format rendering for evaluator values.

use ori_ir::StringLookup;
use ori_patterns::Value;

/// Escape a string for Debug output (newlines, tabs, quotes, backslashes, null).
pub(crate) fn escape_debug_str(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\0' => result.push_str("\\0"),
            c => result.push(c),
        }
    }
    result
}

/// Escape a char for Debug output.
pub(crate) fn escape_debug_char(c: char) -> String {
    match c {
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        '\0' => "\\0".to_string(),
        c => c.to_string(),
    }
}

/// Format a `Value` using Debug semantics (developer-facing structural output).
///
/// This is the recursive workhorse for `.debug()` on collections, Option, Result,
/// tuples, structs, and variants. Each value is formatted as it would appear in
/// a `.debug()` call. The `interner` parameter enables proper formatting of
/// struct/variant field and type names.
pub(crate) fn debug_value(val: &Value, interner: &dyn StringLookup) -> String {
    use std::fmt::Write;

    match val {
        Value::Int(n) => n.raw().to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => format!("\"{}\"", escape_debug_str(s)),
        Value::Char(c) => format!("'{}'", escape_debug_char(*c)),
        Value::Byte(b) => b.to_string(),
        Value::Void => "()".to_string(),
        Value::None => "None".to_string(),
        Value::Some(v) => format!("Some({})", debug_value(v, interner)),
        Value::Ok(v) => format!("Ok({})", debug_value(v, interner)),
        Value::Err(v) => format!("Err({})", debug_value(v, interner)),
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(|v| debug_value(v, interner)).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Map(map) => {
            let mut result = String::from("{");
            let mut first = true;
            for (k, v) in map.iter() {
                if !first {
                    result.push_str(", ");
                }
                first = false;
                // Spec: Clause 8.12.1 — `{K: V}` Debug renders keys with full
                // Debug semantics (`{"x": 1}`): str keys are quoted, like every
                // other Debug position. Keys and values both go through
                // `debug_value`.
                // Why: `String`'s `fmt::Write` implementation is infallible.
                if let Err(error) = write!(
                    result,
                    "{}: {}",
                    debug_value(k, interner),
                    debug_value(v, interner)
                ) {
                    unreachable!("writing debug output to a String failed: {error}");
                }
            }
            result.push('}');
            result
        }
        Value::Set(items) => {
            let parts: Vec<String> = items.values().map(|v| debug_value(v, interner)).collect();
            format!("Set {{{}}}", parts.join(", "))
        }
        Value::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(|v| debug_value(v, interner)).collect();
            format!("({})", parts.join(", "))
        }
        Value::Struct(s) => {
            let type_name = interner.lookup(s.type_name);
            let mut sorted_fields: Vec<_> = s.layout.field_names().collect();
            sorted_fields.sort_by_key(|&(_, idx)| idx);
            let fields_str: Vec<String> = sorted_fields
                .iter()
                .map(|&(name, idx)| {
                    let field_name = interner.lookup(name);
                    let field_val = &s.fields[idx];
                    format!("{field_name}: {}", debug_value(field_val, interner))
                })
                .collect();
            format!("{type_name} {{ {} }}", fields_str.join(", "))
        }
        Value::Variant {
            variant_name,
            fields,
            ..
        } => {
            let name = interner.lookup(*variant_name);
            if fields.is_empty() {
                name.to_string()
            } else {
                let parts: Vec<String> = fields.iter().map(|v| debug_value(v, interner)).collect();
                format!("{name}({})", parts.join(", "))
            }
        }
        Value::Newtype { type_name, inner } => {
            let name = interner.lookup(*type_name);
            format!("{name}({})", debug_value(inner, interner))
        }
        Value::Duration(ns) => super::units::format_duration_debug(*ns),
        Value::Size(bytes) => super::units::format_size_debug(*bytes),
        Value::Ordering(ord) => ord.name().to_string(),
        Value::Range(r) => format!("{r:?}"),
        // Closure, Iterator, etc. — fall back to Display
        other => format!("{other}"),
    }
}

#[cfg(test)]
mod tests;
