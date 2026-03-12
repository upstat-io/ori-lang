//! Tests for AIMS reuse emission.
//!
//! Verifies that `emit_reuse` correctly detects and applies same-block
//! and cross-block reuse opportunities from a converged `AimsStateMap`,
//! including self-set elimination.

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::aims::contract::{FipContract, MemoryContract};

/// Block state triple: `(block_id, entry_states, exit_states)`.
type BlockStateTriple = (
    ArcBlockId,
    FxHashMap<ArcVarId, AimsState>,
    FxHashMap<ArcVarId, AimsState>,
);
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{
    AccessClass, AimsState, Cardinality, Consumption, EffectClass, Locality, ReuseCtorKind,
    ShapeClass, Uniqueness,
};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind, ValueRepr,
};
use crate::uniqueness::drop_hints::DropHints;
use crate::uniqueness::CowAnnotations;

use super::emit_reuse;
use super::fip::FipGateDecision;

/// Helper: create a minimal `ArcFunction` with the given blocks.
fn make_func(blocks: Vec<ArcBlock>, num_vars: usize) -> ArcFunction {
    ArcFunction {
        name: Name::new(0, 0),
        params: Vec::new(),
        return_type: Idx::NONE,
        blocks,
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::NONE; num_vars],
        var_reprs: vec![ValueRepr::RcPointer; num_vars],
        spans: Vec::new(),
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
    }
}

/// Create an Owned, Unique `AimsState` with reusable struct shape.
fn owned_unique_reusable(card: Cardinality) -> AimsState {
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
        shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        effect: EffectClass::NONE,
    }
}

/// Empty contracts map for tests that don't exercise FIP.
fn no_contracts() -> FxHashMap<Name, MemoryContract> {
    FxHashMap::default()
}

/// Same-block reuse: `RcDec` followed by same-type `Construct` → `Reset` + `Reuse`.
#[test]
fn same_block_struct_reuse() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 3);
    // Add spans to match body length.
    func.spans = vec![vec![None; 2]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);

    // v0: owned, unique, reusable struct — used once at entry, dead at exit.
    state_map.update_block_entry(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 1, "expected 1 static reuse");
    assert_eq!(result.dynamic_reuses, 0, "expected 0 dynamic reuses");

    // Verify: RcDec removed, Construct replaced with Set (no projections →
    // no self-sets → all fields get Set instructions).
    let body = &func.blocks[0].body;
    assert_eq!(body.len(), 1, "expected 1 instruction (Set for field 0)");
    assert!(
        matches!(body[0], ArcInstr::Set { base, field: 0, value } if base == v0 && value == v2),
        "expected Set {{ base: v0, field: 0, value: v2 }}, got {:?}",
        body[0]
    );

    // Terminator should use v0 (substituted from v1).
    assert!(
        matches!(func.blocks[0].terminator, ArcTerminator::Return { value } if value == v0),
        "expected Return {{ value: v0 }}, got {:?}",
        func.blocks[0].terminator
    );
}

/// No reuse when types don't match.
#[test]
fn no_reuse_different_types() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_b = Name::new(0, 200);

    // Type indices must differ for different types.
    let ty_a = Idx::from_raw(1);
    let ty_b = Idx::from_raw(2);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: ty_b,
                ctor: CtorKind::Struct(struct_b),
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 3);
    // v0 has type ty_a (different from construct's ty_b).
    func.var_types[0] = ty_a;

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);
    state_map.update_block_entry(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 0, "should not reuse different types");
    assert!(matches!(func.blocks[0].body[0], ArcInstr::RcDec { .. }));
    assert!(matches!(func.blocks[0].body[1], ArcInstr::Construct { .. }));
}

/// No reuse when the dying variable is shared.
#[test]
fn no_reuse_shared_variable() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 3);

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);

    // v0 is Shared — cannot be reused.
    let shared_state = AimsState {
        uniqueness: Uniqueness::Shared,
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_exit(blk, [(v0, shared_state)].into_iter().collect());

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 0, "shared var should not be reused");
}

/// No reuse when there's an intervening use of the dying variable.
#[test]
fn no_reuse_intervening_use() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 100);
    let callee = Name::new(0, 300);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            // v0 is used here — prevents reuse.
            ArcInstr::Apply {
                dst: v2,
                ty: Idx::NONE,
                func: callee,
                args: vec![v0],
                arg_ownership: Vec::new(),
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 3);

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);
    state_map.update_block_entry(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 0, "intervening use prevents reuse");
}

/// Collection constructs (`ListLiteral`, etc.) are NOT candidates for struct reuse.
#[test]
fn collection_construct_not_reusable() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::ListLiteral,
                args: Vec::new(),
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 2);

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);
    state_map.update_block_entry(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 0, "collection construct not reusable");
}

