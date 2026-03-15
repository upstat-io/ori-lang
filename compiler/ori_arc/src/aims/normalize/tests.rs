//! Tests for the TRMC normalization pass (detection, lifting, rewrite,
//! and pipeline integration).

use ori_ir::Name;
use ori_types::Idx;

use crate::aims::contract::{EffectSummary, FipContract, MemoryContract};
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
    let mut func = ArcFunction {
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

    let result = super::normalize_function(&mut func, None);

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
    let mut func = ArcFunction {
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

    let result = super::normalize_function(&mut func, None);
    assert!(
        result.context_regions.is_empty(),
        "non-recursive → no context regions"
    );
}

/// Construct with recursive result not in args → no context regions.
#[test]
fn no_context_regions_when_recursive_result_not_in_construct() {
    let self_name = Name::from_raw(42);
    let mut func = ArcFunction {
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

    let result = super::normalize_function(&mut func, None);
    assert!(result.context_regions.is_empty());
}

/// Enum variant Construct with recursive result → context region detected.
#[test]
fn detect_context_region_for_enum_variant() {
    let self_name = Name::from_raw(42);
    let mut func = ArcFunction {
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

    let result = super::normalize_function(&mut func, None);
    assert_eq!(result.context_regions.len(), 1);
    assert_eq!(result.context_regions[0].hole_field, 0);
}

/// Tuple Construct with recursive result → no context region
/// (only struct/enum are TRMC candidates).
#[test]
fn no_context_regions_for_tuple_construct() {
    let self_name = Name::from_raw(42);
    let mut func = ArcFunction {
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

    let result = super::normalize_function(&mut func, None);
    assert!(
        result.context_regions.is_empty(),
        "Tuple is not a TRMC candidate"
    );
}

/// Recursive result in second field → `hole_field` == 1.
#[test]
fn hole_field_tracks_correct_arg_index() {
    let self_name = Name::from_raw(42);
    let mut func = ArcFunction {
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

    let result = super::normalize_function(&mut func, None);
    assert_eq!(result.context_regions.len(), 1);
    assert_eq!(
        result.context_regions[0].hole_field, 1,
        "recursive arg is at field index 1"
    );
}

/// Function with no Apply/Invoke → no recursive calls → no context regions.
#[test]
fn no_context_regions_for_non_recursive_no_calls() {
    let mut func = ArcFunction {
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

    let result = super::normalize_function(&mut func, None);
    assert!(result.context_regions.is_empty());
}

// Lifting pre-pass tests

/// Well-formed function with Construct: `lift_constructor_args` is a no-op.
/// ARC IR enforces A-normal form by type (`Construct.args: Vec<ArcVarId>`),
/// so the lifting pass is purely a verification assertion.
#[test]
fn lifting_a_normal_form_is_noop() {
    let self_name = Name::from_raw(42);
    let mut func = ArcFunction {
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
    let result = super::normalize_function(&mut func, None);
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

    // Original entry block (block 0) has N fresh param block params +
    // 3 context block params (ctx_has: bool, ctx_res, ctx_hole_obj).
    let entry_params = &func.blocks[0].params;
    let n = original_param_count;
    assert_eq!(
        entry_params.len(),
        n + 3,
        "{n} fresh param + 3 context block params on loop header"
    );
    // Fresh params come first with matching types.
    assert_eq!(
        entry_params[0].1,
        ty(0),
        "fresh param 0 matches func param type"
    );
    // Context triple is the last 3.
    assert_eq!(entry_params[n].1, Idx::BOOL, "ctx_has is bool");
    assert_eq!(entry_params[n + 1].1, ty(0), "ctx_res matches return type");
    assert_eq!(
        entry_params[n + 2].1,
        ty(0),
        "ctx_hole_obj matches return type"
    );

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
    // Loop header has 1 fresh param + 3 context params.
    assert_eq!(func.blocks[0].params.len(), 1 + 3);
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

    // There should be a Jump back to the original entry (loop header)
    // with n_params + 3 args (param args + context triple).
    let n_params = func.params.len();
    let has_loop_back = func.blocks.iter().any(|b| {
        matches!(
            &b.terminator,
            ArcTerminator::Jump { target, args }
                if *target == original_entry && args.len() == n_params + 3
        )
    });
    assert!(
        has_loop_back,
        "should have a Jump back to loop header with {} args (params + context)",
        n_params + 3
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

// Verification tests

/// Correctly rewritten function passes all verification checks.
#[test]
fn verify_clean_rewrite_passes() {
    let mut func = make_recursive_construct_func();
    let regions = super::detect::detect_context_regions(&func);
    assert!(super::rewrite::rewrite_trmc(&mut func, &regions));

    let errors = super::verify::verify_trmc_rewrite(&func, &regions);
    assert!(
        errors.is_empty(),
        "clean rewrite should pass verification, got: {errors:?}"
    );
}

/// Multi-arm function with base-case blocks: clean rewrite passes verification.
#[test]
fn verify_multi_arm_rewrite_passes() {
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
                    value: ArcValue::Literal(LitValue::Int(0)),
                }],
                terminator: ArcTerminator::Return { value: var(3) },
            },
        ],
        ..Default::default()
    };

    let regions = super::detect::detect_context_regions(&func);
    assert!(super::rewrite::rewrite_trmc(&mut func, &regions));

    let errors = super::verify::verify_trmc_rewrite(&func, &regions);
    assert!(
        errors.is_empty(),
        "multi-arm rewrite should pass, got: {errors:?}"
    );
}

/// Manually-constructed bad IR with a residual self-call fails verification.
#[test]
fn verify_non_tail_call_fails() {
    use super::verify::TrmcVerificationError;

    let self_name = Name::from_raw(42);

    // Start with a clean rewrite, then inject a residual self-call.
    let mut func = make_recursive_construct_func();
    let regions = super::detect::detect_context_regions(&func);
    super::rewrite::rewrite_trmc(&mut func, &regions);

    // Inject a residual self-call into the prologue block (simulate bug).
    let prologue_idx = func.entry.index();
    func.blocks[prologue_idx].body.push(ArcInstr::Apply {
        dst: var(50),
        ty: ty(0),
        func: self_name,
        args: vec![var(0)],
        arg_ownership: vec![ArgOwnership::Owned],
    });
    // Ensure var_types is large enough.
    while func.var_types.len() <= 50 {
        func.var_types.push(ty(0));
    }

    let errors = super::verify::verify_trmc_rewrite(&func, &regions);
    assert!(!errors.is_empty(), "should fail with residual self-call");
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, TrmcVerificationError::NonTailRecursiveCall { .. })),
        "should have NonTailRecursiveCall error, got: {errors:?}"
    );
}

/// Context variable passed as an Apply argument fails verification
/// (non-linear use).
#[test]
fn verify_non_linear_context_fails() {
    use super::verify::TrmcVerificationError;

    let other_fn = Name::from_raw(99);

    let mut func = make_recursive_construct_func();
    let regions = super::detect::detect_context_regions(&func);
    super::rewrite::rewrite_trmc(&mut func, &regions);

    // Find the loop header and its ctx_hole_obj (last block param).
    let prologue_idx = func.entry.index();
    let loop_header_id = match &func.blocks[prologue_idx].terminator {
        ArcTerminator::Jump { target, .. } => *target,
        other => panic!("expected Jump, got {other:?}"),
    };
    let loop_header_params = &func.blocks[loop_header_id.index()].params;
    assert!(
        !loop_header_params.is_empty(),
        "loop header should have params"
    );
    let ctx_hole_obj = loop_header_params[loop_header_params.len() - 1].0;

    // Inject an Apply that passes ctx_hole_obj as an argument (non-linear).
    let loop_header_idx = loop_header_id.index();
    func.blocks[loop_header_idx].body.insert(
        0,
        ArcInstr::Apply {
            dst: var(60),
            ty: ty(0),
            func: other_fn,
            args: vec![ctx_hole_obj],
            arg_ownership: vec![ArgOwnership::Owned],
        },
    );
    while func.var_types.len() <= 60 {
        func.var_types.push(ty(0));
    }

    let errors = super::verify::verify_trmc_rewrite(&func, &regions);
    assert!(!errors.is_empty(), "should fail with non-linear context");
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, TrmcVerificationError::NonLinearContext { var, .. } if *var == ctx_hole_obj)),
        "should have NonLinearContext for ctx_hole_obj, got: {errors:?}"
    );
}

