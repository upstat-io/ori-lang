use ori_ir::canon::tree::{DecisionTree, FlatPattern, PathInstruction, PatternRow};
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

#[test]
fn struct_fields_preserve_names_in_binding_and_discard_paths() {
    let discarded_field = Name::from_raw(17);
    let bound_field = Name::from_raw(5);
    let binding = Name::from_raw(29);
    let compiled = compile(
        vec![row(FlatPattern::Struct {
            fields: vec![
                (discarded_field, FlatPattern::Wildcard),
                (bound_field, FlatPattern::Binding(binding)),
            ],
        })],
        vec![vec![]],
    );

    assert_eq!(
        compiled.leaf_discard_paths,
        vec![vec![vec![PathInstruction::StructField(discarded_field)]]]
    );
    assert_eq!(
        compiled.tree,
        DecisionTree::Leaf {
            arm_index: 0,
            bindings: vec![(binding, vec![PathInstruction::StructField(bound_field)])],
        }
    );
}

#[test]
#[should_panic(expected = "column count mismatch at row 0")]
fn compile_rejects_matrix_path_misalignment() {
    let _ = compile(vec![row(FlatPattern::Wildcard)], Vec::new());
}

#[test]
#[should_panic(expected = "column count mismatch at row 1")]
fn compile_later_row_path_misalignment_panics() {
    let later_row = PatternRow {
        patterns: Vec::new(),
        arm_index: 1,
        guard: None,
        bindings: Vec::new(),
        discard_paths: Vec::new(),
    };

    let _ = compile(
        vec![row(FlatPattern::Wildcard), later_row],
        vec![Vec::new()],
    );
}
