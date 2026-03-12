//! Tests for immortal object detection.

use ori_ir::{Name, StringInterner};
use ori_types::Idx;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, LitValue,
};

use super::{count_immortals, detect_immortals};

fn var(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn ty(n: u32) -> Idx {
    Idx::from_raw(n)
}

fn block_id(n: u32) -> ArcBlockId {
    ArcBlockId::new(n)
}

fn name(n: u32) -> Name {
    Name::from_raw(n)
}

/// Build a single-block function with the given body instructions.
fn single_block_func(num_vars: usize, body: Vec<ArcInstr>) -> ArcFunction {
    ArcFunction {
        name: name(1),
        var_types: vec![ty(0); num_vars],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body,
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    }
}

#[test]
fn empty_string_is_immortal() {
    let interner = StringInterner::new();
    let empty_name = interner.intern("");

    let func = single_block_func(
        1,
        vec![ArcInstr::Let {
            dst: var(0),
            ty: ty(0),
            value: ArcValue::Literal(LitValue::String(empty_name)),
        }],
    );

    let immortals = detect_immortals(&func, &interner);
    assert!(immortals[0], "empty string literal should be immortal");
    assert_eq!(count_immortals(&immortals), 1);
}

#[test]
fn non_empty_string_is_not_immortal() {
    let interner = StringInterner::new();
    let hello_name = interner.intern("hello");

    let func = single_block_func(
        1,
        vec![ArcInstr::Let {
            dst: var(0),
            ty: ty(0),
            value: ArcValue::Literal(LitValue::String(hello_name)),
        }],
    );

    let immortals = detect_immortals(&func, &interner);
    assert!(!immortals[0], "non-empty string should not be immortal");
    assert_eq!(count_immortals(&immortals), 0);
}

#[test]
fn int_literal_is_not_immortal() {
    let interner = StringInterner::new();

    let func = single_block_func(
        1,
        vec![ArcInstr::Let {
            dst: var(0),
            ty: ty(0),
            value: ArcValue::Literal(LitValue::Int(42)),
        }],
    );

    let immortals = detect_immortals(&func, &interner);
    assert!(
        !immortals[0],
        "int literal should not be immortal (already scalar)"
    );
}

#[test]
fn bool_literal_is_not_immortal() {
    let interner = StringInterner::new();

    let func = single_block_func(
        1,
        vec![ArcInstr::Let {
            dst: var(0),
            ty: ty(0),
            value: ArcValue::Literal(LitValue::Bool(true)),
        }],
    );

    let immortals = detect_immortals(&func, &interner);
    assert!(
        !immortals[0],
        "bool literal should not be immortal (already scalar)"
    );
}

#[test]
fn unit_literal_is_not_immortal() {
    let interner = StringInterner::new();

    let func = single_block_func(
        1,
        vec![ArcInstr::Let {
            dst: var(0),
            ty: ty(0),
            value: ArcValue::Literal(LitValue::Unit),
        }],
    );

    let immortals = detect_immortals(&func, &interner);
    assert!(
        !immortals[0],
        "unit literal should not be immortal (already scalar)"
    );
}

#[test]
fn multiple_vars_mixed_immortality() {
    let interner = StringInterner::new();
    let empty_name = interner.intern("");
    let hello_name = interner.intern("hello");

    let func = single_block_func(
        3,
        vec![
            ArcInstr::Let {
                dst: var(0),
                ty: ty(0),
                value: ArcValue::Literal(LitValue::String(empty_name)),
            },
            ArcInstr::Let {
                dst: var(1),
                ty: ty(0),
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            ArcInstr::Let {
                dst: var(2),
                ty: ty(0),
                value: ArcValue::Literal(LitValue::String(hello_name)),
            },
        ],
    );

    let immortals = detect_immortals(&func, &interner);
    assert!(immortals[0], "empty string should be immortal");
    assert!(!immortals[1], "int should not be immortal");
    assert!(!immortals[2], "non-empty string should not be immortal");
    assert_eq!(count_immortals(&immortals), 1);
}

#[test]
fn no_instructions_yields_no_immortals() {
    let interner = StringInterner::new();
    let func = single_block_func(2, vec![]);

    let immortals = detect_immortals(&func, &interner);
    assert_eq!(count_immortals(&immortals), 0);
}

#[test]
fn non_literal_var_is_not_immortal() {
    let interner = StringInterner::new();

    let func = single_block_func(
        2,
        vec![ArcInstr::Let {
            dst: var(0),
            ty: ty(0),
            value: ArcValue::Var(var(1)),
        }],
    );

    let immortals = detect_immortals(&func, &interner);
    assert!(!immortals[0], "variable reference should not be immortal");
}
