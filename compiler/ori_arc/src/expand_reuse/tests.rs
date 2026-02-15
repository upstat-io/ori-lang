use ori_ir::Name;
use ori_types::{Idx, Pool};

use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue};
use crate::test_helpers::{b, make_func, owned_param, v};
use crate::ArcClassifier;

use super::expand_reset_reuse;

fn run_expand(mut func: ArcFunction) -> ArcFunction {
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    expand_reset_reuse(&mut func, &classifier);
    func
}

/// Count how many instructions of a given kind exist in all blocks.
fn count_instrs(func: &ArcFunction, pred: impl Fn(&ArcInstr) -> bool) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| b.body.iter())
        .filter(|i| pred(i))
        .count()
}

// Test 1: No Reset/Reuse -> pass-through

#[test]
fn no_pair_passthrough() {
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: v(1),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(42)),
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::INT, Idx::INT],
    );

    let result = run_expand(func);
    assert_eq!(result.blocks.len(), 1, "no new blocks should be created");
}

// Test 2: Basic expansion -- Reset/Reuse with no projections

#[test]
fn basic_expansion() {
    // v0: STR (param), v1: STR (reuse result), v2: token
    // Body: Reset{v0, v2}; Reuse{v2, v1, STR, Struct, []}
    // Term: Return{v1}
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Reset {
                    var: v(0),
                    token: v(2),
                },
                ArcInstr::Reuse {
                    token: v(2),
                    dst: v(1),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_expand(func);

    // Should have 3 blocks: original (IsShared+Branch), fast, slow.
    // No merge block needed since terminator uses reuse_dst → needs merge.
    // Actually, Return{v1} uses reuse_dst → needs merge block.
    assert!(
        result.blocks.len() >= 3,
        "expected at least 3 blocks, got {}",
        result.blocks.len()
    );

    // No Reset or Reuse should remain.
    assert_eq!(
        count_instrs(&result, |i| matches!(i, ArcInstr::Reset { .. })),
        0
    );
    assert_eq!(
        count_instrs(&result, |i| matches!(i, ArcInstr::Reuse { .. })),
        0
    );

    // Should have IsShared in original block.
    assert_eq!(
        count_instrs(&result, |i| matches!(i, ArcInstr::IsShared { .. })),
        1
    );

    // Original block should end with Branch.
    assert!(matches!(
        result.blocks[0].terminator,
        ArcTerminator::Branch { .. }
    ));
}

// Test 3: Self-set elimination

#[test]
fn self_set_eliminated() {
    // Simulates: Cons(head, tail) -> Cons(new_head, tail)
    // v0: STR (param, the list)
    // v1: STR (head = Project{v0, 0})
    // v2: STR (tail = Project{v0, 1})
    // v3: STR (new_head = Apply{f, [v1]})
    // v4: token
    // v5: STR (result = Reuse{v4, Cons, [v3, v2]})
    //
    // Field 1 (v2) is a self-set: Project{v0, 1} → Set{v0, 1, v2} is no-op.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: v(1),
                    ty: Idx::STR,
                    value: v(0),
                    field: 0,
                },
                ArcInstr::Project {
                    dst: v(2),
                    ty: Idx::STR,
                    value: v(0),
                    field: 1,
                },
                ArcInstr::Apply {
                    dst: v(3),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(1)],
                },
                ArcInstr::Reset {
                    var: v(0),
                    token: v(4),
                },
                ArcInstr::Reuse {
                    token: v(4),
                    dst: v(5),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![v(3), v(2)],
                },
            ],
            terminator: ArcTerminator::Return { value: v(5) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_expand(func);

    // Find the fast-path block (the else branch of the Branch).
    let fast_id = match &result.blocks[0].terminator {
        ArcTerminator::Branch { else_block, .. } => *else_block,
        other => panic!("expected Branch, got {other:?}"),
    };
    let fast_block = &result.blocks[fast_id.index()];

    // Fast path should have Set for field 0 only (field 1 is self-set).
    let sets: Vec<_> = fast_block
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::Set { .. }))
        .collect();
    assert_eq!(
        sets.len(),
        1,
        "expected 1 Set (field 1 self-set eliminated)"
    );
    assert!(
        matches!(sets[0], ArcInstr::Set { field: 0, .. }),
        "expected Set for field 0, got {:?}",
        sets[0]
    );
}

// Test 4: Projection-increment erasure

