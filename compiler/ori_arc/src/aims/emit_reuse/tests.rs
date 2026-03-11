//! Tests for AIMS reuse emission.
//!
//! Verifies that `emit_reuse` correctly detects and applies same-block
//! reuse opportunities from a converged `AimsStateMap`, including
//! self-set elimination (§09.5).

use ori_ir::Name;
use ori_types::Idx;

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
    let result = emit_reuse(&mut func, &state_map, &pool);

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
    let result = emit_reuse(&mut func, &state_map, &pool);

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
    let result = emit_reuse(&mut func, &state_map, &pool);

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
    let result = emit_reuse(&mut func, &state_map, &pool);

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
    let result = emit_reuse(&mut func, &state_map, &pool);

    assert_eq!(result.static_reuses, 0, "collection construct not reusable");
}

/// `MaybeShared` reuse is detected but counted as dynamic (not applied in Stage 1).
#[test]
fn maybe_shared_counts_as_dynamic() {
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
    let result = emit_reuse(&mut func, &state_map, &pool);

    assert_eq!(result.static_reuses, 0, "MaybeShared should not be static");
    assert_eq!(
        result.dynamic_reuses, 1,
        "MaybeShared should count as dynamic"
    );

    // Body should be unchanged (dynamic reuse not applied in Stage 1).
    assert!(matches!(func.blocks[0].body[0], ArcInstr::RcDec { .. }));
    assert!(matches!(func.blocks[0].body[1], ArcInstr::Construct { .. }));
}

// Self-set elimination tests (§09.5)

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
    let result = emit_reuse(&mut func, &state_map, &pool);

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
    let result = emit_reuse(&mut func, &state_map, &pool);

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
    let result = emit_reuse(&mut func, &state_map, &pool);

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
    let result = emit_reuse(&mut func, &state_map, &pool);

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
    let result = emit_reuse(&mut func, &state_map, &pool);

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
    let _result = emit_reuse(&mut func, &state_map, &pool);

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
