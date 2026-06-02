//! Tests for purity analysis of ARC functions.

use ori_arc::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    CtorKind, RcStrategy,
};
use ori_arc::{LitValue, Ownership};
use ori_ir::Name;
use ori_types::Idx;

use super::has_only_pure_arc_instructions;

/// Helper to create a minimal `ArcFunction` with the given body instructions.
fn make_func(body: Vec<ArcInstr>) -> ArcFunction {
    ArcFunction {
        name: Name::from_raw(1),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body,
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        ..Default::default()
    }
}

#[test]
fn let_only_is_pure() {
    let func = make_func(vec![ArcInstr::Let {
        dst: ArcVarId::new(1),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(42)),
    }]);
    assert!(has_only_pure_arc_instructions(&func));
}

#[test]
fn construct_does_not_block_purity() {
    let func = make_func(vec![
        ArcInstr::Let {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(1)),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(2),
            ty: Idx::INT,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        },
    ]);
    assert!(has_only_pure_arc_instructions(&func));
}

#[test]
fn project_does_not_block_purity() {
    let func = make_func(vec![
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0)],
        },
        ArcInstr::Project {
            dst: ArcVarId::new(2),
            ty: Idx::INT,
            value: ArcVarId::new(1),
            field: 0,
        },
    ]);
    assert!(has_only_pure_arc_instructions(&func));
}

#[test]
fn apply_blocks_purity() {
    let func = make_func(vec![ArcInstr::Apply {
        dst: ArcVarId::new(1),
        func: Name::from_raw(2),
        ty: Idx::INT,
        args: vec![ArcVarId::new(0)],
        arg_ownership: vec![],
        mono_instance_id: None,
    }]);
    assert!(!has_only_pure_arc_instructions(&func));
}

#[test]
fn rc_inc_blocks_purity() {
    let func = make_func(vec![ArcInstr::RcInc {
        var: ArcVarId::new(0),
        count: 1,
        strategy: RcStrategy::HeapPointer,
    }]);
    assert!(!has_only_pure_arc_instructions(&func));
}

#[test]
fn empty_body_is_pure() {
    let func = make_func(vec![]);
    assert!(has_only_pure_arc_instructions(&func));
}

// Effect dimension → RL-30 consumer (`memory(none)` gating).
//
// The Effect summary feeds RL-29/RL-30 LLVM fact export. The shipped
// `memory(none)` gate (AT-3 / RL-30 partial) consumes `has_only_pure_arc_instructions`
// — a function with no effect-bearing instructions is `memory(none)`-eligible.
// These pins clamp the Effect-row consumer over the post-lowering IR shape and
// confirm the burden-emitted baseline does not poison the purity classification.

/// Effect positive pin: a function whose ARC IR carries only pure
/// instructions (no allocation/sharing/throw effect) is classified
/// `memory(none)`-eligible — RL-30 emits the no-effect attribute.
#[test]
fn effect_pure_function_is_memory_none_eligible() {
    let func = make_func(vec![
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0)],
        },
        ArcInstr::Project {
            dst: ArcVarId::new(2),
            ty: Idx::INT,
            value: ArcVarId::new(1),
            field: 0,
        },
    ]);
    assert!(
        has_only_pure_arc_instructions(&func),
        "pure ARC IR (Construct + Project only) must be memory(none)-eligible (RL-30)"
    );
}

/// Effect negative pin: an effect-bearing instruction (`Apply` — a call
/// with unknown effect summary) blocks `memory(none)`. Emitting `memory(none)`
/// on an effectful call would let LLVM wrongly eliminate the call's
/// side effects. This pin rejects that.
#[test]
fn effect_call_function_is_not_memory_none_eligible() {
    let func = make_func(vec![ArcInstr::Apply {
        dst: ArcVarId::new(1),
        func: Name::from_raw(2),
        ty: Idx::INT,
        args: vec![ArcVarId::new(0)],
        arg_ownership: vec![],
        mono_instance_id: None,
    }]);
    assert!(
        !has_only_pure_arc_instructions(&func),
        "an effect-bearing Apply must NOT be memory(none)-eligible (RL-30 gates on no effects)"
    );
}

/// Effect burden-baseline pin: a residual burden op is NOT in the
/// pure-instruction allow-list, so a function still carrying a `BurdenInc`
/// (i.e. one that Phase 6 did not eliminate and Phase 7 has not yet lowered)
/// is NOT classified `memory(none)`-eligible. This confirms the burden-emitted
/// baseline cannot silently produce a spurious no-effect attribute — the RL-30
/// consumer treats unlowered burden ops conservatively. By codegen, burden ops
/// are fully lowered to `RcInc`/`RcDec` (Phase 7), which also block purity, so
/// the classification is stable across the emission→elimination→lowering shift.
#[test]
fn effect_residual_burden_op_is_not_memory_none_eligible() {
    let func = make_func(vec![ArcInstr::BurdenInc {
        var: ArcVarId::new(0),
    }]);
    assert!(
        !has_only_pure_arc_instructions(&func),
        "a residual BurdenInc must NOT be memory(none)-eligible (burden ops are not pure-allow-listed)"
    );
}