/// `rewrite_and_verify` rollback: verification failure restores original function.
///
/// Constructs a multi-block function with a `LitValue::Null` in the base-case
/// block. `rewrite_trmc` succeeds (the recursive site is well-formed), but the
/// base-case Null survives into the rewritten IR and trips
/// `check_null_containment` → verification fails → rollback.
///
/// In debug builds, `rewrite_and_verify` fires `debug_assert!(false)` after
/// rolling back. We use `catch_unwind` to observe the restored function state.
#[test]
fn verify_rollback_restores_original() {
    let self_name = Name::from_raw(42);

    // Multi-block: Block 0 branches, Block 1 is the recursive site,
    // Block 2 is a base case with a planted LitValue::Null.
    let mut func = ArcFunction {
        name: self_name,
        return_type: ty(0),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0), ty(0), ty(0), ty(0), ty(0)],
        blocks: vec![
            // Block 0: entry — branch on v0.
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
            // Block 1: recursive — Apply + Construct + Return.
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
            // Block 2: base case with a planted Null (will trip verification).
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: var(3),
                        ty: ty(0),
                        value: ArcValue::Literal(LitValue::Null), // planted
                    },
                    ArcInstr::Let {
                        dst: var(4),
                        ty: ty(0),
                        value: ArcValue::Literal(LitValue::Int(0)),
                    },
                ],
                terminator: ArcTerminator::Return { value: var(4) },
            },
        ],
        ..Default::default()
    };

    let original = func.clone();
    let regions = super::detect::detect_context_regions(&func);
    assert_eq!(regions.len(), 1, "one TRMC candidate");

    // Sanity: rewrite_trmc alone succeeds on this function.
    {
        let mut probe = func.clone();
        assert!(
            super::rewrite::rewrite_trmc(&mut probe, &regions),
            "rewrite should succeed"
        );
        // The planted Null in block 2 survives (block 2 body is preserved).
        let errors = super::verify::verify_trmc_rewrite(&probe, &regions);
        assert!(
            !errors.is_empty(),
            "verification should fail due to planted Null"
        );
    }

    // Now exercise the full rewrite_and_verify rollback path.
    // In debug builds, debug_assert!(false) fires after rollback — catch it.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        super::rewrite_and_verify(&mut func, &regions)
    }));

    if let Ok(success) = result {
        // Release builds: returns false after rollback.
        assert!(!success, "should return false on verification failure");
    }
    // Debug builds: debug_assert! panics after rollback — expected (Err case).

    // The rollback in rewrite_and_verify runs BEFORE the debug_assert!,
    // so the function should be restored regardless of panic.
    assert_eq!(
        func, original,
        "function should be restored to pre-rewrite state after rollback"
    );
}

