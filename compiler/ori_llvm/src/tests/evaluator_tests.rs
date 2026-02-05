//! Tests for `LLVMEvaluator` and `OwnedLLVMEvaluator`.

use inkwell::context::Context;
use ori_ir::ast::{Expr, ExprKind, Module, Visibility};
use ori_ir::{ExprArena, Function, GenericParamRange, ParamRange, Span, StringInterner};
use ori_types::Idx;

use crate::evaluator::{FunctionSig, LLVMEvalError, LLVMEvaluator, LLVMValue, OwnedLLVMEvaluator};

/// Helper to create an empty Module for tests.
fn empty_module() -> Module {
    Module {
        imports: vec![],
        consts: vec![],
        functions: vec![],
        tests: vec![],
        types: vec![],
        traits: vec![],
        impls: vec![],
        extends: vec![],
        def_impls: vec![],
    }
}

#[test]
fn test_llvm_value_debug() {
    let void = LLVMValue::Void;
    let int = LLVMValue::Int(42);
    let float = LLVMValue::Float(3.5);
    let bool_val = LLVMValue::Bool(true);

    assert_eq!(format!("{void:?}"), "Void");
    assert_eq!(format!("{int:?}"), "Int(42)");
    assert_eq!(format!("{float:?}"), "Float(3.5)");
    assert_eq!(format!("{bool_val:?}"), "Bool(true)");
}

#[test]
fn test_llvm_value_equality() {
    assert_eq!(LLVMValue::Void, LLVMValue::Void);
    assert_eq!(LLVMValue::Int(42), LLVMValue::Int(42));
    assert_ne!(LLVMValue::Int(42), LLVMValue::Int(43));
    assert_eq!(LLVMValue::Float(3.5), LLVMValue::Float(3.5));
    assert_eq!(LLVMValue::Bool(true), LLVMValue::Bool(true));
    assert_ne!(LLVMValue::Bool(true), LLVMValue::Bool(false));
}