#[test]
fn proj_inc_erasure() {
    // v0: STR (param, the list)
    // v1: STR (head = Project{v0, 0})
    // v2: STR (tail = Project{v0, 1})
    // v3: RcInc{v2}  ← this should be erased
    // v4: STR (new_head)
    // v5: token
    // v6: result = Reuse{Cons, [v4, v2]}
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: v(1),
                    ty: Idx::STR,
                    value: v(0),
                    field: 0,
                },
                ArcInstr::Project {
                    dst: v(2),
                    ty: Idx::STR,
                    value: v(0),
                    field: 1,
                },
                ArcInstr::RcInc {
                    var: v(2),
                    count: 1,
                },
                ArcInstr::Let {
                    dst: v(4),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::Int(99)),
                },
                ArcInstr::Reset {
                    var: v(0),
                    token: v(5),
                },
                ArcInstr::Reuse {
                    token: v(5),
                    dst: v(6),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![v(4), v(2)],
                },
            ],
            terminator: ArcTerminator::Return { value: v(6) },
        }],
        vec![
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
        ],
    );

    let result = run_expand(func);

    // The RcInc{v2} in the original block should have been erased.
    let original_incs: Vec<_> = result.blocks[0]
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::RcInc { var, .. } if *var == v(2)))
        .collect();
    assert!(
        original_incs.is_empty(),
        "RcInc for v2 should be erased from original block"
    );

    // Slow path should have RcInc{v2} restored.
    let slow_id = match &result.blocks[0].terminator {
        ArcTerminator::Branch { then_block, .. } => *then_block,
        other => panic!("expected Branch, got {other:?}"),
    };
    let slow_block = &result.blocks[slow_id.index()];
    let slow_incs: Vec<_> = slow_block
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::RcInc { var, .. } if *var == v(2)))
        .collect();
    assert_eq!(slow_incs.len(), 1, "slow path should restore RcInc for v2");
}

// Test 5: Slow path has RcDec + Construct

#[test]
fn slow_path_dec_construct() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Reset {
                    var: v(0),
                    token: v(2),
                },
                ArcInstr::Reuse {
                    token: v(2),
                    dst: v(1),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_expand(func);

    let slow_id = match &result.blocks[0].terminator {
        ArcTerminator::Branch { then_block, .. } => *then_block,
        other => panic!("expected Branch, got {other:?}"),
    };
    let slow_block = &result.blocks[slow_id.index()];

    // Slow path should start with RcDec{v0}.
    assert!(
        matches!(&slow_block.body[0], ArcInstr::RcDec { var } if *var == v(0)),
        "slow path should start with RcDec, got {:?}",
        slow_block.body[0]
    );

    // Slow path should have a Construct.
    let constructs: Vec<_> = slow_block
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::Construct { .. }))
        .collect();
    assert_eq!(constructs.len(), 1, "slow path should have one Construct");
}

// Test 6: Enum variant -> SetTag on fast path

#[test]
fn enum_variant_set_tag() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Reset {
                    var: v(0),
                    token: v(2),
                },
                ArcInstr::Reuse {
                    token: v(2),
                    dst: v(1),
                    ty: Idx::STR,
                    ctor: CtorKind::EnumVariant {
                        enum_name: Name::from_raw(20),
                        variant: 1,
                    },
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_expand(func);

    let fast_id = match &result.blocks[0].terminator {
        ArcTerminator::Branch { else_block, .. } => *else_block,
        other => panic!("expected Branch, got {other:?}"),
    };
    let fast_block = &result.blocks[fast_id.index()];

    // Fast path should have SetTag with tag=1.
    let set_tags: Vec<_> = fast_block
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::SetTag { .. }))
        .collect();
    assert_eq!(set_tags.len(), 1);
    assert!(
        matches!(set_tags[0], ArcInstr::SetTag { tag: 1, .. }),
        "expected SetTag with tag=1, got {:?}",
        set_tags[0]
    );
}

// Test 7: Suffix instructions create merge block