/// `rewrite_and_verify` with no regions returns false and doesn't modify function.
#[test]
fn rewrite_and_verify_noop_on_empty_regions() {
    let mut func = make_recursive_construct_func();
    let original = func.clone();
    let result = super::rewrite_and_verify(&mut func, &[]);
    assert!(!result, "empty regions → no rewrite");
    assert_eq!(func, original, "function unchanged");
}

/// `rewrite_and_verify` succeeds on a simple recursive construct function.
#[test]
fn rewrite_and_verify_succeeds_on_simple_func() {
    let mut func = make_recursive_construct_func();
    let regions = super::detect::detect_context_regions(&func);
    assert_eq!(regions.len(), 1);

    let result = super::rewrite_and_verify(&mut func, &regions);
    assert!(result, "simple func should rewrite and verify cleanly");

    // No self-calls remain.
    let self_name = func.name;
    let has_self_call = func.blocks.iter().any(|b| {
        b.body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::Apply { func: f, .. } if *f == self_name))
    });
    assert!(!has_self_call, "no self-call should remain");
}

/// Null sentinel contained in prologue only after clean rewrite.
#[test]
fn verify_null_sentinel_contained_in_prologue() {
    let mut func = make_recursive_construct_func();
    let regions = super::detect::detect_context_regions(&func);
    super::rewrite::rewrite_trmc(&mut func, &regions);

    // Check that null sentinels exist only in the prologue.
    let prologue_idx = func.entry.index();
    for (idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            if let ArcInstr::Let {
                value: ArcValue::Literal(LitValue::Null),
                ..
            } = instr
            {
                assert_eq!(
                    idx, prologue_idx,
                    "LitValue::Null should only be in prologue (block {prologue_idx}), found in block {idx}"
                );
            }
        }
    }
}

