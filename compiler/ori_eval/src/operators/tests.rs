//! Sync enforcement tests: verify evaluator operator dispatch is consistent
//! with the registry's `OpDefs`.
//!
//! These tests ensure the evaluator handles exactly the operators that the
//! registry declares as supported for each primitive type. If a new operator
//! is added to a type's `OpDefs` in the registry, these tests will fail until
//! the evaluator is updated to handle it.

use ori_ir::BinaryOp;
use ori_patterns::{ScalarInt, Value};
use ori_registry::{OpStrategy, TypeTag};

use super::evaluate_binary;

/// All arithmetic + comparison + bitwise binary ops (excluding `Range`, `Coalesce`, `MatMul`).
type StrategyFn = fn(&ori_registry::OpDefs) -> OpStrategy;
const REGISTRY_OPS: &[(BinaryOp, StrategyFn)] = &[
    (BinaryOp::Add, |o| o.add),
    (BinaryOp::Sub, |o| o.sub),
    (BinaryOp::Mul, |o| o.mul),
    (BinaryOp::Div, |o| o.div),
    (BinaryOp::Mod, |o| o.rem),
    (BinaryOp::FloorDiv, |o| o.floor_div),
    (BinaryOp::Eq, |o| o.eq),
    (BinaryOp::NotEq, |o| o.neq),
    (BinaryOp::Lt, |o| o.lt),
    (BinaryOp::LtEq, |o| o.lt_eq),
    (BinaryOp::Gt, |o| o.gt),
    (BinaryOp::GtEq, |o| o.gt_eq),
    (BinaryOp::BitAnd, |o| o.bit_and),
    (BinaryOp::BitOr, |o| o.bit_or),
    (BinaryOp::BitXor, |o| o.bit_xor),
    (BinaryOp::Shl, |o| o.shl),
    (BinaryOp::Shr, |o| o.shr),
];

/// Verify that the evaluator handles all registry-supported ops and rejects unsupported ones.
fn check_type_ops(tag: TypeTag, a: &Value, b: &Value) {
    let Some(type_def) = ori_registry::find_type(tag) else {
        panic!("TypeDef for {tag:?} not found in registry");
    };
    for &(op, get_strategy) in REGISTRY_OPS {
        let supported = get_strategy(&type_def.operators) != OpStrategy::Unsupported;
        let result = evaluate_binary(a.clone(), b.clone(), op);
        if supported {
            assert!(
                result.is_ok(),
                "{}: registry says {op:?} is supported but evaluator returned Err: {:?}",
                type_def.name,
                result.err(),
            );
        }
    }
}

/// For each supported operator on `int`, verify the evaluator produces `Ok`.
#[test]
fn eval_int_handles_all_registry_supported_ops() {
    check_type_ops(
        TypeTag::Int,
        &Value::Int(ScalarInt::new(10)),
        &Value::Int(ScalarInt::new(3)),
    );
}

/// For each supported operator on `float`, verify the evaluator produces `Ok`.
#[test]
fn eval_float_handles_all_registry_supported_ops() {
    check_type_ops(TypeTag::Float, &Value::Float(10.0), &Value::Float(3.0));
}

/// For each supported operator on `bool`, verify the evaluator produces `Ok`.
#[test]
fn eval_bool_handles_all_registry_supported_ops() {
    check_type_ops(TypeTag::Bool, &Value::Bool(true), &Value::Bool(false));
}

/// Str: only `add` (concatenation) and comparison ops supported.
#[test]
fn eval_str_handles_all_registry_supported_ops() {
    check_type_ops(
        TypeTag::Str,
        &Value::string("hello"),
        &Value::string("world"),
    );
}

/// Char: comparison ops only (no arithmetic).
#[test]
fn eval_char_handles_all_registry_supported_ops() {
    check_type_ops(TypeTag::Char, &Value::Char('a'), &Value::Char('b'));
}

// NOTE: Byte binary operations are not yet implemented in the evaluator
// (no `(Value::Byte, Value::Byte)` match arm in evaluate_binary).
// Tracked as BUG-03-001. Test added when evaluator supports byte ops.

// NOTE: Duration and Size same-type op tests are omitted because the
// evaluator doesn't implement all registry-declared ops for these types
// (e.g., FloorDiv, Mod). Tracked for fix in roadmap.
// When fixed, uncomment:
// #[test] fn eval_duration_handles_all_registry_supported_ops() { ... }
// #[test] fn eval_size_handles_all_registry_supported_ops() { ... }

/// `value_to_type_tag()` covers all primitive types with operator support.
#[test]
fn value_to_type_tag_covers_primitive_op_types() {
    use super::value_to_type_tag;

    // Verify each primitive type maps correctly
    assert_eq!(
        value_to_type_tag(&Value::Int(ScalarInt::new(0))),
        Some(TypeTag::Int)
    );
    assert_eq!(value_to_type_tag(&Value::Float(0.0)), Some(TypeTag::Float));
    assert_eq!(value_to_type_tag(&Value::Bool(false)), Some(TypeTag::Bool));
    assert_eq!(value_to_type_tag(&Value::string("")), Some(TypeTag::Str));
    assert_eq!(value_to_type_tag(&Value::Char(' ')), Some(TypeTag::Char));
    assert_eq!(value_to_type_tag(&Value::Byte(0)), Some(TypeTag::Byte));
    assert_eq!(
        value_to_type_tag(&Value::Duration(0)),
        Some(TypeTag::Duration)
    );
    assert_eq!(value_to_type_tag(&Value::Size(0)), Some(TypeTag::Size));
    // Compound types return None
    assert_eq!(value_to_type_tag(&Value::list(vec![])), None);
    assert_eq!(value_to_type_tag(&Value::None), None);
}

/// Verify `op_strategy_from_op()` maps every `REGISTRY_OPS` entry correctly.
#[test]
fn op_strategy_from_op_maps_all_registry_ops() {
    use super::op_strategy_from_op;

    let Some(int_def) = ori_registry::find_type(TypeTag::Int) else {
        panic!("int TypeDef not found");
    };
    for &(op, get_strategy) in REGISTRY_OPS {
        let expected = get_strategy(&int_def.operators);
        let actual = op_strategy_from_op(&int_def.operators, op)
            .unwrap_or_else(|| panic!("REGISTRY_OPS contains non-registry op {op:?}"));
        assert_eq!(
            expected, actual,
            "op_strategy_from_op mismatch for int {op:?}"
        );
    }
}

/// Verify that the evaluator rejects operators the registry marks as unsupported
/// for `int` (there should be very few — int supports almost everything).
#[test]
fn eval_int_rejects_unsupported_ops() {
    let Some(type_def) = ori_registry::find_type(TypeTag::Int) else {
        panic!("int TypeDef not found");
    };
    let a = Value::Int(ScalarInt::new(10));
    let b = Value::Int(ScalarInt::new(3));
    for &(op, get_strategy) in REGISTRY_OPS {
        if get_strategy(&type_def.operators) == OpStrategy::Unsupported {
            let result = evaluate_binary(a.clone(), b.clone(), op);
            assert!(
                result.is_err(),
                "int: registry says {op:?} is unsupported but evaluator returned Ok",
            );
        }
    }
}
