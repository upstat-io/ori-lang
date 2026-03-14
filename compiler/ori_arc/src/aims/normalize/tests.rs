//! Tests for the TRMC normalization pass.

use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership,
    CtorKind, LitValue,
};

fn block_id(n: u32) -> ArcBlockId {
    ArcBlockId::new(n)
}

fn var(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn ty(n: u32) -> Idx {
    Idx::from_raw(n)
}

// Detection tests

/// Self-recursive function with Construct wrapping recursive result
/// should produce a context region.
#[test]
fn detect_context_region_for_recursive_construct() {
    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                // v1 = self(v0) — recursive call
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                // v2 = Construct(v1) — constructor wrapping recursive result
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let result = super::normalize_function(&func);

    assert!(!result.was_transformed, "v1: no IR rewriting");
    assert_eq!(
        result.context_regions.len(),
        1,
        "one context region detected"
    );

    let region = &result.context_regions[0];
    assert_eq!(region.open_block, block_id(0));
    assert_eq!(region.open_instr, 2, "Construct is at instr index 2");
    assert_eq!(region.context_var, var(2));
    assert_eq!(region.hole_field, 0, "recursive arg is field 0");
    assert_eq!(region.close_block, block_id(0));
    assert_eq!(region.close_instr, 1, "Apply is at instr index 1");
    assert_eq!(region.hole_var, var(1));
}

/// Non-recursive function → no context regions detected.
#[test]
fn no_context_regions_for_non_recursive_function() {
    let self_name = Name::from_raw(42);
    let other_name = Name::from_raw(99);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                // v1 = other(v0) — NOT recursive
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: other_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let result = super::normalize_function(&func);
    assert!(
        result.context_regions.is_empty(),
        "non-recursive → no context regions"
    );
}

/// Construct with recursive result not in args → no context regions.
#[test]
fn no_context_regions_when_recursive_result_not_in_construct() {
    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                // v1 = self(v0) — recursive, but result unused by Construct
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(2)),
                },
                // v3 = Construct(v2) — uses v2, not v1
                ArcInstr::Construct {
                    dst: var(3),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(2)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..Default::default()
    };

    let result = super::normalize_function(&func);
    assert!(result.context_regions.is_empty());
}

/// Enum variant Construct with recursive result → context region detected.
#[test]
fn detect_context_region_for_enum_variant() {
    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::EnumVariant {
                        enum_name: Name::from_raw(200),
                        variant: 0,
                    },
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let result = super::normalize_function(&func);
    assert_eq!(result.context_regions.len(), 1);
    assert_eq!(result.context_regions[0].hole_field, 0);
}

/// Tuple Construct with recursive result → no context region
/// (only struct/enum are TRMC candidates).
#[test]
fn no_context_regions_for_tuple_construct() {
    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Tuple,
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let result = super::normalize_function(&func);
    assert!(
        result.context_regions.is_empty(),
        "Tuple is not a TRMC candidate"
    );
}

/// Recursive result in second field → `hole_field` == 1.
#[test]
fn hole_field_tracks_correct_arg_index() {
    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Let {
                    dst: var(1),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(2)),
                },
                ArcInstr::Apply {
                    dst: var(2),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                // Construct with non-recursive v1 at field 0, recursive v2 at field 1
                ArcInstr::Construct {
                    dst: var(3),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(1), var(2)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..Default::default()
    };

    let result = super::normalize_function(&func);
    assert_eq!(result.context_regions.len(), 1);
    assert_eq!(
        result.context_regions[0].hole_field, 1,
        "recursive arg is at field index 1"
    );
}

/// Function with no Apply/Invoke → no recursive calls → no context regions.
#[test]
fn no_context_regions_for_non_recursive_no_calls() {
    let func = ArcFunction {
        name: Name::from_raw(42),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Construct {
                    dst: var(1),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(0)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let result = super::normalize_function(&func);
    assert!(result.context_regions.is_empty());
}

// Lifting pre-pass tests

/// Well-formed function with Construct: `lift_constructor_args` is a no-op.
/// ARC IR enforces A-normal form by type (`Construct.args: Vec<ArcVarId>`),
/// so the lifting pass is purely a verification assertion.
#[test]
fn lifting_a_normal_form_is_noop() {
    let self_name = Name::from_raw(42);
    let func = ArcFunction {
        name: self_name,
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    // Should not panic — all Construct args are valid variable references.
    super::lift::lift_constructor_args(&func);

    // normalize_function still works correctly (lifting is transparent).
    let result = super::normalize_function(&func);
    assert!(!result.was_transformed);
    assert_eq!(result.context_regions.len(), 1);
}

/// Lifting verifies multi-field Constructs with all valid var refs.
#[test]
fn lifting_multi_field_construct_valid() {
    let func = ArcFunction {
        name: Name::from_raw(42),
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Let {
                    dst: var(1),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(2)),
                },
                ArcInstr::Let {
                    dst: var(2),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(3)),
                },
                ArcInstr::Construct {
                    dst: var(3),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(0), var(1), var(2)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..Default::default()
    };

    // No panic — all 3 field args are valid.
    super::lift::lift_constructor_args(&func);
}

/// Function with no Construct instructions: lifting is trivially a no-op.
#[test]
fn lifting_no_constructs_is_noop() {
    let func = ArcFunction {
        name: Name::from_raw(42),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: var(0),
                ty: ty(0),
                value: ArcValue::Literal(LitValue::Int(1)),
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    super::lift::lift_constructor_args(&func);
}

/// Debug assertion catches Construct arg referencing an undefined variable.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "Construct field 0 arg v99 out of bounds")]
fn lifting_catches_invalid_arg_var() {
    let func = ArcFunction {
        name: Name::from_raw(42),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Construct {
                    dst: var(1),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(99)], // v99 doesn't exist in var_types
                },
            ],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    super::lift::lift_constructor_args(&func);
}

/// Debug assertion catches Construct dst referencing an undefined variable.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "Construct dst v99 out of bounds")]
fn lifting_catches_invalid_dst_var() {
    let func = ArcFunction {
        name: Name::from_raw(42),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(99), // v99 doesn't exist in var_types
                ty: ty(0),
                ctor: CtorKind::Struct(Name::from_raw(100)),
                args: vec![var(0)],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    super::lift::lift_constructor_args(&func);
}