/// Loop-header jump args match param count after clean rewrite.
#[test]
fn verify_loop_header_arg_consistency() {
    let mut func = make_recursive_construct_func();
    let regions = super::detect::detect_context_regions(&func);
    super::rewrite::rewrite_trmc(&mut func, &regions);

    // Find loop header.
    let prologue_idx = func.entry.index();
    let loop_header_id = match &func.blocks[prologue_idx].terminator {
        ArcTerminator::Jump { target, .. } => *target,
        other => panic!("expected Jump, got {other:?}"),
    };
    let expected_args = func.blocks[loop_header_id.index()].params.len();

    // All jumps to loop header should have correct arg count.
    for block in &func.blocks {
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            if *target == loop_header_id {
                assert_eq!(
                    args.len(),
                    expected_args,
                    "Jump to loop header from block {} should have {expected_args} args, got {}",
                    block.id.raw(),
                    args.len()
                );
            }
        }
    }
}

// Pipeline integration tests (Section 13.6)

/// Helper: build a contract with `may_share` set to the given value.
fn make_contract(may_share: bool) -> MemoryContract {
    MemoryContract {
        params: vec![],
        effects: EffectSummary {
            may_share,
            ..EffectSummary::default()
        },
        fip: FipContract::Never,
        ..MemoryContract::conservative(0)
    }
}

/// `normalize_function` with contract `may_share=false`: rewrite fires,
/// `was_transformed` is `true`, no self-calls remain.
#[test]
fn pipeline_normalize_then_rewrite_with_contract() {
    let mut func = make_recursive_construct_func();
    let self_name = func.name;
    let contract = make_contract(false);

    let result = super::normalize_function(&mut func, Some(&contract));
    assert!(
        result.was_transformed,
        "should rewrite when may_share=false"
    );
    assert!(!result.context_regions.is_empty(), "regions detected");

    // No self-calls remain.
    let has_self_call = func.blocks.iter().any(|b| {
        b.body
            .iter()
            .any(|i| matches!(i, ArcInstr::Apply { func: f, .. } if *f == self_name))
    });
    assert!(!has_self_call, "self-calls eliminated by rewrite");
}

/// `normalize_function` with `None` contract: conservative, no rewrite.
#[test]
fn pipeline_normalize_noop_when_no_contract() {
    let mut func = make_recursive_construct_func();
    let original_blocks = func.blocks.len();

    let result = super::normalize_function(&mut func, None);
    assert!(!result.was_transformed, "no rewrite without contract");
    assert!(!result.context_regions.is_empty(), "detection still works");
    assert_eq!(func.blocks.len(), original_blocks, "no blocks added");
}

