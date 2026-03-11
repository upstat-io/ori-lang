//! Tests for AIMS RC emission.
//!
//! Verifies that `emit_rc_ops` emits correct `RcInc`/`RcDec` operations
//! from a converged `AimsStateMap`.

use rustc_hash::FxHashMap;

use ori_ir::Name;
use ori_types::Idx;

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{
    AccessClass, AimsState, Cardinality, Consumption, EffectClass, Locality, ShapeClass, Uniqueness,
};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    LitValue, ValueRepr,
};
use crate::test_helpers::TestClassifier;
use crate::uniqueness::drop_hints::DropHints;
use crate::uniqueness::CowAnnotations;
use crate::Ownership;

use super::emit_rc_ops;

/// Helper: create a minimal [`ArcFunction`] with the given blocks.
fn make_func(
    blocks: Vec<ArcBlock>,
    num_vars: usize,
    params: Vec<ArcParam>,
    var_reprs: Vec<ValueRepr>,
) -> ArcFunction {
    ArcFunction {
        name: Name::new(0, 0),
        params,
        return_type: Idx::NONE,
        blocks,
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::NONE; num_vars],
        var_reprs,
        spans: Vec::new(),
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
    }
}

/// Helper: create an Owned [`AimsState`] with given cardinality.
fn owned_state(card: Cardinality) -> AimsState {
    AimsState {
        access: AccessClass::Owned,
        consumption: match card {
            Cardinality::Absent => Consumption::Dead,
            Cardinality::Once => Consumption::Linear,
            Cardinality::Many => Consumption::Unrestricted,
        },
        cardinality: card,
        uniqueness: Uniqueness::Unique,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    }
}

/// Single-use variable: no `RcInc`, `RcDec` after last use.
#[test]
fn single_use_emits_dec_only() {
    // v0: parameter (RcPointer), used once, dead at exit.
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Var(v0),
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(
        vec![block],
        2,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![ValueRepr::RcPointer, ValueRepr::Scalar],
    );

    // Set up state map: v0 is Owned, Once at entry; Absent at exit.
    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, owned_state(Cardinality::Once))].into_iter().collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let pool = ori_types::Pool::new();
    let classifier = TestClassifier;
    let sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    emit_rc_ops(&mut func, &state_map, &sigs, &classifier, &pool);

    // Expected: Let(v1 = v0), RcDec(v0), then Return(v1).
    let body = &func.blocks[0].body;
    assert_eq!(body.len(), 2, "body: {body:?}");
    assert!(matches!(body[0], ArcInstr::Let { .. }));
    assert!(
        matches!(body[1], ArcInstr::RcDec { var, .. } if var == v0),
        "expected RcDec(v0), got: {:?}",
        body[1]
    );
}

/// Multi-use variable: `RcInc` before second use.
#[test]
fn multi_use_emits_inc_before_second_use() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);

    // v0 used twice: once in Let(v1 = v0), once in Let(v2 = v0).
    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Let {
                dst: v1,
                ty: Idx::NONE,
                value: ArcValue::Var(v0),
            },
            ArcInstr::Let {
                dst: v2,
                ty: Idx::NONE,
                value: ArcValue::Var(v0),
            },
        ],
        terminator: ArcTerminator::Return { value: v2 },
    };

    let mut func = make_func(
        vec![block],
        3,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![ValueRepr::RcPointer, ValueRepr::Scalar, ValueRepr::Scalar],
    );

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, owned_state(Cardinality::Many))].into_iter().collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let pool = ori_types::Pool::new();
    let classifier = TestClassifier;
    let sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    emit_rc_ops(&mut func, &state_map, &sigs, &classifier, &pool);

    // Expected: RcInc(v0), Let(v1 = v0), Let(v2 = v0), RcDec(v0).
    // The first use (in Let v1) has a future use (Let v2) → RcInc.
    // The second use (in Let v2) is the last use (dead at exit) → no RcInc, RcDec.
    let body = &func.blocks[0].body;
    assert_eq!(body.len(), 4, "body: {body:?}");
    assert!(
        matches!(body[0], ArcInstr::RcInc { var, .. } if var == v0),
        "expected RcInc(v0) before first use, got: {:?}",
        body[0]
    );
    assert!(matches!(body[1], ArcInstr::Let { dst, .. } if dst == v1));
    assert!(matches!(body[2], ArcInstr::Let { dst, .. } if dst == v2));
    assert!(
        matches!(body[3], ArcInstr::RcDec { var, .. } if var == v0),
        "expected RcDec(v0) after last use, got: {:?}",
        body[3]
    );
}

