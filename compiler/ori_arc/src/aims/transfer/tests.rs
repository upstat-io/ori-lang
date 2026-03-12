//! Tests for AIMS transfer functions.

use crate::aims::lattice::{
    AccessClass, AimsState, BorrowSource, Cardinality, Consumption, Locality, ReuseCtorKind,
    ShapeClass, Uniqueness,
};
use crate::ir::{ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, LitValue};

use super::*;

// Helpers

fn var(id: u32) -> ArcVarId {
    ArcVarId::new(id)
}

/// State lookup that returns TOP for all variables.
fn top_lookup(_: ArcVarId) -> AimsState {
    AimsState::TOP
}

/// State lookup that returns FRESH for all variables.
fn fresh_lookup(_: ArcVarId) -> AimsState {
    AimsState::FRESH
}

/// Build a lookup from a slice of (`var_id`, state) pairs.
fn lookup_from(pairs: &[(u32, AimsState)]) -> impl Fn(ArcVarId) -> AimsState + '_ {
    move |v: ArcVarId| {
        pairs
            .iter()
            .find(|(id, _)| *id == v.raw())
            .map_or(AimsState::TOP, |(_, s)| *s)
    }
}

// Forward transfer: Let

#[test]
fn let_var_inherits_source_state() {
    let instr = ArcInstr::Let {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        value: ArcValue::Var(var(0)),
    };
    let result = transfer_def(&instr, &fresh_lookup).expect("should define a variable");
    assert_eq!(result.state, AimsState::FRESH);
    assert_eq!(result.borrow_source, None);
}

