use ori_types::{Idx, Pool};

use ori_ir::Name;

use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcParam, ArcTerminator, CtorKind};
use crate::ownership::Ownership;
use crate::test_helpers::{b, count_rc_ops, make_func, v};
use rustc_hash::FxHashMap;

use crate::{
    compute_liveness, compute_refined_liveness, expand_reset_reuse, ArcClassifier, DominatorTree,
};

/// Run the full ARC pipeline via the public orchestration function.
fn run_full_pipeline(func: &mut ArcFunction, classifier: &dyn crate::ArcClassification) {
    let sigs = FxHashMap::default();
    crate::run_arc_pipeline(func, classifier, &sigs);
}

/// Verifies the correct pipeline order: expand BEFORE eliminate.
///
/// Creates a function with a constructor-reuse pattern. After expansion,
/// new `RcInc`/`RcDec` instructions are generated (slow path `RcDec`, restored
/// `RcInc`, fast path field `RcDec`). Running eliminate AFTER expansion
/// ensures those ops are candidates for optimization.
#[test]
fn pipeline_order_expand_before_eliminate() {
    // fn foo(x: str) -> str
    //   head = Project(x, 0)       -- STR field
    //   tail = Project(x, 1)       -- STR field
    //   new_head = Apply(f, [head]) -- transform head
    //   Reset(x, token)
    //   result = Reuse(token, Struct, [new_head, tail])
    //   Return result
    let func = make_func(
        vec![ArcParam {
            var: v(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
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
        vec![
            Idx::STR, // v0: param
            Idx::STR, // v1: head
            Idx::STR, // v2: tail
            Idx::STR, // v3: new_head
            Idx::STR, // v4: token
            Idx::STR, // v5: result
        ],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);

    // Run pipeline in correct order (skipping detect — IR has pre-placed Reset/Reuse).
    // Uses classic functions directly (pub(crate)) to test ordering invariant.
    let mut func_correct = func.clone();
    {
        let liveness = compute_liveness(&func_correct, &classifier);
        crate::rc_insert::insert_rc_ops(&mut func_correct, &classifier, &liveness);
        // detect_reset_reuse skipped: IR already contains Reset/Reuse from setup
        expand_reset_reuse(&mut func_correct, &classifier);
        crate::rc_elim::eliminate_rc_ops(&mut func_correct);
    }

    // No Reset/Reuse should remain after expansion
    let has_reset = func_correct
        .blocks
        .iter()
        .flat_map(|bl| bl.body.iter())
        .any(|i| matches!(i, ArcInstr::Reset { .. }));
    let has_reuse = func_correct
        .blocks
        .iter()
        .flat_map(|bl| bl.body.iter())
        .any(|i| matches!(i, ArcInstr::Reuse { .. }));
    assert!(!has_reset, "no Reset instructions should remain");
    assert!(!has_reuse, "no Reuse instructions should remain");

    // Should have expanded into multiple blocks (original + fast + slow + merge)
    assert!(
        func_correct.blocks.len() >= 3,
        "pipeline should expand into 3+ blocks, got {}",
        func_correct.blocks.len()
    );

    // Run pipeline in WRONG order (eliminate before expand) for comparison
    let mut func_wrong = func.clone();
    let liveness = compute_liveness(&func_wrong, &classifier);
    crate::rc_insert::insert_rc_ops(&mut func_wrong, &classifier, &liveness);
    crate::rc_elim::eliminate_rc_ops(&mut func_wrong); // wrong: runs too early
                                                       // detect_reset_reuse skipped: IR already contains Reset/Reuse from setup
    expand_reset_reuse(&mut func_wrong, &classifier);

    // Wrong order should have MORE remaining RC ops (expand generated
    // new ones that eliminate already ran and couldn't clean up)
    let correct_rc_count = count_rc_ops(&func_correct);
    let wrong_rc_count = count_rc_ops(&func_wrong);
    assert!(
        correct_rc_count <= wrong_rc_count,
        "correct pipeline order should have <= RC ops ({correct_rc_count}) \
         than wrong order ({wrong_rc_count})"
    );
}

/// The pipeline should handle functions with no Reset/Reuse gracefully.
#[test]
fn pipeline_no_reuse_pattern() {
    let func = make_func(
        vec![ArcParam {
            var: v(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);

    let mut func = func;
    run_full_pipeline(&mut func, &classifier);

    // Should still have exactly 1 block (no expansion needed)
    assert_eq!(func.blocks.len(), 1);
}

/// Full enhanced pipeline on raw IR with reuse pattern.
///
/// Exercises the production pipeline infrastructure:
/// - `infer_derived_ownership` (per-variable ownership)
/// - `DominatorTree` (for cross-block reset/reuse)
/// - `compute_refined_liveness` (for aliasing checks)
/// - `insert_rc_ops_with_ownership` (ownership-aware RC insertion)
/// - `detect_reset_reuse_cfg` (intra + cross-block detection)
/// - `eliminate_rc_ops_dataflow` (full-CFG elimination)
/// - `analyze_fbip` (FBIP diagnostic report)
#[test]
fn full_pipeline_on_reuse_pattern() {
    use crate::fbip::analyze_fbip;

    // Raw IR: Project fields, Apply transform, Construct result.
    // No pre-placed Reset/Reuse — detection passes discover the pattern.
    //
    // fn foo(x: str) -> str
    //   head = Project(x, 0)
    //   tail = Project(x, 1)
    //   new_head = Apply(f, [head])
    //   result = Construct(Struct, [new_head, tail])
    //   Return result
    let func = make_func(
        vec![ArcParam {
            var: v(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
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
                ArcInstr::Construct {
                    dst: v(4),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![v(3), v(2)],
                },
            ],
            terminator: ArcTerminator::Return { value: v(4) },
        }],
        vec![
            Idx::STR, // v0: param
            Idx::STR, // v1: head
            Idx::STR, // v2: tail
            Idx::STR, // v3: new_head
            Idx::STR, // v4: result
        ],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);

    let mut func = func;
    run_full_pipeline(&mut func, &classifier);

    // No unexpanded Reset/Reuse should remain
    let has_unexpanded = func
        .blocks
        .iter()
        .flat_map(|bl| bl.body.iter())
        .any(|i| matches!(i, ArcInstr::Reset { .. } | ArcInstr::Reuse { .. }));
    assert!(
        !has_unexpanded,
        "no Reset/Reuse should remain after expansion"
    );

    // Run FBIP analysis on the result
    let dom_tree = DominatorTree::build(&func);
    let (refined, _) = compute_refined_liveness(&func, &classifier);
    let _fbip_report = analyze_fbip(&func, &classifier, &dom_tree, &refined);
}
