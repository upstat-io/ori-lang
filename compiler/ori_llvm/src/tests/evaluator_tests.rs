//! Tests for `OwnedLLVMEvaluator` and evaluator types.

#![allow(
    clippy::manual_assert,
    clippy::uninlined_format_args,
    clippy::default_trait_access,
    reason = "test ergonomics — relaxed style for clarity"
)]

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
        file_attr: None,
        imports: vec![],
        consts: vec![],
        functions: vec![],
        tests: vec![],
        types: vec![],
        traits: vec![],
        impls: vec![],
        extends: vec![],
        def_impls: vec![],
        extension_imports: vec![],
        extern_blocks: vec![],
    };

    let canon = ori_ir::canon::CanonResult {
        arena: Default::default(),
        constants: Default::default(),
        decision_trees: ori_ir::canon::DecisionTreePool::new(),
        root: ori_ir::canon::CanId::INVALID,
        roots: vec![],
        method_roots: vec![],
        problems: vec![],
    };
    let empty_sigs = rustc_hash::FxHashMap::default();
    let result = evaluator.compile_module_with_tests(
        &module,
        &[],
        &canon,
        &interner,
        &[],
        &[],
        &[],
        &[],
        &[],
        &empty_sigs,
        rustc_hash::FxHashMap::default(),
        None,   // JIT: use env var fallback for narrowing policy
        &[],    // No imported type metadata for empty module test
        &[],    // No imported collection surfaces for empty module test
        &[],    // No trait impl fn names for empty module test
        vec![], // No imported mono functions for empty module test
    );

    assert!(
        result.is_ok(),
        "empty module should compile: {}",
        result.err().map(|e| e.message).unwrap_or_default()
    );
}

/// Regression: ordinal-qualified names are registered as unconstrained.
///
/// When two trait impls on the same type share a method name (e.g., two `Index`
/// impls with `@index`), `collect_unconstrained_fn_names()` must register both
/// the base name (`__impl_42_index`) and ordinal-suffixed names
/// (`__impl_42_index_1`).
#[test]
fn collect_unconstrained_fn_names_registers_ordinal_variants() {
    let interner = StringInterner::new();

    // Simulate type checker output: same type (Idx 42) defines `index` twice
    // (e.g., `impl Index<int>` and `impl Index<str>` on the same type).
    let self_type = ori_types::Idx::from_raw(42);
    let index_name = interner.intern("index");
    let trait_impl_fn_names = vec![
        (self_type, index_name), // First impl: ordinal 0
        (self_type, index_name), // Second impl: ordinal 1
    ];

    let result = crate::collect_unconstrained_fn_names(&[], &trait_impl_fn_names, Some(&interner));

    // Expected registrations:
    // 1. (Some(42), "index") — first trait impl method
    // 2. (None, "__impl_42_index") — base qualified name (ordinal 0)
    // 3. (Some(42), "index") — second trait impl method
    // 4. (None, "__impl_42_index_1") — ordinal-qualified name (ordinal 1)
    let base_qualified = interner.intern("__impl_42_index");
    let ordinal_qualified = interner.intern("__impl_42_index_1");

    assert!(
        result.contains(&(None, base_qualified)),
        "Base qualified name __impl_42_index must be registered"
    );
    assert!(
        result.contains(&(None, ordinal_qualified)),
        "Ordinal-qualified name __impl_42_index_1 must be registered"
    );

    // Negative: ordinal 0 must NOT produce a suffixed name
    let wrong_base = interner.intern("__impl_42_index_0");
    assert!(
        !result.contains(&(None, wrong_base)),
        "Ordinal 0 must use the unsuffixed base name, not __impl_42_index_0"
    );
}