/// `MaybeShared` reuse emits `IsShared` + `Branch` with fast/slow paths.
#[expect(
    clippy::cognitive_complexity,
    reason = "comprehensive structural validation of expanded CFG"
)]
#[test]
fn maybe_shared_emits_conditional_branch() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 3);
    func.spans = vec![vec![None; 2]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);

    // v0 is MaybeShared — dynamic reuse candidate.
    let maybe_shared_entry = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Once)
    };
    let maybe_shared_exit = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_entry(blk, [(v0, maybe_shared_entry)].into_iter().collect());
    state_map.update_block_exit(blk, [(v0, maybe_shared_exit)].into_iter().collect());

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 0, "MaybeShared should not be static");
    assert_eq!(
        result.dynamic_reuses, 1,
        "MaybeShared should count as dynamic"
    );

    // Original block should end with IsShared + Branch.
    assert_eq!(
        func.blocks[0].body.len(),
        1,
        "original block should have only IsShared"
    );
    assert!(
        matches!(func.blocks[0].body[0], ArcInstr::IsShared { var, .. } if var == v0),
        "expected IsShared on v0, got {:?}",
        func.blocks[0].body[0]
    );
    assert!(
        matches!(func.blocks[0].terminator, ArcTerminator::Branch { .. }),
        "expected Branch terminator"
    );

    // Should have created 3 new blocks: fast, slow, merge (terminator uses v1).
    assert_eq!(
        func.blocks.len(),
        4,
        "expected 4 blocks (original + fast + slow + merge)"
    );

    // Fast path (block 1): Set for field 0 + Jump to merge.
    let fast = &func.blocks[1];
    assert_eq!(fast.body.len(), 1, "fast path: 1 Set instruction");
    assert!(
        matches!(fast.body[0], ArcInstr::Set { base, field: 0, value } if base == v0 && value == v2),
        "expected Set {{ base: v0, field: 0, value: v2 }}"
    );
    assert!(
        matches!(fast.terminator, ArcTerminator::Jump { target, ref args } if target == ArcBlockId::new(3) && args == &[v0]),
        "fast path should Jump to merge with v0"
    );

    // Slow path (block 2): RcDec + Construct + Jump to merge.
    let slow = &func.blocks[2];
    assert_eq!(slow.body.len(), 2, "slow path: RcDec + Construct");
    assert!(
        matches!(slow.body[0], ArcInstr::RcDec { var, .. } if var == v0),
        "slow path should RcDec v0"
    );
    assert!(
        matches!(slow.body[1], ArcInstr::Construct { dst, .. } if dst == v1),
        "slow path should Construct v1"
    );
    assert!(
        matches!(slow.terminator, ArcTerminator::Jump { target, ref args } if target == ArcBlockId::new(3) && args == &[v1]),
        "slow path should Jump to merge with v1"
    );

    // Merge block (block 3): receives result via param, returns it.
    let merge = &func.blocks[3];
    assert_eq!(merge.params.len(), 1, "merge should have 1 param");
    let merge_param = merge.params[0].0;
    assert!(
        matches!(merge.terminator, ArcTerminator::Return { value } if value == merge_param),
        "merge should Return the merge param"
    );
}

// Self-set elimination tests

/// Self-set elimination: field projected from source and passed back unchanged is skipped.
///
/// v0: Point { x, y }
/// v1 = Project { v0, field: 0 }    // x
/// v2 = Project { v0, field: 1 }    // y
/// v3 = Apply(f, v1)               // computes new x value
/// dec(v0)
/// Construct(v4, Point, v3, v2)    // build Point with new field 0
///
/// Field 1 (v2) is a self-set → skip Set. Only field 0 (v3) gets Set.
#[test]
fn self_set_elimination_skips_unchanged_field() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let v3 = ArcVarId::new(3);
    let v4 = ArcVarId::new(4);
    let struct_name = Name::new(0, 100);
    let callee = Name::new(0, 300);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Project {
                dst: v1,
                ty: Idx::NONE,
                value: v0,
                field: 0,
            },
            ArcInstr::Project {
                dst: v2,
                ty: Idx::NONE,
                value: v0,
                field: 1,
            },
            ArcInstr::Apply {
                dst: v3,
                ty: Idx::NONE,
                func: callee,
                args: vec![v1],
                arg_ownership: Vec::new(),
            },
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v4,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v3, v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v4 },
    };

    let mut func = make_func(vec![block], 5);
    func.spans = vec![vec![None; 5]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);
    state_map.update_block_entry(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 1);
    assert_eq!(result.fields_skipped, 1, "field 1 should be self-set");

    let body = &func.blocks[0].body;
    // Body: Project, Project, Apply, Set(field 0)
    assert_eq!(
        body.len(),
        4,
        "expected 4 instructions (2 Project + Apply + 1 Set)"
    );

    // The Set should be for field 0 only (field 1 was self-set → skipped).
    assert!(
        matches!(body[3], ArcInstr::Set { base, field: 0, value } if base == v0 && value == v3),
        "expected Set {{ base: v0, field: 0, value: v3 }}, got {:?}",
        body[3]
    );

    // No Set for field 1 anywhere in the body.
    let has_field1_set = body
        .iter()
        .any(|instr| matches!(instr, ArcInstr::Set { field: 1, .. }));
    assert!(!has_field1_set, "field 1 should be eliminated (self-set)");

    // Terminator should use v0 (substituted from v4).
    assert!(
        matches!(func.blocks[0].terminator, ArcTerminator::Return { value } if value == v0),
        "expected Return {{ value: v0 }}"
    );
}

/// No projections → no self-sets → all fields get Set instructions.
#[test]
fn no_projections_all_fields_set() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let v3 = ArcVarId::new(3);
    let struct_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            // No Project instructions — args come from elsewhere.
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2, v3],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 4);
    func.spans = vec![vec![None; 2]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);
    state_map.update_block_entry(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 1);
    assert_eq!(result.fields_skipped, 0, "no projections → no self-sets");

    let body = &func.blocks[0].body;
    // Both RcDec and Construct removed, replaced with 2 Set instructions.
    assert_eq!(body.len(), 2, "expected 2 Set instructions");
    assert!(
        matches!(body[0], ArcInstr::Set { base, field: 0, value } if base == v0 && value == v2),
        "expected Set field 0"
    );
    assert!(
        matches!(body[1], ArcInstr::Set { base, field: 1, value } if base == v0 && value == v3),
        "expected Set field 1"
    );
}

