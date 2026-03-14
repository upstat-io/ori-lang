//! Tests for the TRMC normalization pass (detection, lifting, and rewrite).

use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, CtorKind, LitValue,
};
use crate::Ownership;

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

// Rewrite tests

/// Helper: build a simple self-recursive function with a Construct wrapping
/// the recursive result. Pattern:
///   @f(v0: T) -> T =
///     v1 = self(v0)
///     v2 = Construct { v0, v1 }  // `hole_field` = 1 (v1 is recursive)
///     Return v2
fn make_recursive_construct_func() -> ArcFunction {
    let self_name = Name::from_raw(42);
    ArcFunction {
        name: self_name,
        return_type: ty(0),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                // v1 = self(v0)
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                // v2 = Construct { v0, v1 }
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(0), var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    }
}

/// Basic self-recursive list-map pattern: rewrite succeeds,
/// function signature unchanged, loop-header block has context params.
#[test]
fn rewrite_trmc_simple_list_map() {
    let mut func = make_recursive_construct_func();
    let regions = super::detect::detect_context_regions(&func);
    assert_eq!(regions.len(), 1);

    let original_param_count = func.params.len();
    let result = super::rewrite::rewrite_trmc(&mut func, &regions);
    assert!(result, "rewrite should succeed");

    // Function signature unchanged (loop-header strategy).
    assert_eq!(
        func.params.len(),
        original_param_count,
        "no params added to function signature"
    );

    // Original entry block (block 0) has 3 context block params
    // (ctx_has: bool, ctx_res: return_ty, ctx_hole_obj: return_ty).
    let entry_params = &func.blocks[0].params;
    assert_eq!(
        entry_params.len(),
        3,
        "3 context block params on loop header"
    );
    assert_eq!(entry_params[0].1, Idx::BOOL, "ctx_has is bool");
    assert_eq!(entry_params[1].1, ty(0), "ctx_res matches return type");
    assert_eq!(entry_params[2].1, ty(0), "ctx_hole_obj matches return type");

    // Function entry changed to prologue block.
    assert_ne!(func.entry, block_id(0), "entry changed to prologue");
}

/// Enum variant Construct with recursive result: rewrite applies.
#[test]
fn rewrite_trmc_enum_variant() {
    let self_name = Name::from_raw(42);
    let mut func = ArcFunction {
        name: self_name,
        return_type: ty(0),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
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
                        variant: 1,
                    },
                    args: vec![var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let regions = super::detect::detect_context_regions(&func);
    assert!(super::rewrite::rewrite_trmc(&mut func, &regions));
    // Signature unchanged.
    assert_eq!(func.params.len(), 1);
    // Loop header has context params.
    assert_eq!(func.blocks[0].params.len(), 3);
}

/// No context regions -> rewrite returns false (no-op).
#[test]
fn rewrite_trmc_skipped_when_no_regions() {
    let mut func = make_recursive_construct_func();
    let result = super::rewrite::rewrite_trmc(&mut func, &[]);
    assert!(!result, "empty regions -> no rewrite");
    assert_eq!(func.params.len(), 1, "no params added");
    assert_eq!(func.entry, block_id(0), "entry unchanged");
}

/// Rewrite produces a loop-back Jump to the loop header (original entry),
/// not a self-call. No Apply to self should remain.
#[test]
fn rewrite_trmc_produces_loop_back() {
    let mut func = make_recursive_construct_func();
    let self_name = func.name;
    let original_entry = func.entry;
    let regions = super::detect::detect_context_regions(&func);
    super::rewrite::rewrite_trmc(&mut func, &regions);

    // No Apply to self should remain (recursion converted to loop).
    let has_self_call = func.blocks.iter().any(|b| {
        b.body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::Apply { func: f, .. } if *f == self_name))
    });
    assert!(!has_self_call, "no self-call should remain after rewrite");

    // There should be a Jump back to the original entry (loop header).
    let has_loop_back = func.blocks.iter().any(|b| {
        matches!(
            &b.terminator,
            ArcTerminator::Jump { target, args } if *target == original_entry && args.len() == 3
        )
    });
    assert!(
        has_loop_back,
        "should have a Jump back to loop header with 3 context args"
    );
}

/// Rewrite emits Set instructions for context composition.
#[test]
fn rewrite_trmc_context_operations_emit_set() {
    let mut func = make_recursive_construct_func();
    let regions = super::detect::detect_context_regions(&func);
    super::rewrite::rewrite_trmc(&mut func, &regions);

    let set_count: usize = func
        .blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|instr| matches!(instr, ArcInstr::Set { .. }))
        .count();

    // At least 1 Set for context composition (compose block).
    assert!(
        set_count >= 1,
        "at least 1 Set for context composition, got {set_count}"
    );
}

