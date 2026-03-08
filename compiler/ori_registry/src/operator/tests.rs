use super::*;
use crate::tags::OpStrategy;

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
        add: OpStrategy::IntInstr,
        sub: OpStrategy::IntInstr,
        mul: OpStrategy::IntInstr,
        div: OpStrategy::IntInstr,
        rem: OpStrategy::IntInstr,
        floor_div: OpStrategy::IntInstr,
        eq: OpStrategy::IntInstr,
        neq: OpStrategy::IntInstr,
        lt: OpStrategy::IntInstr,
        gt: OpStrategy::IntInstr,
        lt_eq: OpStrategy::IntInstr,
        gt_eq: OpStrategy::IntInstr,
        neg: OpStrategy::IntInstr,
        not: OpStrategy::Unsupported,
        bit_and: OpStrategy::IntInstr,
        bit_or: OpStrategy::IntInstr,
        bit_xor: OpStrategy::IntInstr,
        bit_not: OpStrategy::IntInstr,
        shl: OpStrategy::IntInstr,
        shr: OpStrategy::IntInstr,
    };

    assert_eq!(INT_OPS.add, OpStrategy::IntInstr);
    assert_eq!(INT_OPS.not, OpStrategy::Unsupported);
    assert_eq!(INT_OPS.bit_and, OpStrategy::IntInstr);
}

#[test]
fn op_defs_size_matches_pointer_width() {
    let expected = if cfg!(target_pointer_width = "64") {
        480 // 20 x 24
    } else {
        240 // 20 x 12
    };
    assert_eq!(
        std::mem::size_of::<OpDefs>(),
        expected,
        "OpDefs should be 20 fields x sizeof(OpStrategy)"
    );
}

#[test]
fn op_defs_field_access_works() {
    let ops = OpDefs {
        add: OpStrategy::RuntimeCall {
            fn_name: "ori_str_concat",
            returns_bool: false,
        },
        eq: OpStrategy::RuntimeCall {
            fn_name: "ori_str_eq",
            returns_bool: true,
        },
        ..OpDefs::UNSUPPORTED
    };

    assert_eq!(
        ops.add,
        OpStrategy::RuntimeCall {
            fn_name: "ori_str_concat",
            returns_bool: false
        }
    );
    assert_eq!(
        ops.eq,
        OpStrategy::RuntimeCall {
            fn_name: "ori_str_eq",
            returns_bool: true
        }
    );
    assert_eq!(ops.sub, OpStrategy::Unsupported);
}