/// All fields unchanged → all self-sets → no Set instructions emitted.
#[test]
fn all_fields_self_set_no_sets() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let v3 = ArcVarId::new(3);
    let struct_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Project {
                dst: v1,
                ty: Idx::NONE,
                value: v0,
                field: 0,
            },
            ArcInstr::Project {
                dst: v2,
                ty: Idx::NONE,
                value: v0,
                field: 1,
            },
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            // Reconstruct with exact same projected fields → all self-sets.
            ArcInstr::Construct {
                dst: v3,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v1, v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v3 },
    };

    let mut func = make_func(vec![block], 4);
    func.spans = vec![vec![None; 4]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);
    state_map.update_block_entry(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 1);
    assert_eq!(result.fields_skipped, 2, "both fields are self-sets");

    let body = &func.blocks[0].body;
    // Only 2 Project instructions remain; RcDec removed, Construct removed,
    // no Set instructions (all self-sets).
    assert_eq!(body.len(), 2, "expected 2 instructions (2 Projects only)");
    assert!(matches!(body[0], ArcInstr::Project { field: 0, .. }));
    assert!(matches!(body[1], ArcInstr::Project { field: 1, .. }));

    // Terminator should use v0 (substituted from v3).
    assert!(
        matches!(func.blocks[0].terminator, ArcTerminator::Return { value } if value == v0),
        "expected Return {{ value: v0 }}"
    );
}

/// Enum variant reuse emits `SetTag` in addition to `Set` instructions.
#[test]
fn enum_variant_reuse_emits_set_tag() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let enum_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::EnumVariant {
                    enum_name,
                    variant: 2,
                },
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 3);
    // Set shape to EnumVariant for reuse detection.
    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);

    let enum_state = AimsState {
        shape: ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_entry(
        blk,
        [(v0, {
            let mut s = enum_state;
            s.cardinality = Cardinality::Once;
            s.consumption = Consumption::Linear;
            s
        })]
        .into_iter()
        .collect(),
    );
    state_map.update_block_exit(blk, [(v0, enum_state)].into_iter().collect());

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 1);

    let body = &func.blocks[0].body;
    // Expect: Set(field 0) + SetTag(tag 2)
    assert_eq!(body.len(), 2, "expected Set + SetTag");
    assert!(
        matches!(body[0], ArcInstr::Set { base, field: 0, value } if base == v0 && value == v2),
        "expected Set field 0"
    );
    assert!(
        matches!(body[1], ArcInstr::SetTag { base, tag: 2 } if base == v0),
        "expected SetTag with tag 2, got {:?}",
        body[1]
    );
}

/// Self-set elimination with enum variant: projected field unchanged, tag changes.
#[test]
fn enum_self_set_with_tag_change() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let enum_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Project {
                dst: v1,
                ty: Idx::NONE,
                value: v0,
                field: 0,
            },
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            // Reconstruct same field, different variant.
            ArcInstr::Construct {
                dst: v2,
                ty: Idx::NONE,
                ctor: CtorKind::EnumVariant {
                    enum_name,
                    variant: 3,
                },
                args: vec![v1],
            },
        ],
        terminator: ArcTerminator::Return { value: v2 },
    };

    let mut func = make_func(vec![block], 3);
    func.spans = vec![vec![None; 3]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);
    let enum_state = AimsState {
        shape: ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_entry(
        blk,
        [(v0, {
            let mut s = enum_state;
            s.cardinality = Cardinality::Once;
            s.consumption = Consumption::Linear;
            s
        })]
        .into_iter()
        .collect(),
    );
    state_map.update_block_exit(blk, [(v0, enum_state)].into_iter().collect());

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 1);
    assert_eq!(result.fields_skipped, 1, "field 0 is self-set");

    let body = &func.blocks[0].body;
    // Project remains, RcDec removed, Construct → SetTag only (field is self-set).
    assert_eq!(body.len(), 2, "expected Project + SetTag");
    assert!(matches!(body[0], ArcInstr::Project { field: 0, .. }));
    assert!(
        matches!(body[1], ArcInstr::SetTag { base, tag: 3 } if base == v0),
        "expected SetTag with tag 3, got {:?}",
        body[1]
    );
}

/// Span vector is correctly rebuilt after self-set elimination.
#[test]
fn spans_rebuilt_correctly() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let v3 = ArcVarId::new(3);
    let struct_name = Name::new(0, 100);
    let callee = Name::new(0, 300);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Project {
                dst: v1,
                ty: Idx::NONE,
                value: v0,
                field: 0,
            },
            ArcInstr::Apply {
                dst: v2,
                ty: Idx::NONE,
                func: callee,
                args: vec![v1],
                arg_ownership: Vec::new(),
            },
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v3,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2, v1], // field 1 is self-set (v1 = v0.0... no, field 1 needs v0.1)
            },
        ],
        terminator: ArcTerminator::Return { value: v3 },
    };

    let span_a = ori_ir::Span::new(0, 10);
    let span_b = ori_ir::Span::new(10, 20);
    let span_c = ori_ir::Span::new(20, 30);
    let span_d = ori_ir::Span::new(30, 40);

    let mut func = make_func(vec![block], 4);
    func.spans = vec![vec![Some(span_a), Some(span_b), Some(span_c), Some(span_d)]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);
    state_map.update_block_entry(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let _result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    // Body should have: Project, Apply, Set(0), Set(1) — no self-sets here
    // since only field 0 is projected but v1 is used at field 1 position
    // (v1 = v0.field_0, but it's placed at Construct arg index 1 = field 1).
    let body = &func.blocks[0].body;

    // Spans must match body length.
    assert_eq!(
        func.spans[0].len(),
        body.len(),
        "spans length must match body length: spans={}, body={}",
        func.spans[0].len(),
        body.len(),
    );

    // Original spans for Project and Apply should be preserved.
    assert_eq!(func.spans[0][0], Some(span_a), "Project span preserved");
    assert_eq!(func.spans[0][1], Some(span_b), "Apply span preserved");
}

