use ori_arc::ir::{
    ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership, LitValue,
};
use ori_arc::uniqueness::{CowAnnotations, DropHints};
use ori_arc::ArcFunction;
use ori_ir::Name;
use ori_types::Idx;

/// Build a minimal `ArcFunction` with the given blocks.
///
/// Only `blocks` matters for `scan_used_fields` — all other fields
/// use sensible defaults.
fn make_test_func(blocks: Vec<ArcBlock>) -> ArcFunction {
    let span_count = blocks.len();
    ArcFunction {
        name: Name::new(0, 0),
        params: Vec::new(),
        return_type: Idx::UNIT,
        blocks,
        entry: ArcBlockId::new(0),
        var_types: Vec::new(),
        var_reprs: Vec::new(),
        spans: vec![Vec::new(); span_count],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
    }
}

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn b(n: u32) -> ArcBlockId {
    ArcBlockId::new(n)
}

/// Single `Project` instruction records the field index for its source var.
#[test]
fn test_basic_field_projection() {
    // v1 = project v0, field 2
    let func = make_test_func(vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![ArcInstr::Project {
            dst: v(1),
            ty: Idx::INT,
            value: v(0),
            field: 2,
        }],
        terminator: ArcTerminator::Unreachable,
    }]);

    let usage = super::scan_used_fields(&func);

    // v0 should have field set {2}.
    let fields = usage.get(&v(0)).expect("v0 should be in usage map");
    let set = fields
        .as_ref()
        .expect("v0 should have specific fields, not None");
    assert_eq!(set.len(), 1);
    assert!(set.contains(&2));
}

/// `Let` with `ArcValue::Var(src)` follows the alias chain; projection
/// on the alias maps to the root var.
#[test]
fn test_alias_resolution() {
    // v1 = alias of v0 (Let { Var(v0) })
    // v2 = project v1, field 1   → should map to v0
    let func = make_test_func(vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v(1),
                ty: Idx::INT,
                value: ArcValue::Var(v(0)),
            },
            ArcInstr::Project {
                dst: v(2),
                ty: Idx::INT,
                value: v(1),
                field: 1,
            },
        ],
        terminator: ArcTerminator::Unreachable,
    }]);

    let usage = super::scan_used_fields(&func);

    // v0 (the root) should have field {1}.
    let fields = usage.get(&v(0)).expect("v0 should be in usage map");
    let set = fields.as_ref().expect("v0 should have specific fields");
    assert!(set.contains(&1));

    // v1 (the alias) should NOT be in the map — it resolved to v0.
    assert!(
        !usage.contains_key(&v(1)),
        "alias v1 should not appear as a separate key"
    );
}

/// Non-projection use of a var (e.g., `Apply { args: [var] }`) sets
/// `None` (whole-variable needed).
#[test]
fn test_whole_variable_usage() {
    // v0 used as arg in Apply → whole-variable needed
    let func = make_test_func(vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![ArcInstr::Apply {
            dst: v(1),
            ty: Idx::INT,
            func: Name::new(0, 1),
            args: vec![v(0)],
            arg_ownership: vec![ArgOwnership::Owned],
        }],
        terminator: ArcTerminator::Unreachable,
    }]);

    let usage = super::scan_used_fields(&func);

    // v0 should be None (whole-variable).
    let entry = usage.get(&v(0)).expect("v0 should be in usage map");
    assert!(entry.is_none(), "whole-variable use should produce None");
}

/// Two `Project` instructions on same root var (fields 0 and 2);
/// both field indices recorded in the `FxHashSet`.
#[test]
fn test_multiple_fields_same_root() {
    let func = make_test_func(vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![
            ArcInstr::Project {
                dst: v(1),
                ty: Idx::INT,
                value: v(0),
                field: 0,
            },
            ArcInstr::Project {
                dst: v(2),
                ty: Idx::INT,
                value: v(0),
                field: 2,
            },
        ],
        terminator: ArcTerminator::Unreachable,
    }]);

    let usage = super::scan_used_fields(&func);

    let fields = usage.get(&v(0)).expect("v0 should be in usage map");
    let set = fields.as_ref().expect("v0 should have specific fields");
    assert_eq!(set.len(), 2);
    assert!(set.contains(&0));
    assert!(set.contains(&2));
}

/// Function with no `Project` instructions returns empty map.
#[test]
fn test_no_projections_returns_empty() {
    let func = make_test_func(vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![ArcInstr::Let {
            dst: v(0),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(42)),
        }],
        terminator: ArcTerminator::Unreachable,
    }]);

    let usage = super::scan_used_fields(&func);
    assert!(usage.is_empty(), "no projections should yield empty map");
}

/// Alias chain > 100 depth terminates without infinite loop.
///
/// Constructs a circular alias chain: v0 → v1 → v0. The depth limit
/// (100 iterations) should break the cycle. In debug builds, the
/// `debug_assert!` fires — `should_panic` validates this.
#[test]
#[should_panic(expected = "alias cycle detected")]
fn test_alias_cycle_terminates() {
    // v0 = alias of v1, v1 = alias of v0 (circular)
    // v2 = project v0, field 0
    let func = make_test_func(vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![
            ArcInstr::Let {
                dst: v(0),
                ty: Idx::INT,
                value: ArcValue::Var(v(1)),
            },
            ArcInstr::Let {
                dst: v(1),
                ty: Idx::INT,
                value: ArcValue::Var(v(0)),
            },
            ArcInstr::Project {
                dst: v(2),
                ty: Idx::INT,
                value: v(0),
                field: 0,
            },
        ],
        terminator: ArcTerminator::Unreachable,
    }]);

    // Should not hang — depth limit prevents infinite loop.
    // The debug_assert! would fire in debug builds, but we just want to
    // verify termination. We don't assert on the exact result because the
    // cycle resolution is best-effort.
    let _usage = super::scan_used_fields(&func);
}

/// Var is first projected (field 0) then used whole (e.g., as `Apply` arg);
/// the entry becomes `None` (whole overrides projection).
#[test]
fn test_whole_variable_overrides_projection() {
    let func = make_test_func(vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![
            // First: project field 0 from v0.
            ArcInstr::Project {
                dst: v(1),
                ty: Idx::INT,
                value: v(0),
                field: 0,
            },
            // Then: use v0 as whole value in Apply.
            ArcInstr::Apply {
                dst: v(2),
                ty: Idx::INT,
                func: Name::new(0, 1),
                args: vec![v(0)],
                arg_ownership: vec![ArgOwnership::Owned],
            },
        ],
        terminator: ArcTerminator::Unreachable,
    }]);

    let usage = super::scan_used_fields(&func);

    // v0 should be None — whole-variable use overrides the earlier projection.
    let entry = usage.get(&v(0)).expect("v0 should be in usage map");
    assert!(
        entry.is_none(),
        "whole-variable use should override prior projections"
    );
}
