//! Method dispatch for Error values (Traceable trait).

use ori_ir::Name;
use ori_patterns::{
    no_such_method, ErrorValue, EvalError, EvalResult, StructValue, TraceEntryData, Value,
};
use rustc_hash::FxHashMap;

use super::helpers::require_args;
use super::DispatchCtx;

/// Dispatch methods on Error values.
///
/// Implements the Traceable trait methods plus `message` accessor:
/// - `trace` → formatted trace string
/// - `trace_entries` → list of `TraceEntry` structs
/// - `has_trace` → bool
/// - `with_trace` → new Error with appended entry
/// - `message` → the error message string
/// - `to_str` / `debug` → string representation
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature"
)]
pub fn dispatch_error_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let n = ctx.names;

    if method == n.trace {
        require_args("trace", 0, args.len())?;
        let ev = extract_error(&receiver);
        Ok(Value::string(ev.format_trace()))
    } else if method == n.trace_entries {
        require_args("trace_entries", 0, args.len())?;
        let ev = extract_error(&receiver);
        let entries: Vec<Value> = ev
            .trace()
            .iter()
            .map(|entry| trace_entry_to_struct(entry, ctx))
            .collect();
        Ok(Value::list(entries))
    } else if method == n.has_trace {
        require_args("has_trace", 0, args.len())?;
        let ev = extract_error(&receiver);
        Ok(Value::Bool(ev.has_trace()))
    } else if method == n.with_trace {
        require_args("with_trace", 1, args.len())?;
        let ev = extract_error(&receiver);
        let entry = struct_to_trace_entry(&args[0], ctx)?;
        let new_ev = ev.with_entry(entry);
        Ok(Value::error_from(new_ev))
    } else if method == n.message {
        require_args("message", 0, args.len())?;
        let ev = extract_error(&receiver);
        Ok(Value::string(ev.message()))
    } else if method == n.to_str || method == n.debug {
        require_args("to_str", 0, args.len())?;
        let ev = extract_error(&receiver);
        Ok(Value::string(format!("{ev}")))
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(receiver)
    } else {
        Err(no_such_method(ctx.interner.lookup(method), "error").into())
    }
}

/// Extract the `ErrorValue` from a `Value::Error`.
fn extract_error(value: &Value) -> &ErrorValue {
    match value {
        Value::Error(ev) => ev,
        _ => unreachable!("dispatch_error_method called with non-Error receiver"),
    }
}

/// Convert a `TraceEntryData` to a `Value::Struct(StructValue)` (`TraceEntry`).
pub(super) fn trace_entry_to_struct(entry: &TraceEntryData, ctx: &DispatchCtx<'_>) -> Value {
    let type_name = ctx.interner.intern("TraceEntry");
    let fn_name = ctx.interner.intern("function");
    let file_name = ctx.interner.intern("file");
    let line_name = ctx.interner.intern("line");
    let column_name = ctx.interner.intern("column");

    let mut fields = FxHashMap::default();
    fields.insert(fn_name, Value::string(&entry.function));
    fields.insert(file_name, Value::string(&entry.file));
    fields.insert(line_name, Value::int(i64::from(entry.line)));
    fields.insert(column_name, Value::int(i64::from(entry.column)));

    Value::Struct(StructValue::new(type_name, fields))
}

/// Convert a `Value::Struct` (`TraceEntry`) to a `TraceEntryData`.
///
/// Returns an error if the input is not a struct or is missing required fields
/// (`function`, `file`, `line`, `column`) with the correct types.
fn struct_to_trace_entry(
    value: &Value,
    ctx: &DispatchCtx<'_>,
) -> Result<TraceEntryData, EvalError> {
    let Value::Struct(sv) = value else {
        return Err(EvalError::new(format!(
            "with_trace: expected TraceEntry struct, got {}",
            value.type_name()
        )));
    };

    let fn_name = ctx.interner.intern("function");
    let file_name = ctx.interner.intern("file");
    let line_name = ctx.interner.intern("line");
    let column_name = ctx.interner.intern("column");

    let get_str = |name: Name, field: &str| -> Result<String, EvalError> {
        let val = sv
            .layout
            .get_index(name)
            .and_then(|i| sv.fields.get(i))
            .ok_or_else(|| {
                EvalError::new(format!("with_trace: TraceEntry missing field '{field}'"))
            })?;
        val.as_str().map(str::to_string).ok_or_else(|| {
            EvalError::new(format!(
                "with_trace: field '{field}' expected str, got {}",
                val.type_name()
            ))
        })
    };
    let get_int = |name: Name, field: &str| -> Result<u32, EvalError> {
        let val = sv
            .layout
            .get_index(name)
            .and_then(|i| sv.fields.get(i))
            .ok_or_else(|| {
                EvalError::new(format!("with_trace: TraceEntry missing field '{field}'"))
            })?;
        let raw = val.as_int().ok_or_else(|| {
            EvalError::new(format!(
                "with_trace: field '{field}' expected int, got {}",
                val.type_name()
            ))
        })?;
        u32::try_from(raw).map_err(|_| {
            EvalError::new(format!(
                "with_trace: field '{field}' value {raw} out of range for u32"
            ))
        })
    };

    Ok(TraceEntryData {
        function: get_str(fn_name, "function")?,
        file: get_str(file_name, "file")?,
        line: get_int(line_name, "line")?,
        column: get_int(column_name, "column")?,
    })
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Tests use unwrap for brevity"
)]
mod tests;