// Dynamic reuse tests

/// Dynamic reuse with between instructions: they're moved before the split point.
#[test]
fn dynamic_reuse_moves_between_instructions() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let v3 = ArcVarId::new(3);
    let struct_name = Name::new(0, 100);
    let callee = Name::new(0, 300);

    // v0 dies (RcDec), then an unrelated Apply, then Construct of same type.
    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            // "Between" instruction: doesn't use v0, safe to move.
            ArcInstr::Apply {
                dst: v2,
                ty: Idx::NONE,
                func: callee,
                args: vec![v3],
                arg_ownership: Vec::new(),
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 4);
    func.spans = vec![vec![None; 3]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);
    let ms_entry = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Once)
    };
    let ms_exit = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_entry(blk, [(v0, ms_entry)].into_iter().collect());
    state_map.update_block_exit(blk, [(v0, ms_exit)].into_iter().collect());

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.dynamic_reuses, 1);

    // Original block: Apply (moved from between) + IsShared + Branch.
    let body = &func.blocks[0].body;
    assert_eq!(body.len(), 2, "expected Apply + IsShared");
    assert!(
        matches!(body[0], ArcInstr::Apply { .. }),
        "between Apply should be moved before split"
    );
    assert!(
        matches!(body[1], ArcInstr::IsShared { var, .. } if var == v0),
        "IsShared should follow the moved between instruction"
    );
}

/// Dynamic reuse without merge block: no suffix, terminator doesn't use dst.
#[test]
fn dynamic_reuse_no_merge_block() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 100);

    // Construct dst (v1) is NOT used by the terminator.
    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v2 }, // uses v2, not v1
    };

    let mut func = make_func(vec![block], 3);
    func.spans = vec![vec![None; 2]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);
    let ms_entry = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Once)
    };
    let ms_exit = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_entry(blk, [(v0, ms_entry)].into_iter().collect());
    state_map.update_block_exit(blk, [(v0, ms_exit)].into_iter().collect());

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.dynamic_reuses, 1);

    // No merge block needed: no suffix and terminator doesn't use v1.
    assert_eq!(
        func.blocks.len(),
        3,
        "expected 3 blocks (original + fast + slow, no merge)"
    );

    // Fast path should have Return with v2 (terminator copied, v1→v0 substitution
    // doesn't affect v2).
    assert!(
        matches!(func.blocks[1].terminator, ArcTerminator::Return { value } if value == v2),
        "fast path should Return v2"
    );

    // Slow path should also Return v2.
    assert!(
        matches!(func.blocks[2].terminator, ArcTerminator::Return { value } if value == v2),
        "slow path should Return v2"
    );
}

/// Dynamic reuse with self-set elimination on the fast path.
#[test]
fn dynamic_reuse_self_set_elimination() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let v3 = ArcVarId::new(3);
    let v4 = ArcVarId::new(4);
    let struct_name = Name::new(0, 100);
    let callee = Name::new(0, 300);

    // v1 = v0.field_0, v2 = v0.field_1
    // v3 = f(v1)  (new field 0 value)
    // dec(v0)
    // Construct(v4, [v3, v2])  -- field 1 (v2) is self-set
    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Project {
                dst: v1,
                ty: Idx::NONE,
                value: v0,
                field: 0,
            },
            ArcInstr::Project {
                dst: v2,
                ty: Idx::NONE,
                value: v0,
                field: 1,
            },
            ArcInstr::Apply {
                dst: v3,
                ty: Idx::NONE,
                func: callee,
                args: vec![v1],
                arg_ownership: Vec::new(),
            },
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v4,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v3, v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v4 },
    };

    let mut func = make_func(vec![block], 5);
    func.spans = vec![vec![None; 5]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);
    let ms_entry = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Once)
    };
    let ms_exit = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_entry(blk, [(v0, ms_entry)].into_iter().collect());
    state_map.update_block_exit(blk, [(v0, ms_exit)].into_iter().collect());

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.dynamic_reuses, 1);
    assert_eq!(
        result.fields_skipped, 1,
        "field 1 should be self-set on fast path"
    );

    // Fast path should only have Set for field 0 (field 1 is self-set).
    let fast = &func.blocks[1];
    assert_eq!(
        fast.body.len(),
        1,
        "fast path: only 1 Set (field 1 self-set eliminated)"
    );
    assert!(
        matches!(fast.body[0], ArcInstr::Set { base, field: 0, value } if base == v0 && value == v3),
        "fast path Set for field 0 with new value v3"
    );

    // Slow path should have RcDec + Construct (with both fields).
    let slow = &func.blocks[2];
    assert_eq!(slow.body.len(), 2, "slow path: RcDec + Construct");
    assert!(matches!(slow.body[0], ArcInstr::RcDec { var, .. } if var == v0));
    if let ArcInstr::Construct { args, .. } = &slow.body[1] {
        assert_eq!(args.len(), 2, "slow path Construct has both fields");
    } else {
        panic!("expected Construct on slow path");
    }
}

