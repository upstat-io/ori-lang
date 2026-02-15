use super::*;
use crate::print_handler::buffer_handler;
use ori_ir::SharedInterner;

#[test]
fn print_handler_integration_println() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let handler = buffer_handler();

    let interpreter = InterpreterBuilder::new(&interner, &arena)
        .print_handler(handler.clone())
        .build();

    // Directly call the print handler
    interpreter.print_handler.println("hello world");

    assert_eq!(interpreter.get_print_output(), "hello world\n");
}

#[test]
fn print_handler_integration_print() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let handler = buffer_handler();

    let interpreter = InterpreterBuilder::new(&interner, &arena)
        .print_handler(handler.clone())
        .build();

    interpreter.print_handler.print("hello");
    interpreter.print_handler.print(" world");

    assert_eq!(interpreter.get_print_output(), "hello world");
}

#[test]
fn print_handler_integration_clear() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let handler = buffer_handler();

    let interpreter = InterpreterBuilder::new(&interner, &arena)
        .print_handler(handler.clone())
        .build();

    interpreter.print_handler.println("first");
    interpreter.clear_print_output();
    interpreter.print_handler.println("second");

    assert_eq!(interpreter.get_print_output(), "second\n");
}

#[test]
fn default_handler_is_stdout() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();

    let interpreter = InterpreterBuilder::new(&interner, &arena).build();

    // Default stdout handler doesn't capture, returns empty
    assert_eq!(interpreter.get_print_output(), "");
}

#[test]
fn handler_shared_between_interpreters() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let handler = buffer_handler();

    let interpreter1 = InterpreterBuilder::new(&interner, &arena)
        .print_handler(handler.clone())
        .build();

    let interpreter2 = InterpreterBuilder::new(&interner, &arena)
        .print_handler(handler.clone())
        .build();

    interpreter1.print_handler.println("from 1");
    interpreter2.print_handler.println("from 2");

    // Both wrote to the same handler
    let output = handler.get_output();
    assert!(output.contains("from 1"));
    assert!(output.contains("from 2"));
}

#[test]
fn call_method_println_uses_handler() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let handler = buffer_handler();

    let mut interpreter = InterpreterBuilder::new(&interner, &arena)
        .print_handler(handler.clone())
        .build();

    let println_name = interner.intern("println");

    // Test that call_method routes println to the handler
    let result = <Interpreter as PatternExecutor>::call_method(
        &mut interpreter,
        Value::Void,
        println_name,
        vec![Value::string("test message")],
    );

    assert!(result.is_ok());
    assert_eq!(interpreter.get_print_output(), "test message\n");
}

#[test]
fn call_method_print_uses_handler() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let handler = buffer_handler();

    let mut interpreter = InterpreterBuilder::new(&interner, &arena)
        .print_handler(handler.clone())
        .build();

    let print_name = interner.intern("print");

    // Test that call_method routes print to the handler
    let result = <Interpreter as PatternExecutor>::call_method(
        &mut interpreter,
        Value::Void,
        print_name,
        vec![Value::string("no newline")],
    );

    assert!(result.is_ok());
    assert_eq!(interpreter.get_print_output(), "no newline");
}

#[test]
fn call_method_builtin_println_uses_handler() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let handler = buffer_handler();

    let mut interpreter = InterpreterBuilder::new(&interner, &arena)
        .print_handler(handler.clone())
        .build();

    let builtin_println_name = interner.intern("__builtin_println");

    // Test the __builtin_println fallback path
    let result = <Interpreter as PatternExecutor>::call_method(
        &mut interpreter,
        Value::Void,
        builtin_println_name,
        vec![Value::string("builtin test")],
    );

    assert!(result.is_ok());
    assert_eq!(interpreter.get_print_output(), "builtin test\n");
}