/// Multi-block function: base-case Return in a separate block gets
/// rewritten to a Branch (conditional context application).
#[test]
fn rewrite_trmc_multi_arm_match() {
    let self_name = Name::from_raw(42);
    let mut func = ArcFunction {
        name: self_name,
        return_type: ty(0),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![
            // Block 0: Branch on condition.
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: var(0),
                    then_block: block_id(1),
                    else_block: block_id(2),
                },
            },
            // Block 1 (recursive): v1 = self(v0), v2 = Construct(v0, v1), Return v2.
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![
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
                        args: vec![var(0), var(1)],
                    },
                ],
                terminator: ArcTerminator::Return { value: var(2) },
            },
            // Block 2 (base case): v3 = 0, Return v3.
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: var(3),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(0)),
                }],
                terminator: ArcTerminator::Return { value: var(3) },
            },
        ],
        ..Default::default()
    };

    let regions = super::detect::detect_context_regions(&func);
    assert_eq!(regions.len(), 1);

    assert!(super::rewrite::rewrite_trmc(&mut func, &regions));

    // Block 2's Return should now be a Branch (base-case context application).
    assert!(
        matches!(func.blocks[2].terminator, ArcTerminator::Branch { .. }),
        "base-case block rewritten to Branch, got {:?}",
        func.blocks[2].terminator
    );

    // Signature unchanged.
    assert_eq!(func.params.len(), 1);
}

/// Non-recursive arm body is preserved by rewrite.
#[test]
fn rewrite_trmc_preserves_non_recursive_arms() {
    let self_name = Name::from_raw(42);
    let mut func = ArcFunction {
        name: self_name,
        return_type: ty(0),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![
            ArcBlock {
                id: block_id(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: var(0),
                    then_block: block_id(1),
                    else_block: block_id(2),
                },
            },
            ArcBlock {
                id: block_id(1),
                params: vec![],
                body: vec![
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
                        args: vec![var(0), var(1)],
                    },
                ],
                terminator: ArcTerminator::Return { value: var(2) },
            },
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: var(3),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(99)),
                }],
                terminator: ArcTerminator::Return { value: var(3) },
            },
        ],
        ..Default::default()
    };

    let regions = super::detect::detect_context_regions(&func);
    super::rewrite::rewrite_trmc(&mut func, &regions);

    // Block 2 body untouched.
    assert_eq!(func.blocks[2].body.len(), 1);
    assert!(
        matches!(
            &func.blocks[2].body[0],
            ArcInstr::Let {
                value: ArcValue::Literal(LitValue::Int(99)),
                ..
            }
        ),
        "base-case body preserved"
    );
}

/// Construct not in tail position (not last body instr): rewrite skips.
#[test]
fn rewrite_trmc_skipped_when_construct_not_tail() {
    let self_name = Name::from_raw(42);
    let mut func = ArcFunction {
        name: self_name,
        return_type: ty(0),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                // Construct is NOT the last instruction — there's a Let after it.
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(0), var(1)],
                },
                ArcInstr::Let {
                    dst: var(3),
                    ty: ty(0),
                    value: ArcValue::Var(var(2)),
                },
            ],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..Default::default()
    };

    let regions = super::detect::detect_context_regions(&func);
    assert_eq!(regions.len(), 1, "detection still finds the region");
    let result = super::rewrite::rewrite_trmc(&mut func, &regions);
    assert!(!result, "rewrite should skip: construct not tail");
    assert_eq!(func.params.len(), 1, "no modification");
}

/// Placeholder uses `ctx_res` (which is the null sentinel on first iteration).
/// The prologue defines a null sentinel (`LitValue::Null`) and passes it to
/// the loop header. The Construct uses `ctx_res` (a block param) for the hole.
#[test]
fn rewrite_trmc_placeholder_is_block_param_not_literal() {
    let mut func = make_recursive_construct_func();
    let regions = super::detect::detect_context_regions(&func);
    super::rewrite::rewrite_trmc(&mut func, &regions);

    // Find the rewritten Construct.
    let construct: Vec<_> = func
        .blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter_map(|instr| match instr {
            ArcInstr::Construct { args, .. } => Some(args.clone()),
            _ => None,
        })
        .collect();
    assert!(!construct.is_empty(), "should have a Construct");
    let construct = &construct[0];

    // The hole field arg should be a variable (ctx_res block param),
    // NOT any of the original function's defined vars (v0, v1, v2).
    let hole_var = construct[1]; // hole_field = 1 in make_recursive_construct_func
    assert!(
        hole_var.raw() >= 3,
        "hole field should be a fresh var (ctx_res), not original v0-v2, got v{}",
        hole_var.raw()
    );

    // The prologue should define a null sentinel (LitValue::Null).
    let prologue_idx = func.entry.index();
    let has_null_sentinel = func.blocks[prologue_idx].body.iter().any(|instr| {
        matches!(
            instr,
            ArcInstr::Let {
                value: ArcValue::Literal(LitValue::Null),
                ..
            }
        )
    });
    assert!(
        has_null_sentinel,
        "prologue should have LitValue::Null sentinel"
    );
}

/// Multi-region functions are explicitly rejected (v1 limitation).
#[test]
fn rewrite_trmc_skipped_when_multiple_regions() {
    use crate::aims::contract::ContextRegion;

    let mut func = make_recursive_construct_func();
    let regions = vec![
        ContextRegion {
            open_block: block_id(0),
            open_instr: 1,
            context_var: var(2),
            hole_field: 1,
            close_block: block_id(0),
            close_instr: 0,
            hole_var: var(1),
        },
        ContextRegion {
            open_block: block_id(0),
            open_instr: 1,
            context_var: var(2),
            hole_field: 0,
            close_block: block_id(0),
            close_instr: 0,
            hole_var: var(1),
        },
    ];

    let result = super::rewrite::rewrite_trmc(&mut func, &regions);
    assert!(!result, "multi-region should be rejected");
    assert_eq!(func.params.len(), 1, "no modification");
}