/// Dynamic reuse with enum variant emits `SetTag` on fast path.
#[test]
fn dynamic_reuse_enum_variant() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let enum_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::EnumVariant {
                    enum_name,
                    variant: 2,
                },
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 3);
    func.spans = vec![vec![None; 2]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);
    let enum_state_entry = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        shape: ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
        ..owned_unique_reusable(Cardinality::Once)
    };
    let enum_state_exit = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        shape: ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_entry(blk, [(v0, enum_state_entry)].into_iter().collect());
    state_map.update_block_exit(blk, [(v0, enum_state_exit)].into_iter().collect());

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.dynamic_reuses, 1);

    // Fast path: Set for field 0 + SetTag.
    let fast = &func.blocks[1];
    assert_eq!(fast.body.len(), 2, "fast path: Set + SetTag");
    assert!(matches!(fast.body[0], ArcInstr::Set { base, field: 0, .. } if base == v0));
    assert!(
        matches!(fast.body[1], ArcInstr::SetTag { base, tag: 2 } if base == v0),
        "fast path SetTag with variant 2"
    );

    // Slow path: RcDec + Construct (with EnumVariant ctor).
    let slow = &func.blocks[2];
    assert!(matches!(slow.body[0], ArcInstr::RcDec { var, .. } if var == v0));
    assert!(matches!(
        slow.body[1],
        ArcInstr::Construct {
            ctor: CtorKind::EnumVariant { variant: 2, .. },
            ..
        }
    ));
}

// Cross-block reuse tests

/// Cross-block static-unique reuse: death in block 0, alloc in block 1.
///
/// Block 0: dec(v0, unique), Jump(block 1)
/// Block 1: Construct(v1, same type), Return(v1)
///
/// Block 0 dominates block 1; block 1 post-dominates block 0.
/// Expected: `RcDec` removed from block 0, `Construct` replaced with `Set` in block 1.
#[test]
fn cross_block_static_unique_reuse() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 100);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::RcDec {
            var: v0,
            strategy: crate::ir::RcStrategy::HeapPointer,
        }],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: Vec::new(),
        },
    };

    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        body: vec![ArcInstr::Construct {
            dst: v1,
            ty: Idx::NONE,
            ctor: CtorKind::Struct(struct_name),
            args: vec![v2],
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block0, block1], 3);

    let mut state_map = AimsStateMap::new(&func);
    let blk0 = ArcBlockId::new(0);
    let blk1 = ArcBlockId::new(1);

    // v0: unique, reusable struct — dead at exit of block 0.
    state_map.update_block_entry(
        blk0,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk0,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );
    // v0 dead in block 1.
    state_map.update_block_entry(
        blk1,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk1,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.cross_block_reuses, 1, "expected 1 cross-block reuse");
    assert_eq!(result.static_reuses, 0, "no same-block static reuses");
    assert_eq!(result.dynamic_reuses, 0, "no dynamic reuses");

    // Block 0: RcDec removed → empty body.
    assert!(
        func.blocks[0].body.is_empty(),
        "RcDec should be removed from block 0, got {:?}",
        func.blocks[0].body
    );

    // Block 1: Construct replaced with Set.
    assert_eq!(
        func.blocks[1].body.len(),
        1,
        "expected 1 Set in block 1, got {:?}",
        func.blocks[1].body
    );
    assert!(
        matches!(func.blocks[1].body[0], ArcInstr::Set { base, field: 0, value } if base == v0 && value == v2),
        "expected Set {{ base: v0, field: 0, value: v2 }}, got {:?}",
        func.blocks[1].body[0]
    );

    // Terminator should use v0 (substituted from v1).
    assert!(
        matches!(func.blocks[1].terminator, ArcTerminator::Return { value } if value == v0),
        "expected Return {{ value: v0 }}, got {:?}",
        func.blocks[1].terminator
    );
}

/// Cross-block reuse with self-set elimination.
///
/// Block 0: Project(v1 = v0.0), Project(v2 = v0.1), v3 = f(v1), dec(v0), Jump(block 1)
/// Block 1: Construct(v4, Struct, [v3, v2]), Return(v4)
///
/// Field 1 (v2) is a self-set → skipped. Only field 0 (v3) gets Set.
#[test]
fn cross_block_self_set_elimination() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let v3 = ArcVarId::new(3);
    let v4 = ArcVarId::new(4);
    let struct_name = Name::new(0, 100);
    let callee = Name::new(0, 300);

    let (func_blocks, state_entries) =
        make_cross_block_self_set_fixture(v0, v1, v2, v3, v4, struct_name, callee);
    let mut func = make_func(func_blocks, 5);

    let mut state_map = AimsStateMap::new(&func);
    for (blk, entry, exit) in state_entries {
        state_map.update_block_entry(blk, entry);
        state_map.update_block_exit(blk, exit);
    }

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.cross_block_reuses, 1);
    assert_eq!(result.fields_skipped, 1, "field 1 should be self-set");

    // Block 0: RcDec removed, 3 instructions remain (Project, Project, Apply).
    assert_eq!(
        func.blocks[0].body.len(),
        3,
        "3 instructions after RcDec removal"
    );
    assert!(matches!(
        func.blocks[0].body[0],
        ArcInstr::Project { field: 0, .. }
    ));
    assert!(matches!(
        func.blocks[0].body[1],
        ArcInstr::Project { field: 1, .. }
    ));
    assert!(matches!(func.blocks[0].body[2], ArcInstr::Apply { .. }));

    // Block 1: Set for field 0 only (field 1 self-set eliminated).
    assert_eq!(func.blocks[1].body.len(), 1, "1 Set instruction in block 1");
    assert!(
        matches!(func.blocks[1].body[0], ArcInstr::Set { base, field: 0, value } if base == v0 && value == v3),
        "expected Set {{ base: v0, field: 0, value: v3 }}",
    );

    // Terminator uses v0 (substituted from v4).
    assert!(matches!(func.blocks[1].terminator, ArcTerminator::Return { value } if value == v0),);
}

