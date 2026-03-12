//! Tests for AIMS RC emission and COW annotation.
//!
//! Verifies that `emit_rc_ops` emits correct `RcInc`/`RcDec` operations
//! from a converged `AimsStateMap`, and that `compute_aims_cow_annotations`
//! applies cross-dimensional optimizations from Section 07.3:
//! - **07.3.1**: COW-aware borrowing (parameter `Owned`+`Linear`+`Once` → `StaticUnique`)
//! - **07.3.2**: Uniqueness-preserving borrows (disjoint-field optimization)
//! - **07.3.3**: Demand-driven RC elimination (`Absent` parameter skips RC ops)

use rustc_hash::FxHashMap;

use ori_ir::Name;
use ori_types::Idx;

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{
    AccessClass, AimsState, BorrowSource, Cardinality, Consumption, EffectClass, Locality,
    ShapeClass, Uniqueness,
};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, LitValue, ValueRepr,
};
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
    emit_rc_ops(&mut func, &state_map, &pool);

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
    emit_rc_ops(&mut func, &state_map, &pool);

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
    emit_rc_ops(&mut func, &state_map, &pool);

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
    emit_rc_ops(&mut func, &state_map, &pool);

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
    emit_rc_ops(&mut func, &state_map, &pool);

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
    emit_rc_ops(&mut func, &state_map, &pool);

    // Expected: Let(v1 = 42) then RcDec(v0).
    // The Let uses a literal (not v0), so the coalescing peephole
    // delays the RcDec flush to block end — equivalent ordering.
    let body = &func.blocks[0].body;
    assert_eq!(body.len(), 2, "body: {body:?}");
    assert!(matches!(body[0], ArcInstr::Let { .. }));
    assert!(
        matches!(body[1], ArcInstr::RcDec { var, .. } if var == v0),
        "expected RcDec(v0), got: {:?}",
        body[1]
    );
}

// COW-aware borrowing tests (Section 07.3.1)

use ori_ir::StringInterner;

use super::cow::compute_aims_cow_annotations;
use crate::CowMode;

/// Helper: create a `StringInterner` with "push" interned (a known COW method).
fn make_cow_interner() -> (StringInterner, Name) {
    let interner = StringInterner::default();
    let push_name = interner.intern("push");
    (interner, push_name)
}

/// COW-aware borrowing: parameter with `(Owned, Linear, Once, MaybeShared)`
/// gets `StaticUnique` via cross-dimensional reasoning. The three non-uniqueness
/// dimensions prove the callee received ownership, never duplicated the reference,
/// and uses it once — safe for in-place mutation.
#[test]
fn cow_aware_borrowing_static_unique_for_linear_owned_unique_param() {
    let v0 = ArcVarId::new(0); // parameter: list (RcPointer)
    let v1 = ArcVarId::new(1); // literal arg
    let v2 = ArcVarId::new(2); // result of push

    let (interner, push_name) = make_cow_interner();

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Let {
                dst: v1,
                ty: Idx::NONE,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            // Apply push(v0, v1) — COW operation on v0.
            ArcInstr::Apply {
                dst: v2,
                ty: Idx::NONE,
                func: push_name,
                args: vec![v0, v1],
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return { value: v2 },
    };

    let func = make_func(
        vec![block],
        3,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
        ],
    );

    // Key: set v0's uniqueness to MaybeShared but access/consumption/cardinality
    // to (Owned, Linear, Once). Without COW-aware borrowing, this would be Dynamic.
    let cow_aware_state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::MaybeShared, // <-- would normally → Dynamic
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, cow_aware_state)].into_iter().collect(),
    );

    let annotations = compute_aims_cow_annotations(&func, &state_map, &interner);

    // The COW operation at (block 0, instr 1) should get StaticUnique
    // because of cross-dimensional reasoning, not Dynamic.
    let mode = annotations.get(0, 1);
    assert_eq!(
        mode,
        CowMode::StaticUnique,
        "parameter with (Owned, Linear, Once, MaybeShared) should get \
         StaticUnique via COW-aware borrowing, not Dynamic"
    );
}