/// Variable used in body AND in terminator (Return): the body use gets `RcInc`
/// because the Return is a future use.
#[test]
fn body_and_terminator_use() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    // v0 used in body (Let v1 = v0) and in terminator (Return v0).
    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Var(v0),
        }],
        terminator: ArcTerminator::Return { value: v0 },
    };

    let mut func = make_func(
        vec![block],
        2,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![ValueRepr::RcPointer, ValueRepr::Scalar],
    );

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, owned_state(Cardinality::Many))].into_iter().collect(),
    );
    // Dead at exit (Return has no successor blocks).
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let pool = ori_types::Pool::new();
    let classifier = TestClassifier;
    let sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    emit_rc_ops(&mut func, &state_map, &sigs, &classifier, &pool);

    // body use of v0 has future use in terminator → RcInc.
    // Return transfers ownership → no RcDec.
    let body = &func.blocks[0].body;
    assert_eq!(body.len(), 2, "body: {body:?}");
    assert!(
        matches!(body[0], ArcInstr::RcInc { var, .. } if var == v0),
        "expected RcInc(v0), got: {:?}",
        body[0]
    );
    assert!(matches!(body[1], ArcInstr::Let { dst, .. } if dst == v1));
}

/// Scalar variables are completely skipped.
#[test]
fn scalar_variables_skipped() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Literal(LitValue::Int(42)),
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(
        vec![block],
        2,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![ValueRepr::Scalar, ValueRepr::Scalar],
    );

    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v0);
    state_map.set_permanent_scalar(v1);

    let pool = ori_types::Pool::new();
    let classifier = TestClassifier;
    let sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    emit_rc_ops(&mut func, &state_map, &sigs, &classifier, &pool);

    // No RC operations inserted.
    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "expected no RC ops for scalars, body: {body:?}"
    );
    assert!(matches!(body[0], ArcInstr::Let { .. }));
}

/// Borrowed variables are skipped (access != Owned).
#[test]
fn borrowed_variables_skipped() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Var(v0),
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(
        vec![block],
        2,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Borrowed,
        }],
        vec![ValueRepr::RcPointer, ValueRepr::Scalar],
    );

    // v0 is Borrowed at entry.
    let borrowed_state = AimsState {
        access: AccessClass::Borrowed,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::Unique,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };
    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, borrowed_state)].into_iter().collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let pool = ori_types::Pool::new();
    let classifier = TestClassifier;
    let sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    emit_rc_ops(&mut func, &state_map, &sigs, &classifier, &pool);

    // No RC operations for borrowed v0.
    let body = &func.blocks[0].body;
    assert_eq!(
        body.len(),
        1,
        "expected no RC ops for borrowed var, body: {body:?}"
    );
}

/// Dead parameter at entry: `RcDec` emitted at block start.
#[test]
fn dead_parameter_emits_dec_at_entry() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    // v0 is a parameter that's never used in the body.
    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Literal(LitValue::Int(42)),
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(
        vec![block],
        2,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![ValueRepr::RcPointer, ValueRepr::Scalar],
    );

    // v0 is Owned at entry but never used in this block and dead at exit.
    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, owned_state(Cardinality::Once))].into_iter().collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let pool = ori_types::Pool::new();
    let classifier = TestClassifier;
    let sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    emit_rc_ops(&mut func, &state_map, &sigs, &classifier, &pool);

    // Expected: RcDec(v0) at the start, then Let(v1 = 42).
    let body = &func.blocks[0].body;
    assert_eq!(body.len(), 2, "body: {body:?}");
    assert!(
        matches!(body[0], ArcInstr::RcDec { var, .. } if var == v0),
        "expected RcDec(v0) at entry, got: {:?}",
        body[0]
    );
    assert!(matches!(body[1], ArcInstr::Let { .. }));
}