/// `normalize_function` with `may_share=true`: rewrite PROCEEDS because
/// `may_share` is not an enforced gate — per-variable uniqueness (checked
/// in `detect_trmc_candidates`) is the sole soundness condition. Every
/// TRMC-eligible function has `may_share == true` due to the
/// `HeapEscaping → may_share` accumulation rule.
#[test]
fn pipeline_normalize_proceeds_despite_may_share() {
    let mut func = make_recursive_construct_func();
    let self_name = func.name;
    let contract = make_contract(true);

    let result = super::normalize_function(&mut func, Some(&contract));
    assert!(
        result.was_transformed,
        "rewrite should proceed despite may_share=true"
    );
    assert!(!result.context_regions.is_empty(), "detection works");

    // No self-calls remain (rewrite converted them to loop-back Jumps).
    let has_self_call = func.blocks.iter().any(|b| {
        b.body
            .iter()
            .any(|i| matches!(i, ArcInstr::Apply { func: f, .. } if *f == self_name))
    });
    assert!(!has_self_call, "self-calls eliminated by rewrite");
}

/// `normalize_function` with no TRMC candidates: no rewrite regardless
/// of contract.
#[test]
fn pipeline_normalize_noop_when_no_candidates() {
    // Non-recursive function: just a Let + Return.
    let mut func = ArcFunction {
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
    let contract = make_contract(false);

    let result = super::normalize_function(&mut func, Some(&contract));
    assert!(!result.was_transformed, "no candidates → no rewrite");
    assert!(result.context_regions.is_empty(), "no regions");
}

/// After TRMC rewrite, new variables exist (prologue, context params,
/// compose block). Re-running detection on the rewritten function finds
/// no new candidates (idempotent).
#[test]
fn pipeline_rewrite_idempotent() {
    let mut func = make_recursive_construct_func();
    let contract = make_contract(false);

    // First run: rewrite fires.
    let r1 = super::normalize_function(&mut func, Some(&contract));
    assert!(r1.was_transformed);

    // Second run on rewritten function: no candidates found.
    let r2 = super::normalize_function(&mut func, Some(&contract));
    assert!(!r2.was_transformed, "idempotent: no new candidates");
    assert!(
        r2.context_regions.is_empty(),
        "no TRMC regions in rewritten function"
    );
}

/// New variables are allocated by the rewrite. Verify they have valid
/// indices in `var_types`.
#[test]
fn pipeline_rerun_var_reprs_after_transform() {
    let mut func = make_recursive_construct_func();
    let original_var_count = func.var_types.len();
    let contract = make_contract(false);

    let result = super::normalize_function(&mut func, Some(&contract));
    assert!(result.was_transformed);

    // Rewrite allocates fresh variables (ctx_has, ctx_res, ctx_hole_obj,
    // false_var, null_sentinel, new_res, true_var = 7 new vars).
    assert!(
        func.var_types.len() > original_var_count,
        "new variables allocated: {} > {}",
        func.var_types.len(),
        original_var_count
    );

    // All variable references in the rewritten IR are in bounds.
    for block in &func.blocks {
        for (var, _ty) in &block.params {
            assert!(
                (var.raw() as usize) < func.var_types.len(),
                "block param v{} out of bounds (var_types len {})",
                var.raw(),
                func.var_types.len()
            );
        }
    }
}

/// Rollback on verification failure: contract `may_share=false` but
/// planted Null in base case trips verification → rollback → original.
#[test]
fn pipeline_rollback_on_verification_failure() {
    let self_name = Name::from_raw(42);
    let mut func = ArcFunction {
        name: self_name,
        return_type: ty(0),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0), ty(0), ty(0), ty(0), ty(0)],
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
            // Planted Null in base case → trips verification.
            ArcBlock {
                id: block_id(2),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: var(3),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Null),
                }],
                terminator: ArcTerminator::Return { value: var(3) },
            },
        ],
        ..Default::default()
    };

    let original = func.clone();
    let contract = make_contract(false);

    // In debug builds, rewrite_and_verify fires debug_assert after rollback.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        super::normalize_function(&mut func, Some(&contract))
    }));

    if let Ok(r) = result {
        // Release builds: rollback succeeds, was_transformed=false.
        assert!(!r.was_transformed, "verification failed → not transformed");
    }
    // Debug builds: debug_assert! fires after rollback — expected (Err case).

    assert_eq!(
        func, original,
        "function restored after verification failure"
    );
}

