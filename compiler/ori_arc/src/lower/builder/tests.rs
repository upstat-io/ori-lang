//! Tests for `ArcIrBuilder` utility methods.

use ori_types::Idx;

use crate::ir::{ArcInstr, ArcValue, ArcVarId, CtorKind, LitValue};

use super::ArcIrBuilder;

#[test]
fn get_literal_int_finds_definition() {
    let mut builder = ArcIrBuilder::new();
    let var = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Let {
        dst: var,
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(42)),
    });

    assert_eq!(builder.get_literal_int(var), Some(42));
}

#[test]
fn get_literal_int_returns_none_for_non_literal() {
    let mut builder = ArcIrBuilder::new();
    let var_a = builder.fresh_var(Idx::INT);
    let var_b = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Let {
        dst: var_b,
        ty: Idx::INT,
        value: ArcValue::Var(var_a),
    });

    assert_eq!(builder.get_literal_int(var_b), None);
}

#[test]
fn get_literal_int_returns_none_for_unknown_var() {
    let builder = ArcIrBuilder::new();
    let unknown = ArcVarId::new(999);
    assert_eq!(builder.get_literal_int(unknown), None);
}

#[test]
fn get_literal_int_finds_across_blocks() {
    let mut builder = ArcIrBuilder::new();
    let block1 = builder.new_block();

    let var = builder.fresh_var(Idx::INT);
    builder.blocks[block1.index()].body.push(ArcInstr::Let {
        dst: var,
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(-1)),
    });

    assert_eq!(builder.get_literal_int(var), Some(-1));
}

#[test]
fn get_literal_int_ignores_non_int_literals() {
    let mut builder = ArcIrBuilder::new();
    let var = builder.fresh_var(Idx::BOOL);
    builder.blocks[0].body.push(ArcInstr::Let {
        dst: var,
        ty: Idx::BOOL,
        value: ArcValue::Literal(LitValue::Bool(true)),
    });

    assert_eq!(builder.get_literal_int(var), None);
}

#[test]
fn get_literal_int_zero() {
    let mut builder = ArcIrBuilder::new();
    let var = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Let {
        dst: var,
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(0)),
    });

    assert_eq!(builder.get_literal_int(var), Some(0));
}

#[test]
fn get_literal_int_traces_through_project_construct() {
    // Simulates the range struct pattern:
    //   step_let = Let(Literal(Int(1)))
    //   struct_var = Construct(Tuple, [_, _, step_let, _])
    //   step_proj = Project(struct_var, field=2)
    // get_literal_int(step_proj) should return Some(1)
    let mut builder = ArcIrBuilder::new();

    let dummy = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Let {
        dst: dummy,
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(0)),
    });

    let step_let = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Let {
        dst: step_let,
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(1)),
    });

    let incl_let = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Let {
        dst: incl_let,
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(0)),
    });

    let struct_var = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Construct {
        dst: struct_var,
        ty: Idx::INT,
        ctor: CtorKind::Tuple,
        args: vec![dummy, dummy, step_let, incl_let],
    });

    // Project field 2 (step) and field 3 (inclusive)
    let step_proj = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Project {
        dst: step_proj,
        ty: Idx::INT,
        value: struct_var,
        field: 2,
    });

    let incl_proj = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Project {
        dst: incl_proj,
        ty: Idx::INT,
        value: struct_var,
        field: 3,
    });

    assert_eq!(builder.get_literal_int(step_proj), Some(1));
    assert_eq!(builder.get_literal_int(incl_proj), Some(0));
}

#[test]
fn get_literal_int_project_non_literal_arg() {
    // Project from Construct where the arg is not a literal → None
    let mut builder = ArcIrBuilder::new();

    let runtime_var = builder.fresh_var(Idx::INT);
    // runtime_var is defined as Var (not Literal)
    let other = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Let {
        dst: runtime_var,
        ty: Idx::INT,
        value: ArcValue::Var(other),
    });

    let struct_var = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Construct {
        dst: struct_var,
        ty: Idx::INT,
        ctor: CtorKind::Tuple,
        args: vec![runtime_var],
    });

    let proj = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Project {
        dst: proj,
        ty: Idx::INT,
        value: struct_var,
        field: 0,
    });

    assert_eq!(builder.get_literal_int(proj), None);
}

#[test]
fn get_field_literal_int_without_project() {
    // Query a field's literal value without emitting a Project instruction.
    let mut builder = ArcIrBuilder::new();

    let step_let = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Let {
        dst: step_let,
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(1)),
    });

    let incl_let = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Let {
        dst: incl_let,
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(0)),
    });

    let struct_var = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Construct {
        dst: struct_var,
        ty: Idx::INT,
        ctor: CtorKind::Tuple,
        args: vec![step_let, incl_let],
    });

    // No Project emitted — query directly from the Construct
    assert_eq!(builder.get_field_literal_int(struct_var, 0), Some(1));
    assert_eq!(builder.get_field_literal_int(struct_var, 1), Some(0));
    assert_eq!(builder.get_field_literal_int(struct_var, 2), None); // out of bounds
}

#[test]
fn get_field_literal_int_runtime_value() {
    let mut builder = ArcIrBuilder::new();

    let runtime = builder.fresh_var(Idx::INT);
    let other = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Let {
        dst: runtime,
        ty: Idx::INT,
        value: ArcValue::Var(other),
    });

    let struct_var = builder.fresh_var(Idx::INT);
    builder.blocks[0].body.push(ArcInstr::Construct {
        dst: struct_var,
        ty: Idx::INT,
        ctor: CtorKind::Tuple,
        args: vec![runtime],
    });

    assert_eq!(builder.get_field_literal_int(struct_var, 0), None);
}
