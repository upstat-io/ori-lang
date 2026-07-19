//! Tests for `ArcIrBuilder` utility methods.

use ori_types::Idx;

use crate::ir::{ArcBlockId, ArcValue, ArcVarId, CtorKind, LitValue};

use super::ArcIrBuilder;

#[test]
fn get_literal_int_finds_definition() {
    let mut builder = ArcIrBuilder::new();
    let var = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(42)), None);

    assert_eq!(builder.get_literal_int(var), Some(42));
}

#[test]
#[should_panic(expected = "ARC variable table exceeded ArcVarId capacity")]
fn fresh_var_at_carrier_capacity_panics() {
    let mut builder = ArcIrBuilder::new();
    builder.next_var = u32::MAX;

    builder.fresh_var(Idx::INT);
}

#[test]
#[should_panic(expected = "ArcBlockId 1 out of bounds (have 1 blocks)")]
fn position_at_unallocated_block_panics() {
    let mut builder = ArcIrBuilder::new();

    builder.position_at(ArcBlockId::new(1));
}

#[test]
#[should_panic(expected = "ARC variable 0 must have a registered type before use")]
fn var_type_for_unregistered_variable_panics() {
    let builder = ArcIrBuilder::new();

    builder.var_type(ArcVarId::new(0));
}

#[test]
#[should_panic(expected = "block 0 already terminated")]
fn replacing_existing_terminator_panics() {
    let mut builder = ArcIrBuilder::new();
    builder.terminate_unreachable();

    builder.terminate_resume();
}

#[test]
fn get_literal_int_returns_none_for_non_literal() {
    let mut builder = ArcIrBuilder::new();
    let var_a = builder.fresh_var(Idx::INT);
    let var_b = builder.emit_let(Idx::INT, ArcValue::Var(var_a), None);

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
    builder.position_at(block1);
    let var = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(-1)), None);

    assert_eq!(builder.get_literal_int(var), Some(-1));
}

#[test]
fn get_literal_int_ignores_non_int_literals() {
    let mut builder = ArcIrBuilder::new();
    let var = builder.emit_let(Idx::BOOL, ArcValue::Literal(LitValue::Bool(true)), None);

    assert_eq!(builder.get_literal_int(var), None);
}

#[test]
fn get_literal_int_zero() {
    let mut builder = ArcIrBuilder::new();
    let var = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);

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

    let dummy = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
    let step_let = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(1)), None);
    let incl_let = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
    let struct_var = builder.emit_construct(
        Idx::INT,
        CtorKind::Tuple,
        vec![dummy, dummy, step_let, incl_let],
        None,
    );

    // Project field 2 (step) and field 3 (inclusive)
    let step_proj = builder.emit_project(Idx::INT, struct_var, 2, None);
    let incl_proj = builder.emit_project(Idx::INT, struct_var, 3, None);

    assert_eq!(builder.get_literal_int(step_proj), Some(1));
    assert_eq!(builder.get_literal_int(incl_proj), Some(0));
}

#[test]
fn get_literal_int_project_non_literal_arg() {
    // Project from Construct where the arg is not a literal → None
    let mut builder = ArcIrBuilder::new();

    let other = builder.fresh_var(Idx::INT);
    let runtime_var = builder.emit_let(Idx::INT, ArcValue::Var(other), None);
    let struct_var = builder.emit_construct(Idx::INT, CtorKind::Tuple, vec![runtime_var], None);
    let proj = builder.emit_project(Idx::INT, struct_var, 0, None);

    assert_eq!(builder.get_literal_int(proj), None);
}

#[test]
fn get_field_literal_int_without_project() {
    // Query a field's literal value without emitting a Project instruction.
    let mut builder = ArcIrBuilder::new();

    let step_let = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(1)), None);
    let incl_let = builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
    let struct_var =
        builder.emit_construct(Idx::INT, CtorKind::Tuple, vec![step_let, incl_let], None);

    // No Project emitted — query directly from the Construct
    assert_eq!(builder.get_field_literal_int(struct_var, 0), Some(1));
    assert_eq!(builder.get_field_literal_int(struct_var, 1), Some(0));
    assert_eq!(builder.get_field_literal_int(struct_var, 2), None); // out of bounds
}

#[test]
fn get_field_literal_int_runtime_value() {
    let mut builder = ArcIrBuilder::new();

    let other = builder.fresh_var(Idx::INT);
    let runtime = builder.emit_let(Idx::INT, ArcValue::Var(other), None);
    let struct_var = builder.emit_construct(Idx::INT, CtorKind::Tuple, vec![runtime], None);

    assert_eq!(builder.get_field_literal_int(struct_var, 0), None);
}
