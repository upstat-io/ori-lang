//! Tests for built-in type conversion functions (str, int, float, byte).

use std::collections::HashMap;

use inkwell::context::Context;
use ori_ir::StringInterner;

use crate::builder::Builder;
use crate::context::CodegenCx;

/// Helper to create test context and builder.
fn setup_builder<'ll, 'tcx>(
    context: &'ll Context,
    interner: &'tcx StringInterner,
) -> (CodegenCx<'ll, 'tcx>, inkwell::values::FunctionValue<'ll>, inkwell::basic_block::BasicBlock<'ll>) {
    let cx = CodegenCx::new(context, interner, "test");
    cx.declare_runtime_functions();

    let fn_type = cx.scx.type_i64().fn_type(&[], false);
    let function = cx.llmod().add_function("test_fn", fn_type, None);
    let entry_bb = cx.llcx().append_basic_block(function, "entry");

    (cx, function, entry_bb)
}

#[test]
fn test_builtin_str_from_i64() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let int_val = cx.scx.type_i64().const_int(42, false);
    let result = builder.compile_builtin_str(int_val.into());

    assert!(result.is_some(), "str(i64) should produce a value");
}

#[test]
fn test_builtin_str_from_bool() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let bool_val = cx.scx.type_i1().const_int(1, false);
    let result = builder.compile_builtin_str(bool_val.into());

    assert!(result.is_some(), "str(bool) should produce a value");
}

#[test]
fn test_builtin_str_from_other_int() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    // Test with i32 (not i64 or i1)
    let i32_val = cx.scx.type_i32().const_int(42, false);
    let result = builder.compile_builtin_str(i32_val.into());

    assert!(result.is_some(), "str(i32) should produce a value after extension");
}

#[test]
fn test_builtin_str_from_float() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let float_val = cx.scx.type_f64().const_float(3.14);
    let result = builder.compile_builtin_str(float_val.into());

    assert!(result.is_some(), "str(float) should produce a value");
}

#[test]
fn test_builtin_str_from_struct() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    // Create a struct value (representing a string)
    let struct_type = cx.llcx().struct_type(&[
        cx.scx.type_i64().into(),
        cx.scx.type_ptr().into(),
    ], false);
    let struct_val = struct_type.const_zero();
    let result = builder.compile_builtin_str(struct_val.into());

    assert!(result.is_some(), "str(struct) should return the struct");
}

#[test]
fn test_builtin_int_already_i64() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let int_val = cx.scx.type_i64().const_int(42, false);
    let result = builder.compile_builtin_int(int_val.into());

    assert!(result.is_some(), "int(i64) should return the value");
    assert_eq!(
        result.unwrap().into_int_value().get_zero_extended_constant(),
        Some(42)
    );
}

#[test]
fn test_builtin_int_from_bool() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let bool_val = cx.scx.type_i1().const_int(1, false);
    let result = builder.compile_builtin_int(bool_val.into());

    assert!(result.is_some(), "int(bool) should produce a value");
}

#[test]
fn test_builtin_int_from_smaller_int() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    // Test with i32
    let i32_val = cx.scx.type_i32().const_int(42, false);
    let result = builder.compile_builtin_int(i32_val.into());

    assert!(result.is_some(), "int(i32) should produce a value after extension");
}

#[test]
fn test_builtin_int_from_float() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let float_val = cx.scx.type_f64().const_float(3.7);
    let result = builder.compile_builtin_int(float_val.into());

    assert!(result.is_some(), "int(float) should produce a value");
}

#[test]
fn test_builtin_int_from_unknown() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    // Pointer type (unknown)
    let ptr_val = cx.scx.type_ptr().const_null();
    let result = builder.compile_builtin_int(ptr_val.into());

    assert!(result.is_some(), "int(ptr) should return 0");
}

#[test]
fn test_builtin_float_from_bool() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let bool_val = cx.scx.type_i1().const_int(1, false);
    let result = builder.compile_builtin_float(bool_val.into());

    assert!(result.is_some(), "float(bool) should produce a value");
}

#[test]
fn test_builtin_float_from_int() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let int_val = cx.scx.type_i64().const_int(42, false);
    let result = builder.compile_builtin_float(int_val.into());

    assert!(result.is_some(), "float(int) should produce a value");
}

#[test]
fn test_builtin_float_already_float() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let float_val = cx.scx.type_f64().const_float(3.14);
    let result = builder.compile_builtin_float(float_val.into());

    assert!(result.is_some(), "float(float) should return the value");
}

#[test]
fn test_builtin_float_from_unknown() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let ptr_val = cx.scx.type_ptr().const_null();
    let result = builder.compile_builtin_float(ptr_val.into());

    assert!(result.is_some(), "float(ptr) should return 0.0");
}

#[test]
fn test_builtin_byte_already_byte() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let byte_val = cx.scx.type_i8().const_int(42, false);
    let result = builder.compile_builtin_byte(byte_val.into());

    assert!(result.is_some(), "byte(i8) should return the value");
    assert_eq!(
        result.unwrap().into_int_value().get_zero_extended_constant(),
        Some(42)
    );
}

#[test]
fn test_builtin_byte_from_i64() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let int_val = cx.scx.type_i64().const_int(300, false);
    let result = builder.compile_builtin_byte(int_val.into());

    assert!(result.is_some(), "byte(i64) should truncate");
}

#[test]
fn test_builtin_byte_from_unknown() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, _function, entry_bb) = setup_builder(&context, &interner);

    let builder = Builder::build(&cx, entry_bb);

    let ptr_val = cx.scx.type_ptr().const_null();
    let result = builder.compile_builtin_byte(ptr_val.into());

    assert!(result.is_some(), "byte(ptr) should return 0");
}
