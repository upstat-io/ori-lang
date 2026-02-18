//! Tests for error method dispatch (Traceable trait).

use ori_ir::StringInterner;
use ori_patterns::{StructValue, Value};
use rustc_hash::FxHashMap;

use super::struct_to_trace_entry;
use crate::methods::BuiltinMethodNames;
use crate::methods::DispatchCtx;

fn make_ctx(interner: &StringInterner) -> DispatchCtx<'_> {
    // Leak the BuiltinMethodNames to get a stable reference for the test.
    // This is fine in tests — the process exits after.
    let names = Box::leak(Box::new(BuiltinMethodNames::new(interner)));
    DispatchCtx { names, interner }
}

fn make_trace_entry_struct(interner: &StringInterner) -> Value {
    let type_name = interner.intern("TraceEntry");
    let mut fields = FxHashMap::default();
    fields.insert(interner.intern("function"), Value::string("my_fn"));
    fields.insert(interner.intern("file"), Value::string("test.ori"));
    fields.insert(interner.intern("line"), Value::int(42));
    fields.insert(interner.intern("column"), Value::int(7));
    Value::Struct(StructValue::new(type_name, fields))
}

#[test]
fn valid_trace_entry_struct() {
    let interner = StringInterner::new();
    let ctx = make_ctx(&interner);
    let value = make_trace_entry_struct(&interner);

    let result = struct_to_trace_entry(&value, &ctx);
    let entry = result.expect("should parse valid struct");
    assert_eq!(entry.function, "my_fn");
    assert_eq!(entry.file, "test.ori");
    assert_eq!(entry.line, 42);
    assert_eq!(entry.column, 7);
}

#[test]
fn rejects_non_struct_value() {
    let interner = StringInterner::new();
    let ctx = make_ctx(&interner);

    let err = struct_to_trace_entry(&Value::int(42), &ctx).unwrap_err();
    assert!(
        err.message.contains("expected TraceEntry struct"),
        "got: {}",
        err.message
    );
}

#[test]
fn rejects_struct_missing_field() {
    let interner = StringInterner::new();
    let ctx = make_ctx(&interner);

    let type_name = interner.intern("TraceEntry");
    let mut fields = FxHashMap::default();
    // Only provide "function", missing file/line/column
    fields.insert(interner.intern("function"), Value::string("my_fn"));
    let value = Value::Struct(StructValue::new(type_name, fields));

    let err = struct_to_trace_entry(&value, &ctx).unwrap_err();
    assert!(
        err.message.contains("missing field"),
        "got: {}",
        err.message
    );
}

#[test]
fn rejects_wrong_field_type() {
    let interner = StringInterner::new();
    let ctx = make_ctx(&interner);

    let type_name = interner.intern("TraceEntry");
    let mut fields = FxHashMap::default();
    fields.insert(interner.intern("function"), Value::int(999)); // wrong type
    fields.insert(interner.intern("file"), Value::string("test.ori"));
    fields.insert(interner.intern("line"), Value::int(1));
    fields.insert(interner.intern("column"), Value::int(1));
    let value = Value::Struct(StructValue::new(type_name, fields));

    let err = struct_to_trace_entry(&value, &ctx).unwrap_err();
    assert!(err.message.contains("expected str"), "got: {}", err.message);
}