/// Non-parameter variable with `(Owned, Linear, Once, MaybeShared)` does NOT
/// get the COW-aware override — it gets `Dynamic` from the uniqueness dimension.
/// The optimization applies only to function parameters.
#[test]
fn cow_aware_borrowing_non_param_stays_dynamic() {
    let v0 = ArcVarId::new(0); // non-param variable (defined in block)
    let v1 = ArcVarId::new(1); // literal arg
    let v2 = ArcVarId::new(2); // result of push

    let (interner, push_name) = make_cow_interner();

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            // v0 defined by some other instruction (not a parameter).
            ArcInstr::Let {
                dst: v0,
                ty: Idx::NONE,
                value: ArcValue::Literal(LitValue::Int(0)),
            },
            ArcInstr::Let {
                dst: v1,
                ty: Idx::NONE,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            ArcInstr::Apply {
                dst: v2,
                ty: Idx::NONE,
                func: push_name,
                args: vec![v0, v1],
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return { value: v2 },
    };

    // No parameters — v0 is a local, not a param.
    let func = make_func(
        vec![block],
        3,
        vec![],
        vec![
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
        ],
    );

    let cow_state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::MaybeShared,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(ArcBlockId::new(0), [(v0, cow_state)].into_iter().collect());

    let annotations = compute_aims_cow_annotations(&func, &state_map, &interner);

    // Non-param variable: no COW-aware override → MaybeShared → Dynamic.
    let mode = annotations.get(0, 2);
    assert_eq!(
        mode,
        CowMode::Dynamic,
        "non-parameter variable with MaybeShared should get Dynamic, \
         COW-aware borrowing only applies to function parameters"
    );
}

/// Parameter with `(Owned, Unrestricted, Many, MaybeShared)` does NOT qualify
/// for COW-aware borrowing — consumption must be `Linear` and cardinality `Once`.
#[test]
fn cow_aware_borrowing_multi_use_param_stays_dynamic() {
    let v0 = ArcVarId::new(0); // parameter: used many times
    let v1 = ArcVarId::new(1); // literal arg
    let v2 = ArcVarId::new(2); // result of push

    let (interner, push_name) = make_cow_interner();

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Let {
                dst: v1,
                ty: Idx::NONE,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            ArcInstr::Apply {
                dst: v2,
                ty: Idx::NONE,
                func: push_name,
                args: vec![v0, v1],
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return { value: v2 },
    };

    let func = make_func(
        vec![block],
        3,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
        ],
    );

    // Multi-use parameter: (Owned, Unrestricted, Many, MaybeShared).
    // COW-aware borrowing requires (Owned, Linear, Once) — this doesn't qualify.
    let multi_use_state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Unrestricted,
        cardinality: Cardinality::Many,
        uniqueness: Uniqueness::MaybeShared,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, multi_use_state)].into_iter().collect(),
    );

    let annotations = compute_aims_cow_annotations(&func, &state_map, &interner);

    // Multi-use parameter: no COW-aware override → MaybeShared → Dynamic.
    let mode = annotations.get(0, 1);
    assert_eq!(
        mode,
        CowMode::Dynamic,
        "parameter with (Owned, Unrestricted, Many, MaybeShared) should get Dynamic — \
         COW-aware borrowing requires (Owned, Linear, Once)"
    );
}

// Uniqueness-preserving borrow tests (Section 07.3.2)

