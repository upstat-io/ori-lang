use ori_ir::canon::tree::{FlatPattern, PathInstruction, PatternRow};
use ori_ir::Name;

use super::compile;

fn row(pattern: FlatPattern) -> PatternRow {
    PatternRow {
        patterns: vec![pattern],
        arm_index: 0,
        guard: None,
        bindings: vec![],
        discard_paths: vec![],
    }
}

#[test]
fn tuple_wildcard_reaches_leaf_cleanup_carrier() {
    let value = Name::from_raw(1);
    let compiled = compile(
        vec![row(FlatPattern::Tuple(vec![
            FlatPattern::Binding(value),
            FlatPattern::Wildcard,
        ]))],
        vec![vec![]],
    );

    assert_eq!(
        compiled.leaf_discard_paths,
        vec![vec![vec![PathInstruction::TupleIndex(1)]]]
    );
}

#[test]
fn ancestor_at_binding_suppresses_descendant_wildcard_cleanup() {
    let whole = Name::from_raw(1);
    let value = Name::from_raw(2);
    let compiled = compile(
        vec![row(FlatPattern::At {
            name: whole,
            inner: Box::new(FlatPattern::Tuple(vec![
                FlatPattern::Binding(value),
                FlatPattern::Wildcard,
            ])),
        })],
        vec![vec![]],
    );

    assert_eq!(compiled.leaf_discard_paths.len(), 1);
    assert!(compiled.leaf_discard_paths[0].is_empty());
}
