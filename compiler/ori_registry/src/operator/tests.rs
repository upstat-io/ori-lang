use super::*;
use crate::tags::{OpStrategy, RuntimeOperator};

#[test]
fn op_defs_unsupported_has_all_fields_unsupported() {
    let u = OpDefs::UNSUPPORTED;
    assert_eq!(u.add, OpStrategy::Unsupported);
    assert_eq!(u.sub, OpStrategy::Unsupported);
    assert_eq!(u.mul, OpStrategy::Unsupported);
    assert_eq!(u.div, OpStrategy::Unsupported);
    assert_eq!(u.rem, OpStrategy::Unsupported);
    assert_eq!(u.floor_div, OpStrategy::Unsupported);
    assert_eq!(u.eq, OpStrategy::Unsupported);
    assert_eq!(u.neq, OpStrategy::Unsupported);
    assert_eq!(u.lt, OpStrategy::Unsupported);
    assert_eq!(u.gt, OpStrategy::Unsupported);
    assert_eq!(u.lt_eq, OpStrategy::Unsupported);
    assert_eq!(u.gt_eq, OpStrategy::Unsupported);
    assert_eq!(u.neg, OpStrategy::Unsupported);
    assert_eq!(u.not, OpStrategy::Unsupported);
    assert_eq!(u.bit_and, OpStrategy::Unsupported);
    assert_eq!(u.bit_or, OpStrategy::Unsupported);
    assert_eq!(u.bit_xor, OpStrategy::Unsupported);
    assert_eq!(u.bit_not, OpStrategy::Unsupported);
    assert_eq!(u.shl, OpStrategy::Unsupported);
    assert_eq!(u.shr, OpStrategy::Unsupported);
}

#[test]
fn op_defs_const_constructible_with_mixed_strategies() {
    const INT_OPS: OpDefs = OpDefs {
        add: OpStrategy::SignedInteger,
        sub: OpStrategy::SignedInteger,
        mul: OpStrategy::SignedInteger,
        div: OpStrategy::SignedInteger,
        rem: OpStrategy::SignedInteger,
        floor_div: OpStrategy::SignedInteger,
        eq: OpStrategy::SignedInteger,
        neq: OpStrategy::SignedInteger,
        lt: OpStrategy::SignedInteger,
        gt: OpStrategy::SignedInteger,
        lt_eq: OpStrategy::SignedInteger,
        gt_eq: OpStrategy::SignedInteger,
        neg: OpStrategy::SignedInteger,
        not: OpStrategy::Unsupported,
        bit_and: OpStrategy::SignedInteger,
        bit_or: OpStrategy::SignedInteger,
        bit_xor: OpStrategy::SignedInteger,
        bit_not: OpStrategy::SignedInteger,
        shl: OpStrategy::SignedInteger,
        shr: OpStrategy::SignedInteger,
    };

    assert_eq!(INT_OPS.add, OpStrategy::SignedInteger);
    assert_eq!(INT_OPS.not, OpStrategy::Unsupported);
    assert_eq!(INT_OPS.bit_and, OpStrategy::SignedInteger);
}

#[test]
fn op_defs_size_is_compact() {
    assert_eq!(std::mem::size_of::<OpDefs>(), 20);
}

#[test]
fn op_defs_field_access_works() {
    let ops = OpDefs {
        add: OpStrategy::RuntimeCall(RuntimeOperator::StringConcat),
        eq: OpStrategy::RuntimeCall(RuntimeOperator::StringEqual),
        ..OpDefs::UNSUPPORTED
    };

    assert_eq!(
        ops.add,
        OpStrategy::RuntimeCall(RuntimeOperator::StringConcat)
    );
    assert_eq!(
        ops.eq,
        OpStrategy::RuntimeCall(RuntimeOperator::StringEqual)
    );
    assert_eq!(ops.sub, OpStrategy::Unsupported);
}