/// When `v1 = Project(src, field_a)` and there's a sibling borrow
/// `v2 = Project(src, field_b)` (disjoint field), a COW operation on `v1`
/// can use `StaticUnique` even though `v1` is `MaybeShared`, because the
/// source is `Unique` and no sibling borrow aliases the receiver's field.
#[test]
fn uniqueness_preserving_borrow_disjoint_field_cow_is_static() {
    let v_src = ArcVarId::new(0); // source struct (parameter)
    let v1 = ArcVarId::new(1); // borrow of src.field_0
    let v2 = ArcVarId::new(2); // borrow of src.field_1 (disjoint)
    let v_arg = ArcVarId::new(3); // literal arg for push
    let v_result = ArcVarId::new(4); // result of push

    let (interner, push_name) = make_cow_interner();

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            // v1 = Project(v_src, field 0)
            ArcInstr::Project {
                dst: v1,
                ty: Idx::NONE,
                value: v_src,
                field: 0,
            },
            // v2 = Project(v_src, field 1) — disjoint sibling borrow
            ArcInstr::Project {
                dst: v2,
                ty: Idx::NONE,
                value: v_src,
                field: 1,
            },
            ArcInstr::Let {
                dst: v_arg,
                ty: Idx::NONE,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            // COW push on v1 (field 0 borrow) — disjoint from v2 (field 1)
            ArcInstr::Apply {
                dst: v_result,
                ty: Idx::NONE,
                func: push_name,
                args: vec![v1, v_arg],
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return { value: v_result },
    };

    let func = make_func(
        vec![block],
        5,
        vec![ArcParam {
            var: v_src,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
        ],
    );

    // v_src is Unique (the source struct is uniquely owned).
    let src_state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::Unique,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    // v1 borrows from src — MaybeShared (conservative, e.g., from a join).
    let borrow_state = AimsState {
        access: AccessClass::Borrowed,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::MaybeShared, // would normally → Dynamic
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v_src, src_state), (v1, borrow_state), (v2, borrow_state)]
            .into_iter()
            .collect(),
    );
    // Record borrow provenance: v1 borrows src.field_0, v2 borrows src.field_1
    state_map.set_borrow_source(v1, BorrowSource::exact_field(v_src, 0));
    state_map.set_borrow_source(v2, BorrowSource::exact_field(v_src, 1));

    let annotations = compute_aims_cow_annotations(&func, &state_map, &interner);

    // COW on v1 at (block 0, instr 3): disjoint-field optimization → StaticUnique
    let mode = annotations.get(0, 3);
    assert_eq!(
        mode,
        CowMode::StaticUnique,
        "borrow of field 0 with disjoint sibling borrow on field 1 should \
         get StaticUnique via uniqueness-preserving borrow optimization"
    );
}

/// When `v1 = Project(src, field_a)` and there's a sibling borrow
/// `v2 = Project(src, field_a)` (SAME field), a COW operation on `v1`
/// must stay `Dynamic` — the sibling borrow aliases the same field.
#[test]
fn uniqueness_preserving_borrow_same_field_cow_is_dynamic() {
    let v_src = ArcVarId::new(0); // source struct (parameter)
    let v1 = ArcVarId::new(1); // borrow of src.field_0
    let v2 = ArcVarId::new(2); // borrow of src.field_0 (SAME field!)
    let v_arg = ArcVarId::new(3); // literal arg
    let v_result = ArcVarId::new(4); // result

    let (interner, push_name) = make_cow_interner();

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Project {
                dst: v1,
                ty: Idx::NONE,
                value: v_src,
                field: 0,
            },
            ArcInstr::Project {
                dst: v2,
                ty: Idx::NONE,
                value: v_src,
                field: 0, // same field as v1!
            },
            ArcInstr::Let {
                dst: v_arg,
                ty: Idx::NONE,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            // COW push on v1 (field 0) — conflicts with v2 (also field 0)
            ArcInstr::Apply {
                dst: v_result,
                ty: Idx::NONE,
                func: push_name,
                args: vec![v1, v_arg],
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return { value: v_result },
    };

    let func = make_func(
        vec![block],
        5,
        vec![ArcParam {
            var: v_src,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
        ],
    );

    let src_state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::Unique,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    let borrow_state = AimsState {
        access: AccessClass::Borrowed,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::MaybeShared,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v_src, src_state), (v1, borrow_state), (v2, borrow_state)]
            .into_iter()
            .collect(),
    );
    // Both v1 and v2 borrow src.field_0 — same field!
    state_map.set_borrow_source(v1, BorrowSource::exact_field(v_src, 0));
    state_map.set_borrow_source(v2, BorrowSource::exact_field(v_src, 0));

    let annotations = compute_aims_cow_annotations(&func, &state_map, &interner);

    // COW on v1 at (block 0, instr 3): same-field sibling → Dynamic (soundness guard)
    let mode = annotations.get(0, 3);
    assert_eq!(
        mode,
        CowMode::Dynamic,
        "borrow of field 0 with sibling borrow on SAME field 0 must stay Dynamic — \
         aliasing borrows prevent in-place mutation"
    );
}