/// Build the two-block IR and state map entries for `cross_block_self_set_elimination`.
fn make_cross_block_self_set_fixture(
    v0: ArcVarId,
    v1: ArcVarId,
    v2: ArcVarId,
    v3: ArcVarId,
    v4: ArcVarId,
    struct_name: Name,
    callee: Name,
) -> (Vec<ArcBlock>, Vec<BlockStateTriple>) {
    let blk0 = ArcBlockId::new(0);
    let blk1 = ArcBlockId::new(1);

    let block0 = ArcBlock {
        id: blk0,
        params: Vec::new(),
        body: vec![
            ArcInstr::Project {
                dst: v1,
                ty: Idx::NONE,
                value: v0,
                field: 0,
            },
            ArcInstr::Project {
                dst: v2,
                ty: Idx::NONE,
                value: v0,
                field: 1,
            },
            ArcInstr::Apply {
                dst: v3,
                ty: Idx::NONE,
                func: callee,
                args: vec![v1],
                arg_ownership: Vec::new(),
            },
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
        ],
        terminator: ArcTerminator::Jump {
            target: blk1,
            args: Vec::new(),
        },
    };

    let block1 = ArcBlock {
        id: blk1,
        params: Vec::new(),
        body: vec![ArcInstr::Construct {
            dst: v4,
            ty: Idx::NONE,
            ctor: CtorKind::Struct(struct_name),
            args: vec![v3, v2],
        }],
        terminator: ArcTerminator::Return { value: v4 },
    };

    let states = vec![
        (
            blk0,
            [(v0, owned_unique_reusable(Cardinality::Once))]
                .into_iter()
                .collect(),
            [(v0, owned_unique_reusable(Cardinality::Absent))]
                .into_iter()
                .collect(),
        ),
        (
            blk1,
            [(v0, owned_unique_reusable(Cardinality::Absent))]
                .into_iter()
                .collect(),
            [(v0, owned_unique_reusable(Cardinality::Absent))]
                .into_iter()
                .collect(),
        ),
    ];

    (vec![block0, block1], states)
}

/// No cross-block reuse when alloc block does NOT post-dominate death block.
///
/// Block 0: dec(v0), Branch(cond, block 1, block 2)
/// Block 1: Construct(v1, same type), Return(v1)
/// Block 2: Return(v3)
///
/// Block 1 does NOT post-dominate block 0 (block 2 is an alternate exit).
#[test]
fn no_cross_block_reuse_without_post_dominance() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let v3 = ArcVarId::new(3);
    let cond = ArcVarId::new(4);
    let struct_name = Name::new(0, 100);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::RcDec {
            var: v0,
            strategy: crate::ir::RcStrategy::HeapPointer,
        }],
        terminator: ArcTerminator::Branch {
            cond,
            then_block: ArcBlockId::new(1),
            else_block: ArcBlockId::new(2),
        },
    };

    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        body: vec![ArcInstr::Construct {
            dst: v1,
            ty: Idx::NONE,
            ctor: CtorKind::Struct(struct_name),
            args: vec![v2],
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Return { value: v3 },
    };

    let mut func = make_func(vec![block0, block1, block2], 5);

    let mut state_map = AimsStateMap::new(&func);
    let blk0 = ArcBlockId::new(0);
    state_map.update_block_entry(
        blk0,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk0,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(
        result.cross_block_reuses, 0,
        "no reuse: missing post-dominance"
    );
    assert_eq!(result.static_reuses, 0);
    // RcDec should still be present.
    assert!(matches!(func.blocks[0].body[0], ArcInstr::RcDec { .. }));
    // Construct should still be present.
    assert!(matches!(func.blocks[1].body[0], ArcInstr::Construct { .. }));
}

/// No cross-block reuse for `MaybeShared` sources (Stage 1: unique only).
#[test]
fn no_cross_block_reuse_maybe_shared() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 100);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::RcDec {
            var: v0,
            strategy: crate::ir::RcStrategy::HeapPointer,
        }],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: Vec::new(),
        },
    };

    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        body: vec![ArcInstr::Construct {
            dst: v1,
            ty: Idx::NONE,
            ctor: CtorKind::Struct(struct_name),
            args: vec![v2],
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block0, block1], 3);

    let mut state_map = AimsStateMap::new(&func);
    let blk0 = ArcBlockId::new(0);

    // v0 is MaybeShared — not eligible for cross-block reuse in Stage 1.
    let ms_state = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_entry(
        blk0,
        [(v0, {
            let mut s = ms_state;
            s.cardinality = Cardinality::Once;
            s.consumption = Consumption::Linear;
            s
        })]
        .into_iter()
        .collect(),
    );
    state_map.update_block_exit(blk0, [(v0, ms_state)].into_iter().collect());

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(
        result.cross_block_reuses, 0,
        "MaybeShared: no cross-block reuse"
    );
    // RcDec and Construct should be unchanged.
    assert!(matches!(func.blocks[0].body[0], ArcInstr::RcDec { .. }));
    assert!(matches!(func.blocks[1].body[0], ArcInstr::Construct { .. }));
}