#[test]
fn test_llvm_value_clone() {
    let original = LLVMValue::Int(42);
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn test_llvm_eval_error_new() {
    let error = LLVMEvalError::new("test error");
    assert_eq!(error.message, "test error");
}

#[test]
fn test_llvm_eval_error_display() {
    let error = LLVMEvalError::new("display test");
    assert_eq!(format!("{error}"), "display test");
}

#[test]
fn test_llvm_eval_error_from_string() {
    let error = LLVMEvalError::new(String::from("from string"));
    assert_eq!(error.message, "from string");
}

#[test]
fn test_llvm_evaluator_new() {
    let context = Context::create();
    let interner = StringInterner::new();
    let evaluator = LLVMEvaluator::new(&context, &interner);

    // Verify it was created (no panic)
    drop(evaluator);
}

#[test]
fn test_llvm_evaluator_register_prelude() {
    let context = Context::create();
    let interner = StringInterner::new();
    let mut evaluator = LLVMEvaluator::new(&context, &interner);

    // Should not panic
    evaluator.register_prelude();
}

#[test]
fn test_llvm_evaluator_load_module() {
    let context = Context::create();
    let interner = StringInterner::new();
    let mut evaluator = LLVMEvaluator::new(&context, &interner);

    let mut arena = ExprArena::new();

    // Create a simple module with one function
    let body = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let func_name = interner.intern("test_func");
    let func = Function {
        name: func_name,
        generics: GenericParamRange::EMPTY,
        params: ParamRange::EMPTY,
        return_ty: None,
        capabilities: vec![],
        where_clauses: vec![],
        guard: None,
        body,
        span: Span::new(0, 1),
        visibility: Visibility::Private,
    };

    let mut module = empty_module();
    module.functions.push(func);

    let result = evaluator.load_module(&module, &arena);
    assert!(result.is_ok(), "load_module should succeed");
}

#[test]
fn test_llvm_evaluator_eval_simple() {
    let context = Context::create();
    let interner = StringInterner::new();
    let evaluator = LLVMEvaluator::new(&context, &interner);

    let mut arena = ExprArena::new();
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let result = evaluator.eval(expr, &arena);
    assert!(result.is_ok(), "eval should succeed");
    assert_eq!(result.unwrap(), LLVMValue::Void);
}

#[test]
fn test_owned_llvm_evaluator_new() {
    let evaluator = OwnedLLVMEvaluator::new();
    drop(evaluator);
}

#[test]
fn test_owned_llvm_evaluator_default() {
    let evaluator = OwnedLLVMEvaluator::default();
    drop(evaluator);
}

#[test]
fn test_owned_llvm_evaluator_load_module() {
    let mut evaluator = OwnedLLVMEvaluator::new();
    let interner = StringInterner::new();

    let mut arena = ExprArena::new();

    let body = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let func_name = interner.intern("test_func");
    let func = Function {
        name: func_name,
        generics: GenericParamRange::EMPTY,
        params: ParamRange::EMPTY,
        return_ty: None,
        capabilities: vec![],
        where_clauses: vec![],
        guard: None,
        body,
        span: Span::new(0, 1),
        visibility: Visibility::Private,
    };

    let mut module = empty_module();
    module.functions.push(func);

    let result = evaluator.load_module(&module, &arena);
    assert!(result.is_ok(), "load_module should succeed");
}

#[test]
fn test_owned_llvm_evaluator_eval_test() {
    let evaluator = OwnedLLVMEvaluator::new();
    let interner = StringInterner::new();

    let mut arena = ExprArena::new();

    // Create a test body: just return unit
    let test_body = arena.alloc_expr(Expr {
        kind: ExprKind::Unit,
        span: Span::new(0, 1),
    });

    let test_name = interner.intern("my_test");

    let module = empty_module();

    let expr_types = vec![Idx::UNIT];
    let function_sigs = vec![];

    let result = evaluator.eval_test(
        test_name,
        test_body,
        &arena,
        &module,
        &interner,
        &expr_types,
        &function_sigs,
    );

    assert!(result.is_ok(), "eval_test should succeed");
    assert_eq!(result.unwrap(), LLVMValue::Void);
}

#[test]
fn test_function_sig_debug() {
    let sig = FunctionSig {
        params: vec![Idx::INT, Idx::BOOL],
        return_type: Idx::STR,
        is_generic: false,
    };

    let debug_str = format!("{sig:?}");
    assert!(debug_str.contains("params"), "Debug should show params");
    assert!(
        debug_str.contains("return_type"),
        "Debug should show return_type"
    );
}

#[test]
fn test_function_sig_clone() {
    let sig = FunctionSig {
        params: vec![Idx::INT],
        return_type: Idx::BOOL,
        is_generic: false,
    };

    let cloned = sig.clone();
    assert_eq!(sig.params.len(), cloned.params.len());
    assert_eq!(sig.return_type, cloned.return_type);
}

#[test]
fn test_llvm_evaluator_eval_bool() {
    let context = Context::create();
    let interner = StringInterner::new();
    let evaluator = LLVMEvaluator::new(&context, &interner);

    let mut arena = ExprArena::new();
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: Span::new(0, 1),
    });

    let result = evaluator.eval(expr, &arena);
    assert!(result.is_ok(), "eval bool should succeed");
}

#[test]
fn test_llvm_evaluator_eval_float() {
    let context = Context::create();
    let interner = StringInterner::new();
    let evaluator = LLVMEvaluator::new(&context, &interner);

    let mut arena = ExprArena::new();
    let expr = arena.alloc_expr(Expr {
        kind: ExprKind::Float(3.5f64.to_bits()),
        span: Span::new(0, 1),
    });

    let result = evaluator.eval(expr, &arena);
    assert!(result.is_ok(), "eval float should succeed");
}