// Demand-driven RC elimination tests (Section 07.3.3)

/// Parameter with `Absent` cardinality (never used): no `RcDec` emitted at
/// function entry. The caller passes as `Borrowed`, retaining ownership.
#[test]
fn absent_param_no_rc_dec_at_entry() {
    let v0 = ArcVarId::new(0); // parameter: never used (Absent)
    let v1 = ArcVarId::new(1); // result (scalar literal)

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Literal(LitValue::Int(0)),
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

    // v0 is Absent — never used by the callee.
    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, owned_state(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let pool = ori_types::Pool::new();
    emit_rc_ops(&mut func, &state_map, &pool);

    // No RcDec for v0 — it's Absent, so the callee doesn't need to release it.
    let body = &func.blocks[0].body;
    for instr in body {
        if let ArcInstr::RcDec { var, .. } = instr {
            assert_ne!(
                *var, v0,
                "Absent parameter should NOT get RcDec — caller retains ownership"
            );
        }
    }
}

/// Parameter that IS used (Once cardinality) gets normal `RcDec` after last use.
/// Contrast with `absent_param_no_rc_dec_at_entry` to verify the optimization
/// only applies to truly unused parameters.
#[test]
fn used_param_gets_rc_dec() {
    let v0 = ArcVarId::new(0); // parameter: used once
    let v1 = ArcVarId::new(1); // result

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Var(v0), // uses v0
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

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, owned_state(Cardinality::Once))].into_iter().collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());

    let pool = ori_types::Pool::new();
    emit_rc_ops(&mut func, &state_map, &pool);

    // v0 IS used (Once), so it should get RcDec after last use.
    let has_dec = func.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::RcDec { var, .. } if *var == v0));
    assert!(
        has_dec,
        "used parameter (Once cardinality) should get RcDec after last use"
    );
}

// Cross-dimension synergy integration test (Section 07.3.4)