/// No cross-block reuse when types differ.
#[test]
fn no_cross_block_reuse_different_types() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 200);

    let ty_a = Idx::from_raw(1);
    let ty_b = Idx::from_raw(2);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::RcDec {
            var: v0,
            strategy: crate::ir::RcStrategy::HeapPointer,
        }],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: Vec::new(),
        },
    };

    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        body: vec![ArcInstr::Construct {
            dst: v1,
            ty: ty_b,
            ctor: CtorKind::Struct(struct_name),
            args: vec![v2],
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block0, block1], 3);
    func.var_types[0] = ty_a; // v0 has type ty_a (differs from ty_b).

    let mut state_map = AimsStateMap::new(&func);
    let blk0 = ArcBlockId::new(0);
    state_map.update_block_entry(
        blk0,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk0,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(
        result.cross_block_reuses, 0,
        "different types: no cross-block reuse"
    );
}

/// Cross-block reuse with enum variant emits `SetTag`.
#[test]
fn cross_block_enum_variant_reuse() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let enum_name = Name::new(0, 100);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::RcDec {
            var: v0,
            strategy: crate::ir::RcStrategy::HeapPointer,
        }],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: Vec::new(),
        },
    };

    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        body: vec![ArcInstr::Construct {
            dst: v1,
            ty: Idx::NONE,
            ctor: CtorKind::EnumVariant {
                enum_name,
                variant: 2,
            },
            args: vec![v2],
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block0, block1], 3);

    let mut state_map = AimsStateMap::new(&func);
    let blk0 = ArcBlockId::new(0);

    let enum_state = AimsState {
        shape: ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_entry(
        blk0,
        [(v0, {
            let mut s = enum_state;
            s.cardinality = Cardinality::Once;
            s.consumption = Consumption::Linear;
            s
        })]
        .into_iter()
        .collect(),
    );
    state_map.update_block_exit(blk0, [(v0, enum_state)].into_iter().collect());

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.cross_block_reuses, 1);

    // Block 0: RcDec removed.
    assert!(func.blocks[0].body.is_empty());

    // Block 1: Set for field 0 + SetTag.
    assert_eq!(func.blocks[1].body.len(), 2);
    assert!(
        matches!(func.blocks[1].body[0], ArcInstr::Set { base, field: 0, value } if base == v0 && value == v2),
    );
    assert!(matches!(func.blocks[1].body[1], ArcInstr::SetTag { base, tag: 2 } if base == v0),);

    // Return uses v0 (substituted from v1).
    assert!(matches!(func.blocks[1].terminator, ArcTerminator::Return { value } if value == v0),);
}

/// Cross-block reuse with intervening block (3-block chain).
///
/// Block 0: dec(v0), Jump(block 1)
/// Block 1: [computation], Jump(block 2)
/// Block 2: Construct(v1), Return(v1)
///
/// Block 0 dominates block 2; block 2 post-dominates block 0.
#[test]
fn cross_block_reuse_through_intervening_block() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let v3 = ArcVarId::new(3);
    let struct_name = Name::new(0, 100);
    let callee = Name::new(0, 300);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::RcDec {
            var: v0,
            strategy: crate::ir::RcStrategy::HeapPointer,
        }],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: Vec::new(),
        },
    };

    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        body: vec![ArcInstr::Apply {
            dst: v3,
            ty: Idx::NONE,
            func: callee,
            args: vec![v2],
            arg_ownership: Vec::new(),
        }],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(2),
            args: Vec::new(),
        },
    };

    let block2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: Vec::new(),
        body: vec![ArcInstr::Construct {
            dst: v1,
            ty: Idx::NONE,
            ctor: CtorKind::Struct(struct_name),
            args: vec![v3],
        }],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block0, block1, block2], 4);

    let mut state_map = AimsStateMap::new(&func);
    let blk0 = ArcBlockId::new(0);
    state_map.update_block_entry(
        blk0,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk0,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.cross_block_reuses, 1, "expected 1 cross-block reuse");

    // Block 0: RcDec removed.
    assert!(func.blocks[0].body.is_empty());

    // Block 1: unchanged (Apply still there).
    assert_eq!(func.blocks[1].body.len(), 1);
    assert!(matches!(func.blocks[1].body[0], ArcInstr::Apply { .. }));

    // Block 2: Construct replaced with Set.
    assert_eq!(func.blocks[2].body.len(), 1);
    assert!(
        matches!(func.blocks[2].body[0], ArcInstr::Set { base, field: 0, value } if base == v0 && value == v3),
    );

    // Return uses v0 (substituted from v1).
    assert!(matches!(func.blocks[2].terminator, ArcTerminator::Return { value } if value == v0),);
}

// FIP contract integration tests (Section 05.4)

/// FIP-certified upgrades `MaybeShared` reuse to static-unique path.
#[test]
fn fip_certified_upgrades_maybe_shared_to_static() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 3);
    func.spans = vec![vec![None; 2]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);

    // v0: owned, MaybeShared, reusable struct.
    let maybe_shared_entry = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Once)
    };
    let maybe_shared_exit = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_entry(blk, [(v0, maybe_shared_entry)].into_iter().collect());
    state_map.update_block_exit(blk, [(v0, maybe_shared_exit)].into_iter().collect());

    // FIP-certified contract: upgrade MaybeShared → static-unique.
    let mut contracts = FxHashMap::default();
    contracts.insert(
        func.name,
        MemoryContract::all_borrowed(0, FipContract::Certified),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &contracts);

    // FIP-certified: MaybeShared upgraded to static → static reuse path.
    assert_eq!(result.static_reuses, 1, "should use static-unique path");
    assert_eq!(result.dynamic_reuses, 0, "no dynamic reuses");
    assert_eq!(result.fip_gates.len(), 1, "one FIP gate record");
    assert_eq!(
        result.fip_gates[0].decision,
        FipGateDecision::UpgradedToStaticUnique
    );
    assert_eq!(result.fip_gates[0].source_var, v0);

    // Static path: no IsShared instruction.
    let has_is_shared = func.blocks.iter().any(|b| {
        b.body
            .iter()
            .any(|i| matches!(i, ArcInstr::IsShared { .. }))
    });
    assert!(!has_is_shared, "FIP-certified should skip IsShared check");
}