// Matrix D tests — TRMC rewrite argument threading (Bug 1)

/// Helper: build a 2-arg self-recursive function.
/// Pattern:
///   @map(v0: T, v1: T) -> T =
///     v2 = self(v0, v1)
///     v3 = Construct { v0, v2 }  // `hole_field` = 1 (v2 is recursive)
///     Return v3
fn make_two_arg_recursive_func() -> ArcFunction {
    let self_name = Name::from_raw(42);
    ArcFunction {
        name: self_name,
        return_type: ty(0),
        params: vec![
            ArcParam {
                var: var(0),
                ty: ty(0),
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: var(1),
                ty: ty(1),
                ownership: Ownership::Owned,
            },
        ],
        var_types: vec![ty(0), ty(1), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                // v2 = self(v0, v1)
                ArcInstr::Apply {
                    dst: var(2),
                    ty: ty(0),
                    func: self_name,
                    args: vec![var(0), var(1)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
                },
                // v3 = Construct { v0, v2 }
                ArcInstr::Construct {
                    dst: var(3),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(100)),
                    args: vec![var(0), var(2)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..Default::default()
    }
}

/// D1: Single-arg self-recursion with arg change. The loop-back Jump
/// must pass the recursive call's arguments followed by context values.
/// Without Bug 1 fix, the jump only has 3 context args.
#[test]
fn d1_single_arg_loop_back_threads_rec_args() {
    let mut func = make_recursive_construct_func();
    let original_entry = func.entry;
    let n_params = func.params.len(); // 1
    let regions = super::detect::detect_context_regions(&func);
    assert!(super::rewrite::rewrite_trmc(&mut func, &regions));

    // Find all Jumps back to the loop header (original entry).
    let loop_back_jumps: Vec<_> = func
        .blocks
        .iter()
        .filter_map(|b| {
            if let ArcTerminator::Jump { target, args } = &b.terminator {
                if *target == original_entry {
                    Some(args.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    // Every Jump to the loop header must pass n_params + 3 args.
    for args in &loop_back_jumps {
        assert_eq!(
            args.len(),
            n_params + 3,
            "Jump to loop header should have {n_p} param args + 3 context args = {total}, got {actual}",
            n_p = n_params,
            total = n_params + 3,
            actual = args.len()
        );
    }
}

/// D2: Multi-arg self-recursion. Both function parameters must be
/// threaded through the loop header.
#[test]
fn d2_multi_arg_loop_back_threads_both_args() {
    let mut func = make_two_arg_recursive_func();
    let original_entry = func.entry;
    let n_params = func.params.len(); // 2
    let regions = super::detect::detect_context_regions(&func);
    assert!(super::rewrite::rewrite_trmc(&mut func, &regions));

    // Loop header must have n_params + 3 block params.
    let header_params = &func.blocks[original_entry.index()].params;
    assert_eq!(
        header_params.len(),
        n_params + 3,
        "loop header should have {n_params} fresh param + 3 context params"
    );

    // All Jumps to header have correct arg count.
    for block in &func.blocks {
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            if *target == original_entry {
                assert_eq!(
                    args.len(),
                    n_params + 3,
                    "Jump to loop header from block {} should have {} args, got {}",
                    block.id.raw(),
                    n_params + 3,
                    args.len()
                );
            }
        }
    }
}

/// D3: Prologue block passes original function param vars as the first
/// N args, followed by 3 context init values (false, null, null).
#[test]
fn d3_prologue_passes_original_params_and_ctx_init() {
    let mut func = make_recursive_construct_func();
    let original_entry = func.entry;
    let original_param_vars: Vec<_> = func.params.iter().map(|p| p.var).collect();
    let regions = super::detect::detect_context_regions(&func);
    assert!(super::rewrite::rewrite_trmc(&mut func, &regions));

    // Prologue is the new entry.
    let prologue_idx = func.entry.index();
    let prologue_args = match &func.blocks[prologue_idx].terminator {
        ArcTerminator::Jump { target, args } => {
            assert_eq!(*target, original_entry, "prologue jumps to loop header");
            args.clone()
        }
        other => panic!("prologue should be Jump, got {other:?}"),
    };

    let n_params = original_param_vars.len();
    assert_eq!(
        prologue_args.len(),
        n_params + 3,
        "prologue should pass {} args (params + ctx init)",
        n_params + 3
    );

    // First N args should be the original function parameter vars.
    for (i, param_var) in original_param_vars.iter().enumerate() {
        assert_eq!(
            prologue_args[i],
            *param_var,
            "prologue arg {i} should be original param v{}, got v{}",
            param_var.raw(),
            prologue_args[i].raw()
        );
    }
}

/// D4: Let bindings in the loop header body bridge fresh block params
/// to the original param var names.
#[test]
fn d4_let_bindings_bridge_fresh_params_to_original_vars() {
    let mut func = make_recursive_construct_func();
    let original_entry = func.entry;
    let original_param_vars: Vec<_> = func.params.iter().map(|p| p.var).collect();
    let n_params = original_param_vars.len();
    let regions = super::detect::detect_context_regions(&func);
    assert!(super::rewrite::rewrite_trmc(&mut func, &regions));

    let header_body = &func.blocks[original_entry.index()].body;
    let header_params = &func.blocks[original_entry.index()].params;

    // The first N instructions in the header body should be Let bindings
    // that define original param vars from fresh block params.
    assert!(
        header_body.len() >= n_params,
        "header body should have at least {n_params} Let bindings, has {}",
        header_body.len()
    );
    for i in 0..n_params {
        match &header_body[i] {
            ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } => {
                assert_eq!(
                    *dst,
                    original_param_vars[i],
                    "Let binding {i} should define original param v{}, got v{}",
                    original_param_vars[i].raw(),
                    dst.raw()
                );
                assert_eq!(
                    *src,
                    header_params[i].0,
                    "Let binding {i} should read from fresh block param v{}, got v{}",
                    header_params[i].0.raw(),
                    src.raw()
                );
            }
            other => panic!("header body[{i}] should be Let(dst, Var(src)), got {other:?}"),
        }
    }
}

/// D5: Dominance check: context vars used in helper blocks are dominated
/// by the loop header. This test verifies the check passes for a correctly
/// rewritten function.
#[test]
fn d5_context_var_dominance_verified() {
    use super::verify::TrmcVerificationError;

    let mut func = make_recursive_construct_func();
    let regions = super::detect::detect_context_regions(&func);
    assert!(super::rewrite::rewrite_trmc(&mut func, &regions));

    // Full verification including dominance check should pass.
    let errors = super::verify::verify_trmc_rewrite(&func, &regions);
    assert!(
        errors.is_empty(),
        "dominance check should pass for correct rewrite: {errors:?}"
    );

    // No ContextVarDominanceViolation errors.
    assert!(
        !errors.iter().any(|e| matches!(
            e,
            TrmcVerificationError::ContextVarDominanceViolation { .. }
        )),
        "no dominance violations in correct rewrite"
    );
}

/// Context events are naturally empty after TRMC rewrite: the rewrite
/// eliminates self-calls, so `detect_trmc_candidates` finds no
/// `ContextHole` shapes and `populate_context_events` records nothing.
/// This verifies the no-op nature of event consumption for v1.
#[test]
fn pipeline_context_events_empty_after_rewrite() {
    let mut func = make_recursive_construct_func();
    let contract = make_contract(false);

    // Before rewrite: detection finds candidates.
    let pre_regions = super::detect::detect_context_regions(&func);
    assert!(!pre_regions.is_empty(), "pre-rewrite: has candidates");

    // After rewrite: no candidates (self-calls eliminated).
    let result = super::normalize_function(&mut func, Some(&contract));
    assert!(result.was_transformed);

    let post_regions = super::detect::detect_context_regions(&func);
    assert!(
        post_regions.is_empty(),
        "post-rewrite: no TRMC candidates (self-calls eliminated)"
    );
}