/// Demonstrates that AIMS cross-dimensional reasoning eliminates a runtime
/// check that NEITHER the uniqueness dimension NOR the cardinality dimension
/// could eliminate alone.
///
/// Setup: function parameter with state `(Owned, Linear, Once, MaybeShared)`.
/// - Uniqueness dimension alone: `MaybeShared` → `Dynamic` (runtime check)
/// - Cardinality dimension alone: `Once` says nothing about uniqueness
/// - Access+Consumption+Cardinality combined: `(Owned, Linear, Once)` proves
///   the callee received ownership, never duplicated it, and uses it exactly
///   once → provably unique at the COW point → `StaticUnique` (no runtime check)
///
/// This is the canonical example of cross-dimensional optimization: three
/// non-uniqueness dimensions combine to override the uniqueness dimension.
#[test]
fn cross_dimension_synergy_cow_aware_borrowing() {
    let v0 = ArcVarId::new(0); // parameter
    let v1 = ArcVarId::new(1); // literal
    let v2 = ArcVarId::new(2); // result

    let (interner, push_name) = make_cow_interner();

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Let {
                dst: v1,
                ty: Idx::NONE,
                value: ArcValue::Literal(LitValue::Int(1)),
            },
            ArcInstr::Apply {
                dst: v2,
                ty: Idx::NONE,
                func: push_name,
                args: vec![v0, v1],
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return { value: v2 },
    };

    let func = make_func(
        vec![block],
        3,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
        ],
    );

    // The cross-dimensional state: access+consumption+cardinality prove uniqueness,
    // but the uniqueness dimension itself says MaybeShared.
    let state = AimsState {
        access: AccessClass::Owned,          // dimension 1: received ownership
        consumption: Consumption::Linear,    // dimension 2: no duplication
        cardinality: Cardinality::Once,      // dimension 3: single use
        uniqueness: Uniqueness::MaybeShared, // dimension 4: alone → Dynamic
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(ArcBlockId::new(0), [(v0, state)].into_iter().collect());

    let annotations = compute_aims_cow_annotations(&func, &state_map, &interner);
    let mode = annotations.get(0, 1);

    // Without cross-dimensional reasoning: MaybeShared → Dynamic (runtime check).
    // With cross-dimensional reasoning: (Owned, Linear, Once) → StaticUnique.
    // This proves the optimization requires MULTIPLE dimensions working together.
    assert_eq!(
        mode,
        CowMode::StaticUnique,
        "cross-dimensional synergy: (Owned, Linear, Once) overrides MaybeShared → \
         StaticUnique; neither uniqueness alone (Dynamic) nor cardinality alone \
         (no COW info) could achieve this"
    );
}

// Transfer fusion tests (Section 09.1)