#[test]
fn let_literal_is_scalar() {
    let instr = ArcInstr::Let {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        value: ArcValue::Literal(LitValue::Int(42)),
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert!(result.state.is_scalar());
}

#[test]
fn let_primop_is_scalar() {
    let instr = ArcInstr::Let {
        dst: var(2),
        ty: ori_types::Idx::from_raw(0),
        value: ArcValue::PrimOp {
            op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
            args: vec![var(0), var(1)],
        },
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert!(result.state.is_scalar());
}

// Forward transfer: Construct

#[test]
fn construct_struct_is_fresh_reusable() {
    let instr = ArcInstr::Construct {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        ctor: CtorKind::Struct(ori_ir::Name::new(0, 0)),
        args: vec![var(0)],
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert_eq!(result.state.access, AccessClass::Owned);
    assert_eq!(result.state.consumption, Consumption::Linear);
    assert_eq!(result.state.uniqueness, Uniqueness::Unique);
    assert_eq!(
        result.state.shape,
        ShapeClass::ReusableCtor(ReuseCtorKind::Struct)
    );
}

#[test]
fn construct_enum_variant_is_fresh_reusable() {
    let instr = ArcInstr::Construct {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        ctor: CtorKind::EnumVariant {
            enum_name: ori_ir::Name::new(0, 0),
            variant: 0,
        },
        args: vec![var(0)],
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert_eq!(
        result.state.shape,
        ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant)
    );
}

#[test]
fn construct_list_is_collection_buffer() {
    let instr = ArcInstr::Construct {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        ctor: CtorKind::ListLiteral,
        args: vec![],
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert_eq!(result.state.shape, ShapeClass::CollectionBuffer);
    assert_eq!(result.state.uniqueness, Uniqueness::Unique);
}

#[test]
fn construct_map_is_collection_buffer() {
    let instr = ArcInstr::Construct {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        ctor: CtorKind::MapLiteral,
        args: vec![],
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert_eq!(result.state.shape, ShapeClass::CollectionBuffer);
}

#[test]
fn construct_tuple_is_non_reusable() {
    let instr = ArcInstr::Construct {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        ctor: CtorKind::Tuple,
        args: vec![var(0)],
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert_eq!(result.state.shape, ShapeClass::NonReusable);
    assert_eq!(result.state.uniqueness, Uniqueness::Unique);
}

#[test]
fn construct_closure_is_non_reusable() {
    let instr = ArcInstr::Construct {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        ctor: CtorKind::Closure {
            func: ori_ir::Name::new(0, 0),
        },
        args: vec![],
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert_eq!(result.state.shape, ShapeClass::NonReusable);
}

// Forward transfer: Project

#[test]
fn project_is_borrowed_with_source() {
    let source_state = AimsState {
        uniqueness: Uniqueness::Unique,
        locality: Locality::FunctionLocal,
        ..AimsState::FRESH
    };
    let pairs = [(0, source_state)];
    let instr = ArcInstr::Project {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        value: var(0),
        field: 0,
    };
    let result = transfer_def(&instr, &lookup_from(&pairs)).expect("should define a variable");
    assert_eq!(result.state.access, AccessClass::Borrowed);
    assert_eq!(result.state.consumption, Consumption::Linear);
    assert_eq!(result.state.cardinality, Cardinality::Once);
    assert_eq!(result.state.uniqueness, Uniqueness::Unique);
    assert_eq!(result.state.locality, Locality::FunctionLocal);
    assert_eq!(
        result.borrow_source,
        Some(BorrowSource::exact_field(var(0), 0))
    );
}

#[test]
fn project_inherits_source_uniqueness() {
    let source_state = AimsState {
        uniqueness: Uniqueness::Shared,
        ..AimsState::FRESH
    };
    let pairs = [(0, source_state)];
    let instr = ArcInstr::Project {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        value: var(0),
        field: 0,
    };
    let result = transfer_def(&instr, &lookup_from(&pairs)).expect("should define a variable");
    assert_eq!(result.state.uniqueness, Uniqueness::Shared);
}

// Forward transfer: Apply

#[test]
fn apply_conservative_is_owned_maybe_shared() {
    let instr = ArcInstr::Apply {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        func: ori_ir::Name::new(0, 0),
        args: vec![var(0)],
        arg_ownership: vec![crate::ir::ArgOwnership::Owned],
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert_eq!(result.state.access, AccessClass::Owned);
    assert_eq!(result.state.uniqueness, Uniqueness::MaybeShared);
    assert_eq!(result.state.consumption, Consumption::Unrestricted);
    assert_eq!(result.state.locality, Locality::Unknown);
}

// Forward transfer: ApplyIndirect

#[test]
fn apply_indirect_is_top() {
    let instr = ArcInstr::ApplyIndirect {
        dst: var(2),
        ty: ori_types::Idx::from_raw(0),
        closure: var(0),
        args: vec![var(1)],
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert_eq!(result.state, AimsState::TOP);
}

// Forward transfer: PartialApply

#[test]
fn partial_apply_is_fresh_non_reusable() {
    let instr = ArcInstr::PartialApply {
        dst: var(2),
        ty: ori_types::Idx::from_raw(0),
        func: ori_ir::Name::new(0, 0),
        args: vec![var(0), var(1)],
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert_eq!(result.state, AimsState::FRESH);
    assert_eq!(result.state.shape, ShapeClass::NonReusable);
}

// Forward transfer: Select

#[test]
fn select_joins_branch_states() {
    let true_state = AimsState {
        uniqueness: Uniqueness::Unique,
        ..AimsState::FRESH
    };
    let false_state = AimsState {
        uniqueness: Uniqueness::Shared,
        ..AimsState::FRESH
    };
    let pairs = [(1, true_state), (2, false_state)];
    let instr = ArcInstr::Select {
        dst: var(3),
        ty: ori_types::Idx::from_raw(0),
        cond: var(0),
        true_val: var(1),
        false_val: var(2),
    };
    let result = transfer_def(&instr, &lookup_from(&pairs)).expect("should define a variable");
    // join(Unique, Shared) = Shared
    assert_eq!(result.state.uniqueness, Uniqueness::Shared);
}

// Forward transfer: CollectionReuse

#[test]
fn collection_reuse_is_fresh_collection_buffer() {
    let instr = ArcInstr::CollectionReuse {
        old_var: var(0),
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        ctor: CtorKind::ListLiteral,
        args: vec![],
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert_eq!(result.state.uniqueness, Uniqueness::Unique);
    assert_eq!(result.state.shape, ShapeClass::CollectionBuffer);
}

// Forward transfer: IsShared / Reset

#[test]
fn is_shared_is_scalar() {
    let instr = ArcInstr::IsShared {
        dst: var(1),
        var: var(0),
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert!(result.state.is_scalar());
}

#[test]
fn reset_token_is_scalar() {
    let instr = ArcInstr::Reset {
        var: var(0),
        token: var(1),
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert!(result.state.is_scalar());
}

// Forward transfer: Reuse

#[test]
fn reuse_struct_is_fresh_with_shape() {
    let instr = ArcInstr::Reuse {
        token: var(0),
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        ctor: CtorKind::Struct(ori_ir::Name::new(0, 0)),
        args: vec![],
    };
    let result = transfer_def(&instr, &top_lookup).expect("should define a variable");
    assert_eq!(result.state.uniqueness, Uniqueness::Unique);
    assert_eq!(
        result.state.shape,
        ShapeClass::ReusableCtor(ReuseCtorKind::Struct)
    );
}

// Forward transfer: side-effect-only instructions

#[test]
fn rc_inc_has_no_def() {
    let instr = ArcInstr::RcInc {
        var: var(0),
        count: 1,
        strategy: crate::ir::RcStrategy::HeapPointer,
    };
    assert!(transfer_def(&instr, &top_lookup).is_none());
}

#[test]
fn rc_dec_has_no_def() {
    let instr = ArcInstr::RcDec {
        var: var(0),
        strategy: crate::ir::RcStrategy::HeapPointer,
    };
    assert!(transfer_def(&instr, &top_lookup).is_none());
}

#[test]
fn set_has_no_def() {
    let instr = ArcInstr::Set {
        base: var(0),
        field: 0,
        value: var(1),
    };
    assert!(transfer_def(&instr, &top_lookup).is_none());
}

#[test]
fn set_tag_has_no_def() {
    let instr = ArcInstr::SetTag {
        base: var(0),
        tag: 0,
    };
    assert!(transfer_def(&instr, &top_lookup).is_none());
}

// Forward transfer: terminators

#[test]
fn invoke_def_is_conservative() {
    let term = ArcTerminator::Invoke {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        func: ori_ir::Name::new(0, 0),
        args: vec![var(0)],
        arg_ownership: vec![crate::ir::ArgOwnership::Owned],
        normal: crate::ir::ArcBlockId::new(1),
        unwind: crate::ir::ArcBlockId::new(2),
    };
    let result = transfer_terminator_def(&term).expect("Invoke should define a variable");
    assert_eq!(result.state.access, AccessClass::Owned);
    assert_eq!(result.state.uniqueness, Uniqueness::MaybeShared);
}

#[test]
fn return_has_no_def() {
    let term = ArcTerminator::Return { value: var(0) };
    assert!(transfer_terminator_def(&term).is_none());
}

#[test]
fn jump_has_no_def() {
    let term = ArcTerminator::Jump {
        target: crate::ir::ArcBlockId::new(0),
        args: vec![var(0)],
    };
    assert!(transfer_terminator_def(&term).is_none());
}

// Backward demands

#[test]
fn backward_construct_demands_once_per_arg() {
    let instr = ArcInstr::Construct {
        dst: var(3),
        ty: ori_types::Idx::from_raw(0),
        ctor: CtorKind::Struct(ori_ir::Name::new(0, 0)),
        args: vec![var(0), var(1), var(2)],
    };
    let demands = backward_demands(&instr);
    assert_eq!(demands.len(), 3);
    for (_, card) in &demands {
        assert_eq!(*card, Cardinality::Once);
    }
}

#[test]
fn backward_partial_apply_returns_empty() {
    // PartialApply captured arg demand is handled entirely by
    // capture_state_update in block.rs. backward_demands returns empty
    // to avoid double-counting.
    let instr = ArcInstr::PartialApply {
        dst: var(2),
        ty: ori_types::Idx::from_raw(0),
        func: ori_ir::Name::new(0, 0),
        args: vec![var(0), var(1)],
    };
    let demands = backward_demands(&instr);
    assert!(
        demands.is_empty(),
        "PartialApply demand handled by capture_state_update"
    );
}

#[test]
fn backward_apply_indirect_includes_closure() {
    let instr = ArcInstr::ApplyIndirect {
        dst: var(3),
        ty: ori_types::Idx::from_raw(0),
        closure: var(0),
        args: vec![var(1), var(2)],
    };
    let demands = backward_demands(&instr);
    assert_eq!(demands.len(), 3);
    assert_eq!(demands[0], (var(0), Cardinality::Once));
}

#[test]
fn backward_select_demands_all_three() {
    let instr = ArcInstr::Select {
        dst: var(3),
        ty: ori_types::Idx::from_raw(0),
        cond: var(0),
        true_val: var(1),
        false_val: var(2),
    };
    let demands = backward_demands(&instr);
    assert_eq!(demands.len(), 3);
    assert_eq!(demands[0].0, var(0)); // cond
    assert_eq!(demands[1].0, var(1)); // true_val
    assert_eq!(demands[2].0, var(2)); // false_val
}

#[test]
fn backward_rc_ops_have_no_demands() {
    let inc = ArcInstr::RcInc {
        var: var(0),
        count: 1,
        strategy: crate::ir::RcStrategy::HeapPointer,
    };
    let dec = ArcInstr::RcDec {
        var: var(0),
        strategy: crate::ir::RcStrategy::HeapPointer,
    };
    assert!(backward_demands(&inc).is_empty());
    assert!(backward_demands(&dec).is_empty());
}

#[test]
fn backward_let_literal_has_no_demands() {
    let instr = ArcInstr::Let {
        dst: var(1),
        ty: ori_types::Idx::from_raw(0),
        value: ArcValue::Literal(LitValue::Bool(true)),
    };
    assert!(backward_demands(&instr).is_empty());
}

#[test]
fn backward_collection_reuse_includes_old_var() {
    let instr = ArcInstr::CollectionReuse {
        old_var: var(0),
        dst: var(3),
        ty: ori_types::Idx::from_raw(0),
        ctor: CtorKind::ListLiteral,
        args: vec![var(1), var(2)],
    };
    let demands = backward_demands(&instr);
    assert_eq!(demands.len(), 3);
    assert_eq!(demands[0], (var(0), Cardinality::Once));
}

#[test]
fn backward_reuse_includes_token() {
    let instr = ArcInstr::Reuse {
        token: var(0),
        dst: var(2),
        ty: ori_types::Idx::from_raw(0),
        ctor: CtorKind::Struct(ori_ir::Name::new(0, 0)),
        args: vec![var(1)],
    };
    let demands = backward_demands(&instr);
    assert_eq!(demands.len(), 2);
    assert_eq!(demands[0], (var(0), Cardinality::Once)); // token
    assert_eq!(demands[1], (var(1), Cardinality::Once)); // arg
}

// Backward terminator demands

#[test]
fn backward_return_demands_value() {
    let term = ArcTerminator::Return { value: var(0) };
    let demands = backward_terminator_demands(&term);
    assert_eq!(demands.len(), 1);
    assert_eq!(demands[0], (var(0), Cardinality::Once));
}

#[test]
fn backward_branch_demands_cond() {
    let term = ArcTerminator::Branch {
        cond: var(0),
        then_block: crate::ir::ArcBlockId::new(1),
        else_block: crate::ir::ArcBlockId::new(2),
    };
    let demands = backward_terminator_demands(&term);
    assert_eq!(demands.len(), 1);
    assert_eq!(demands[0], (var(0), Cardinality::Once));
}

#[test]
fn backward_resume_has_no_demands() {
    assert!(backward_terminator_demands(&ArcTerminator::Resume).is_empty());
}

#[test]
fn backward_unreachable_has_no_demands() {
    assert!(backward_terminator_demands(&ArcTerminator::Unreachable).is_empty());
}

// RC reasoning predicates

#[test]
fn rc_dec_unnecessary_when_dead() {
    let state = AimsState {
        consumption: Consumption::Dead,
        cardinality: Cardinality::Absent,
        ..AimsState::FRESH
    };
    assert!(is_rc_dec_unnecessary(&state));
}

#[test]
fn rc_dec_necessary_when_live() {
    let state = AimsState {
        consumption: Consumption::Unrestricted,
        cardinality: Cardinality::Many,
        ..AimsState::FRESH
    };
    assert!(!is_rc_dec_unnecessary(&state));
}

#[test]
fn rc_inc_elidable_for_linear_once() {
    let state = AimsState {
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        ..AimsState::FRESH
    };
    assert!(is_rc_inc_elidable(&state));
}

#[test]
fn rc_inc_not_elidable_for_many() {
    let state = AimsState {
        consumption: Consumption::Linear,
        cardinality: Cardinality::Many,
        ..AimsState::FRESH
    };
    assert!(!is_rc_inc_elidable(&state));
}

#[test]
fn rc_inc_not_elidable_for_unrestricted() {
    let state = AimsState {
        consumption: Consumption::Unrestricted,
        cardinality: Cardinality::Once,
        ..AimsState::FRESH
    };
    assert!(!is_rc_inc_elidable(&state));
}

// COW mode

#[test]
fn cow_mode_unique_is_static_unique() {
    assert_eq!(
        cow_mode_from_uniqueness(Uniqueness::Unique),
        CowModeFromAims::StaticUnique
    );
}

#[test]
fn cow_mode_maybe_shared_is_dynamic() {
    assert_eq!(
        cow_mode_from_uniqueness(Uniqueness::MaybeShared),
        CowModeFromAims::Dynamic
    );
}

#[test]
fn cow_mode_shared_is_static_shared() {
    assert_eq!(
        cow_mode_from_uniqueness(Uniqueness::Shared),
        CowModeFromAims::StaticShared
    );
}

// Mutation constraint

#[test]
fn can_mutate_owned_unique() {
    let state = AimsState {
        access: AccessClass::Owned,
        uniqueness: Uniqueness::Unique,
        ..AimsState::FRESH
    };
    assert!(can_mutate_in_place(&state));
}

#[test]
fn cannot_mutate_borrowed() {
    let state = AimsState {
        access: AccessClass::Borrowed,
        uniqueness: Uniqueness::Unique,
        ..AimsState::FRESH
    };
    assert!(!can_mutate_in_place(&state));
}

#[test]
fn cannot_mutate_shared() {
    let state = AimsState {
        access: AccessClass::Owned,
        uniqueness: Uniqueness::Shared,
        ..AimsState::FRESH
    };
    assert!(!can_mutate_in_place(&state));
}

#[test]
fn cannot_mutate_maybe_shared() {
    let state = AimsState {
        access: AccessClass::Owned,
        uniqueness: Uniqueness::MaybeShared,
        ..AimsState::FRESH
    };
    assert!(!can_mutate_in_place(&state));
}

// Capture state update

#[test]
fn capture_update_escaping_closure_promotes_to_heap_escaping() {
    let state = AimsState::FRESH;
    // Closure that escapes (returned/stored in heap).
    let closure_state = AimsState {
        locality: Locality::HeapEscaping,
        cardinality: Cardinality::Many,
        consumption: Consumption::Unrestricted,
        ..AimsState::TOP
    };
    let updated = capture_state_update(&state, &closure_state);
    assert_eq!(updated.access, AccessClass::Owned);
    assert_eq!(updated.consumption, Consumption::Unrestricted);
    assert_eq!(updated.cardinality, Cardinality::Many);
    assert_eq!(updated.locality, Locality::HeapEscaping);
}

#[test]
fn capture_update_local_closure_gets_function_local() {
    let state = AimsState::FRESH;
    // Closure that stays function-local (used Many times, but doesn't escape).
    let closure_state = AimsState {
        locality: Locality::FunctionLocal,
        cardinality: Cardinality::Many,
        consumption: Consumption::Unrestricted,
        ..AimsState::BOTTOM
    };
    let updated = capture_state_update(&state, &closure_state);
    assert_eq!(updated.access, AccessClass::Owned);
    assert_eq!(updated.consumption, Consumption::Unrestricted);
    assert_eq!(updated.cardinality, Cardinality::Many);
    assert_eq!(updated.locality, Locality::FunctionLocal);
}

#[test]
fn capture_update_once_closure_preserves_linearity() {
    let state = AimsState::FRESH;
    // Once-closure: consumed linearly, stays function-local.
    let closure_state = AimsState {
        locality: Locality::FunctionLocal,
        cardinality: Cardinality::Once,
        consumption: Consumption::Linear,
        ..AimsState::BOTTOM
    };
    let updated = capture_state_update(&state, &closure_state);
    assert_eq!(updated.access, AccessClass::Owned);
    // Once-closure: captured var is used at most once through the closure,
    // so consumption stays Affine (may be dropped) not Unrestricted.
    assert_eq!(updated.consumption, Consumption::Affine);
    assert_eq!(updated.cardinality, Cardinality::Once);
    assert_eq!(updated.locality, Locality::FunctionLocal);
}

#[test]
fn capture_update_affine_once_closure_preserves_linearity() {
    let state = AimsState::FRESH;
    // Closure with Affine consumption (may be dropped) but Once cardinality.
    // The once-closure optimization should still fire — what matters is that
    // the closure is invoked at most once, not whether it might be dropped.
    let closure_state = AimsState {
        locality: Locality::FunctionLocal,
        cardinality: Cardinality::Once,
        consumption: Consumption::Affine,
        ..AimsState::BOTTOM
    };
    let updated = capture_state_update(&state, &closure_state);
    assert_eq!(updated.access, AccessClass::Owned);
    // Once-closure: captured var is used at most once through the closure.
    assert_eq!(updated.consumption, Consumption::Affine);
    assert_eq!(updated.cardinality, Cardinality::Once);
    assert_eq!(updated.locality, Locality::FunctionLocal);
}

#[test]
fn capture_update_block_local_closure_widens_to_function_local() {
    let state = AimsState::FRESH;
    // Closure with BlockLocal locality — captured vars still need at least
    // FunctionLocal because they escape the defining block.
    let closure_state = AimsState {
        locality: Locality::BlockLocal,
        cardinality: Cardinality::Once,
        consumption: Consumption::Linear,
        ..AimsState::BOTTOM
    };
    let updated = capture_state_update(&state, &closure_state);
    // Capture locality is max(closure.locality, FunctionLocal) = FunctionLocal.
    assert_eq!(updated.locality, Locality::FunctionLocal);
}

#[test]
fn capture_update_preserves_scalar() {
    let scalar = AimsState::SCALAR;
    let closure_state = AimsState::TOP;
    let updated = capture_state_update(&scalar, &closure_state);
    assert_eq!(updated, AimsState::SCALAR);
}

// Consumed state

#[test]
fn consumed_state_is_dead_absent() {
    let state = consumed_state();
    assert_eq!(state.consumption, Consumption::Dead);
    assert_eq!(state.cardinality, Cardinality::Absent);
}

// Shape mapping

#[test]
fn shape_from_ctor_covers_all_variants() {
    let cases = [
        (
            CtorKind::Struct(ori_ir::Name::new(0, 0)),
            ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        ),
        (
            CtorKind::EnumVariant {
                enum_name: ori_ir::Name::new(0, 0),
                variant: 0,
            },
            ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
        ),
        (CtorKind::ListLiteral, ShapeClass::CollectionBuffer),
        (CtorKind::MapLiteral, ShapeClass::CollectionBuffer),
        (CtorKind::SetLiteral, ShapeClass::CollectionBuffer),
        (CtorKind::Tuple, ShapeClass::NonReusable),
        (
            CtorKind::Closure {
                func: ori_ir::Name::new(0, 0),
            },
            ShapeClass::NonReusable,
        ),
    ];
    for (ctor, expected) in &cases {
        assert_eq!(
            shape_from_ctor(ctor),
            *expected,
            "CtorKind::{ctor:?} should map to {expected:?}"
        );
    }
}

// All ArcInstr variants covered

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive coverage test over all ArcInstr variants"
)]
fn transfer_def_covers_all_instr_variants() {
    // Ensure every ArcInstr variant returns Some or None as expected.
    // This is a compile-time exhaustiveness guard — the match in transfer_def
    // will fail to compile if a new variant is added without handling it.
    let instrs: Vec<(ArcInstr, bool)> = vec![
        (
            ArcInstr::Let {
                dst: var(0),
                ty: ori_types::Idx::from_raw(0),
                value: ArcValue::Literal(LitValue::Unit),
            },
            true,
        ),
        (
            ArcInstr::Apply {
                dst: var(0),
                ty: ori_types::Idx::from_raw(0),
                func: ori_ir::Name::new(0, 0),
                args: vec![],
                arg_ownership: vec![],
            },
            true,
        ),
        (
            ArcInstr::ApplyIndirect {
                dst: var(0),
                ty: ori_types::Idx::from_raw(0),
                closure: var(1),
                args: vec![],
            },
            true,
        ),
        (
            ArcInstr::PartialApply {
                dst: var(0),
                ty: ori_types::Idx::from_raw(0),
                func: ori_ir::Name::new(0, 0),
                args: vec![],
            },
            true,
        ),
        (
            ArcInstr::Project {
                dst: var(0),
                ty: ori_types::Idx::from_raw(0),
                value: var(1),
                field: 0,
            },
            true,
        ),
        (
            ArcInstr::Construct {
                dst: var(0),
                ty: ori_types::Idx::from_raw(0),
                ctor: CtorKind::Tuple,
                args: vec![],
            },
            true,
        ),
        (
            ArcInstr::RcInc {
                var: var(0),
                count: 1,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            false,
        ),
        (
            ArcInstr::RcDec {
                var: var(0),
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            false,
        ),
        (
            ArcInstr::IsShared {
                dst: var(0),
                var: var(1),
            },
            true,
        ),
        (
            ArcInstr::Set {
                base: var(0),
                field: 0,
                value: var(1),
            },
            false,
        ),
        (
            ArcInstr::SetTag {
                base: var(0),
                tag: 0,
            },
            false,
        ),
        (
            ArcInstr::Reset {
                var: var(0),
                token: var(1),
            },
            true,
        ),
        (
            ArcInstr::Reuse {
                token: var(0),
                dst: var(1),
                ty: ori_types::Idx::from_raw(0),
                ctor: CtorKind::Tuple,
                args: vec![],
            },
            true,
        ),
        (
            ArcInstr::CollectionReuse {
                old_var: var(0),
                dst: var(1),
                ty: ori_types::Idx::from_raw(0),
                ctor: CtorKind::ListLiteral,
                args: vec![],
            },
            true,
        ),
        (
            ArcInstr::Select {
                dst: var(0),
                ty: ori_types::Idx::from_raw(0),
                cond: var(1),
                true_val: var(2),
                false_val: var(3),
            },
            true,
        ),
    ];
    for (instr, expect_some) in &instrs {
        let result = transfer_def(instr, &top_lookup);
        assert_eq!(
            result.is_some(),
            *expect_some,
            "ArcInstr::{instr:?} — expected is_some={expect_some}"
        );
    }
}
