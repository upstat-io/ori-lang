//! Tests for `OwnedLLVMEvaluator` and evaluator types.

use ori_ir::StringInterner;
use ori_types::Pool;

use crate::evaluator::{LLVMEvalError, LLVMValue, OwnedLLVMEvaluator};

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
fn test_owned_llvm_evaluator_with_pool() {
    let pool = Pool::new();
    let evaluator = OwnedLLVMEvaluator::with_pool(&pool);
    drop(evaluator);
}

#[test]
fn test_compile_module_with_tests_empty() {
    let pool = Pool::new();
    let evaluator = OwnedLLVMEvaluator::with_pool(&pool);
    let interner = StringInterner::new();

    let module = ori_ir::ast::Module {
        imports: vec![],
        consts: vec![],
        functions: vec![],
        tests: vec![],
        types: vec![],
        traits: vec![],
        impls: vec![],
        extends: vec![],
        def_impls: vec![],
    };

    let arena = ori_ir::ExprArena::new();
    let result = evaluator.compile_module_with_tests(
        &module,
        &[],
        &arena,
        &interner,
        &[],
        &[],
        &[],
        &[],
        &[],
    );

    assert!(
        result.is_ok(),
        "empty module should compile: {}",
        result.err().map(|e| e.message).unwrap_or_default()
    );
}