/// Transfer Fusion Rule 1: Unique source projection eliminates COW check.
///
/// When a struct is `Unique` and a field is projected from it, `transfer_project`
/// propagates the source's uniqueness to the projected field. This means a COW
/// operation on the projected field gets `StaticUnique` — no runtime uniqueness
/// check needed.
///
/// Without uniqueness propagation through Project, the projected field would
/// default to `MaybeShared` (conservative for borrows) → `Dynamic` → runtime check.
/// With the transfer rule, it inherits `Unique` from the source → `StaticUnique`.
#[test]
fn unique_source_projection_eliminates_cow_check() {
    let v_src = ArcVarId::new(0); // Unique struct parameter
    let v_field = ArcVarId::new(1); // Project(v_src, field 0)
    let v_arg = ArcVarId::new(2); // literal arg for push
    let v_result = ArcVarId::new(3); // result of push

    let (interner, push_name) = make_cow_interner();

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            // v_field = Project(v_src, field 0)
            ArcInstr::Project {
                dst: v_field,
                ty: Idx::NONE,
                value: v_src,
                field: 0,
            },
            ArcInstr::Let {
                dst: v_arg,
                ty: Idx::NONE,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            // COW push on the projected field
            ArcInstr::Apply {
                dst: v_result,
                ty: Idx::NONE,
                func: push_name,
                args: vec![v_field, v_arg],
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return { value: v_result },
    };

    let func = make_func(
        vec![block],
        4,
        vec![ArcParam {
            var: v_src,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![
            ValueRepr::RcPointer, // v_src: struct
            ValueRepr::RcPointer, // v_field: projected list field
            ValueRepr::Scalar,    // v_arg: int literal
            ValueRepr::RcPointer, // v_result: push result
        ],
    );

    // Source struct is Unique — only one reference exists.
    let src_state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::Unique,
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    // Projected field: Borrowed, Unique (inherited from source via transfer_project).
    // This is exactly what transfer_project() produces when the source is Unique.
    // Without the transfer rule, this would be MaybeShared → Dynamic COW.
    let field_state = AimsState {
        access: AccessClass::Borrowed,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::Unique, // inherited from source
        locality: Locality::FunctionLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v_src, src_state), (v_field, field_state)]
            .into_iter()
            .collect(),
    );

    let annotations = compute_aims_cow_annotations(&func, &state_map, &interner);

    // COW on v_field at (block 0, instr 2): Unique → StaticUnique.
    // Without the unique-source-projection transfer rule, the projected field
    // would have MaybeShared uniqueness → Dynamic → runtime check.
    let mode = annotations.get(0, 2);
    assert_eq!(
        mode,
        CowMode::StaticUnique,
        "projected field from Unique source should get StaticUnique — \
         transfer_project preserves source uniqueness, eliminating the COW check"
    );
}

/// Contrast test: projected field from `MaybeShared` source gets `Dynamic` COW.
///
/// Same setup as `unique_source_projection_eliminates_cow_check`, but the
/// source struct is `MaybeShared` instead of `Unique`. The projected field
/// inherits `MaybeShared` → `Dynamic` → runtime check required.
///
/// This proves the optimization is specifically enabled by `transfer_project`
/// propagating source uniqueness — it's not a blanket optimization on all
/// projected fields.
#[test]
fn maybe_shared_source_projection_requires_cow_check() {
    let v_src = ArcVarId::new(0); // MaybeShared struct parameter
    let v_field = ArcVarId::new(1); // Project(v_src, field 0)
    let v_arg = ArcVarId::new(2);
    let v_result = ArcVarId::new(3);

    let (interner, push_name) = make_cow_interner();

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Project {
                dst: v_field,
                ty: Idx::NONE,
                value: v_src,
                field: 0,
            },
            ArcInstr::Let {
                dst: v_arg,
                ty: Idx::NONE,
                value: ArcValue::Literal(LitValue::Int(42)),
            },
            ArcInstr::Apply {
                dst: v_result,
                ty: Idx::NONE,
                func: push_name,
                args: vec![v_field, v_arg],
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return { value: v_result },
    };

    let func = make_func(
        vec![block],
        4,
        vec![ArcParam {
            var: v_src,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
        ],
    );

    // Source is MaybeShared — other references may exist.
    let src_state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Unrestricted,
        cardinality: Cardinality::Many,
        uniqueness: Uniqueness::MaybeShared,
        locality: Locality::Unknown,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::ALL,
    };

    // Projected field inherits MaybeShared from source.
    let field_state = AimsState {
        access: AccessClass::Borrowed,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: Uniqueness::MaybeShared, // inherited from MaybeShared source
        locality: Locality::Unknown,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };

    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v_src, src_state), (v_field, field_state)]
            .into_iter()
            .collect(),
    );

    let annotations = compute_aims_cow_annotations(&func, &state_map, &interner);

    // MaybeShared source → MaybeShared projected field → Dynamic COW
    let mode = annotations.get(0, 2);
    assert_eq!(
        mode,
        CowMode::Dynamic,
        "projected field from MaybeShared source should get Dynamic — \
         source's MaybeShared propagates through Project, requiring runtime check"
    );
}

// Edge cleanup tests (borrowed Invoke arg RcDec)

