use ori_types::{Idx, Pool};

use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArgOwnership, CtorKind};
use crate::ownership::Ownership;
use crate::test_helpers::{b, make_func, v};
use rustc_hash::FxHashMap;

use ori_ir::Name;

/// Run the full ARC pipeline via the public orchestration function.
fn run_full_pipeline(
    func: &mut ArcFunction,
    classifier: &dyn crate::ArcClassification,
    pool: &Pool,
) {
    let sigs = FxHashMap::default();
    let interner = ori_ir::StringInterner::new();
    crate::annotate_arg_ownership(
        func,
        &sigs,
        &interner,
        &crate::BuiltinOwnershipSets::empty(),
        pool,
    );
    let uniqueness_summaries = FxHashMap::default();
    let aims_contracts = FxHashMap::default();
    crate::run_arc_pipeline(
        func,
        classifier,
        &sigs,
        pool,
        &interner,
        &uniqueness_summaries,
        &aims_contracts,
        false,
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
    let classifier = crate::ArcClassifier::new(&pool);

    let mut func = func;
    run_full_pipeline(&mut func, &classifier, &pool);

    // Should still have exactly 1 block (no expansion needed)
    assert_eq!(func.blocks.len(), 1);
}

/// Full AIMS pipeline on raw IR with reuse pattern.
///
/// Exercises the production pipeline infrastructure via `run_full_pipeline`:
/// - AIMS backward dataflow analysis
/// - RC emission from state map
/// - Reuse emission from state map
/// - Tail call detection + rewrite
/// - Block merge
/// - COW annotations + drop hints (post-merge)
/// - FBIP enforcement
#[test]
fn full_pipeline_on_reuse_pattern() {
    use crate::fbip::analyze_fbip;

    // Raw IR: Project fields, Apply transform, Construct result.
    // No pre-placed Reset/Reuse — detection passes discover the pattern.
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
                    arg_ownership: vec![ArgOwnership::Owned],
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
    let classifier = crate::ArcClassifier::new(&pool);

    let mut func = func;
    run_full_pipeline(&mut func, &classifier, &pool);

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
    let dom_tree = crate::DominatorTree::build(&func);
    let (refined, _) = crate::compute_refined_liveness(&func, &classifier);
    let _fbip_report = analyze_fbip(&func, &classifier, &dom_tree, &refined);
}

/// Pipeline output must be identical across multiple runs on the same input.
///
/// Guards against non-deterministic iteration of `FxHashMap`/`FxHashSet`
/// leaking into IR ordering.
#[test]
fn pipeline_determinism() {
    let base = make_func(
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
                    arg_ownership: vec![ArgOwnership::Owned],
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
    let classifier = crate::ArcClassifier::new(&pool);

    let mut reference = base.clone();
    run_full_pipeline(&mut reference, &classifier, &pool);

    // Run 10 additional times and compare.
    for i in 1..=10 {
        let mut trial = base.clone();
        run_full_pipeline(&mut trial, &classifier, &pool);
        assert_eq!(
            reference, trial,
            "pipeline run #{i} produced different output — non-deterministic IR"
        );
    }
}