#[test]
fn suffix_creates_merge_block() {
    // Body: Reset; Reuse; Apply (suffix)
    // Term: Return{suffix_result}
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Reset {
                    var: v(0),
                    token: v(2),
                },
                ArcInstr::Reuse {
                    token: v(2),
                    dst: v(1),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Apply {
                    dst: v(3),
                    ty: Idx::INT,
                    func: Name::from_raw(99),
                    args: vec![v(1)], // uses reuse_dst
                },
            ],
            terminator: ArcTerminator::Return { value: v(3) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR, Idx::INT],
    );

    let result = run_expand(func);

    // Should have 4 blocks: original, fast, slow, merge.
    assert_eq!(
        result.blocks.len(),
        4,
        "expected 4 blocks (original + fast + slow + merge), got {}",
        result.blocks.len()
    );

    // The merge block should have the Apply instruction.
    let merge_block = &result.blocks[3];
    assert!(
        !merge_block.params.is_empty(),
        "merge block should have a parameter"
    );

    let has_apply = merge_block
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::Apply { .. }));
    assert!(has_apply, "merge block should contain the suffix Apply");
}

// Test 8: Between instructions moved to prefix

#[test]
fn between_instrs_moved() {
    // Body: Reset{v0}; Let{v3=42}; Reuse{token, v1, ...}
    // The Let should be moved before the Reset.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Reset {
                    var: v(0),
                    token: v(2),
                },
                ArcInstr::Let {
                    dst: v(3),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(42)),
                },
                ArcInstr::Reuse {
                    token: v(2),
                    dst: v(1),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![v(3)],
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR, Idx::INT],
    );

    let result = run_expand(func);

    // Original block should have: Let, IsShared, Branch.
    let original = &result.blocks[0];

    // The Let should be in the original block (moved before IsShared).
    let has_let = original
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::Let { dst, .. } if *dst == v(3)));
    assert!(has_let, "Let should be in original block (moved to prefix)");

    // No Reset or Reuse should remain anywhere.
    assert_eq!(
        count_instrs(&result, |i| matches!(i, ArcInstr::Reset { .. })),
        0
    );
    assert_eq!(
        count_instrs(&result, |i| matches!(i, ArcInstr::Reuse { .. })),
        0
    );
}

// Test 9: Fast path substitutes reuse_dst -> reset_var

#[test]
fn fast_path_variable_substitution() {
    // Term: Return{v1} where v1 is reuse_dst.
    // Fast path should return v0 (reset_var) instead.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Reset {
                    var: v(0),
                    token: v(2),
                },
                ArcInstr::Reuse {
                    token: v(2),
                    dst: v(1),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_expand(func);

    // Find merge block (has parameter).
    let merge_block = result
        .blocks
        .iter()
        .find(|b| !b.params.is_empty())
        .unwrap_or_else(|| panic!("should have a merge block"));

    // Fast path should jump to merge with reset_var (v0).
    let fast_id = match &result.blocks[0].terminator {
        ArcTerminator::Branch { else_block, .. } => *else_block,
        other => panic!("expected Branch, got {other:?}"),
    };
    let fast_block = &result.blocks[fast_id.index()];
    match &fast_block.terminator {
        ArcTerminator::Jump { args, target } => {
            assert_eq!(*target, merge_block.id);
            assert!(
                args.contains(&v(0)),
                "fast path should pass reset_var (v0) to merge, got {args:?}"
            );
        }
        other => panic!("expected Jump to merge, got {other:?}"),
    }
}

// Test 10: Dec unclaimed replaced fields on fast path

#[test]
fn fast_path_dec_unclaimed_field() {
    // v0: STR (list param)
    // v1: STR (old_field = Project{v0, 0}) — NOT claimed (no RcInc erased)
    // v2: STR (new_value)
    // v3: token
    // v4: result = Reuse{Cons, [v2]}
    //
    // Field 0 is being replaced (v2 != v1), and it's unclaimed and RC'd.
    // Fast path should emit RcDec{v1} before Set{v0, 0, v2}.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: v(1),
                    ty: Idx::STR,
                    value: v(0),
                    field: 0,
                },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::Int(99)),
                },
                ArcInstr::Reset {
                    var: v(0),
                    token: v(3),
                },
                ArcInstr::Reuse {
                    token: v(3),
                    dst: v(4),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![v(2)],
                },
            ],
            terminator: ArcTerminator::Return { value: v(4) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_expand(func);

    let fast_id = match &result.blocks[0].terminator {
        ArcTerminator::Branch { else_block, .. } => *else_block,
        other => panic!("expected Branch, got {other:?}"),
    };
    let fast_block = &result.blocks[fast_id.index()];

    // Fast path should have RcDec{v1} (unclaimed old field) before Set.
    let has_dec = fast_block
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::RcDec { var } if *var == v(1)));
    assert!(
        has_dec,
        "fast path should dec unclaimed old field v1, body: {:?}",
        fast_block.body
    );
}