/// FIP-conditional records gate but keeps dynamic reuse.
#[test]
fn fip_conditional_records_gate_keeps_dynamic() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 3);
    func.spans = vec![vec![None; 2]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);

    // v0: owned, MaybeShared, reusable struct.
    let maybe_shared_entry = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Once)
    };
    let maybe_shared_exit = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_entry(blk, [(v0, maybe_shared_entry)].into_iter().collect());
    state_map.update_block_exit(blk, [(v0, maybe_shared_exit)].into_iter().collect());

    // FIP-conditional: requires param 0 to be unique.
    let mut contracts = FxHashMap::default();
    contracts.insert(
        func.name,
        MemoryContract::all_borrowed(
            0,
            FipContract::Conditional {
                requires_unique_params: vec![],
            },
        ),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &contracts);

    // FIP-conditional: keeps dynamic reuse, records gate.
    assert_eq!(result.static_reuses, 0, "should keep dynamic path");
    assert_eq!(result.dynamic_reuses, 1, "one dynamic reuse");
    assert_eq!(result.fip_gates.len(), 1, "one FIP gate record");
    assert_eq!(
        result.fip_gates[0].decision,
        FipGateDecision::ConditionalDynamic
    );

    // Dynamic path: has IsShared instruction.
    let has_is_shared = func.blocks.iter().any(|b| {
        b.body
            .iter()
            .any(|i| matches!(i, ArcInstr::IsShared { .. }))
    });
    assert!(has_is_shared, "FIP-conditional should use IsShared check");
}

/// FIP-never (or absent) does not change reuse decisions.
#[test]
fn fip_never_no_change() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 3);
    func.spans = vec![vec![None; 2]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);

    // v0: owned, MaybeShared, reusable struct.
    let maybe_shared_entry = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Once)
    };
    let maybe_shared_exit = AimsState {
        uniqueness: Uniqueness::MaybeShared,
        ..owned_unique_reusable(Cardinality::Absent)
    };
    state_map.update_block_entry(blk, [(v0, maybe_shared_entry)].into_iter().collect());
    state_map.update_block_exit(blk, [(v0, maybe_shared_exit)].into_iter().collect());

    // FIP-never: no FIP influence.
    let mut contracts = FxHashMap::default();
    contracts.insert(
        func.name,
        MemoryContract::all_borrowed(0, FipContract::Never),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &contracts);

    // FIP-never: standard dynamic reuse, no gates.
    assert_eq!(result.dynamic_reuses, 1, "standard dynamic reuse");
    assert!(result.fip_gates.is_empty(), "no FIP gates for Never");
}

/// No contract found for function → same as FIP-never.
#[test]
fn no_contract_no_fip_influence() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 3);
    func.spans = vec![vec![None; 2]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);

    state_map.update_block_entry(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    // Empty contracts: function not found → no FIP influence.
    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 1, "unique → static reuse unchanged");
    assert!(result.fip_gates.is_empty(), "no FIP gates");
}

/// `missed_reuses` counts death events with no compatible allocation.
#[test]
fn missed_reuses_counted() {
    let v0 = ArcVarId::new(0);

    // One death (RcDec), no Construct → 1 missed reuse.
    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::RcDec {
            var: v0,
            strategy: crate::ir::RcStrategy::HeapPointer,
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    };

    let mut func = make_func(vec![block], 2);
    func.spans = vec![vec![None; 1]];
    // Give v0 a specific struct type so it's a valid death event.
    func.var_types[0] = Idx::from_raw(100);

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);

    state_map.update_block_entry(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &no_contracts());

    assert_eq!(result.static_reuses, 0, "no allocations to reuse into");
    assert_eq!(result.missed_reuses, 1, "one unmatched death event");
}

/// FIP-certified with only `Unique` sources produces no FIP gate records.
#[test]
fn fip_certified_unique_source_no_gate() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let v2 = ArcVarId::new(2);
    let struct_name = Name::new(0, 100);

    let block = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::RcDec {
                var: v0,
                strategy: crate::ir::RcStrategy::HeapPointer,
            },
            ArcInstr::Construct {
                dst: v1,
                ty: Idx::NONE,
                ctor: CtorKind::Struct(struct_name),
                args: vec![v2],
            },
        ],
        terminator: ArcTerminator::Return { value: v1 },
    };

    let mut func = make_func(vec![block], 3);
    func.spans = vec![vec![None; 2]];

    let mut state_map = AimsStateMap::new(&func);
    let blk = ArcBlockId::new(0);

    // v0: already Unique — FIP-certified upgrade is a no-op.
    state_map.update_block_entry(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Once))]
            .into_iter()
            .collect(),
    );
    state_map.update_block_exit(
        blk,
        [(v0, owned_unique_reusable(Cardinality::Absent))]
            .into_iter()
            .collect(),
    );

    let mut contracts = FxHashMap::default();
    contracts.insert(
        func.name,
        MemoryContract::all_borrowed(0, FipContract::Certified),
    );

    let pool = ori_types::Pool::new();
    let result = emit_reuse(&mut func, &state_map, &pool, &contracts);

    // Already unique: static reuse, no FIP gate (no upgrade needed).
    assert_eq!(result.static_reuses, 1);
    assert!(
        result.fip_gates.is_empty(),
        "no gate record when source was already Unique"
    );
}