/// Borrowed Invoke arg whose last use is the Invoke terminator itself must
/// get `RcDec` on both normal and unwind continuation edges.
///
/// Setup: 3 blocks.
/// - bb0: Invoke with arg v0 as `Borrowed` → normal: bb1, unwind: bb2
/// - bb1: Return (normal path)
/// - bb2: Resume (unwind path)
///
/// The backward analysis doesn't propagate v0 to `block_exit_states` because
/// no successor uses it. Without the Category 2 fix in `collect_invoke_edge_decs`,
/// v0 would leak (no `RcDec` emitted anywhere).
#[test]
fn borrowed_invoke_arg_gets_rc_dec_on_both_edges() {
    let v0 = ArcVarId::new(0); // Borrowed Invoke arg (RcPointer)
    let v1 = ArcVarId::new(1); // Invoke destination
    let bb0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![],
        terminator: ArcTerminator::Invoke {
            dst: v1,
            ty: Idx::NONE,
            func: Name::new(0, 0),
            args: vec![v0],
            arg_ownership: vec![ArgOwnership::Borrowed],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
    };

    let bb1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        body: vec![],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let bb2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: Vec::new(),
        body: vec![],
        terminator: ArcTerminator::Resume,
    };

    let mut func = make_func(
        vec![bb0, bb1, bb2],
        3,
        vec![ArcParam {
            var: v0,
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        vec![ValueRepr::RcPointer, ValueRepr::Scalar, ValueRepr::Scalar],
    );

    // State map: v0 is Owned+Once at bb0 entry (function parameter).
    // NOT in exit_states — the backward analysis sees no successor using v0.
    let mut state_map = AimsStateMap::new(&func);
    state_map.update_block_entry(
        ArcBlockId::new(0),
        [(v0, owned_state(Cardinality::Once))].into_iter().collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());
    state_map.update_block_entry(ArcBlockId::new(1), FxHashMap::default());
    state_map.update_block_exit(ArcBlockId::new(1), FxHashMap::default());
    state_map.update_block_entry(ArcBlockId::new(2), FxHashMap::default());
    state_map.update_block_exit(ArcBlockId::new(2), FxHashMap::default());

    let pool = ori_types::Pool::new();
    emit_rc_ops(&mut func, &state_map, &pool);

    // bb1 (normal) must have RcDec(v0) prepended.
    let bb1_has_dec = func.blocks[1]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::RcDec { var, .. } if *var == v0));
    assert!(
        bb1_has_dec,
        "normal continuation (bb1) must have RcDec for borrowed Invoke arg, body: {:?}",
        func.blocks[1].body
    );

    // bb2 (unwind) must have RcDec(v0) prepended.
    let bb2_has_dec = func.blocks[2]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::RcDec { var, .. } if *var == v0));
    assert!(
        bb2_has_dec,
        "unwind continuation (bb2) must have RcDec for borrowed Invoke arg, body: {:?}",
        func.blocks[2].body
    );
}

/// Owned Invoke arg should NOT get the Category 2 edge cleanup treatment —
/// ownership is transferred to the callee, so the caller must not `RcDec`.
#[test]
fn owned_invoke_arg_no_extra_rc_dec() {
    let v0 = ArcVarId::new(0); // Owned Invoke arg (transferred to callee)
    let v1 = ArcVarId::new(1); // Invoke destination

    let bb0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![],
        terminator: ArcTerminator::Invoke {
            dst: v1,
            ty: Idx::NONE,
            func: Name::new(0, 0),
            args: vec![v0],
            arg_ownership: vec![ArgOwnership::Owned],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
    };

    let bb1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        body: vec![],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let bb2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: Vec::new(),
        body: vec![],
        terminator: ArcTerminator::Resume,
    };

    let mut func = make_func(
        vec![bb0, bb1, bb2],
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
        [(v0, owned_state(Cardinality::Once))].into_iter().collect(),
    );
    state_map.update_block_exit(ArcBlockId::new(0), FxHashMap::default());
    state_map.update_block_entry(ArcBlockId::new(1), FxHashMap::default());
    state_map.update_block_exit(ArcBlockId::new(1), FxHashMap::default());
    state_map.update_block_entry(ArcBlockId::new(2), FxHashMap::default());
    state_map.update_block_exit(ArcBlockId::new(2), FxHashMap::default());

    let pool = ori_types::Pool::new();
    emit_rc_ops(&mut func, &state_map, &pool);

    // Neither bb1 nor bb2 should have RcDec for v0 — ownership was transferred.
    for (idx, label) in [(1, "normal"), (2, "unwind")] {
        let has_dec = func.blocks[idx]
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::RcDec { var, .. } if *var == v0));
        assert!(
            !has_dec,
            "{label} continuation (bb{idx}) must NOT have RcDec for owned Invoke arg \
             (ownership transferred to callee), body: {:?}",
            func.blocks[idx].body
        );
    }
}
